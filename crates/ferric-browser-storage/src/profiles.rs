use std::{
    fs::{self, File, OpenOptions},
    io::{self, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, Weak},
};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    StorageRoots,
    durable::{RegistryLock, write_atomic},
};

const MAX_NAME_LENGTH: usize = 32;
const MAX_LABEL_LENGTH: usize = 128;
const DELETION_MANIFEST_VERSION: u32 = 1;

static HELD_PROFILE_LOCKS: OnceLock<
    Mutex<std::collections::HashMap<PathBuf, Weak<ProfileLockState>>>,
> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct ProfileLock {
    #[allow(dead_code)]
    inner: Arc<ProfileLockState>,
}

#[derive(Debug)]
struct ProfileLockState {
    _file: File,
}

impl ProfileLock {
    /// Acquires the exclusive durable lock for one profile.
    ///
    /// Multiple windows in this process share one lock; another process is
    /// refused with the recorded owner information.
    ///
    /// # Errors
    ///
    /// Returns an error when the lock cannot be created or another process
    /// already owns it.
    pub fn acquire(roots: &StorageRoots, profile_id: Uuid) -> Result<Self, ProfileLockError> {
        roots.ensure().map_err(|error| ProfileLockError::Io {
            path: roots.runtime.clone(),
            message: error.to_string(),
        })?;
        let directory = roots.runtime.join("profiles");
        fs::create_dir_all(&directory).map_err(|error| lock_io(&directory, &error))?;
        set_private_permissions(&directory).map_err(|error| lock_io(&directory, &error))?;
        let path = directory.join(format!("{profile_id}.lock"));
        let locks = HELD_PROFILE_LOCKS.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
        let mut held = locks.lock().map_err(|_| ProfileLockError::Io {
            path: path.clone(),
            message: "profile lock registry is poisoned".into(),
        })?;
        if let Some(existing) = held.get(&path).and_then(Weak::upgrade) {
            return Ok(Self { inner: existing });
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options
            .open(&path)
            .map_err(|error| lock_io(&path, &error))?;
        match file.try_lock_exclusive() {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                let owner = fs::read_to_string(&path).ok();
                return Err(ProfileLockError::Busy { path, owner });
            }
            Err(error) => return Err(lock_io(&path, &error)),
        }
        if let Err(error) = set_private_file_permissions(&path) {
            return Err(lock_io(&path, &error));
        }
        if let Err(error) = file
            .set_len(0)
            .and_then(|()| file.seek(SeekFrom::Start(0)).map(|_| ()))
            .and_then(|()| file.write_all(format!("pid={}\n", std::process::id()).as_bytes()))
            .and_then(|()| file.sync_all())
        {
            return Err(lock_io(&path, &error));
        }
        if let Err(error) = File::open(&directory).and_then(|directory| directory.sync_all()) {
            return Err(lock_io(&path, &error));
        }
        let inner = Arc::new(ProfileLockState { _file: file });
        held.insert(path, Arc::downgrade(&inner));
        Ok(Self { inner })
    }

    #[must_use]
    pub fn is_held(roots: &StorageRoots, profile_id: Uuid) -> bool {
        let path = roots
            .runtime
            .join("profiles")
            .join(format!("{profile_id}.lock"));
        HELD_PROFILE_LOCKS
            .get()
            .and_then(|locks| locks.lock().ok())
            .and_then(|held| held.get(&path).and_then(Weak::upgrade))
            .is_some()
    }
}

#[derive(Debug)]
pub enum ProfileLockError {
    Io {
        path: PathBuf,
        message: String,
    },
    Busy {
        path: PathBuf,
        owner: Option<String>,
    },
}

impl std::fmt::Display for ProfileLockError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => write!(formatter, "{}: {message}", path.display()),
            Self::Busy { path, owner } => write!(
                formatter,
                "profile is already in use ({}; owner {})",
                path.display(),
                owner.as_deref().unwrap_or("unknown")
            ),
        }
    }
}

impl std::error::Error for ProfileLockError {}

#[derive(Debug)]
pub enum ProfileDataError {
    Io { path: PathBuf, message: String },
    UnsafePath(PathBuf),
}

