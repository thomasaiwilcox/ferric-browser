//! Profile-scoped media-source fingerprints backed by the Linux Secret Service.
//!
//! Secret Service calls are synchronous. Call [`MediaFingerprintKey::open_blocking`]
//! only from a worker thread, never from the Qt GUI thread. There is deliberately
//! no plaintext profile-file, environment-variable, or mock-backend fallback.

use std::{
    collections::HashMap,
    fmt, fs,
    io::{self, Read},
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard, OnceLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

use dbus_secret_service::{Collection, EncryptionType, Item, SecretService};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use url::Url;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{ProfileLock, StorageRoots};

const SERVICE_NAME: &str = "com.ferric.browser.media-fingerprint";
const FORMAT_VERSION: u16 = 1;
const INITIAL_KEY_EPOCH: u16 = 1;
const KEY_BYTES: usize = 32;
const MAX_SOURCE_URL_BYTES: usize = 2048;
const FINGERPRINT_DOMAIN_PREFIX: &[u8] = b"ferric:media-source-fingerprint:format-v";
static MEDIA_FINGERPRINT_KEY_OPERATION: Mutex<()> = Mutex::new(());
static PROFILE_KEY_GENERATIONS: OnceLock<Mutex<HashMap<Uuid, Weak<AtomicU64>>>> = OnceLock::new();

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CredentialStoreError {
    Missing,
    Unavailable,
}

/// Private seam for deterministic lifecycle fault tests. Production entry
/// points always use `SecretServiceCredentialStore`; locked items and
/// collections fail closed and are never unlocked by Ferric.
trait MediaFingerprintCredentialStore {
    fn get_secret(&self, profile_id: Uuid, epoch: u16) -> Result<Vec<u8>, CredentialStoreError>;

    fn set_secret(
        &self,
        profile_id: Uuid,
        epoch: u16,
        secret: &[u8],
    ) -> Result<(), CredentialStoreError>;

    fn delete_secret(&self, profile_id: Uuid, epoch: u16) -> Result<(), CredentialStoreError>;
}

struct SecretServiceCredentialStore;

impl MediaFingerprintCredentialStore for SecretServiceCredentialStore {
    fn get_secret(&self, profile_id: Uuid, epoch: u16) -> Result<Vec<u8>, CredentialStoreError> {
        let service = Self::connect()?;
        let (_, item) = Self::find_unique_item(&service, profile_id, epoch)?;
        item.ok_or(CredentialStoreError::Missing)?
            .get_secret()
            .map_err(|_| CredentialStoreError::Unavailable)
    }

    fn set_secret(
        &self,
        profile_id: Uuid,
        epoch: u16,
        secret: &[u8],
    ) -> Result<(), CredentialStoreError> {
        let service = Self::connect()?;
        let (collection, item) = Self::find_unique_item(&service, profile_id, epoch)?;
        if let Some(item) = item {
            return item
                .set_secret(secret, "text/plain")
                .map_err(|_| CredentialStoreError::Unavailable);
        }

        let user = MediaFingerprintKey::credential_user(profile_id, epoch);
        let attributes = Self::attributes(&user);
        let label = format!("{user}@{SERVICE_NAME}:default (keyring v3.6.3)");
        collection
            .create_item(&label, attributes, secret, true, "text/plain")
            .map(|_| ())
            .map_err(|_| CredentialStoreError::Unavailable)
    }

    fn delete_secret(&self, profile_id: Uuid, epoch: u16) -> Result<(), CredentialStoreError> {
        let service = Self::connect()?;
        let (_, item) = Self::find_unique_item(&service, profile_id, epoch)?;
        item.ok_or(CredentialStoreError::Missing)?
            .delete()
            .map_err(|_| CredentialStoreError::Unavailable)
    }
}

impl SecretServiceCredentialStore {
    fn connect() -> Result<SecretService, CredentialStoreError> {
        // A zero prompt timeout prevents the backend from waiting for or
        // displaying an authorization prompt. We also reject locked items
        // explicitly below instead of asking the Secret Service to unlock.
        SecretService::connect_with_max_prompt_timeout(EncryptionType::Dh, 0)
            .map_err(|_| CredentialStoreError::Unavailable)
    }

    fn attributes(user: &str) -> HashMap<&str, &str> {
        HashMap::from([
            ("target", "default"),
            ("service", SERVICE_NAME),
            ("username", user),
        ])
    }

    fn find_unique_item<'a>(
        service: &'a SecretService,
        profile_id: Uuid,
        epoch: u16,
    ) -> Result<(Collection<'a>, Option<Item<'a>>), CredentialStoreError> {
        let collection = service
            .get_default_collection()
            .map_err(|_| CredentialStoreError::Unavailable)?;
        if collection
            .is_locked()
            .map_err(|_| CredentialStoreError::Unavailable)?
        {
            return Err(CredentialStoreError::Unavailable);
        }

        let user = MediaFingerprintKey::credential_user(profile_id, epoch);
        let search = service
            .search_items(Self::attributes(&user))
            .map_err(|_| CredentialStoreError::Unavailable)?;
        if !search.locked.is_empty() || search.unlocked.len() > 1 {
            return Err(CredentialStoreError::Unavailable);
        }
        Ok((collection, search.unlocked.into_iter().next()))
    }
}

/// Native-only profile key for keyed equality fingerprints of eligible media
/// source URLs. `epoch` identifies a particular profile key generation.
///
/// INVARIANT: key bytes stay in native memory and are zeroized on drop; they
/// are never serializable, exposed through `Debug`, or sent to ApplicationWorld.
pub struct MediaFingerprintKey {
    key: Zeroizing<[u8; KEY_BYTES]>,
    epoch: u16,
    generation: Arc<AtomicU64>,
    generation_at_load: u64,
}

impl fmt::Debug for MediaFingerprintKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MediaFingerprintKey([redacted])")
    }
}

