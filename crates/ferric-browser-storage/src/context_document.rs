//! Versioned generated catalog; legacy records are retained without guessing ownership.
use super::{ContextRecord, ContextRegistryError, context_io, validate_name, validate_records};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, io, path::Path};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ContextDocument {
    schema_version: u32,
    pub revision: u64,
    pub configured_names: BTreeSet<String>,
    pub contexts: Vec<ContextRecord>,
}
impl ContextDocument {
    fn legacy(contexts: Vec<ContextRecord>) -> Self {
        Self {
            schema_version: 1,
            revision: 0,
            configured_names: BTreeSet::new(),
            contexts,
        }
    }
    pub(super) fn validate(&self, path: &Path) -> Result<(), ContextRegistryError> {
        if self.schema_version != 1 {
            return Err(ContextRegistryError::Json {
                path: path.to_owned(),
                message: "unsupported context catalog schema".into(),
            });
        }
        validate_records(&self.contexts)?;
        for name in &self.configured_names {
            validate_name(name)?;
            if !self.contexts.iter().any(|record| &record.name == name) {
                return Err(ContextRegistryError::Json {
                    path: path.to_owned(),
                    message: "configured context identity is missing".into(),
                });
            }
        }
        Ok(())
    }
}
pub(super) fn load(path: &Path) -> Result<ContextDocument, ContextRegistryError> {
    let document = match fs::read(path) {
        Ok(bytes) => {
            let json_error = |error: serde_json::Error| ContextRegistryError::Json {
                path: path.to_owned(),
                message: error.to_string(),
            };
            let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(json_error)?;
            if value.is_array() {
                ContextDocument::legacy(serde_json::from_value(value).map_err(json_error)?)
            } else {
                serde_json::from_value(value).map_err(json_error)?
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            ContextDocument::legacy(Vec::new())
        }
        Err(error) => return Err(context_io(path, &error)),
    };
    document.validate(path)?;
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContextDefinitionUpdate, ContextRegistry, RootSpec, StorageRoots};
    fn definition(name: &str) -> ContextDefinitionUpdate {
        ContextDefinitionUpdate {
            name: name.into(),
            label: name.into(),
            profile: "default".into(),
            sessions: vec![],
            workspace: None,
            accent: None,
            default_target: "reuse".into(),
        }
    }
    #[test]
    fn legacy_identity_survives_upgrade_and_offline_removal_keeps_command_contexts() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        let path = roots.state.join("contexts.json");
        let mut original = ContextRegistry::open(&roots).unwrap();
        let id = original.create("manual", "Manual", "default").unwrap().id;
        let legacy = serde_json::to_vec(original.contexts()).unwrap();
        fs::write(&path, &legacy).unwrap();
        let mut upgraded = ContextRegistry::open(&roots).unwrap();
        assert_eq!(upgraded.revision(), 0);
        assert_eq!(fs::read(&path).unwrap(), legacy);
        upgraded
            .reconcile_configured(&[], &[definition("work")])
            .unwrap();
        let revision = upgraded.revision();
        let mut reopened = ContextRegistry::open(&roots).unwrap();
        reopened.reconcile_configured(&[], &[]).unwrap();
        assert!(reopened.revision() > revision);
        assert_eq!(reopened.contexts().len(), 1);
        assert_eq!(reopened.contexts()[0].id, id);
        roots.cleanup().unwrap();
    }
    #[test]
    fn single_definition_upserts_retain_other_authored_definitions() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        let mut registry = ContextRegistry::open(&roots).unwrap();
        registry
            .upsert_configured_definition(&definition("work"))
            .unwrap();
        registry
            .upsert_configured_definition(&definition("research"))
            .unwrap();
        assert_eq!(registry.contexts().len(), 2);
        registry
            .reconcile_configured(&[], &[definition("work")])
            .unwrap();
        assert_eq!(registry.contexts().len(), 1);
        assert_eq!(registry.contexts()[0].name, "work");
        roots.cleanup().unwrap();
    }
    #[test]
    fn no_op_refresh_keeps_the_durable_revision() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        let mut registry = ContextRegistry::open(&roots).unwrap();
        registry
            .reconcile_configured(&[], &[definition("work")])
            .unwrap();
        let revision = registry.revision();
        registry
            .reconcile_configured(&[], &[definition("work")])
            .unwrap();
        assert_eq!(registry.revision(), revision);
        roots.cleanup().unwrap();
    }
    #[test]
    fn malformed_envelopes_are_rejected_without_rewriting() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        roots.ensure().unwrap();
        let path = roots.state.join("contexts.json");
        for document in [
            serde_json::json!({"schema_version":2,"revision":1,"configured_names":[],"contexts":[]}),
            serde_json::json!({"schema_version":1,"revision":1,"configured_names":["missing"],"contexts":[]}),
            serde_json::json!({"schema_version":1,"revision":1,"configured_names":[],"contexts":[],"extra":true}),
        ] {
            let bytes = serde_json::to_vec(&document).unwrap();
            fs::write(&path, &bytes).unwrap();
            assert!(ContextRegistry::open(&roots).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        roots.cleanup().unwrap();
    }
    #[test]
    fn exhausted_revision_cannot_publish_or_overwrite_a_record() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        let mut registry = ContextRegistry::open(&roots).unwrap();
        registry.create("manual", "Manual", "default").unwrap();
        let path = roots.state.join("contexts.json");
        let mut document: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        document["revision"] = u64::MAX.into();
        let bytes = serde_json::to_vec(&document).unwrap();
        fs::write(&path, &bytes).unwrap();
        let mut registry = ContextRegistry::open(&roots).unwrap();
        assert!(registry.create("extra", "Extra", "default").is_err());
        assert_eq!(registry.contexts().len(), 1);
        assert_eq!(registry.revision(), u64::MAX);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        roots.cleanup().unwrap();
    }
    #[test]
    fn native_context_descriptors_round_trip_and_stay_out_of_visit_history() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        let mut registry = ContextRegistry::open(&roots).unwrap();
        registry.create("work", "Work", "default").unwrap();
        for url in ["rb://start", "rb://blank"] {
            assert!(!crate::is_safe_history_url(url));
            registry.save_membership("work", vec![member(url)]).unwrap();
            let reopened = ContextRegistry::open(&roots).unwrap();
            assert_eq!(
                reopened.contexts()[0].members[0].tab_descriptors[0]
                    .safe_restore_url
                    .as_deref(),
                Some(url)
            );
        }
        roots.cleanup().unwrap();
    }
    #[test]
    fn context_descriptor_policy_rejects_unsafe_or_extended_native_addresses_atomically() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        let mut registry = ContextRegistry::open(&roots).unwrap();
        registry.create("work", "Work", "default").unwrap();
        registry
            .save_membership("work", vec![member("rb://blank")])
            .unwrap();
        let before = fs::read(roots.state.join("contexts.json")).unwrap();
        for url in [
            "rb://start?token=secret",
            "rb://blank#secret",
            "rb://settings",
            "javascript:alert(1)",
            "data:text/html,hello",
            "https://example.test/?token=secret",
            "https://user:password@example.test/",
        ] {
            assert!(
                registry.save_membership("work", vec![member(url)]).is_err(),
                "{url}"
            );
            assert_eq!(fs::read(roots.state.join("contexts.json")).unwrap(), before);
            assert_eq!(
                registry.contexts()[0].members[0].tab_descriptors[0]
                    .safe_restore_url
                    .as_deref(),
                Some("rb://blank")
            );
        }
        roots.cleanup().unwrap();
    }
    fn member(url: &str) -> crate::ContextMember {
        crate::ContextMember {
            window_id: "window-1".into(),
            tabs: vec!["tab-1".into()],
            selected_tab: Some("tab-1".into()),
            tab_descriptors: vec![crate::ContextTabDescriptor {
                tab_id: "tab-1".into(),
                safe_restore_url: Some(url.into()),
                title: String::new(),
                pinned: false,
                muted: false,
                zoom: 1.0,
            }],
            selected_index: Some(0),
        }
    }
    #[test]
    fn startup_lookup_reads_created_context_identity_without_rewriting() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        let mut registry = ContextRegistry::open(&roots).unwrap();
        let created = registry.create("saved", "Saved", "work").unwrap().clone();
        drop(registry);
        let path = roots.state.join("contexts.json");
        let bytes = fs::read(&path).unwrap();
        let loaded = ContextRegistry::load_created_context(&roots, "saved")
            .unwrap()
            .unwrap();
        assert_eq!(loaded, created);
        assert!(
            ContextRegistry::load_created_context(&roots, "missing")
                .unwrap()
                .is_none()
        );
        assert_eq!(fs::read(path).unwrap(), bytes);
        roots.cleanup().unwrap();
    }

    #[test]
    fn startup_lookup_excludes_configuration_owned_identities_after_restart() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        let mut registry = ContextRegistry::open(&roots).unwrap();
        let id = registry.create("saved", "Saved", "default").unwrap().id;
        registry
            .reconcile_configured(&[], &[definition("configured")])
            .unwrap();
        drop(registry);
        assert!(
            ContextRegistry::load_created_context(&roots, "configured")
                .unwrap()
                .is_none()
        );
        assert_eq!(
            ContextRegistry::load_created_context(&roots, "saved")
                .unwrap()
                .unwrap()
                .id,
            id
        );
        let mut reopened = ContextRegistry::open(&roots).unwrap();
        reopened.reconcile_configured(&[], &[]).unwrap();
        assert_eq!(reopened.contexts().len(), 1);
        assert_eq!(reopened.contexts()[0].id, id);
        roots.cleanup().unwrap();
    }

    #[test]
    fn startup_lookup_reports_corrupt_catalog_without_changing_it() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        roots.ensure().unwrap();
        let path = roots.state.join("contexts.json");
        let corrupt = b"{invalid context catalog";
        fs::write(&path, corrupt).unwrap();
        assert!(ContextRegistry::load_created_context(&roots, "saved").is_err());
        assert_eq!(fs::read(path).unwrap(), corrupt);
        roots.cleanup().unwrap();
    }
}