impl std::fmt::Display for ProfileDataError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => write!(formatter, "{}: {message}", path.display()),
            Self::UnsafePath(path) => write!(
                formatter,
                "refusing to delete unsafe path: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ProfileDataError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileDeletionOutcome {
    pub profile_name: String,
    pub cleanup_pending: bool,
}

#[derive(Debug)]
pub enum ProfileDeletionError {
    Lock(ProfileLockError),
    Registry(RegistryError),
    Data(ProfileDataError),
}

impl std::fmt::Display for ProfileDeletionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lock(error) => write!(formatter, "profile deletion refused: {error}"),
            Self::Registry(error) => write!(formatter, "profile registry update failed: {error}"),
            Self::Data(error) => write!(formatter, "profile data deletion failed: {error}"),
        }
    }
}

impl std::error::Error for ProfileDeletionError {}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeletionManifest {
    version: u32,
    profile_id: Uuid,
    profile_name: String,
    transaction_id: Uuid,
    committed: bool,
}

/// Atomically removes a profile registry entry while staging its
/// Ferric Browser-owned data for crash-recoverable cleanup.
///
/// `QtWebEngine`'s opaque storage remains outside this operation.
///
/// # Errors
///
/// Returns an error before the registry commit when the profile is missing,
/// locked by another process, or its data cannot be safely staged. Once the
/// registry commit succeeds, cleanup failures are reported through
/// [`ProfileDeletionOutcome::cleanup_pending`] instead of turning a committed
/// deletion into an error.
pub fn delete_profile_transaction(
    roots: &StorageRoots,
    name: &str,
) -> Result<ProfileDeletionOutcome, ProfileDeletionError> {
    roots.ensure().map_err(|error| {
        ProfileDeletionError::Data(ProfileDataError::Io {
            path: roots.state.clone(),
            message: error.to_string(),
        })
    })?;
    let registry_path = roots.state.join("profiles.json");
    let _registry_lock = RegistryLock::acquire(&registry_path)
        .map_err(|error| ProfileDeletionError::Registry(io_error(&registry_path, &error)))?;
    let mut profiles = load_records(&registry_path).map_err(ProfileDeletionError::Registry)?;
    recover_deletion_manifests(roots, &profiles).map_err(ProfileDeletionError::Data)?;
    let profile = profiles
        .iter()
        .find(|profile| profile.name == name)
        .cloned()
        .ok_or(ProfileDeletionError::Registry(RegistryError::NotFound))?;
    let _profile_lock =
        ProfileLock::acquire(roots, profile.id).map_err(ProfileDeletionError::Lock)?;

    let transaction_id = Uuid::new_v4();
    let mut manifest = DeletionManifest {
        version: DELETION_MANIFEST_VERSION,
        profile_id: profile.id,
        profile_name: profile.name.clone(),
        transaction_id,
        committed: false,
    };
    let manifest_path = deletion_manifest_path(roots, profile.id, transaction_id);
    prepare_manifest_directory(roots).map_err(ProfileDeletionError::Data)?;
    save_deletion_manifest(&manifest_path, &manifest).map_err(ProfileDeletionError::Data)?;

    if let Err(error) = stage_profile_directories(roots, &manifest) {
        if restore_profile_tombstones(roots, &manifest).is_ok() {
            let _ = remove_manifest(&manifest_path);
        }
        return Err(ProfileDeletionError::Data(error));
    }

    profiles.retain(|candidate| candidate.id != profile.id);
    if let Err(error) = save_records(&registry_path, &profiles) {
        let rollback = restore_profile_tombstones(roots, &manifest);
        if rollback.is_ok() {
            let _ = remove_manifest(&manifest_path);
        }
        return Err(ProfileDeletionError::Registry(error));
    }

    manifest.committed = true;
    let mut cleanup_pending = save_deletion_manifest(&manifest_path, &manifest).is_err();
    if delete_profile_tombstones(roots, &manifest)
        .and_then(|()| remove_manifest(&manifest_path))
        .is_err()
    {
        cleanup_pending = true;
    }

    Ok(ProfileDeletionOutcome {
        profile_name: profile.name,
        cleanup_pending,
    })
}

