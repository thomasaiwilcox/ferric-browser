use std::{
    fmt,
    path::{Path, PathBuf},
};

use ferric_browser_core::PrivacyKind;
use ferric_browser_storage::{
    ContextMember, ContextRecord, ContextRegistry, ProfileLock, StorageRoots,
};
use uuid::Uuid;

/// Resources transferred into the application when a profile becomes active.
pub(crate) struct ProfileActivation {
    pub(crate) name: String,
    pub(crate) id: Option<Uuid>,
    pub(crate) privacy: PrivacyKind,
    pub(crate) roots: Option<StorageRoots>,
    pub(crate) lock: Option<ProfileLock>,
    pub(crate) contexts: Option<ContextRegistry>,
    pub(crate) storage_path: Option<PathBuf>,
}

impl fmt::Debug for ProfileActivation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProfileActivation")
            .field("name", &"[redacted]")
            .field("has_id", &self.id.is_some())
            .field("privacy", &self.privacy)
            .field("has_roots", &self.roots.is_some())
            .field("has_lock", &self.lock.is_some())
            .field(
                "context_count",
                &self
                    .contexts
                    .as_ref()
                    .map_or(0, |contexts| contexts.contexts().len()),
            )
            .field(
                "storage_path",
                &self.storage_path.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ProfileSnapshot {
    pub generation: u64,
    pub name: String,
    pub id: Option<Uuid>,
    pub privacy: PrivacyKind,
    pub durable: bool,
    pub roots: Option<StorageRoots>,
}

impl std::fmt::Debug for ProfileSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProfileSnapshot")
            .field("generation", &self.generation)
            .field("name", &"[redacted]")
            .field("has_id", &self.id.is_some())
            .field("privacy", &self.privacy)
            .field("durable", &self.durable)
            .field("roots", &self.roots.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub enum ContextCommand {
    Create {
        name: String,
        label: String,
        profile: String,
        workspace: Option<String>,
    },
    Remove {
        name: String,
    },
    SaveMembership {
        name: String,
        members: Vec<ContextMember>,
    },
}

impl fmt::Debug for ContextCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create { .. } => formatter
                .debug_struct("ContextCommand::Create")
                .field("name", &"[redacted]")
                .field("label", &"[redacted]")
                .field("profile", &"[redacted]")
                .field("workspace", &"[redacted]")
                .finish(),
            Self::Remove { .. } => formatter
                .debug_struct("ContextCommand::Remove")
                .field("name", &"[redacted]")
                .finish(),
            Self::SaveMembership { members, .. } => formatter
                .debug_struct("ContextCommand::SaveMembership")
                .field("name", &"[redacted]")
                .field("member_count", &members.len())
                .finish(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextChange {
    Created,
    Removed,
    MembershipSaved,
}

#[derive(Clone, PartialEq)]
pub struct ContextEffect {
    pub generation: u64,
    pub change: ContextChange,
    pub context: ContextRecord,
}

impl fmt::Debug for ContextEffect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContextEffect")
            .field("generation", &self.generation)
            .field("change", &self.change)
            .field("context", &"[redacted]")
            .finish()
    }
}

pub(crate) struct ActiveProfile {
    pub(crate) generation: u64,
    name: String,
    id: Option<Uuid>,
    privacy: PrivacyKind,
    roots: Option<StorageRoots>,
    _lock: Option<ProfileLock>,
    pub(crate) contexts: Option<ContextRegistry>,
    storage_path: Option<PathBuf>,
}

impl fmt::Debug for ActiveProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveProfile")
            .field("generation", &self.generation)
            .field("name", &"[redacted]")
            .field("has_id", &self.id.is_some())
            .field("privacy", &self.privacy)
            .field("has_roots", &self.roots.is_some())
            .field("has_lock", &self._lock.is_some())
            .field(
                "context_count",
                &self
                    .contexts
                    .as_ref()
                    .map_or(0, |contexts| contexts.contexts().len()),
            )
            .field(
                "storage_path",
                &self.storage_path.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}

impl ActiveProfile {
    pub(crate) fn activate(generation: u64, activation: ProfileActivation) -> Self {
        Self {
            generation,
            name: activation.name,
            id: activation.id,
            privacy: activation.privacy,
            roots: activation.roots,
            _lock: activation.lock,
            contexts: activation.contexts,
            storage_path: activation.storage_path,
        }
    }

    pub(crate) fn snapshot(&self) -> ProfileSnapshot {
        ProfileSnapshot {
            generation: self.generation,
            name: self.name.clone(),
            id: self.id,
            privacy: self.privacy,
            durable: self.storage_path.is_some(),
            roots: self.roots.clone(),
        }
    }

    pub(crate) fn storage_path(&self) -> Option<&Path> {
        self.storage_path.as_deref()
    }

    pub(crate) fn versioning_recovery_scope(&self) -> Option<(&Path, Uuid)> {
        self.roots
            .as_ref()
            .zip(self.id)
            .map(|(roots, id)| (roots.state.as_path(), id))
    }
}

#[cfg(test)]
mod privacy_debug_tests {
    use super::*;
    use ferric_browser_core::PrivacyKind;

    #[test]
    fn profile_and_context_projections_redact_names_and_paths() {
        let profile_name = "private-profile-name-sentinel";
        let private_path = "/home/person/private-profile/browser.sqlite";
        let activation = ProfileActivation {
            name: profile_name.into(),
            id: Some(Uuid::new_v4()),
            privacy: PrivacyKind::Normal,
            roots: None,
            lock: None,
            contexts: None,
            storage_path: Some(PathBuf::from(private_path)),
        };
        let activation_debug = format!("{activation:?}");
        let active_debug = format!("{:?}", ActiveProfile::activate(1, activation));
        let context_debug = format!(
            "{:?}",
            ContextCommand::Create {
                name: "private-context-name".into(),
                label: "private-context-label".into(),
                profile: "private-context-profile".into(),
                workspace: Some("private-workspace".into()),
            }
        );
        for debug in [activation_debug, active_debug, context_debug] {
            for secret in [
                profile_name,
                private_path,
                "private-context-name",
                "private-context-label",
                "private-context-profile",
                "private-workspace",
            ] {
                assert!(!debug.contains(secret));
            }
        }
    }
}
