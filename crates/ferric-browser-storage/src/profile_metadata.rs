//! Exact metadata inputs for a deferred profile open. Preparation is read-only;
//! admission rechecks under the existing profile/context registry locks.
use crate::{
    ContextDefinitionUpdate, ContextRegistry, ContextRegistryError, ProfileRecord, ProfileRegistry,
    RegistryError, StorageRoots, durable::RegistryLock,
};
use std::{fs, io, path::Path, time::SystemTime};

#[derive(Eq, PartialEq)]
struct FileVersion {
    length: u64,
    modified: SystemTime,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64),
}

fn file_version(path: &Path) -> io::Result<Option<FileVersion>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Profile metadata is not a regular file: {}", path.display()),
        ));
    }
    #[cfg(unix)]
    let identity = {
        use std::os::unix::fs::MetadataExt;
        (
            metadata.dev(),
            metadata.ino(),
            metadata.ctime(),
            metadata.ctime_nsec(),
        )
    };
    Ok(Some(FileVersion {
        length: metadata.len(),
        modified: metadata.modified()?,
        #[cfg(unix)]
        identity,
    }))
}

/// An owned, non-authoritative revision capture of the exact resolved roots.
/// Unix identity includes change time, so restoring file bytes/mtime or
/// replacing a catalog with equal bytes cannot revive an old capture.
pub struct ProfileMetadataSnapshot {
    roots: StorageRoots,
    versions: [Option<FileVersion>; 3],
}

impl ProfileMetadataSnapshot {
    /// Reads only filesystem metadata; absent catalogs remain absent.
    pub fn capture(roots: &StorageRoots) -> io::Result<Self> {
        Ok(Self {
            roots: roots.clone(),
            versions: Self::versions(roots)?,
        })
    }

    fn versions(roots: &StorageRoots) -> io::Result<[Option<FileVersion>; 3]> {
        Ok([
            file_version(&roots.state.join("profiles.json"))?,
            file_version(&roots.state.join("contexts.json"))?,
            file_version(&roots.state.join("runtime-overrides.toml"))?,
        ])
    }

    /// A scheduling check only; commit repeats it under registry locks.
    pub fn check_current(&self) -> io::Result<()> {
        if self.versions != Self::versions(&self.roots)? {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Profile, context or saved Runtime metadata changed during profile validation",
            ));
        }
        Ok(())
    }

    /// Reserve both existing registries in a fixed order for the bounded
    /// metadata commit, never while waiting for engine validation.
    pub fn begin_commit(self) -> io::Result<ProfileMetadataCommit> {
        self.roots.ensure().map_err(io::Error::other)?;
        let profiles = RegistryLock::acquire(&self.roots.state.join("profiles.json"))?;
        let contexts = RegistryLock::acquire(&self.roots.state.join("contexts.json"))?;
        self.check_current()?;
        Ok(ProfileMetadataCommit {
            roots: self.roots,
            profiles,
            contexts,
        })
    }
}

/// A short metadata lease used only by the admitted bootstrap worker. Existing
/// registry validation and atomic writers perform every mutation.
pub struct ProfileMetadataCommit {
    roots: StorageRoots,
    profiles: RegistryLock,
    contexts: RegistryLock,
}

impl ProfileMetadataCommit {
    pub fn get_or_create_profile(
        &self,
        name: &str,
        label: &str,
    ) -> Result<(ProfileRecord, Vec<String>), RegistryError> {
        let mut registry = ProfileRegistry::open_locked(&self.roots, &self.profiles)?;
        let selected = registry
            .get_or_create_locked(name, label, &self.profiles)?
            .clone();
        let names = registry
            .profiles()
            .iter()
            .map(|profile| profile.name.clone())
            .collect();
        Ok((selected, names))
    }

    pub fn open_contexts(&self) -> Result<ContextRegistry, ContextRegistryError> {
        ContextRegistry::open_locked(&self.roots, &self.contexts)
    }

    pub fn reconcile_contexts(
        &self,
        registry: &mut ContextRegistry,
        definitions: &[ContextDefinitionUpdate],
    ) -> Result<(), ContextRegistryError> {
        registry.reconcile_configured_locked(&[], definitions, &self.contexts)
    }
}