fn prepare_manifest_directory(roots: &StorageRoots) -> Result<(), ProfileDataError> {
    let path = roots.state.join("profile-deletions");
    fs::create_dir_all(&path).map_err(|error| profile_data_io(&path, &error))?;
    set_private_permissions(&path).map_err(|error| profile_data_io(&path, &error))?;
    File::open(&roots.state)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| profile_data_io(&roots.state, &error))
}

fn deletion_manifest_path(roots: &StorageRoots, profile_id: Uuid, transaction_id: Uuid) -> PathBuf {
    roots
        .state
        .join("profile-deletions")
        .join(format!("{profile_id}-{transaction_id}.json"))
}

fn save_deletion_manifest(
    path: &Path,
    manifest: &DeletionManifest,
) -> Result<(), ProfileDataError> {
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|error| ProfileDataError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    write_atomic(path, &bytes).map_err(|error| profile_data_io(path, &error))
}

fn profile_directories(roots: &StorageRoots, profile_id: Uuid) -> [PathBuf; 3] {
    [
        roots.data.join("profiles").join(profile_id.to_string()),
        roots.state.join("sessions").join(profile_id.to_string()),
        roots.cache.join("profiles").join(profile_id.to_string()),
    ]
}

fn profile_tombstone(path: &Path, transaction_id: Uuid) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("profile");
    path.with_file_name(format!(".{name}.deleting-{transaction_id}"))
}

fn validate_profile_directory(path: &Path) -> Result<bool, ProfileDataError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(true),
        Ok(_) => Err(ProfileDataError::UnsafePath(path.to_owned())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(profile_data_io(path, &error)),
    }
}

fn sync_parent(path: &Path) -> Result<(), ProfileDataError> {
    let parent = path
        .parent()
        .ok_or_else(|| ProfileDataError::UnsafePath(path.to_owned()))?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| profile_data_io(parent, &error))
}

fn stage_profile_directories(
    roots: &StorageRoots,
    manifest: &DeletionManifest,
) -> Result<(), ProfileDataError> {
    for path in profile_directories(roots, manifest.profile_id) {
        if !validate_profile_directory(&path)? {
            continue;
        }
        let tombstone = profile_tombstone(&path, manifest.transaction_id);
        if fs::symlink_metadata(&tombstone).is_ok() {
            return Err(ProfileDataError::UnsafePath(tombstone));
        }
        fs::rename(&path, &tombstone).map_err(|error| profile_data_io(&path, &error))?;
        sync_parent(&path)?;
    }
    Ok(())
}

fn restore_profile_tombstones(
    roots: &StorageRoots,
    manifest: &DeletionManifest,
) -> Result<(), ProfileDataError> {
    for path in profile_directories(roots, manifest.profile_id) {
        let tombstone = profile_tombstone(&path, manifest.transaction_id);
        if !validate_profile_directory(&tombstone)? {
            continue;
        }
        if fs::symlink_metadata(&path).is_ok() {
            return Err(ProfileDataError::UnsafePath(path));
        }
        fs::rename(&tombstone, &path).map_err(|error| profile_data_io(&tombstone, &error))?;
        sync_parent(&path)?;
    }
    Ok(())
}

fn delete_profile_tombstones(
    roots: &StorageRoots,
    manifest: &DeletionManifest,
) -> Result<(), ProfileDataError> {
    for path in profile_directories(roots, manifest.profile_id) {
        let tombstone = profile_tombstone(&path, manifest.transaction_id);
        if !validate_profile_directory(&tombstone)? {
            continue;
        }
        fs::remove_dir_all(&tombstone).map_err(|error| profile_data_io(&tombstone, &error))?;
        sync_parent(&tombstone)?;
    }
    Ok(())
}

