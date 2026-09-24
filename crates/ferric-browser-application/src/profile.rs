use std::path::{Path, PathBuf};

use ferric_browser_core::PrivacyKind;
use ferric_browser_storage::{
    ContextMember, ContextRecord, ContextRegistry, ProfileLock, StorageRoots,
};
use uuid::Uuid;

/// Resources transferred into the application when a profile becomes active.
#[derive(Debug)]
pub(crate) struct ProfileActivation {
    pub(crate) name: String,
    pub(crate) id: Option<Uuid>,
    pub(crate) privacy: PrivacyKind,
    pub(crate) roots: Option<StorageRoots>,
    pub(crate) lock: Option<ProfileLock>,
    pub(crate) contexts: Option<ContextRegistry>,
    pub(crate) storage_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileSnapshot {
    pub generation: u64,
    pub name: String,
    pub id: Option<Uuid>,
    pub privacy: PrivacyKind,
    pub durable: bool,
    pub roots: Option<StorageRoots>,
}

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextChange {
    Created,
    Removed,
    MembershipSaved,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContextEffect {
    pub generation: u64,
    pub change: ContextChange,
    pub context: ContextRecord,
}

#[derive(Debug)]
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
}
