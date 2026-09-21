use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{StorageRoots, durable::RegistryLock};

const MAX_NAME_LENGTH: usize = 32;
const MAX_LABEL_LENGTH: usize = 128;
const MAX_SESSION_REFERENCES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContextSaveStep {
    TempSynced,
    Renamed,
    DirectorySynced,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextMember {
    pub window_id: String,
    #[serde(default)]
    pub tabs: Vec<String>,
    pub selected_tab: Option<String>,
    #[serde(default)]
    pub tab_descriptors: Vec<ContextTabDescriptor>,
    #[serde(default)]
    pub selected_index: Option<usize>,
}

/// A safe, profile-local descriptor used to lazily reconstruct one context
/// tab after the original runtime identity no longer exists.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextTabDescriptor {
    pub tab_id: String,
    pub safe_restore_url: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub muted: bool,
    #[serde(default = "default_zoom")]
    pub zoom: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextRecord {
    pub id: Uuid,
    pub name: String,
    pub label: String,
    pub profile: String,
    #[serde(default)]
    pub sessions: Vec<String>,
    pub workspace: Option<String>,
    pub accent: Option<String>,
    pub default_target: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub members: Vec<ContextMember>,
}

#[derive(Debug)]
pub enum ContextRegistryError {
    Io { path: PathBuf, message: String },
    Json { path: PathBuf, message: String },
    InvalidName,
    InvalidLabel,
    InvalidProfile,
    InvalidTarget,
    InvalidWorkspace,
    InvalidAccent,
    ProfileChangeRequiresMigration,
    DuplicateName,
    DuplicateId,
    NotFound,
}

impl std::fmt::Display for ContextRegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } | Self::Json { path, message } => {
                write!(formatter, "{}: {message}", path.display())
            }
            Self::InvalidName => {
                formatter.write_str("context name must be a 1–32 character lowercase slug")
            }
            Self::InvalidLabel => {
                formatter.write_str("context label must be nonempty and at most 128 characters")
            }
            Self::InvalidProfile => {
                formatter.write_str("context profile must be a 1–32 character lowercase slug")
            }
            Self::InvalidTarget => formatter
                .write_str("context default_target must be reuse, reuse-or-window, or window"),
            Self::InvalidWorkspace => formatter.write_str(
                "context workspace must be a nonempty selector of at most 128 characters",
            ),
            Self::InvalidAccent => {
                formatter.write_str("context accent must be #RRGGBB or #RRGGBBAA")
            }
            Self::ProfileChangeRequiresMigration => formatter.write_str(
                "context profile cannot change while saved membership exists; migrate it explicitly",
            ),
            Self::DuplicateName => formatter.write_str("context name already exists"),
            Self::DuplicateId => formatter.write_str("context id already exists"),
            Self::NotFound => formatter.write_str("context was not found"),
        }
    }
}

impl std::error::Error for ContextRegistryError {}

#[derive(Debug)]
pub struct ContextRegistry {
    path: PathBuf,
    contexts: Vec<ContextRecord>,
}

impl ContextRegistry {
    /// Opens the generated context membership registry without touching the
    /// user-maintained context configuration file.
    ///
    /// # Errors
    ///
    /// Returns an error when the state root cannot be created, the registry
    /// cannot be decoded, or a stored record violates its bounds.
    pub fn open(roots: &StorageRoots) -> Result<Self, ContextRegistryError> {
        roots.ensure().map_err(|error| ContextRegistryError::Io {
            path: roots.state.clone(),
            message: error.to_string(),
        })?;
        let path = roots.state.join("contexts.json");
        let _lock = RegistryLock::acquire(&path).map_err(|error| context_io(&path, &error))?;
        let contexts = load_records(&path)?;
        Ok(Self { path, contexts })
    }

    #[must_use]
    pub fn contexts(&self) -> &[ContextRecord] {
        &self.contexts
    }

    /// Creates a context with an empty generated membership snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid or duplicate metadata, or an atomic-write
    /// failure.
    ///
    /// # Panics
    ///
    /// This relies on the just-pushed record remaining in the vector until the
    /// final lookup.
    pub fn create(
        &mut self,
        name: &str,
        label: &str,
        profile: &str,
    ) -> Result<&ContextRecord, ContextRegistryError> {
        self.create_with_workspace(name, label, profile, None)
    }