fn remove_manifest(path: &Path) -> Result<(), ProfileDataError> {
    match fs::remove_file(path) {
        Ok(()) => sync_parent(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(profile_data_io(path, &error)),
    }
}

fn recover_deletion_manifests(
    roots: &StorageRoots,
    profiles: &[ProfileRecord],
) -> Result<(), ProfileDataError> {
    let directory = roots.state.join("profile-deletions");
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(profile_data_io(&directory, &error)),
    };
    for entry in entries {
        let entry = entry.map_err(|error| profile_data_io(&directory, &error))?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let metadata =
            fs::symlink_metadata(&path).map_err(|error| profile_data_io(&path, &error))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(ProfileDataError::UnsafePath(path));
        }
        let bytes = fs::read(&path).map_err(|error| profile_data_io(&path, &error))?;
        let manifest: DeletionManifest =
            serde_json::from_slice(&bytes).map_err(|error| ProfileDataError::Io {
                path: path.clone(),
                message: error.to_string(),
            })?;
        if manifest.version != DELETION_MANIFEST_VERSION
            || validate_name(&manifest.profile_name).is_err()
        {
            return Err(ProfileDataError::UnsafePath(path));
        }
        if profiles
            .iter()
            .any(|profile| profile.id == manifest.profile_id)
        {
            restore_profile_tombstones(roots, &manifest)?;
        } else {
            delete_profile_tombstones(roots, &manifest)?;
        }
        remove_manifest(&path)?;
    }
    Ok(())
}