impl MediaFingerprintKey {
    /// Loads the active key or provisions a fresh epoch after explicit capture.
    ///
    /// This performs profile-lock and Secret Service I/O synchronously. Run it
    /// on a dedicated worker. Temporary roots and unavailable/locked Secret
    /// Service instances fail closed; there is no weaker fallback.
    pub fn open_blocking(
        roots: &StorageRoots,
        profile_id: Uuid,
    ) -> Result<Self, MediaFingerprintKeyError> {
        Self::open_with_store(roots, profile_id, &SecretServiceCredentialStore)
    }

    fn open_with_store(
        roots: &StorageRoots,
        profile_id: Uuid,
        store: &dyn MediaFingerprintCredentialStore,
    ) -> Result<Self, MediaFingerprintKeyError> {
        let _operation = Self::operation_guard()?;
        let _profile_lock = Self::lock_profile(roots, profile_id)?;
        match Self::cleanup_marker_version(roots, profile_id)? {
            Some(epoch) => {
                match store.get_secret(profile_id, epoch) {
                    Ok(secret) => match Self::from_secret(profile_id, epoch, secret) {
                        Ok(key) => Ok(key),
                        Err(MediaFingerprintKeyError::InvalidKey) => {
                            Self::start_new_epoch(roots, profile_id, epoch, store)
                        }
                        Err(error) => Err(error),
                    },
                    Err(CredentialStoreError::Missing) => {
                        // A missing credential is never recreated under its
                        // former key identity: advance the persisted epoch so
                        // old sidecar hints can no longer appear current.
                        Self::start_new_epoch(roots, profile_id, epoch, store)
                    }
                    Err(CredentialStoreError::Unavailable) => {
                        Err(MediaFingerprintKeyError::Unavailable)
                    }
                }
            }
            None => {
                match store.get_secret(profile_id, INITIAL_KEY_EPOCH) {
                    Ok(secret) => match Self::from_secret(profile_id, INITIAL_KEY_EPOCH, secret) {
                        Ok(key) => {
                            Self::write_cleanup_marker(roots, profile_id, INITIAL_KEY_EPOCH)?;
                            Ok(key)
                        }
                        Err(MediaFingerprintKeyError::InvalidKey) => {
                            Self::start_new_epoch(roots, profile_id, INITIAL_KEY_EPOCH, store)
                        }
                        Err(error) => Err(error),
                    },
                    Err(CredentialStoreError::Missing) => {
                        // Persist cleanup intent before a new credential can
                        // be created, so a crash cannot leave an untracked
                        // Secret Service item behind.
                        Self::write_cleanup_marker(roots, profile_id, INITIAL_KEY_EPOCH)?;
                        Self::provision(profile_id, INITIAL_KEY_EPOCH, store)
                    }
                    Err(CredentialStoreError::Unavailable) => {
                        Err(MediaFingerprintKeyError::Unavailable)
                    }
                }
            }
        }
    }

    /// Loads the active key without creating or rotating a replacement.
    /// Restore callers use this form so a missing/deleted key cannot make an
    /// old sidecar appear current under a newly provisioned secret.
    pub fn load_blocking(
        roots: &StorageRoots,
        profile_id: Uuid,
    ) -> Result<Self, MediaFingerprintKeyError> {
        Self::load_with_store(roots, profile_id, &SecretServiceCredentialStore)
    }

    fn load_with_store(
        roots: &StorageRoots,
        profile_id: Uuid,
        store: &dyn MediaFingerprintCredentialStore,
    ) -> Result<Self, MediaFingerprintKeyError> {
        let _operation = Self::operation_guard()?;
        let _profile_lock = Self::lock_profile(roots, profile_id)?;
        let epoch = Self::cleanup_marker_version(roots, profile_id)?
            .ok_or(MediaFingerprintKeyError::Unavailable)?;
        let secret = store
            .get_secret(profile_id, epoch)
            .map_err(|_| MediaFingerprintKeyError::Unavailable)?;
        Self::from_secret(profile_id, epoch, secret)
    }

    /// Rotates the profile's active key epoch during an explicit operation.
    /// Existing media hints become ineligible; no rekey or sidecar rewrite is
    /// attempted. The caller must run this on a worker thread.
    pub fn rotate_blocking(
        roots: &StorageRoots,
        profile_id: Uuid,
    ) -> Result<Self, MediaFingerprintKeyError> {
        Self::rotate_with_store(roots, profile_id, &SecretServiceCredentialStore)
    }

    fn rotate_with_store(
        roots: &StorageRoots,
        profile_id: Uuid,
        store: &dyn MediaFingerprintCredentialStore,
    ) -> Result<Self, MediaFingerprintKeyError> {
        let _operation = Self::operation_guard()?;
        let _profile_lock = Self::lock_profile(roots, profile_id)?;
        let current_epoch =
            Self::cleanup_marker_version(roots, profile_id)?.unwrap_or(INITIAL_KEY_EPOCH);
        Self::start_new_epoch(roots, profile_id, current_epoch, store)
    }

    /// Deletes the profile's active key. Profile deletion calls this from its
    /// background cleanup path; unavailable Secret Service leaves the durable
    /// deletion marker and manifest pending for a later retry.
    pub fn delete_blocking(
        roots: &StorageRoots,
        profile_id: Uuid,
    ) -> Result<(), MediaFingerprintKeyError> {
        Self::delete_with_store(roots, profile_id, &SecretServiceCredentialStore)
    }

    fn delete_with_store(
        roots: &StorageRoots,
        profile_id: Uuid,
        store: &dyn MediaFingerprintCredentialStore,
    ) -> Result<(), MediaFingerprintKeyError> {
        let _operation = Self::operation_guard()?;
        Self::delete_profile_key_under_operation(roots, profile_id, store)
    }

