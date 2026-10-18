//! Atomic refresh of authored context metadata and its generated identity.

use std::collections::BTreeSet;

use super::{
    ContextRecord, ContextRegistry, ContextRegistryError, MAX_SESSION_REFERENCES, Uuid,
    unix_timestamp, validate_accent, validate_label, validate_name, validate_profile,
    validate_target, validate_workspace,
};

/// Validated by the registry before any generated state is changed.
/// Profile existence remains application policy; no profile is created here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextDefinitionUpdate {
    pub name: String,
    pub label: String,
    pub profile: String,
    pub sessions: Vec<String>,
    pub workspace: Option<String>,
    pub accent: Option<String>,
    pub default_target: String,
}

impl ContextRegistry {
    /// Applies one complete configuration-domain change under one registry lock.
    /// Existing IDs and members survive metadata updates. Previously committed
    /// document ownership survives restarts; explicit previous names also cover
    /// a legacy in-process update. Other command-created records are retained. Any rejected definition leaves the whole batch unchanged.
    ///
    /// # Errors
    /// Rejects invalid/duplicate metadata, unsafe profile-affinity changes, and
    /// durable write failures.
    pub fn reconcile_configured(
        &mut self,
        previous_names: &[String],
        definitions: &[ContextDefinitionUpdate],
    ) -> Result<(), ContextRegistryError> {
        let mut names = BTreeSet::new();
        for definition in definitions {
            validate_definition(definition)?;
            if !names.insert(definition.name.as_str()) {
                return Err(ContextRegistryError::DuplicateName);
            }
        }
        for name in previous_names {
            validate_name(name)?;
        }
        self.mutate_document(
            |document| {
                let removed = document
                    .configured_names
                    .iter()
                    .map(String::as_str)
                    .chain(previous_names.iter().map(String::as_str))
                    .filter(|name| !names.contains(name))
                    .collect::<BTreeSet<_>>();
                let count = document.contexts.len();
                document
                    .contexts
                    .retain(|record| !removed.contains(record.name.as_str()));
                let next_names = names.iter().map(|name| (*name).to_owned()).collect();
                let mut changed =
                    document.contexts.len() != count || document.configured_names != next_names;
                document.configured_names = next_names;
                for definition in definitions {
                    changed |= apply_definition(&mut document.contexts, definition)?;
                }
                Ok(changed)
            },
            |_| Ok(()),
        )
    }

    pub(super) fn upsert_configured_definition(
        &mut self,
        definition: &ContextDefinitionUpdate,
    ) -> Result<(), ContextRegistryError> {
        validate_definition(definition)?;
        self.mutate_document(
            |document| {
                let changed = apply_definition(&mut document.contexts, definition)?;
                Ok(document.configured_names.insert(definition.name.clone()) || changed)
            },
            |_| Ok(()),
        )
    }
}

fn validate_definition(definition: &ContextDefinitionUpdate) -> Result<(), ContextRegistryError> {
    validate_name(&definition.name)?;
    validate_label(&definition.label)?;
    validate_profile(&definition.profile)?;
    validate_target(&definition.default_target)?;
    validate_workspace(definition.workspace.as_deref())?;
    validate_accent(definition.accent.as_deref())?;
    if definition.sessions.len() > MAX_SESSION_REFERENCES
        || definition
            .sessions
            .iter()
            .any(|session| validate_profile(session).is_err())
    {
        return Err(ContextRegistryError::InvalidProfile);
    }
    Ok(())
}