    /// Creates a context with an optional compositor workspace selector.
    ///
    /// The workspace is presentation intent only; it does not change profile
    /// storage or attempt to move a native window.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid or duplicate metadata, or an atomic-write
    /// failure.
    ///
    /// # Panics
    ///
    /// This relies on the just-pushed record remaining in the vector until the
    /// final lookup.
    pub fn create_with_workspace(
        &mut self,
        name: &str,
        label: &str,
        profile: &str,
        workspace: Option<&str>,
    ) -> Result<&ContextRecord, ContextRegistryError> {
        validate_name(name)?;
        validate_label(label)?;
        validate_profile(profile)?;
        validate_workspace(workspace)?;
        let now = unix_timestamp();
        let id = Uuid::new_v4();
        self.mutate(|contexts| {
            if contexts.iter().any(|context| context.name == name) {
                return Err(ContextRegistryError::DuplicateName);
            }
            contexts.push(ContextRecord {
                id,
                name: name.into(),
                label: label.into(),
                profile: profile.into(),
                sessions: Vec::new(),
                workspace: workspace.map(ToOwned::to_owned),
                accent: None,
                default_target: "reuse-or-window".into(),
                created_at: now,
                updated_at: now,
                members: Vec::new(),
            });
            Ok(())
        })?;
        Ok(self
            .contexts
            .iter()
            .find(|context| context.id == id)
            .expect("created context remains present"))
    }

    /// Adds or refreshes metadata from `contexts.toml` while preserving the
    /// generated UUID and saved membership for an existing context.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid metadata or an atomic-write failure.
    ///
    /// # Panics
    ///
    /// This relies on the just-pushed record remaining in the vector until
    /// the final lookup.
    #[allow(clippy::too_many_arguments)]
    pub fn ensure_definition(
        &mut self,
        name: &str,
        label: &str,
        profile: &str,
        sessions: Vec<String>,
        workspace: Option<String>,
        accent: Option<String>,
        default_target: &str,
    ) -> Result<&ContextRecord, ContextRegistryError> {
        validate_name(name)?;
        validate_label(label)?;
        validate_profile(profile)?;
        validate_target(default_target)?;
        validate_workspace(workspace.as_deref())?;
        validate_accent(accent.as_deref())?;
        if sessions.len() > MAX_SESSION_REFERENCES
            || sessions
                .iter()
                .any(|session| validate_profile(session).is_err())
        {
            return Err(ContextRegistryError::InvalidProfile);
        }
        let now = unix_timestamp();
        let mut selected_id = None;
        self.mutate(|contexts| {
            if let Some(context) = contexts.iter_mut().find(|context| context.name == name) {
                if context.profile != profile && !context.members.is_empty() {
                    return Err(ContextRegistryError::ProfileChangeRequiresMigration);
                }
                context.label = label.into();
                context.profile = profile.into();
                context.sessions = sessions;
                context.workspace = workspace;
                context.accent = accent;
                context.default_target = default_target.into();
                context.updated_at = now;
                selected_id = Some(context.id);
                return Ok(());
            }
            let id = Uuid::new_v4();
            contexts.push(ContextRecord {
                id,
                name: name.into(),
                label: label.into(),
                profile: profile.into(),
                sessions,
                workspace,
                accent,
                default_target: default_target.into(),
                created_at: now,
                updated_at: now,
                members: Vec::new(),
            });
            selected_id = Some(id);
            Ok(())
        })?;
        let id = selected_id.expect("definition mutation selects a context");
        Ok(self
            .contexts
            .iter()
            .find(|context| context.id == id)
            .expect("defined context remains present"))
    }

    /// Removes one context record without touching profiles or sessions.
    ///
    /// # Errors
    ///
    /// Returns an error when the context is absent or the atomic write fails.
    pub fn remove(&mut self, name: &str) -> Result<ContextRecord, ContextRegistryError> {
        let mut removed = None;
        self.mutate(|contexts| {
            let Some(index) = contexts.iter().position(|context| context.name == name) else {
                return Err(ContextRegistryError::NotFound);
            };
            removed = Some(contexts.remove(index));
            Ok(())
        })?;
        removed.ok_or(ContextRegistryError::NotFound)
    }

    /// Replaces saved membership after validating only the bounded generated
    /// descriptors. This never changes the context's profile affinity.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid membership, an unknown context, or an
    /// atomic-write failure.
    pub fn save_membership(
        &mut self,
        name: &str,
        members: Vec<ContextMember>,
    ) -> Result<(), ContextRegistryError> {
        self.save_membership_with_hook(name, members, |_| Ok(()))
    }

    fn save_membership_with_hook(
        &mut self,
        name: &str,
        members: Vec<ContextMember>,
        hook: impl FnMut(ContextSaveStep) -> Result<(), ContextRegistryError>,
    ) -> Result<(), ContextRegistryError> {
        validate_members(&members)?;
        self.mutate_with_hook(
            |contexts| {
                let Some(context) = contexts.iter_mut().find(|context| context.name == name) else {
                    return Err(ContextRegistryError::NotFound);
                };
                context.members = members;
                context.updated_at = unix_timestamp();
                Ok(())
            },
            hook,
        )
    }