    fn delete_profile_key_under_operation(
        roots: &StorageRoots,
        profile_id: Uuid,
        store: &dyn MediaFingerprintCredentialStore,
    ) -> Result<(), MediaFingerprintKeyError> {
        if roots.temporary_root().is_some() {
            // Key creation and loading are disabled for temporary roots, so
            // an absent marker proves there is no managed credential to
            // delete. A marker there is unexpected and must remain fail-closed.
            return match Self::cleanup_marker_version(roots, profile_id)? {
                None => Ok(()),
                Some(_) => Err(MediaFingerprintKeyError::Unavailable),
            };
        }
        let _profile_lock = Self::lock_profile(roots, profile_id)?;
        Self::invalidate_cached_keys(profile_id)?;
        let Some(epoch) = Self::cleanup_marker_version(roots, profile_id)? else {
            // Profiles that never opted into media capture have no key to
            // delete; profile deletion must not depend on Secret Service.
            return Ok(());
        };
        match store.delete_secret(profile_id, epoch) {
            Ok(()) | Err(CredentialStoreError::Missing) => {
                Self::remove_cleanup_marker(roots, profile_id)
            }
            Err(CredentialStoreError::Unavailable) => Err(MediaFingerprintKeyError::Unavailable),
        }
    }

    /// Removes all marked profile keys before Ferric-owned roots are reset.
    /// Profiles without a marker are deliberately skipped and require no
    /// Secret Service access.
    pub(crate) fn delete_all_marked(roots: &StorageRoots) -> Result<(), MediaFingerprintKeyError> {
        Self::delete_all_marked_with_store(roots, &SecretServiceCredentialStore)
    }