/// Deletes only the Ferric Browser-owned metadata directories for one profile.
/// `QtWebEngine`'s opaque storage is intentionally outside this operation.
///
/// # Errors
///
/// Returns an error when a target is not a real directory or cannot be
/// removed.
pub fn delete_profile_data(roots: &StorageRoots, profile_id: Uuid) -> Result<(), ProfileDataError> {
    for path in [
        roots.data.join("profiles").join(profile_id.to_string()),
        roots.state.join("sessions").join(profile_id.to_string()),
        roots.cache.join("profiles").join(profile_id.to_string()),
    ] {
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(profile_data_io(&path, &error)),
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(ProfileDataError::UnsafePath(path));
        }
        fs::remove_dir_all(&path).map_err(|error| profile_data_io(&path, &error))?;
        if let Some(parent) = path.parent()
            && let Ok(directory) = File::open(parent)
        {
            directory
                .sync_all()
                .map_err(|error| profile_data_io(parent, &error))?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ProfilePrivacy {
    Normal,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProfileRecord {
    pub id: Uuid,
    pub name: String,
    pub label: String,
    pub privacy: ProfilePrivacy,
}

#[derive(Debug)]
pub enum RegistryError {
    Io { path: PathBuf, message: String },
    Json { path: PathBuf, message: String },
    InvalidName,
    InvalidLabel,
    DuplicateName,
    NotFound,
    PrivateProfile,
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } | Self::Json { path, message } => {
                write!(formatter, "{}: {message}", path.display())
            }
            Self::InvalidName => {
                formatter.write_str("profile name must be a 1–32 character lowercase slug")
            }
            Self::InvalidLabel => {
                formatter.write_str("profile label must be nonempty and at most 128 characters")
            }
            Self::DuplicateName => formatter.write_str("profile name already exists"),
            Self::NotFound => formatter.write_str("profile was not found"),
            Self::PrivateProfile => {
                formatter.write_str("private profiles are not stored in the durable registry")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

#[derive(Debug)]
pub struct ProfileRegistry {
    roots: StorageRoots,
    path: PathBuf,
    profiles: Vec<ProfileRecord>,
}

impl ProfileRegistry {
    /// Loads the durable registry without deleting entries absent from config.
    ///
    /// # Errors
    ///
    /// Returns an error for unreadable or malformed registry data.
    pub fn open(roots: &StorageRoots) -> Result<Self, RegistryError> {
        roots.ensure().map_err(|error| RegistryError::Io {
            path: roots.state.clone(),
            message: error.to_string(),
        })?;
        let path = roots.state.join("profiles.json");
        let _lock = RegistryLock::acquire(&path).map_err(|error| io_error(&path, &error))?;
        let profiles = load_records(&path)?;
        recover_deletion_manifests(roots, &profiles).map_err(|error| RegistryError::Io {
            path: roots.state.join("profile-deletions"),
            message: error.to_string(),
        })?;
        Ok(Self {
            roots: roots.clone(),
            path,
            profiles,
        })
    }

    #[must_use]
    pub fn profiles(&self) -> &[ProfileRecord] {
        &self.profiles
    }

    ///
    /// # Errors
    ///
    /// Returns a registry validation or atomic-write error while creating the
    /// default profile.
    pub fn get_or_create_default(&mut self) -> Result<&ProfileRecord, RegistryError> {
        self.get_or_create("default", "Default")
    }

    /// Returns an existing normal profile by stable name, or creates it with
    /// a new UUID and persists the registry atomically.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid/duplicate profile data or a failed write.
    pub fn get_or_create(
        &mut self,
        name: &str,
        label: &str,
    ) -> Result<&ProfileRecord, RegistryError> {
        validate_name(name)?;
        validate_label(label)?;
        let mut selected_id = None;
        self.mutate(|profiles| {
            if let Some(profile) = profiles.iter().find(|profile| profile.name == name) {
                selected_id = Some(profile.id);
                return Ok(());
            }
            let id = Uuid::new_v4();
            profiles.push(ProfileRecord {
                id,
                name: name.into(),
                label: label.into(),
                privacy: ProfilePrivacy::Normal,
            });
            selected_id = Some(id);
            Ok(())
        })?;
        let id = selected_id.ok_or(RegistryError::NotFound)?;
        self.profiles
            .iter()
            .find(|profile| profile.id == id)
            .ok_or(RegistryError::NotFound)
    }

    /// Creates a stable UUID-backed normal profile and atomically persists the registry.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid/duplicate names, invalid labels, private
    /// privacy, or a failed atomic write.
    ///
    /// # Panics
    ///
    /// This method relies on the just-pushed profile remaining in the vector;
    /// the vector is not modified between the push and the final lookup.
    pub fn create(
        &mut self,
        name: &str,
        label: &str,
        privacy: ProfilePrivacy,
    ) -> Result<&ProfileRecord, RegistryError> {
        validate_name(name)?;
        validate_label(label)?;
        if privacy != ProfilePrivacy::Normal {
            return Err(RegistryError::PrivateProfile);
        }
        let id = Uuid::new_v4();
        self.mutate(|profiles| {
            if profiles.iter().any(|profile| profile.name == name) {
                return Err(RegistryError::DuplicateName);
            }
            profiles.push(ProfileRecord {
                id,
                name: name.into(),
                label: label.into(),
                privacy,
            });
            Ok(())
        })?;
        Ok(self
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .expect("created profile remains present"))
    }

    /// Renames only a profile's display label; its stable name, UUID, and
    /// filesystem storage paths remain unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing profile, invalid label, or failed
    /// atomic registry write.
    pub fn rename_label(
        &mut self,
        name: &str,
        label: &str,
    ) -> Result<&ProfileRecord, RegistryError> {
        validate_label(label)?;
        let mut renamed_id = None;
        self.mutate(|profiles| {
            let Some(profile) = profiles.iter_mut().find(|profile| profile.name == name) else {
                return Err(RegistryError::NotFound);
            };
            profile.label = label.into();
            renamed_id = Some(profile.id);
            Ok(())
        })?;
        let id = renamed_id.ok_or(RegistryError::NotFound)?;
        self.profiles
            .iter()
            .find(|profile| profile.id == id)
            .ok_or(RegistryError::NotFound)
    }

    /// Removes one profile record while leaving its stable UUID available to
    /// the caller for separately confirmed data cleanup.
    ///
    /// # Errors
    ///
    /// Returns an error when the profile is missing or the registry cannot be
    /// atomically updated.
    pub fn remove(&mut self, name: &str) -> Result<ProfileRecord, RegistryError> {
        let mut removed = None;
        self.mutate(|profiles| {
            let Some(index) = profiles.iter().position(|profile| profile.name == name) else {
                return Err(RegistryError::NotFound);
            };
            removed = Some(profiles.remove(index));
            Ok(())
        })?;
        removed.ok_or(RegistryError::NotFound)
    }

    fn mutate(
        &mut self,
        mutation: impl FnOnce(&mut Vec<ProfileRecord>) -> Result<(), RegistryError>,
    ) -> Result<(), RegistryError> {
        let _lock =
            RegistryLock::acquire(&self.path).map_err(|error| io_error(&self.path, &error))?;
        let mut profiles = load_records(&self.path)?;
        recover_deletion_manifests(&self.roots, &profiles).map_err(|error| RegistryError::Io {
            path: self.roots.state.join("profile-deletions"),
            message: error.to_string(),
        })?;
        mutation(&mut profiles)?;
        validate_records(&profiles)?;
        save_records(&self.path, &profiles)?;
        self.profiles = profiles;
        Ok(())
    }
}

fn load_records(path: &Path) -> Result<Vec<ProfileRecord>, RegistryError> {
    let profiles = match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| RegistryError::Json {
            path: path.to_owned(),
            message: error.to_string(),
        })?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(io_error(path, &error)),
    };
    validate_records(&profiles)?;
    Ok(profiles)
}

fn save_records(path: &Path, profiles: &[ProfileRecord]) -> Result<(), RegistryError> {
    let bytes = serde_json::to_vec_pretty(profiles).map_err(|error| RegistryError::Json {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    write_atomic(path, &bytes).map_err(|error| io_error(path, &error))
}

fn validate_records(profiles: &[ProfileRecord]) -> Result<(), RegistryError> {
    let mut names = std::collections::BTreeSet::new();
    for profile in profiles {
        validate_name(&profile.name)?;
        validate_label(&profile.label)?;
        if profile.privacy != ProfilePrivacy::Normal {
            return Err(RegistryError::PrivateProfile);
        }
        if !names.insert(&profile.name) {
            return Err(RegistryError::DuplicateName);
        }
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), RegistryError> {
    if name.is_empty()
        || name.len() > MAX_NAME_LENGTH
        || name.starts_with('-')
        || name.ends_with('-')
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(RegistryError::InvalidName);
    }
    Ok(())
}

fn validate_label(label: &str) -> Result<(), RegistryError> {
    if label.trim().is_empty()
        || label.chars().count() > MAX_LABEL_LENGTH
        || label.chars().any(char::is_control)
    {
        return Err(RegistryError::InvalidLabel);
    }
    Ok(())
}

fn io_error(path: &Path, error: &io::Error) -> RegistryError {
    RegistryError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    }
}

fn lock_io(path: &Path, error: &io::Error) -> ProfileLockError {
    ProfileLockError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    }
}

fn profile_data_io(path: &Path, error: &io::Error) -> ProfileDataError {
    ProfileDataError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    }
}

fn set_private_permissions(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn set_private_file_permissions(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RootSpec, StorageRoots};
    use std::{process::Command, thread, time::Duration};

    #[test]
    fn profile_lock_crash_test_child() {
        let Ok(base) = std::env::var("FERRIC_BROWSER_PROFILE_LOCK_CHILD_ROOT") else {
            return;
        };
        let profile_id = std::env::var("FERRIC_BROWSER_PROFILE_LOCK_CHILD_ID")
            .expect("child profile id")
            .parse()
            .expect("valid child profile id");
        let ready = PathBuf::from(
            std::env::var("FERRIC_BROWSER_PROFILE_LOCK_CHILD_READY").expect("child ready path"),
        );
        let roots =
            StorageRoots::resolve(RootSpec::Base(PathBuf::from(base))).expect("child roots");
        let _lock = ProfileLock::acquire(&roots, profile_id).expect("child lock");
        std::fs::write(ready, b"ready").expect("signal ready");
        thread::sleep(Duration::from_secs(30));
    }

    #[test]
    fn default_profile_uuid_survives_reopen() {
        let base =
            std::env::temp_dir().join(format!("ferric-browser-profiles-{}", std::process::id()));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("roots");
        let mut first = ProfileRegistry::open(&roots).expect("open");
        let id = first.get_or_create_default().expect("default").id;
        drop(first);
        let mut second = ProfileRegistry::open(&roots).expect("reopen");
        assert_eq!(second.get_or_create_default().expect("default").id, id);
        assert!(matches!(
            second.create("Bad", "Bad", ProfilePrivacy::Normal),
            Err(RegistryError::InvalidName)
        ));
        assert!(
            second
                .create("private", "Private", ProfilePrivacy::Normal)
                .is_ok()
        );
        let renamed = second
            .rename_label("default", "Renamed default")
            .expect("rename");
        assert_eq!(renamed.label, "Renamed default");
        drop(second);
        let reopened = ProfileRegistry::open(&roots).expect("reopen renamed");
        assert_eq!(reopened.profiles()[0].label, "Renamed default");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn concurrent_registry_snapshots_preserve_both_mutations() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        let mut first = ProfileRegistry::open(&roots).expect("first registry");
        let mut second = ProfileRegistry::open(&roots).expect("second registry");

        first
            .create("first", "First", ProfilePrivacy::Normal)
            .expect("first mutation");
        second
            .create("second", "Second", ProfilePrivacy::Normal)
            .expect("second mutation");

        let reopened = ProfileRegistry::open(&roots).expect("reopen");
        assert!(
            reopened
                .profiles()
                .iter()
                .any(|profile| profile.name == "first")
        );
        assert!(
            reopened
                .profiles()
                .iter()
                .any(|profile| profile.name == "second")
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn profile_lock_is_shared_in_process_and_refuses_external_owner() {
        let base = std::env::temp_dir().join(format!(
            "ferric-browser-profile-lock-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("roots");
        let profile_id = Uuid::new_v4();
        let first = ProfileLock::acquire(&roots, profile_id).expect("first lock");
        let second = ProfileLock::acquire(&roots, profile_id).expect("shared lock");
        let path = roots
            .runtime
            .join("profiles")
            .join(format!("{profile_id}.lock"));
        assert!(path.exists());
        drop(second);
        assert!(path.exists());
        drop(first);
        assert!(path.exists());

        let reacquired = ProfileLock::acquire(&roots, profile_id).expect("released kernel lock");
        drop(reacquired);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn profile_lock_is_released_when_owner_is_killed() {
        let base = std::env::temp_dir().join(format!(
            "ferric-browser-profile-lock-crash-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("roots");
        let profile_id = Uuid::new_v4();
        let ready = base.join("child-ready");
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "profiles::tests::profile_lock_crash_test_child",
                "--nocapture",
            ])
            .env("FERRIC_BROWSER_PROFILE_LOCK_CHILD_ROOT", &base)
            .env(
                "FERRIC_BROWSER_PROFILE_LOCK_CHILD_ID",
                profile_id.to_string(),
            )
            .env("FERRIC_BROWSER_PROFILE_LOCK_CHILD_READY", &ready)
            .spawn()
            .expect("spawn lock owner");
        for _ in 0..200 {
            if ready.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(ready.exists(), "child did not acquire the profile lock");
        assert!(matches!(
            ProfileLock::acquire(&roots, profile_id),
            Err(ProfileLockError::Busy { .. })
        ));
        child.kill().expect("kill lock owner");
        child.wait().expect("reap lock owner");
        let reacquired = ProfileLock::acquire(&roots, profile_id).expect("lock after owner death");
        drop(reacquired);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn profile_data_deletion_is_exact_and_keeps_other_profiles() {
        let base = std::env::temp_dir().join(format!(
            "ferric-browser-profile-data-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("roots");
        roots.ensure().expect("ensure");
        let profile_id = Uuid::new_v4();
        let other_id = Uuid::new_v4();
        std::fs::create_dir_all(roots.data.join("profiles").join(profile_id.to_string()))
            .expect("target data dir");
        std::fs::create_dir_all(roots.state.join("sessions").join(profile_id.to_string()))
            .expect("target session dir");
        std::fs::create_dir_all(roots.cache.join("profiles").join(profile_id.to_string()))
            .expect("target cache dir");
        std::fs::create_dir_all(roots.data.join("profiles").join(other_id.to_string()))
            .expect("other data dir");
        std::fs::create_dir_all(roots.state.join("sessions").join(other_id.to_string()))
            .expect("other session dir");
        std::fs::create_dir_all(roots.cache.join("profiles").join(other_id.to_string()))
            .expect("other cache dir");
        delete_profile_data(&roots, profile_id).expect("delete target");
        assert!(
            !roots
                .data
                .join("profiles")
                .join(profile_id.to_string())
                .exists()
        );
        assert!(
            !roots
                .state
                .join("sessions")
                .join(profile_id.to_string())
                .exists()
        );
        assert!(
            !roots
                .cache
                .join("profiles")
                .join(profile_id.to_string())
                .exists()
        );
        assert!(
            roots
                .data
                .join("profiles")
                .join(other_id.to_string())
                .exists()
        );
        assert!(
            roots
                .state
                .join("sessions")
                .join(other_id.to_string())
                .exists()
        );
        assert!(
            roots
                .cache
                .join("profiles")
                .join(other_id.to_string())
                .exists()
        );
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn transactional_profile_deletion_commits_registry_and_data_together() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        let mut registry = ProfileRegistry::open(&roots).expect("registry");
        let profile = registry
            .create("delete-me", "Delete me", ProfilePrivacy::Normal)
            .expect("profile")
            .clone();
        drop(registry);
        for path in profile_directories(&roots, profile.id) {
            std::fs::create_dir_all(&path).expect("profile directory");
            std::fs::write(path.join("marker"), b"data").expect("profile marker");
        }

        let outcome = delete_profile_transaction(&roots, &profile.name).expect("delete profile");
        assert_eq!(outcome.profile_name, profile.name);
        assert!(!outcome.cleanup_pending);
        assert!(
            !ProfileRegistry::open(&roots)
                .expect("reopen")
                .profiles()
                .iter()
                .any(|candidate| candidate.id == profile.id)
        );
        assert!(
            profile_directories(&roots, profile.id)
                .into_iter()
                .all(|path| !path.exists())
        );
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn startup_recovers_precommit_profile_deletion() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        let mut registry = ProfileRegistry::open(&roots).expect("registry");
        let profile = registry
            .create("recover-me", "Recover me", ProfilePrivacy::Normal)
            .expect("profile")
            .clone();
        drop(registry);
        for path in profile_directories(&roots, profile.id) {
            std::fs::create_dir_all(&path).expect("profile directory");
        }
        let manifest = DeletionManifest {
            version: DELETION_MANIFEST_VERSION,
            profile_id: profile.id,
            profile_name: profile.name.clone(),
            transaction_id: Uuid::new_v4(),
            committed: false,
        };
        prepare_manifest_directory(&roots).expect("manifest directory");
        let manifest_path = deletion_manifest_path(&roots, profile.id, manifest.transaction_id);
        save_deletion_manifest(&manifest_path, &manifest).expect("manifest");
        stage_profile_directories(&roots, &manifest).expect("stage");

        let reopened = ProfileRegistry::open(&roots).expect("recovery open");
        assert!(
            reopened
                .profiles()
                .iter()
                .any(|candidate| candidate.id == profile.id)
        );
        assert!(
            profile_directories(&roots, profile.id)
                .into_iter()
                .all(|path| path.is_dir())
        );
        assert!(!manifest_path.exists());
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn startup_finishes_postcommit_profile_deletion() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        let mut registry = ProfileRegistry::open(&roots).expect("registry");
        let profile = registry
            .create("finish-me", "Finish me", ProfilePrivacy::Normal)
            .expect("profile")
            .clone();
        drop(registry);
        for path in profile_directories(&roots, profile.id) {
            std::fs::create_dir_all(&path).expect("profile directory");
        }
        let manifest = DeletionManifest {
            version: DELETION_MANIFEST_VERSION,
            profile_id: profile.id,
            profile_name: profile.name.clone(),
            transaction_id: Uuid::new_v4(),
            committed: false,
        };
        prepare_manifest_directory(&roots).expect("manifest directory");
        let manifest_path = deletion_manifest_path(&roots, profile.id, manifest.transaction_id);
        save_deletion_manifest(&manifest_path, &manifest).expect("manifest");
        stage_profile_directories(&roots, &manifest).expect("stage");
        let registry_path = roots.state.join("profiles.json");
        {
            let _lock = RegistryLock::acquire(&registry_path).expect("registry lock");
            let mut records = load_records(&registry_path).expect("records");
            records.retain(|candidate| candidate.id != profile.id);
            save_records(&registry_path, &records).expect("commit registry deletion");
        }

        let reopened = ProfileRegistry::open(&roots).expect("recovery open");
        assert!(
            reopened
                .profiles()
                .iter()
                .all(|candidate| candidate.id != profile.id)
        );
        assert!(
            profile_directories(&roots, profile.id)
                .into_iter()
                .all(|path| !path.exists())
        );
        assert!(!manifest_path.exists());
        roots.cleanup().expect("cleanup");
    }
}