    fn mutate(
        &mut self,
        mutation: impl FnOnce(&mut Vec<ContextRecord>) -> Result<(), ContextRegistryError>,
    ) -> Result<(), ContextRegistryError> {
        self.mutate_with_hook(mutation, |_| Ok(()))
    }

    fn mutate_with_hook(
        &mut self,
        mutation: impl FnOnce(&mut Vec<ContextRecord>) -> Result<(), ContextRegistryError>,
        mut hook: impl FnMut(ContextSaveStep) -> Result<(), ContextRegistryError>,
    ) -> Result<(), ContextRegistryError> {
        let _lock =
            RegistryLock::acquire(&self.path).map_err(|error| context_io(&self.path, &error))?;
        let mut contexts = load_records(&self.path)?;
        mutation(&mut contexts)?;
        validate_records(&contexts)?;
        let bytes =
            serde_json::to_vec_pretty(&contexts).map_err(|error| ContextRegistryError::Json {
                path: self.path.clone(),
                message: error.to_string(),
            })?;
        let temp_path = self
            .path
            .with_file_name(format!(".contexts.json.tmp-{}", Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| context_io(&temp_path, &error))?;
        if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
            let _ = fs::remove_file(&temp_path);
            return Err(context_io(&self.path, &error));
        }
        if let Err(error) = hook(ContextSaveStep::TempSynced) {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
        if let Err(error) = fs::rename(&temp_path, &self.path) {
            let _ = fs::remove_file(&temp_path);
            return Err(context_io(&self.path, &error));
        }
        hook(ContextSaveStep::Renamed)?;
        let parent = self.path.parent().ok_or_else(|| ContextRegistryError::Io {
            path: self.path.clone(),
            message: "context registry path has no parent".into(),
        })?;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| context_io(parent, &error))?;
        hook(ContextSaveStep::DirectorySynced)?;
        self.contexts = contexts;
        Ok(())
    }
}

fn load_records(path: &Path) -> Result<Vec<ContextRecord>, ContextRegistryError> {
    let contexts = match fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|error| ContextRegistryError::Json {
                path: path.to_owned(),
                message: error.to_string(),
            })?
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(context_io(path, &error)),
    };
    validate_records(&contexts)?;
    Ok(contexts)
}

fn validate_records(contexts: &[ContextRecord]) -> Result<(), ContextRegistryError> {
    let mut names = std::collections::BTreeSet::new();
    let mut ids = std::collections::BTreeSet::new();
    for context in contexts {
        validate_name(&context.name)?;
        validate_label(&context.label)?;
        validate_profile(&context.profile)?;
        validate_target(&context.default_target)?;
        validate_workspace(context.workspace.as_deref())?;
        validate_accent(context.accent.as_deref())?;
        if !names.insert(&context.name) {
            return Err(ContextRegistryError::DuplicateName);
        }
        if !ids.insert(context.id) {
            return Err(ContextRegistryError::DuplicateId);
        }
        if context.created_at > context.updated_at {
            return Err(ContextRegistryError::Json {
                path: PathBuf::from("contexts.json"),
                message: "context timestamps are out of order".into(),
            });
        }
        if context.sessions.len() > MAX_SESSION_REFERENCES {
            return Err(ContextRegistryError::Json {
                path: PathBuf::from("contexts.json"),
                message: "context has too many session references".into(),
            });
        }
        validate_members(&context.members)?;
    }
    Ok(())
}