    fn delete_all_marked_with_store(
        roots: &StorageRoots,
        store: &dyn MediaFingerprintCredentialStore,
    ) -> Result<(), MediaFingerprintKeyError> {
        // Prevent a same-process capture/rotation from creating a marker after
        // this scan but before reset clears the state root.
        let _operation = Self::operation_guard()?;
        let state_metadata = match fs::symlink_metadata(&roots.state) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err(MediaFingerprintKeyError::Unavailable),
        };
        if state_metadata.file_type().is_symlink() || !state_metadata.is_dir() {
            return Err(MediaFingerprintKeyError::Unavailable);
        }
        let entries =
            fs::read_dir(&roots.state).map_err(|_| MediaFingerprintKeyError::Unavailable)?;
        for entry in entries {
            let entry = entry.map_err(|_| MediaFingerprintKeyError::Unavailable)?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(profile_id_text) = name
                .strip_prefix("media-fingerprint-")
                .and_then(|value| value.strip_suffix(".key-marker"))
            else {
                continue;
            };
            let profile_id = Uuid::parse_str(profile_id_text)
                .map_err(|_| MediaFingerprintKeyError::Unavailable)?;
            if name != format!("media-fingerprint-{profile_id}.key-marker") {
                return Err(MediaFingerprintKeyError::Unavailable);
            }
            Self::delete_profile_key_under_operation(roots, profile_id, store)?;
        }
        Ok(())
    }

    fn lock_profile(
        roots: &StorageRoots,
        profile_id: Uuid,
    ) -> Result<ProfileLock, MediaFingerprintKeyError> {
        if roots.temporary_root.is_some() {
            return Err(MediaFingerprintKeyError::Unavailable);
        }
        ProfileLock::acquire(roots, profile_id).map_err(|_| MediaFingerprintKeyError::Unavailable)
    }

    fn operation_guard() -> Result<MutexGuard<'static, ()>, MediaFingerprintKeyError> {
        MEDIA_FINGERPRINT_KEY_OPERATION
            .lock()
            .map_err(|_| MediaFingerprintKeyError::Unavailable)
    }

    fn generation_for(profile_id: Uuid) -> Result<Arc<AtomicU64>, MediaFingerprintKeyError> {
        let generations = PROFILE_KEY_GENERATIONS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut generations = generations
            .lock()
            .map_err(|_| MediaFingerprintKeyError::Unavailable)?;
        generations.retain(|_, generation| generation.strong_count() > 0);
        if let Some(generation) = generations.get(&profile_id).and_then(Weak::upgrade) {
            return Ok(generation);
        }
        let generation = Arc::new(AtomicU64::new(0));
        generations.insert(profile_id, Arc::downgrade(&generation));
        Ok(generation)
    }

    fn invalidate_cached_keys(profile_id: Uuid) -> Result<(), MediaFingerprintKeyError> {
        let generation = Self::generation_for(profile_id)?;
        generation
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map(|_| ())
            .map_err(|_| MediaFingerprintKeyError::Unavailable)
    }

    fn credential_user(profile_id: Uuid, epoch: u16) -> String {
        format!("{profile_id}:format-v{FORMAT_VERSION}:key-v{epoch}")
    }

    fn marker_path(roots: &StorageRoots, profile_id: Uuid) -> PathBuf {
        roots
            .state
            .join(format!("media-fingerprint-{profile_id}.key-marker"))
    }

    fn write_cleanup_marker(
        roots: &StorageRoots,
        profile_id: Uuid,
        epoch: u16,
    ) -> Result<(), MediaFingerprintKeyError> {
        let marker =
            format!("ferric-media-fingerprint-key:format-v{FORMAT_VERSION}:key-v{epoch}\n");
        crate::durable::write_atomic(&Self::marker_path(roots, profile_id), marker.as_bytes())
            .map_err(|_| MediaFingerprintKeyError::Unavailable)
    }

    fn cleanup_marker_version(
        roots: &StorageRoots,
        profile_id: Uuid,
    ) -> Result<Option<u16>, MediaFingerprintKeyError> {
        let path = Self::marker_path(roots, profile_id);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(MediaFingerprintKeyError::Unavailable),
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > 128 {
            return Err(MediaFingerprintKeyError::Unavailable);
        }
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options
            .open(path)
            .map_err(|_| MediaFingerprintKeyError::Unavailable)?;
        let mut actual = Vec::with_capacity(metadata.len() as usize);
        file.by_ref()
            .take(129)
            .read_to_end(&mut actual)
            .map_err(|_| MediaFingerprintKeyError::Unavailable)?;
        let marker =
            std::str::from_utf8(&actual).map_err(|_| MediaFingerprintKeyError::Unavailable)?;
        let prefix = format!("ferric-media-fingerprint-key:format-v{FORMAT_VERSION}:");
        let suffix = marker
            .strip_prefix(&prefix)
            .and_then(|value| value.strip_suffix('\n'))
            .ok_or(MediaFingerprintKeyError::Unavailable)?;
        let epoch = suffix
            .strip_prefix("key-v")
            .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|value| value.parse::<u16>().ok())
            .filter(|epoch| *epoch > 0)
            .ok_or(MediaFingerprintKeyError::Unavailable)?;
        let canonical = format!("{prefix}key-v{epoch}\n");
        if canonical == marker {
            Ok(Some(epoch))
        } else {
            Err(MediaFingerprintKeyError::Unavailable)
        }
    }

    fn remove_cleanup_marker(
        roots: &StorageRoots,
        profile_id: Uuid,
    ) -> Result<(), MediaFingerprintKeyError> {
        let path = Self::marker_path(roots, profile_id);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err(MediaFingerprintKeyError::Unavailable),
        }
        let parent = path.parent().ok_or(MediaFingerprintKeyError::Unavailable)?;
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| MediaFingerprintKeyError::Unavailable)
    }

    fn load_or_provision(
        profile_id: Uuid,
        epoch: u16,
        store: &dyn MediaFingerprintCredentialStore,
    ) -> Result<Self, MediaFingerprintKeyError> {
        match store.get_secret(profile_id, epoch) {
            Ok(secret) => Self::from_secret(profile_id, epoch, secret),
            Err(CredentialStoreError::Missing) => Self::provision(profile_id, epoch, store),
            Err(CredentialStoreError::Unavailable) => Err(MediaFingerprintKeyError::Unavailable),
        }
    }

    fn start_new_epoch(
        roots: &StorageRoots,
        profile_id: Uuid,
        current_epoch: u16,
        store: &dyn MediaFingerprintCredentialStore,
    ) -> Result<Self, MediaFingerprintKeyError> {
        let next_epoch = current_epoch
            .checked_add(1)
            .ok_or(MediaFingerprintKeyError::Unavailable)?;
        Self::invalidate_cached_keys(profile_id)?;
        match store.delete_secret(profile_id, current_epoch) {
            Ok(()) | Err(CredentialStoreError::Missing) => {}
            Err(CredentialStoreError::Unavailable) => {
                return Err(MediaFingerprintKeyError::Unavailable);
            }
        }
        // Publish the new epoch before provisioning. A crash or unavailable
        // store leaves restore fail-closed rather than treating old hints as
        // authenticated by a replacement key.
        Self::write_cleanup_marker(roots, profile_id, next_epoch)?;
        Self::load_or_provision(profile_id, next_epoch, store)
    }

    /// Returns whether a cached key instance survived in-process rotation or
    /// profile deletion.
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.generation.load(Ordering::Acquire) == self.generation_at_load
    }

    /// Computes a profile-local HMAC-SHA-256 fingerprint for one bounded
    /// canonical HTTP(S) URL. The returned bytes are safe to persist as an
    /// equality hint, but do not conceal a URL from a process that can access
    /// this profile's unlocked Secret Service item.
    #[must_use]
    pub fn fingerprint(&self, source_url: &str) -> Option<String> {
        if !self.is_current() || source_url.is_empty() || source_url.len() > MAX_SOURCE_URL_BYTES {
            return None;
        }
        let url = Url::parse(source_url).ok()?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.as_str().len() > MAX_SOURCE_URL_BYTES
        {
            return None;
        }

        let mut mac = HmacSha256::new_from_slice(self.key.as_ref()).ok()?;
        mac.update(FINGERPRINT_DOMAIN_PREFIX);
        mac.update(FORMAT_VERSION.to_string().as_bytes());
        mac.update(b":key-v");
        mac.update(self.epoch.to_string().as_bytes());
        mac.update(&[0]);
        mac.update(url.as_str().as_bytes());
        let digest = mac.finalize().into_bytes();
        Some(format!(
            "hmac-sha256-v{FORMAT_VERSION}-k{}:{}",
            self.epoch,
            hex_lowercase(&digest)
        ))
    }

    fn from_secret(
        profile_id: Uuid,
        epoch: u16,
        secret: Vec<u8>,
    ) -> Result<Self, MediaFingerprintKeyError> {
        // keyring returns an ordinary Vec; wrap that exact allocation before
        // validating or copying so every exit path wipes the returned bytes.
        let secret = Zeroizing::new(secret);
        if secret.len() != KEY_BYTES {
            return Err(MediaFingerprintKeyError::InvalidKey);
        }
        let mut key = Zeroizing::new([0_u8; KEY_BYTES]);
        key.copy_from_slice(&secret);
        let generation = Self::generation_for(profile_id)?;
        let generation_at_load = generation.load(Ordering::Acquire);
        Ok(Self {
            key,
            epoch,
            generation,
            generation_at_load,
        })
    }

    fn provision(
        profile_id: Uuid,
        epoch: u16,
        store: &dyn MediaFingerprintCredentialStore,
    ) -> Result<Self, MediaFingerprintKeyError> {
        let mut key = Zeroizing::new([0_u8; KEY_BYTES]);
        getrandom::fill(key.as_mut()).map_err(|_| MediaFingerprintKeyError::Unavailable)?;
        store
            .set_secret(profile_id, epoch, key.as_ref())
            .map_err(|_| MediaFingerprintKeyError::Unavailable)?;

        // Read back the stored item before allowing the key to authorize a
        // fingerprint. A partial or substituted credential fails closed.
        let stored = store
            .get_secret(profile_id, epoch)
            .map_err(|_| MediaFingerprintKeyError::Unavailable)?;
        let stored = Zeroizing::new(stored);
        if !constant_time_equal(key.as_ref(), &stored) {
            return Err(MediaFingerprintKeyError::InvalidKey);
        }
        Self::from_secret(profile_id, epoch, (*key).to_vec())
    }
}

/// Deliberately coarse failures: OS credential-store diagnostics can disclose
/// account/session details and must not leak into shared UI or logs.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum MediaFingerprintKeyError {
    Unavailable,
    InvalidKey,
}

impl fmt::Debug for MediaFingerprintKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "MediaFingerprintKeyError::Unavailable",
            Self::InvalidKey => "MediaFingerprintKeyError::InvalidKey",
        })
    }
}

impl fmt::Display for MediaFingerprintKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("media fingerprint key is unavailable")
    }
}

impl std::error::Error for MediaFingerprintKeyError {}