fn apply_definition(
    records: &mut Vec<ContextRecord>,
    definition: &ContextDefinitionUpdate,
) -> Result<bool, ContextRegistryError> {
    if let Some(record) = records
        .iter_mut()
        .find(|record| record.name == definition.name)
    {
        if record.profile != definition.profile && !record.members.is_empty() {
            return Err(ContextRegistryError::ProfileChangeRequiresMigration);
        }
        if record.label == definition.label
            && record.profile == definition.profile
            && record.sessions == definition.sessions
            && record.workspace == definition.workspace
            && record.accent == definition.accent
            && record.default_target == definition.default_target
        {
            return Ok(false);
        }
        record.label.clone_from(&definition.label);
        record.profile.clone_from(&definition.profile);
        record.sessions.clone_from(&definition.sessions);
        record.workspace.clone_from(&definition.workspace);
        record.accent.clone_from(&definition.accent);
        record.default_target.clone_from(&definition.default_target);
        record.updated_at = unix_timestamp().max(record.updated_at);
        return Ok(true);
    }
    let now = unix_timestamp();
    records.push(ContextRecord {
        id: Uuid::new_v4(),
        name: definition.name.clone(),
        label: definition.label.clone(),
        profile: definition.profile.clone(),
        sessions: definition.sessions.clone(),
        workspace: definition.workspace.clone(),
        accent: definition.accent.clone(),
        default_target: definition.default_target.clone(),
        created_at: now,
        updated_at: now,
        members: Vec::new(),
    });
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContextMember, RootSpec, StorageRoots};

    fn definition(name: &str, label: &str) -> ContextDefinitionUpdate {
        ContextDefinitionUpdate {
            name: name.into(),
            label: label.into(),
            profile: "work".into(),
            sessions: vec!["work-session".into()],
            workspace: Some("3".into()),
            accent: Some("#7aa2f7".into()),
            default_target: "reuse".into(),
        }
    }
    fn members() -> Vec<ContextMember> {
        vec![ContextMember {
            window_id: "owner".into(),
            tabs: vec!["tab".into()],
            selected_tab: Some("tab".into()),
            tab_descriptors: Vec::new(),
            selected_index: None,
        }]
    }
    fn registry() -> (StorageRoots, ContextRegistry) {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        let registry = ContextRegistry::open(&roots).expect("registry");
        (roots, registry)
    }

    #[test]
    fn configured_deletion_preserves_identity_membership_and_document_across_owners() {
        let (roots, mut owner) = registry();
        owner
            .create("work", "Command-created work", "work")
            .unwrap();
        let mut stale = ContextRegistry::open(&roots).unwrap();
        owner
            .reconcile_configured(&[], &[definition("work", "Configured work")])
            .unwrap();
        owner.save_membership("work", members()).unwrap();
        let original = owner.contexts()[0].clone();
        let revision = owner.revision();
        let path = roots.state.join("contexts.json");
        let bytes = std::fs::read(&path).unwrap();
        assert!(matches!(
            stale.remove("work"),
            Err(ContextRegistryError::DefinedInConfiguration)
        ));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let mut reopened = ContextRegistry::open(&roots).unwrap();
        assert_eq!(reopened.revision(), revision);
        assert_eq!(reopened.contexts(), std::slice::from_ref(&original));
        assert!(matches!(
            reopened.remove("work"),
            Err(ContextRegistryError::DefinedInConfiguration)
        ));
        reopened
            .reconcile_configured(&[], &[definition("work", "Configured work")])
            .unwrap();
        assert_eq!(reopened.contexts(), &[original]);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        // Removing the authored definition remains the supported deletion path.
        reopened.reconcile_configured(&[], &[]).unwrap();
        assert!(ContextRegistry::open(&roots).unwrap().contexts().is_empty());
        roots.cleanup().unwrap();
    }

    #[test]
    fn metadata_refresh_preserves_identity_and_members() {
        let (roots, mut registry) = registry();
        registry
            .reconcile_configured(&[], &[definition("work", "Before")])
            .unwrap();
        registry.save_membership("work", members()).unwrap();
        let original = registry.contexts()[0].clone();
        let mut updated = definition("work", "After");
        updated.workspace = Some("7".into());
        registry
            .reconcile_configured(&["work".into()], &[updated])
            .unwrap();
        let reopened = ContextRegistry::open(&roots).unwrap();
        let current = &reopened.contexts()[0];
        assert_eq!(current.id, original.id);
        assert_eq!(current.created_at, original.created_at);
        assert_eq!(current.members, original.members);
        assert_eq!(current.label, "After");
        assert_eq!(current.workspace.as_deref(), Some("7"));
    }

    #[test]
    fn removal_is_limited_to_previously_authored_names() {
        let (_, mut registry) = registry();
        registry.create("other", "Other", "default").unwrap();
        registry
            .reconcile_configured(&[], &[definition("work", "Work")])
            .unwrap();
        let other = registry
            .contexts()
            .iter()
            .find(|record| record.name == "other")
            .unwrap()
            .id;
        registry
            .reconcile_configured(&["work".into()], &[definition("research", "Research")])
            .unwrap();
        assert!(
            !registry
                .contexts()
                .iter()
                .any(|record| record.name == "work")
        );
        assert!(
            registry
                .contexts()
                .iter()
                .any(|record| record.name == "research")
        );
        assert_eq!(
            registry
                .contexts()
                .iter()
                .find(|record| record.name == "other")
                .unwrap()
                .id,
            other
        );
    }

    #[test]
    fn unsafe_affinity_change_rolls_back_the_entire_batch() {
        let (roots, mut registry) = registry();
        registry
            .reconcile_configured(&[], &[definition("work", "Work")])
            .unwrap();
        registry.save_membership("work", members()).unwrap();
        let path = roots.state.join("contexts.json");
        let before = std::fs::read(&path).unwrap();
        let mut moved = definition("work", "Moved");
        moved.profile = "personal".into();
        assert!(matches!(
            registry.reconcile_configured(&["work".into()], &[definition("new", "New"), moved]),
            Err(ContextRegistryError::ProfileChangeRequiresMigration)
        ));
        assert_eq!(std::fs::read(path).unwrap(), before);
        assert!(
            !registry
                .contexts()
                .iter()
                .any(|record| record.name == "new")
        );
    }

    #[test]
    fn refresh_uses_fresh_membership_from_other_owners() {
        let (roots, mut source) = registry();
        source
            .reconcile_configured(&[], &[definition("work", "Before")])
            .unwrap();
        let mut stale = ContextRegistry::open(&roots).unwrap();
        source.save_membership("work", members()).unwrap();
        stale
            .reconcile_configured(&["work".into()], &[definition("work", "After")])
            .unwrap();
        assert_eq!(
            ContextRegistry::open(&roots).unwrap().contexts()[0].members,
            members()
        );
    }

    #[test]
    fn invalid_batch_does_not_partially_create_records() {
        let (roots, mut registry) = registry();
        registry
            .reconcile_configured(&[], &[definition("work", "Work")])
            .unwrap();
        let path = roots.state.join("contexts.json");
        let before = std::fs::read(&path).unwrap();
        let mut bad = definition("bad", "Bad");
        bad.accent = Some("not-a-color".into());
        assert!(
            registry
                .reconcile_configured(&["work".into()], &[definition("new", "New"), bad])
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(matches!(
            registry
                .reconcile_configured(&[], &[definition("same", "One"), definition("same", "Two")]),
            Err(ContextRegistryError::DuplicateName)
        ));
        assert_eq!(std::fs::read(path).unwrap(), before);
    }

    #[test]
    fn unchanged_definitions_do_not_rewrite_generated_state() {
        let (roots, mut registry) = registry();
        let value = definition("work", "Work");
        registry
            .reconcile_configured(&[], std::slice::from_ref(&value))
            .unwrap();
        let path = roots.state.join("contexts.json");
        let before = std::fs::metadata(&path).unwrap();
        registry
            .reconcile_configured(&["work".into()], &[value])
            .unwrap();
        let after = std::fs::metadata(path).unwrap();
        assert_eq!(before.modified().unwrap(), after.modified().unwrap());
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(before.ino(), after.ino());
        }
    }
}