fn validate_members(members: &[ContextMember]) -> Result<(), ContextRegistryError> {
    if members.len() > 256 {
        return Err(ContextRegistryError::Json {
            path: PathBuf::from("contexts.json"),
            message: "context has too many saved windows".into(),
        });
    }
    for member in members {
        if member.window_id.is_empty() || member.window_id.len() > 128 {
            return Err(ContextRegistryError::Json {
                path: PathBuf::from("contexts.json"),
                message: "context membership has an invalid window id".into(),
            });
        }
        if member.tabs.len() > 512
            || member
                .tabs
                .iter()
                .any(|tab| tab.is_empty() || tab.len() > 128)
        {
            return Err(ContextRegistryError::Json {
                path: PathBuf::from("contexts.json"),
                message: "context membership has invalid tabs".into(),
            });
        }
        if member.tab_descriptors.len() > 512
            || member
                .selected_index
                .is_some_and(|index| index >= member.tab_descriptors.len())
        {
            return Err(ContextRegistryError::Json {
                path: PathBuf::from("contexts.json"),
                message: "context membership has invalid tab descriptors".into(),
            });
        }
        for descriptor in &member.tab_descriptors {
            if descriptor.tab_id.is_empty()
                || descriptor.tab_id.len() > 128
                || descriptor.tab_id.chars().any(char::is_control)
                || descriptor.title.len() > 4 * 1024
                || descriptor.title.chars().any(char::is_control)
                || !descriptor.zoom.is_finite()
                || !(0.25..=5.0).contains(&descriptor.zoom)
                || descriptor
                    .safe_restore_url
                    .as_deref()
                    .is_some_and(|url| !crate::is_safe_history_url(url))
            {
                return Err(ContextRegistryError::Json {
                    path: PathBuf::from("contexts.json"),
                    message: "context membership has an unsafe tab descriptor".into(),
                });
            }
        }
    }
    Ok(())
}

fn default_zoom() -> f64 {
    1.0
}

fn validate_name(name: &str) -> Result<(), ContextRegistryError> {
    if name.is_empty()
        || name.len() > MAX_NAME_LENGTH
        || !name.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
        || name.starts_with('-')
        || name.ends_with('-')
    {
        return Err(ContextRegistryError::InvalidName);
    }
    Ok(())
}

fn validate_label(label: &str) -> Result<(), ContextRegistryError> {
    if label.trim().is_empty() || label.chars().count() > MAX_LABEL_LENGTH {
        return Err(ContextRegistryError::InvalidLabel);
    }
    Ok(())
}

fn validate_profile(profile: &str) -> Result<(), ContextRegistryError> {
    validate_name(profile).map_err(|_| ContextRegistryError::InvalidProfile)
}

fn validate_target(target: &str) -> Result<(), ContextRegistryError> {
    if matches!(target, "reuse" | "reuse-or-window" | "window") {
        Ok(())
    } else {
        Err(ContextRegistryError::InvalidTarget)
    }
}

fn validate_workspace(workspace: Option<&str>) -> Result<(), ContextRegistryError> {
    if workspace.is_some_and(|workspace| {
        workspace.is_empty() || workspace.len() > 128 || workspace.chars().any(char::is_control)
    }) {
        Err(ContextRegistryError::InvalidWorkspace)
    } else {
        Ok(())
    }
}