pub(crate) fn delete_all_marked(roots: &StorageRoots) -> Result<(), MediaFingerprintKeyError> {
    MediaFingerprintKey::delete_all_marked(roots)
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn hex_lowercase(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        CredentialStoreError, MediaFingerprintCredentialStore, MediaFingerprintKey,
        SecretServiceCredentialStore, Zeroizing, constant_time_equal,
    };
    use crate::{RootSpec, StorageRoots};
    use std::{collections::HashMap, sync::Mutex};
    use uuid::Uuid;

    #[derive(Default)]
    struct MemoryCredentialStore {
        credentials: Mutex<HashMap<(Uuid, u16), Zeroizing<Vec<u8>>>>,
        fail_next_get: Mutex<bool>,
        fail_next_set: Mutex<bool>,
        fail_next_delete: Mutex<bool>,
    }

    impl MemoryCredentialStore {
        fn fail_one_set(&self) {
            *self.fail_next_set.lock().expect("set-fault lock") = true;
        }

        fn fail_one_get(&self) {
            *self.fail_next_get.lock().expect("get-fault lock") = true;
        }

        fn fail_one_delete(&self) {
            *self.fail_next_delete.lock().expect("delete-fault lock") = true;
        }

        fn take_fault(fault: &Mutex<bool>) -> Result<bool, CredentialStoreError> {
            let mut fault = fault
                .lock()
                .map_err(|_| CredentialStoreError::Unavailable)?;
            Ok(std::mem::take(&mut *fault))
        }

        fn contains(&self, profile_id: Uuid, epoch: u16) -> bool {
            self.credentials
                .lock()
                .is_ok_and(|credentials| credentials.contains_key(&(profile_id, epoch)))
        }
    }

    impl MediaFingerprintCredentialStore for MemoryCredentialStore {
        fn get_secret(
            &self,
            profile_id: Uuid,
            epoch: u16,
        ) -> Result<Vec<u8>, CredentialStoreError> {
            if Self::take_fault(&self.fail_next_get)? {
                return Err(CredentialStoreError::Unavailable);
            }
            self.credentials
                .lock()
                .map_err(|_| CredentialStoreError::Unavailable)?
                .get(&(profile_id, epoch))
                .map(|secret| secret.to_vec())
                .ok_or(CredentialStoreError::Missing)
        }

        fn set_secret(
            &self,
            profile_id: Uuid,
            epoch: u16,
            secret: &[u8],
        ) -> Result<(), CredentialStoreError> {
            if Self::take_fault(&self.fail_next_set)? {
                return Err(CredentialStoreError::Unavailable);
            }
            self.credentials
                .lock()
                .map_err(|_| CredentialStoreError::Unavailable)?
                .insert((profile_id, epoch), Zeroizing::new(secret.to_vec()));
            Ok(())
        }

        fn delete_secret(&self, profile_id: Uuid, epoch: u16) -> Result<(), CredentialStoreError> {
            if Self::take_fault(&self.fail_next_delete)? {
                return Err(CredentialStoreError::Unavailable);
            }
            self.credentials
                .lock()
                .map_err(|_| CredentialStoreError::Unavailable)?
                .remove(&(profile_id, epoch))
                .map(|_| ())
                .ok_or(CredentialStoreError::Missing)
        }
    }

    fn key(bytes: u8) -> MediaFingerprintKey {
        MediaFingerprintKey::from_secret(Uuid::new_v4(), 1, vec![bytes; 32])
            .expect("test key has exact length")
    }

    fn key_test_roots() -> (std::path::PathBuf, StorageRoots) {
        let base = std::env::temp_dir().join(format!(
            "ferric-media-fingerprint-lifecycle-{}",
            Uuid::new_v4()
        ));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("test roots");
        roots.ensure().expect("create test roots");
        (base, roots)
    }

    fn on_worker<T: Send + 'static>(operation: impl FnOnce() -> T + Send + 'static) -> T {
        std::thread::spawn(operation)
            .join()
            .expect("isolated credential-store worker")
    }

    #[cfg(target_os = "linux")]
    fn isolated_secret_service_root() -> std::path::PathBuf {
        assert_eq!(
            std::env::var("FERRIC_TEST_SECRET_SERVICE_ISOLATED").as_deref(),
            Ok("yes"),
            "run only through the isolated Secret Service harness"
        );
        assert!(std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some());
        let root = std::path::PathBuf::from(
            std::env::var_os("FERRIC_TEST_SECRET_SERVICE_ROOT")
                .expect("isolated Secret Service root"),
        );
        assert!(root.starts_with("/tmp"));
        assert!(
            root.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("ferric-secret-service."))
        );
        assert_eq!(
            std::env::var_os("XDG_DATA_HOME").map(std::path::PathBuf::from),
            Some(root.join("data"))
        );
        assert_eq!(
            std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from),
            Some(root.join("config"))
        );
        root
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires tools/test-isolated-media-fingerprint-keyring.sh"]
    fn isolated_secret_service_round_trip_rotation_and_locked_refusal() {
        // INVARIANT: This integration test must never contact a desktop
        // keyring. Its only supported entry point creates a fresh D-Bus
        // session and a disposable XDG data directory before reaching the
        // production SecretServiceCredentialStore adapter.
        let isolated_root = isolated_secret_service_root();
        let data_home = isolated_root.join("data");

        let base = data_home.join(format!("ferric-profile-{}", Uuid::new_v4()));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("isolated roots");
        roots.ensure().expect("create isolated profile roots");
        let profile_id = Uuid::new_v4();
        let source = "https://media.example.invalid/isolated-test.mp4";

        let opened = on_worker({
            let roots = roots.clone();
            move || MediaFingerprintKey::open_blocking(&roots, profile_id)
        })
        .expect("provision via isolated Secret Service");
        let original = opened
            .fingerprint(source)
            .expect("fingerprint under first key");
        assert!(original.starts_with("hmac-sha256-v1-k1:"));

        let loaded = on_worker({
            let roots = roots.clone();
            move || MediaFingerprintKey::load_blocking(&roots, profile_id)
        })
        .expect("load provisioned key from isolated Secret Service");
        assert_eq!(
            loaded.fingerprint(source).as_deref(),
            Some(original.as_str())
        );

        let rotated = on_worker({
            let roots = roots.clone();
            move || MediaFingerprintKey::rotate_blocking(&roots, profile_id)
        })
        .expect("rotate isolated profile key");
        assert!(!opened.is_current());
        assert!(!loaded.is_current());
        let rotated_fingerprint = rotated
            .fingerprint(source)
            .expect("fingerprint after rotation");
        assert!(rotated_fingerprint.starts_with("hmac-sha256-v1-k2:"));
        assert_ne!(rotated_fingerprint, original);

        let other_profile_id = Uuid::new_v4();
        let other_profile = on_worker({
            let roots = roots.clone();
            move || MediaFingerprintKey::open_blocking(&roots, other_profile_id)
        })
        .expect("provision a second isolated profile key");
        assert_ne!(
            other_profile.fingerprint(source).as_deref(),
            Some(rotated_fingerprint.as_str())
        );

        on_worker({
            let roots = roots.clone();
            move || MediaFingerprintKey::delete_blocking(&roots, profile_id)
        })
        .expect("delete rotated profile credential");
        on_worker({
            let roots = roots.clone();
            move || MediaFingerprintKey::delete_blocking(&roots, other_profile_id)
        })
        .expect("delete second profile credential");
        assert!(
            on_worker({
                let roots = roots.clone();
                move || MediaFingerprintKey::load_blocking(&roots, profile_id)
            })
            .is_err()
        );
        assert!(
            on_worker({
                let roots = roots.clone();
                move || MediaFingerprintKey::load_blocking(&roots, other_profile_id)
            })
            .is_err()
        );
        assert!(!MediaFingerprintKey::marker_path(&roots, profile_id).exists());
        assert!(!MediaFingerprintKey::marker_path(&roots, other_profile_id).exists());

        let locked_collection_profile_id = Uuid::new_v4();
        let collection_key = on_worker({
            let roots = roots.clone();
            move || MediaFingerprintKey::open_blocking(&roots, locked_collection_profile_id)
        })
        .expect("provision key for locked-collection refusal test");
        let service = SecretServiceCredentialStore::connect().expect("connect to isolated service");
        let collection = service
            .get_default_collection()
            .expect("get isolated default collection");
        collection.lock().expect("lock isolated default collection");
        assert!(collection.is_locked().expect("check collection lock"));
        drop(service);

        assert!(
            on_worker({
                let roots = roots.clone();
                move || MediaFingerprintKey::load_blocking(&roots, locked_collection_profile_id)
            })
            .is_err()
        );
        assert!(collection_key.is_current());
        assert!(
            on_worker({
                let roots = roots.clone();
                move || MediaFingerprintKey::delete_blocking(&roots, locked_collection_profile_id)
            })
            .is_err()
        );
        assert!(!collection_key.is_current());
        assert_eq!(
            MediaFingerprintKey::cleanup_marker_version(&roots, locked_collection_profile_id)
                .expect("locked collection keeps deletion marker"),
            Some(1)
        );

        std::fs::remove_dir_all(base).expect("remove isolated profile roots");
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires tools/test-isolated-media-fingerprint-keyring.sh"]
    fn isolated_missing_secret_service_fails_closed_without_marker() {
        let isolated_root = isolated_secret_service_root();
        let expected_bus = format!("unix:path={}/missing-session-bus", isolated_root.display());
        assert_eq!(
            std::env::var("DBUS_SESSION_BUS_ADDRESS").as_deref(),
            Ok(expected_bus.as_str()),
            "the unavailable-service test must use its nonexistent private bus socket"
        );

        let base = isolated_root
            .join("data")
            .join(format!("missing-service-profile-{}", Uuid::new_v4()));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("isolated roots");
        roots.ensure().expect("create isolated profile roots");
        let profile_id = Uuid::new_v4();

        assert!(
            on_worker({
                let roots = roots.clone();
                move || MediaFingerprintKey::open_blocking(&roots, profile_id)
            })
            .is_err()
        );
        assert_eq!(
            MediaFingerprintKey::cleanup_marker_version(&roots, profile_id)
                .expect("missing service must not create key state"),
            None
        );
        assert!(!std::path::Path::new(&expected_bus["unix:path=".len()..]).exists());

        std::fs::remove_dir_all(base).expect("remove isolated profile roots");
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires tools/test-isolated-media-fingerprint-keyring.sh"]
    fn isolated_active_bus_without_service_owner_fails_closed_without_marker() {
        // INVARIANT: the harness supplies a live private bus whose config has
        // no service directories, so this must not activate the desktop
        // keyring or any other Secret Service provider.
        let isolated_root = isolated_secret_service_root();
        let bus_address = std::env::var("DBUS_SESSION_BUS_ADDRESS").expect("private session bus");
        assert!(!bus_address.contains("missing-session-bus"));

        let base = isolated_root
            .join("data")
            .join(format!("active-bus-no-service-profile-{}", Uuid::new_v4()));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("isolated roots");
        roots.ensure().expect("create isolated profile roots");
        let profile_id = Uuid::new_v4();

        assert!(
            on_worker({
                let roots = roots.clone();
                move || MediaFingerprintKey::open_blocking(&roots, profile_id)
            })
            .is_err()
        );
        assert_eq!(
            MediaFingerprintKey::cleanup_marker_version(&roots, profile_id)
                .expect("missing service owner must not create key state"),
            None
        );

        std::fs::remove_dir_all(base).expect("remove isolated profile roots");
    }

    #[test]
    fn provisioning_failure_leaves_cleanup_marker_before_secret_creation() {
        let (base, roots) = key_test_roots();
        let profile_id = Uuid::new_v4();
        let store = MemoryCredentialStore::default();
        store.fail_one_set();

        assert!(MediaFingerprintKey::open_with_store(&roots, profile_id, &store).is_err());
        assert_eq!(
            MediaFingerprintKey::cleanup_marker_version(&roots, profile_id)
                .expect("marker remains readable"),
            Some(1)
        );
        assert!(!store.contains(profile_id, 1));

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn temporary_root_without_key_marker_allows_profile_key_deletion() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        roots.ensure().expect("create temporary roots");
        MediaFingerprintKey::delete_blocking(&roots, Uuid::new_v4())
            .expect("temporary profile without a key is a no-op");
        roots.cleanup().expect("remove temporary roots");
    }

    #[test]
    fn temporary_root_with_key_marker_refuses_profile_key_deletion() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        roots.ensure().expect("create temporary roots");
        let profile_id = Uuid::new_v4();
        MediaFingerprintKey::write_cleanup_marker(&roots, profile_id, 1)
            .expect("write unexpected marker");

        assert!(MediaFingerprintKey::delete_blocking(&roots, profile_id).is_err());
        assert!(MediaFingerprintKey::marker_path(&roots, profile_id).exists());
        roots.cleanup().expect("remove temporary roots");
    }

    #[test]
    fn unavailable_secret_store_never_triggers_plaintext_or_new_key_fallback() {
        let (base, roots) = key_test_roots();
        let profile_id = Uuid::new_v4();
        let store = MemoryCredentialStore::default();
        store.fail_one_get();

        assert!(MediaFingerprintKey::open_with_store(&roots, profile_id, &store).is_err());
        assert!(
            MediaFingerprintKey::cleanup_marker_version(&roots, profile_id)
                .expect("no marker was needed")
                .is_none()
        );
        assert!(!store.contains(profile_id, 1));

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn failed_rotation_revokes_old_instance_without_advancing_active_epoch() {
        let (base, roots) = key_test_roots();
        let profile_id = Uuid::new_v4();
        let store = MemoryCredentialStore::default();
        let cached = MediaFingerprintKey::open_with_store(&roots, profile_id, &store)
            .expect("provision key");
        let source = "https://media.example.test/file.mp4";
        let before = cached.fingerprint(source).expect("active key");
        store.fail_one_delete();

        assert!(MediaFingerprintKey::rotate_with_store(&roots, profile_id, &store).is_err());
        assert!(!cached.is_current());
        assert_eq!(
            MediaFingerprintKey::cleanup_marker_version(&roots, profile_id)
                .expect("active marker remains readable"),
            Some(1)
        );
        assert!(store.contains(profile_id, 1));
        let reloaded = MediaFingerprintKey::load_with_store(&roots, profile_id, &store)
            .expect("still-active credential may be loaded again");
        assert_eq!(
            reloaded.fingerprint(source).as_deref(),
            Some(before.as_str())
        );

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn successful_rotation_deletes_old_epoch_and_refuses_cached_key() {
        let (base, roots) = key_test_roots();
        let profile_id = Uuid::new_v4();
        let store = MemoryCredentialStore::default();
        let cached = MediaFingerprintKey::open_with_store(&roots, profile_id, &store)
            .expect("provision key");
        let source = "https://media.example.test/file.mp4";
        let before = cached.fingerprint(source).expect("old active key");

        let rotated = MediaFingerprintKey::rotate_with_store(&roots, profile_id, &store)
            .expect("rotate to a new key");

        assert!(!cached.is_current());
        assert!(cached.fingerprint(source).is_none());
        assert_ne!(rotated.fingerprint(source), Some(before));
        assert_eq!(
            MediaFingerprintKey::cleanup_marker_version(&roots, profile_id).expect("new marker"),
            Some(2)
        );
        assert!(!store.contains(profile_id, 1));
        assert!(store.contains(profile_id, 2));

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn failed_deletion_retains_marker_and_retry_removes_the_credential() {
        let (base, roots) = key_test_roots();
        let profile_id = Uuid::new_v4();
        let store = MemoryCredentialStore::default();
        let cached = MediaFingerprintKey::open_with_store(&roots, profile_id, &store)
            .expect("provision key");
        store.fail_one_delete();

        assert!(MediaFingerprintKey::delete_with_store(&roots, profile_id, &store).is_err());
        assert!(!cached.is_current());
        assert_eq!(
            MediaFingerprintKey::cleanup_marker_version(&roots, profile_id)
                .expect("retry marker remains"),
            Some(1)
        );
        assert!(store.contains(profile_id, 1));

        MediaFingerprintKey::delete_with_store(&roots, profile_id, &store)
            .expect("retry key deletion");
        assert!(
            MediaFingerprintKey::cleanup_marker_version(&roots, profile_id)
                .expect("marker removed")
                .is_none()
        );
        assert!(!store.contains(profile_id, 1));

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn restore_never_reprovisions_a_missing_key_but_capture_advances_epoch() {
        let (base, roots) = key_test_roots();
        let profile_id = Uuid::new_v4();
        let store = MemoryCredentialStore::default();
        let cached = MediaFingerprintKey::open_with_store(&roots, profile_id, &store)
            .expect("provision key");
        store
            .delete_secret(profile_id, 1)
            .expect("simulate external credential removal");

        assert!(MediaFingerprintKey::load_with_store(&roots, profile_id, &store).is_err());
        assert!(!store.contains(profile_id, 1));
        assert_eq!(
            MediaFingerprintKey::cleanup_marker_version(&roots, profile_id)
                .expect("restore left marker unchanged"),
            Some(1)
        );

        let replacement = MediaFingerprintKey::open_with_store(&roots, profile_id, &store)
            .expect("explicit capture provisions a new epoch");
        assert!(!cached.is_current());
        assert_eq!(replacement.epoch, 2);
        assert!(!store.contains(profile_id, 1));
        assert!(store.contains(profile_id, 2));

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn reset_key_scan_retains_marker_when_credential_deletion_fails() {
        let (base, roots) = key_test_roots();
        let profile_id = Uuid::new_v4();
        let store = MemoryCredentialStore::default();
        MediaFingerprintKey::open_with_store(&roots, profile_id, &store).expect("provision key");
        store.fail_one_delete();

        assert!(MediaFingerprintKey::delete_all_marked_with_store(&roots, &store).is_err());
        assert_eq!(
            MediaFingerprintKey::cleanup_marker_version(&roots, profile_id)
                .expect("reset retry marker"),
            Some(1)
        );
        assert!(store.contains(profile_id, 1));

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn keyed_fingerprint_is_stable_profile_scoped_and_does_not_include_source() {
        let first_key = key(0x41);
        let other_key = key(0x42);
        let source = "https://media.example.test/private/lecture.mp4?token=secret";

        let first = first_key.fingerprint(source).expect("supported source");
        assert!(first.starts_with("hmac-sha256-v1-k1:"));
        let digest = first
            .strip_prefix("hmac-sha256-v1-k1:")
            .expect("format and key epoch tag");
        assert_eq!(digest.len(), 64);
        assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(
            first_key.fingerprint(source).as_deref(),
            Some(first.as_str())
        );
        assert_ne!(
            other_key.fingerprint(source).as_deref(),
            Some(first.as_str())
        );
        assert!(!first.contains("lecture"));
        assert!(!first.contains("secret"));
    }

    #[test]
    fn canonical_url_forms_produce_the_same_fingerprint() {
        let key = key(0x51);
        assert_eq!(
            key.fingerprint("HTTPS://Media.Example.Test:443/a/../b.mp4")
                .as_deref(),
            key.fingerprint("https://media.example.test/b.mp4")
                .as_deref()
        );
    }

    #[test]
    fn key_epoch_is_bound_to_the_fingerprint_and_changes_after_rotation() {
        let profile_id = Uuid::new_v4();
        let first = MediaFingerprintKey::from_secret(profile_id, 1, vec![0x71; 32]).expect("key");
        let rotated =
            MediaFingerprintKey::from_secret(profile_id, 2, vec![0x71; 32]).expect("rotated key");
        let source = "https://media.example.test/file.mp4";
        assert_ne!(first.fingerprint(source), rotated.fingerprint(source));
        assert!(
            rotated
                .fingerprint(source)
                .is_some_and(|value| value.starts_with("hmac-sha256-v1-k2:"))
        );
    }

    #[test]
    fn profile_key_invalidation_revokes_cached_fingerprint_instances() {
        let profile_id = Uuid::new_v4();
        let cached = MediaFingerprintKey::from_secret(profile_id, 1, vec![0x72; 32]).expect("key");
        assert!(cached.is_current());
        MediaFingerprintKey::invalidate_cached_keys(profile_id).expect("invalidate key");
        assert!(!cached.is_current());
        assert!(
            cached
                .fingerprint("https://media.example.test/file.mp4")
                .is_none()
        );
    }

    #[test]
    fn rejects_unsupported_or_credential_bearing_source_urls() {
        let key = key(0x61);
        assert!(
            key.fingerprint("blob:https://media.example.test/id")
                .is_none()
        );
        assert!(
            key.fingerprint("https://user:password@media.example.test/file.mp4")
                .is_none()
        );
        assert!(
            key.fingerprint(&format!("https://media.example.test/{}", "x".repeat(2050)))
                .is_none()
        );
    }

    #[test]
    fn secret_length_and_constant_time_comparison_are_enforced() {
        let profile_id = Uuid::new_v4();
        assert!(MediaFingerprintKey::from_secret(profile_id, 1, vec![0; 31]).is_err());
        assert!(MediaFingerprintKey::from_secret(profile_id, 1, vec![0; 33]).is_err());
        assert!(constant_time_equal(&[0, 1, 2], &[0, 1, 2]));
        assert!(!constant_time_equal(&[0, 1, 2], &[0, 1, 3]));
        assert!(!constant_time_equal(&[0, 1], &[0, 1, 0]));
    }

    #[test]
    fn profile_without_media_key_does_not_need_secret_service_for_deletion() {
        let base = std::env::temp_dir().join(format!(
            "ferric-media-fingerprint-marker-{}",
            Uuid::new_v4()
        ));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("test roots");
        let profile_id = Uuid::new_v4();

        assert!(MediaFingerprintKey::load_blocking(&roots, profile_id).is_err());
        MediaFingerprintKey::delete_blocking(&roots, profile_id)
            .expect("no marker means no credential-store access");

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn root_cleanup_rejects_malformed_marker_identity_before_key_store_access() {
        let base =
            std::env::temp_dir().join(format!("ferric-media-fingerprint-reset-{}", Uuid::new_v4()));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("test roots");
        roots.ensure().expect("create test roots");
        std::fs::write(
            roots.state.join("media-fingerprint-not-a-uuid.key-marker"),
            b"irrelevant",
        )
        .expect("write malformed marker identity");

        assert!(MediaFingerprintKey::delete_all_marked(&roots).is_err());

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn key_cleanup_marker_is_durable_and_corruption_fails_closed() {
        let base = std::env::temp_dir().join(format!(
            "ferric-media-fingerprint-marker-{}",
            Uuid::new_v4()
        ));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("test roots");
        roots.ensure().expect("create test roots");
        let profile_id = Uuid::new_v4();

        MediaFingerprintKey::write_cleanup_marker(&roots, profile_id, 1).expect("write marker");
        assert!(
            MediaFingerprintKey::cleanup_marker_version(&roots, profile_id).expect("valid marker")
                == Some(1)
        );
        MediaFingerprintKey::remove_cleanup_marker(&roots, profile_id).expect("remove marker");
        assert!(
            MediaFingerprintKey::cleanup_marker_version(&roots, profile_id)
                .expect("marker removed")
                .is_none()
        );
        MediaFingerprintKey::write_cleanup_marker(&roots, profile_id, 2).expect("write epoch 2");
        assert_eq!(
            MediaFingerprintKey::cleanup_marker_version(&roots, profile_id).expect("read epoch 2"),
            Some(2)
        );
        MediaFingerprintKey::remove_cleanup_marker(&roots, profile_id).expect("remove epoch 2");
        crate::durable::write_atomic(
            &MediaFingerprintKey::marker_path(&roots, profile_id),
            b"unexpected-marker",
        )
        .expect("write malformed marker fixture");
        assert!(MediaFingerprintKey::cleanup_marker_version(&roots, profile_id).is_err());
        assert!(MediaFingerprintKey::delete_blocking(&roots, profile_id).is_err());
        assert!(MediaFingerprintKey::marker_path(&roots, profile_id).exists());

        let _ = std::fs::remove_dir_all(base);
    }
}