fn validate_accent(accent: Option<&str>) -> Result<(), ContextRegistryError> {
    if accent.is_some_and(|accent| {
        !((accent.len() == 7 || accent.len() == 9)
            && accent.starts_with('#')
            && accent[1..]
                .chars()
                .all(|character| character.is_ascii_hexdigit()))
    }) {
        Err(ContextRegistryError::InvalidAccent)
    } else {
        Ok(())
    }
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

fn context_io(path: &Path, error: &io::Error) -> ContextRegistryError {
    ContextRegistryError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RootSpec, StorageRoots};

    #[test]
    fn context_registry_round_trips_and_preserves_profile_boundary() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        let mut registry = ContextRegistry::open(&roots).expect("open");
        let record = registry
            .create_with_workspace("work", "Work", "work", Some("3"))
            .expect("create");
        let id = record.id;
        assert_eq!(record.workspace.as_deref(), Some("3"));
        registry
            .save_membership(
                "work",
                vec![ContextMember {
                    window_id: "window-1".into(),
                    tabs: vec!["tab-1".into()],
                    selected_tab: Some("tab-1".into()),
                    tab_descriptors: vec![ContextTabDescriptor {
                        tab_id: "tab-1".into(),
                        safe_restore_url: Some("https://example.test/".into()),
                        title: "Example".into(),
                        pinned: false,
                        muted: false,
                        zoom: 1.0,
                    }],
                    selected_index: Some(0),
                }],
            )
            .expect("membership");
        let reopened = ContextRegistry::open(&roots).expect("reopen");
        assert_eq!(reopened.contexts()[0].id, id);
        assert_eq!(reopened.contexts()[0].profile, "work");
        assert_eq!(reopened.contexts()[0].members.len(), 1);
        assert_eq!(
            reopened.contexts()[0].members[0].tab_descriptors[0]
                .safe_restore_url
                .as_deref(),
            Some("https://example.test/")
        );
        assert_eq!(reopened.contexts()[0].members[0].selected_index, Some(0));
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn context_registry_rejects_invalid_records() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        let mut registry = ContextRegistry::open(&roots).expect("open");
        assert_eq!(
            registry
                .create("Work", "Work", "work")
                .unwrap_err()
                .to_string(),
            "context name must be a 1–32 character lowercase slug"
        );
        assert_eq!(
            registry
                .create("work", "Work", "Private")
                .unwrap_err()
                .to_string(),
            "context profile must be a 1–32 character lowercase slug"
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn context_records_reject_unknown_generated_fields() {
        let value = serde_json::json!({
            "id": Uuid::new_v4(),
            "name": "research",
            "label": "Research",
            "profile": "default",
            "sessions": [],
            "workspace": null,
            "accent": null,
            "default_target": "reuse-or-window",
            "created_at": 1,
            "updated_at": 1,
            "members": [],
            "unexpected": true
        });
        assert!(serde_json::from_value::<ContextRecord>(value).is_err());

        let value = serde_json::json!({
            "window_id": "window-1",
            "tabs": [],
            "selected_tab": null,
            "tab_descriptors": [],
            "selected_index": null,
            "unexpected": true
        });
        assert!(serde_json::from_value::<ContextMember>(value).is_err());
    }

    #[test]
    fn context_membership_faults_keep_registry_recoverable() {
        fn member(window_id: &str) -> ContextMember {
            ContextMember {
                window_id: window_id.into(),
                tabs: Vec::new(),
                selected_tab: None,
                tab_descriptors: Vec::new(),
                selected_index: None,
            }
        }

        for failure_step in [
            ContextSaveStep::TempSynced,
            ContextSaveStep::Renamed,
            ContextSaveStep::DirectorySynced,
        ] {
            let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
            let mut registry = ContextRegistry::open(&roots).expect("open");
            registry
                .create("research", "Research", "default")
                .expect("create");
            registry
                .save_membership("research", vec![member("window-old")])
                .expect("initial membership");
            let old_members = registry.contexts()[0].members.clone();
            let old_updated_at = registry.contexts()[0].updated_at;
            let error = registry
                .save_membership_with_hook("research", vec![member("window-new")], |step| {
                    (step != failure_step)
                        .then_some(())
                        .ok_or_else(|| ContextRegistryError::Json {
                            path: PathBuf::from("contexts.json"),
                            message: "injected interruption".into(),
                        })
                })
                .expect_err("injected interruption");
            assert!(error.to_string().contains("injected interruption"));
            assert_eq!(registry.contexts()[0].members, old_members);
            assert_eq!(registry.contexts()[0].updated_at, old_updated_at);

            let reopened = ContextRegistry::open(&roots).expect("recoverable registry");
            let members = &reopened.contexts()[0].members;
            if failure_step == ContextSaveStep::TempSynced {
                assert_eq!(members[0].window_id, "window-old");
            } else {
                assert_eq!(members[0].window_id, "window-new");
            }
            roots.cleanup().expect("cleanup");
        }
    }

    #[test]
    fn context_profile_change_requires_explicit_migration_for_saved_members() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        let mut registry = ContextRegistry::open(&roots).expect("open");
        registry
            .create("research", "Research", "default")
            .expect("create");
        registry
            .save_membership(
                "research",
                vec![ContextMember {
                    window_id: "window-1".into(),
                    tabs: Vec::new(),
                    selected_tab: None,
                    tab_descriptors: Vec::new(),
                    selected_index: None,
                }],
            )
            .expect("membership");
        assert_eq!(
            registry
                .ensure_definition(
                    "research",
                    "Research",
                    "work",
                    Vec::new(),
                    None,
                    None,
                    "reuse-or-window",
                )
                .unwrap_err()
                .to_string(),
            "context profile cannot change while saved membership exists; migrate it explicitly"
        );
        assert_eq!(registry.contexts()[0].profile, "default");
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn removing_context_does_not_touch_the_profile_name() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        let mut registry = ContextRegistry::open(&roots).expect("open");
        registry
            .create("research", "Research", "default")
            .expect("create");
        let removed = registry.remove("research").expect("remove");
        assert_eq!(removed.profile, "default");
        assert!(registry.contexts().is_empty());
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn concurrent_context_snapshots_preserve_both_mutations() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        let mut first = ContextRegistry::open(&roots).expect("first registry");
        let mut second = ContextRegistry::open(&roots).expect("second registry");

        first
            .create("first", "First", "default")
            .expect("first mutation");
        second
            .create("second", "Second", "default")
            .expect("second mutation");

        let reopened = ContextRegistry::open(&roots).expect("reopen");
        assert!(
            reopened
                .contexts()
                .iter()
                .any(|context| context.name == "first")
        );
        assert!(
            reopened
                .contexts()
                .iter()
                .any(|context| context.name == "second")
        );
        roots.cleanup().expect("cleanup");
    }
}
