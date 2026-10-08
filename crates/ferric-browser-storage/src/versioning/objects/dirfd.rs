//! Descriptor-relative operations for private versioning directories.
//!
//! A verified pathname is not a stable capability: its parent can be replaced
//! before a later pathname-based unlink or rename. These helpers open every
//! ancestor without following symlinks, then mutate names relative to the
//! resulting directory descriptors. This narrows the same-UID race surface;
//! it is not process isolation from another process with the same UID.

use std::{fs::File, io, path::Path};

#[cfg(not(target_os = "redox"))]
use std::os::{fd::AsFd, unix::fs::MetadataExt};
#[cfg(not(target_os = "redox"))]
use std::path::Component;

#[cfg(not(any(target_os = "redox", target_os = "espidf")))]
use rustix::fs::AtFlags;
#[cfg(not(target_os = "redox"))]
use rustix::fs::{self as rustix_fs, Mode, OFlags};

use super::ObjectPathError;

/// INVARIANT: Descriptor-relative helpers accept only one non-empty leaf
/// component. Callers cannot turn a pinned parent descriptor back into an
/// arbitrary path traversal by supplying separators, dot components, or NUL.
fn is_leaf_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.as_bytes().contains(&b'/')
        && !name.as_bytes().contains(&0)
}

fn invalid_leaf_name() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "descriptor-relative name must be one leaf component",
    )
}

#[cfg(not(target_os = "redox"))]
fn directory_open_error(error: io::Error) -> ObjectPathError {
    let is_link_loop = error.raw_os_error() == Some(libc::ELOOP);
    if is_link_loop || error.kind() == io::ErrorKind::NotADirectory {
        ObjectPathError::UnsafeEntry
    } else {
        ObjectPathError::Io(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RegularEntryIdentity {
    pub(super) device: u64,
    pub(super) inode: u64,
    pub(super) links: u64,
    pub(super) bytes: u64,
}

/// Opens an absolute directory path by walking from `/` with `openat` and
/// `O_NOFOLLOW` for each component. Only the final directory must have the
/// profile owner's identity and private mode; intermediate system directories
/// retain their normal platform permissions.
pub(super) fn open_private_directory(path: &Path, owner_uid: u32) -> Result<File, ObjectPathError> {
    #[cfg(target_os = "redox")]
    {
        let _ = (path, owner_uid);
        Err(ObjectPathError::UnsupportedPlatform)
    }
    #[cfg(not(target_os = "redox"))]
    {
        if !path.is_absolute() {
            return Err(ObjectPathError::UnsafeEntry);
        }

        let mut directory = File::open("/")?;
        for component in path.components() {
            match component {
                Component::RootDir => {}
                Component::Normal(name) => {
                    let descriptor = rustix_fs::openat(
                        &directory,
                        name,
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(|error| directory_open_error(error.into()))?;
                    directory = File::from(descriptor);
                }
                _ => return Err(ObjectPathError::UnsafeEntry),
            }
        }

        let metadata = directory.metadata()?;
        if !metadata.is_dir() || metadata.uid() != owner_uid || metadata.mode() & 0o077 != 0 {
            return Err(ObjectPathError::UnsafeEntry);
        }
        Ok(directory)
    }
}

/// Reads and validates one regular child without resolving the final name as a
/// symlink. The returned identity supports the existing bounded replacement
/// checks before a descriptor-relative unlink or quarantine move.
pub(super) fn regular_entry_identity_at(
    directory: &File,
    name: &str,
    owner_uid: u32,
) -> Result<RegularEntryIdentity, ObjectPathError> {
    if !is_leaf_name(name) {
        return Err(ObjectPathError::UnsafeEntry);
    }
    #[cfg(any(target_os = "redox", target_os = "espidf"))]
    {
        let _ = (directory, name, owner_uid);
        Err(ObjectPathError::UnsupportedPlatform)
    }
    #[cfg(not(any(target_os = "redox", target_os = "espidf")))]
    {
        let metadata = rustix_fs::statat(directory.as_fd(), name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|error| ObjectPathError::Io(error.into()))?;
        if rustix_fs::FileType::from_raw_mode(metadata.st_mode) != rustix_fs::FileType::RegularFile
            || u64::from(metadata.st_uid) != u64::from(owner_uid)
            || metadata.st_mode & 0o077 != 0
        {
            return Err(ObjectPathError::UnsafeEntry);
        }
        Ok(RegularEntryIdentity {
            device: u64::from(metadata.st_dev),
            inode: u64::from(metadata.st_ino),
            links: u64::from(metadata.st_nlink),
            bytes: u64::try_from(metadata.st_size).map_err(|_| ObjectPathError::UnsafeEntry)?,
        })
    }
}

/// Opens one private directory child relative to an already pinned parent.
pub(super) fn open_private_directory_at(
    parent: &File,
    name: &str,
    owner_uid: u32,
) -> Result<File, ObjectPathError> {
    if !is_leaf_name(name) {
        return Err(ObjectPathError::UnsafeEntry);
    }
    #[cfg(any(target_os = "redox", target_os = "espidf"))]
    {
        let _ = (parent, name, owner_uid);
        Err(ObjectPathError::UnsupportedPlatform)
    }
    #[cfg(not(any(target_os = "redox", target_os = "espidf")))]
    {
        let entry = rustix_fs::statat(parent.as_fd(), name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|error| ObjectPathError::Io(error.into()))?;
        if rustix_fs::FileType::from_raw_mode(entry.st_mode) != rustix_fs::FileType::Directory {
            return Err(ObjectPathError::UnsafeEntry);
        }
        let descriptor = rustix_fs::openat(
            parent.as_fd(),
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| directory_open_error(error.into()))?;
        let directory = File::from(descriptor);
        let metadata = directory.metadata()?;
        if !metadata.is_dir()
            || metadata.uid() != owner_uid
            || metadata.mode() & 0o077 != 0
            || metadata.dev() != u64::from(entry.st_dev)
            || metadata.ino() != u64::from(entry.st_ino)
        {
            return Err(ObjectPathError::UnsafeEntry);
        }
        Ok(directory)
    }
}

/// Opens or exclusively creates one private directory child under a pinned
/// parent. If another entry appears after the missing-name check, the final
/// no-follow open still validates the exact child type and ownership.
pub(super) fn ensure_private_directory_at(
    parent: &File,
    name: &str,
    owner_uid: u32,
) -> Result<File, ObjectPathError> {
    match open_private_directory_at(parent, name, owner_uid) {
        Ok(directory) => Ok(directory),
        Err(ObjectPathError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
            match mkdir_at(parent, name) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
            open_private_directory_at(parent, name, owner_uid)
        }
        Err(error) => Err(error),
    }
}

/// Exclusively creates a private regular staging file relative to an already
/// pinned directory. A pre-existing leaf is never truncated or followed.
pub(super) fn create_private_regular_file_at(
    directory: &File,
    name: &str,
    owner_uid: u32,
) -> Result<File, ObjectPathError> {
    if !is_leaf_name(name) {
        return Err(ObjectPathError::UnsafeEntry);
    }
    #[cfg(any(target_os = "redox", target_os = "espidf"))]
    {
        let _ = (directory, name, owner_uid);
        Err(ObjectPathError::UnsupportedPlatform)
    }
    #[cfg(not(any(target_os = "redox", target_os = "espidf")))]
    {
        let descriptor = rustix_fs::openat(
            directory.as_fd(),
            name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|error| ObjectPathError::Io(error.into()))?;
        let file = File::from(descriptor);
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.uid() != owner_uid
            || metadata.mode() & 0o077 != 0
            || metadata.nlink() != 1
        {
            return Err(ObjectPathError::UnsafeEntry);
        }
        Ok(file)
    }
}

/// Opens one private regular file relative to a pinned directory without
/// following symlinks or blocking on a substituted FIFO.
pub(super) fn open_private_regular_file_at(
    directory: &File,
    name: &str,
    owner_uid: u32,
) -> Result<File, ObjectPathError> {
    if !is_leaf_name(name) {
        return Err(ObjectPathError::UnsafeEntry);
    }
    #[cfg(any(target_os = "redox", target_os = "espidf"))]
    {
        let _ = (directory, name, owner_uid);
        Err(ObjectPathError::UnsupportedPlatform)
    }
    #[cfg(not(any(target_os = "redox", target_os = "espidf")))]
    {
        open_private_regular_file_at_with_flags(directory, name, owner_uid, OFlags::RDONLY)
    }
}

/// Opens one existing private regular file for bounded in-place population
/// after validating its identity without following a replacement symlink.
pub(super) fn open_private_regular_file_update_at(
    directory: &File,
    name: &str,
    owner_uid: u32,
) -> Result<File, ObjectPathError> {
    if !is_leaf_name(name) {
        return Err(ObjectPathError::UnsafeEntry);
    }
    #[cfg(any(target_os = "redox", target_os = "espidf"))]
    {
        let _ = (directory, name, owner_uid);
        Err(ObjectPathError::UnsupportedPlatform)
    }
    #[cfg(not(any(target_os = "redox", target_os = "espidf")))]
    {
        open_private_regular_file_at_with_flags(directory, name, owner_uid, OFlags::RDWR)
    }
}

#[cfg(not(any(target_os = "redox", target_os = "espidf")))]
fn open_private_regular_file_at_with_flags(
    directory: &File,
    name: &str,
    owner_uid: u32,
    access: OFlags,
) -> Result<File, ObjectPathError> {
    if !is_leaf_name(name) {
        return Err(ObjectPathError::UnsafeEntry);
    }
    let expected = regular_entry_identity_at(directory, name, owner_uid)?;
    if expected.links != 1 {
        return Err(ObjectPathError::UnsafeEntry);
    }
    let descriptor = rustix_fs::openat(
        directory.as_fd(),
        name,
        access | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| ObjectPathError::Io(error.into()))?;
    let file = File::from(descriptor);
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != owner_uid
        || metadata.mode() & 0o077 != 0
        || metadata.nlink() != expected.links
        || metadata.dev() != expected.device
        || metadata.ino() != expected.inode
        || metadata.len() != expected.bytes
    {
        return Err(ObjectPathError::UnsafeEntry);
    }
    Ok(file)
}

/// Hard-links one fixed staging leaf into a pinned shard directory. The
/// destination is never replaced; unsupported targets fail closed. Both
/// parent directories remain anchored if their pathnames are concurrently
/// moved or replaced.
pub(super) fn link_at(
    source_directory: &File,
    source_name: &str,
    destination_directory: &File,
    destination_name: &str,
) -> io::Result<()> {
    if !is_leaf_name(source_name) || !is_leaf_name(destination_name) {
        return Err(invalid_leaf_name());
    }
    #[cfg(any(target_os = "redox", target_os = "espidf"))]
    {
        let _ = (
            source_directory,
            source_name,
            destination_directory,
            destination_name,
        );
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "descriptor-relative hard links are unavailable on this Unix target",
        ))
    }
    #[cfg(not(any(target_os = "redox", target_os = "espidf")))]
    {
        rustix_fs::linkat(
            source_directory.as_fd(),
            source_name,
            destination_directory.as_fd(),
            destination_name,
            AtFlags::empty(),
        )
        .map_err(Into::into)
    }
}

/// Creates one private child directory relative to a pinned parent.
pub(super) fn mkdir_at(parent: &File, name: &str) -> io::Result<()> {
    if !is_leaf_name(name) {
        return Err(invalid_leaf_name());
    }
    #[cfg(target_os = "redox")]
    {
        let _ = (parent, name);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "descriptor-relative directory creation is unavailable on this Unix target",
        ))
    }
    #[cfg(not(target_os = "redox"))]
    {
        rustix_fs::mkdirat(parent.as_fd(), name, Mode::from_raw_mode(0o700)).map_err(Into::into)
    }
}

/// Removes one child name without resolving it through the filesystem
/// namespace. `remove_directory` selects `AT_REMOVEDIR`; either form refuses
/// an entry whose kind no longer matches the syscall rather than recursing.
pub(super) fn unlink_at(directory: &File, name: &str, remove_directory: bool) -> io::Result<()> {
    if !is_leaf_name(name) {
        return Err(invalid_leaf_name());
    }
    #[cfg(any(target_os = "redox", target_os = "espidf"))]
    {
        let _ = (directory, name, remove_directory);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "descriptor-relative unlink is unavailable on this Unix target",
        ))
    }
    #[cfg(not(any(target_os = "redox", target_os = "espidf")))]
    {
        let flags = if remove_directory {
            AtFlags::REMOVEDIR
        } else {
            AtFlags::empty()
        };
        rustix_fs::unlinkat(directory.as_fd(), name, flags).map_err(Into::into)
    }
}

/// Renames one leaf between two already-open private directories. Neither
/// parent path is re-resolved during the move, and a source symlink is moved
/// as a link rather than followed. Targets without an atomic no-replace
/// primitive receive `Unsupported`; check-then-rename is not a safe fallback.
pub(super) fn rename_at(
    source_directory: &File,
    source_name: &str,
    destination_directory: &File,
    destination_name: &str,
) -> io::Result<()> {
    if !is_leaf_name(source_name) || !is_leaf_name(destination_name) {
        return Err(invalid_leaf_name());
    }
    #[cfg(any(target_os = "linux", target_vendor = "apple"))]
    {
        rustix_fs::renameat_with(
            source_directory.as_fd(),
            source_name,
            destination_directory.as_fd(),
            destination_name,
            rustix_fs::RenameFlags::NOREPLACE,
        )
        .map_err(Into::into)
    }
    #[cfg(not(any(target_os = "linux", target_vendor = "apple")))]
    {
        let _ = (
            source_directory,
            source_name,
            destination_directory,
            destination_name,
        );
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "atomic no-replace rename is unavailable on this Unix target",
        ))
    }
}

#[cfg(all(test, not(any(target_os = "redox", target_os = "espidf"))))]
mod tests {
    use std::{
        fs,
        io::{Read, Write},
        os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    };

    use uuid::Uuid;

    use super::*;

    const HAS_ATOMIC_NOREPLACE_RENAME: bool =
        cfg!(any(target_os = "linux", target_vendor = "apple"));

    #[test]
    fn pinned_directory_descriptor_does_not_follow_replaced_parent_symlink() {
        let temporary = std::env::temp_dir().join(format!("ferric-dirfd-{}", Uuid::new_v4()));
        fs::create_dir(&temporary).expect("create isolated test root");
        let root = fs::canonicalize(&temporary).expect("canonicalize isolated test root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
            .expect("make test root private");
        let parent = root.join("parent");
        let moved_parent = root.join("moved-parent");
        let outside = root.join("outside");
        let quarantine = root.join("quarantine");
        fs::create_dir(&parent).expect("create source directory");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("make source directory private");
        fs::create_dir(&outside).expect("create outside directory");
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o700))
            .expect("make outside directory private");
        fs::create_dir(&quarantine).expect("create quarantine directory");
        fs::set_permissions(&quarantine, fs::Permissions::from_mode(0o700))
            .expect("make quarantine directory private");
        fs::write(parent.join("candidate"), b"profile-owned candidate").expect("create candidate");
        fs::set_permissions(parent.join("candidate"), fs::Permissions::from_mode(0o600))
            .expect("make candidate private");
        fs::write(parent.join("rename-candidate"), b"profile-owned move")
            .expect("create rename candidate");
        fs::write(outside.join("sentinel"), b"user data").expect("create outside sentinel");

        let owner_uid = fs::metadata(&root).expect("root metadata").uid();
        let pinned = open_private_directory(&parent, owner_uid).expect("pin private parent");
        let quarantine_directory =
            open_private_directory(&quarantine, owner_uid).expect("pin quarantine directory");
        assert!(
            create_private_regular_file_at(&pinned, "candidate", owner_uid).is_err(),
            "exclusive staging creation must preserve an existing leaf"
        );
        assert_eq!(
            fs::read(parent.join("candidate")).expect("existing staging bytes"),
            b"profile-owned candidate"
        );
        fs::rename(&parent, &moved_parent).expect("move pinned directory");
        symlink(&outside, &parent).expect("replace original path with outside symlink");

        assert!(open_private_directory(&parent, owner_uid).is_err());
        let shard = ensure_private_directory_at(&pinned, "shard", owner_uid)
            .expect("create child directory through pinned parent");
        assert!(shard.metadata().expect("shard metadata").is_dir());
        assert!(moved_parent.join("shard").is_dir());
        assert!(!outside.join("shard").exists());
        let mut pinned_candidate = open_private_regular_file_at(&pinned, "candidate", owner_uid)
            .expect("open leaf through pinned directory");
        let mut candidate_bytes = Vec::new();
        pinned_candidate
            .read_to_end(&mut candidate_bytes)
            .expect("read pinned candidate");
        assert_eq!(candidate_bytes, b"profile-owned candidate");
        drop(pinned_candidate);
        unlink_at(&pinned, "candidate", false).expect("unlink through pinned directory");
        assert!(!moved_parent.join("candidate").exists());
        let rename = rename_at(
            &pinned,
            "rename-candidate",
            &quarantine_directory,
            "moved-candidate",
        );
        if HAS_ATOMIC_NOREPLACE_RENAME {
            rename.expect("rename through pinned directory");
            assert!(!moved_parent.join("rename-candidate").exists());
            assert_eq!(
                fs::read(quarantine.join("moved-candidate")).expect("quarantined bytes"),
                b"profile-owned move"
            );
        } else {
            assert_eq!(
                rename
                    .expect_err("unsupported target refuses a non-atomic rename")
                    .kind(),
                io::ErrorKind::Unsupported
            );
            assert!(moved_parent.join("rename-candidate").exists());
            assert!(!quarantine.join("moved-candidate").exists());
        }
        let mut temporary = create_private_regular_file_at(&pinned, "anchored.tmp", owner_uid)
            .expect("create temporary through pinned directory");
        temporary
            .write_all(b"descriptor-relative object")
            .expect("write pinned temporary");
        temporary.sync_all().expect("sync pinned temporary");
        link_at(
            &pinned,
            "anchored.tmp",
            &quarantine_directory,
            "anchored-object",
        )
        .expect("link object between pinned directories");
        unlink_at(&pinned, "anchored.tmp", false).expect("retire pinned temporary");
        drop(temporary);
        assert!(!moved_parent.join("anchored.tmp").exists());
        assert_eq!(
            fs::read(quarantine.join("anchored-object")).expect("linked object bytes"),
            b"descriptor-relative object"
        );
        assert_eq!(
            fs::read(outside.join("sentinel")).expect("outside data remains"),
            b"user data"
        );

        fs::remove_file(parent).expect("remove test symlink");
        drop(shard);
        fs::remove_dir(moved_parent.join("shard")).expect("remove descriptor-relative shard");
        if HAS_ATOMIC_NOREPLACE_RENAME {
            fs::remove_file(quarantine.join("moved-candidate"))
                .expect("remove quarantined test candidate");
        } else {
            fs::remove_file(moved_parent.join("rename-candidate"))
                .expect("remove candidate retained by unsupported target");
        }
        fs::remove_dir(moved_parent).expect("remove moved test directory");
        fs::remove_file(outside.join("sentinel")).expect("remove test sentinel");
        fs::remove_dir(outside).expect("remove outside test directory");
        fs::remove_file(quarantine.join("anchored-object"))
            .expect("remove descriptor-relative test object");
        fs::remove_dir(quarantine).expect("remove quarantine test directory");
        fs::remove_dir(root).expect("remove test root");
    }

    #[test]
    fn pinned_regular_file_identity_detects_a_replaced_leaf() {
        let temporary = std::env::temp_dir().join(format!("ferric-dirfd-{}", Uuid::new_v4()));
        fs::create_dir(&temporary).expect("create isolated test root");
        let root = fs::canonicalize(&temporary).expect("canonicalize isolated test root");
        let parent = root.join("parent");
        fs::create_dir(&parent).expect("create private parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("make parent private");
        fs::write(parent.join("candidate"), b"original bytes").expect("create source leaf");
        fs::set_permissions(parent.join("candidate"), fs::Permissions::from_mode(0o600))
            .expect("make source leaf private");
        let owner_uid = fs::metadata(&root).expect("root metadata").uid();
        let directory = open_private_directory(&parent, owner_uid).expect("pin parent");
        let pinned_file =
            open_private_regular_file_at(&directory, "candidate", owner_uid).expect("open leaf");
        let pinned_identity = pinned_file.metadata().expect("opened identity");

        fs::rename(parent.join("candidate"), parent.join("original")).expect("move original leaf");
        fs::write(parent.join("candidate"), b"replacement bytes").expect("create replacement leaf");
        fs::set_permissions(parent.join("candidate"), fs::Permissions::from_mode(0o600))
            .expect("make replacement private");
        let replacement = regular_entry_identity_at(&directory, "candidate", owner_uid)
            .expect("inspect replacement through pinned parent");
        assert_eq!(replacement.device, pinned_identity.dev());
        assert_ne!(replacement.inode, pinned_identity.ino());
        assert_eq!(
            fs::read(parent.join("original")).expect("original bytes retained"),
            b"original bytes"
        );

        drop(pinned_file);
        fs::remove_file(parent.join("candidate")).expect("remove replacement");
        fs::remove_file(parent.join("original")).expect("remove original");
        fs::remove_dir(parent).expect("remove parent");
        fs::remove_dir(root).expect("remove test root");
    }

    #[test]
    fn quarantine_rename_does_not_replace_an_existing_entry() {
        let temporary = std::env::temp_dir().join(format!("ferric-dirfd-{}", Uuid::new_v4()));
        fs::create_dir(&temporary).expect("create isolated test root");
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o700))
            .expect("make test root private");
        let root = fs::canonicalize(&temporary).expect("canonicalize isolated test root");
        let source = root.join("source");
        let destination = root.join("destination");
        for directory in [&source, &destination] {
            fs::create_dir(directory).expect("create private test directory");
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
                .expect("make test directory private");
        }
        fs::write(source.join("candidate"), b"source bytes").expect("create source entry");
        fs::write(destination.join("candidate"), b"existing bytes")
            .expect("create destination collision");

        let owner_uid = fs::metadata(&root).expect("root metadata").uid();
        let source_directory =
            open_private_directory(&source, owner_uid).expect("open source directory");
        let destination_directory =
            open_private_directory(&destination, owner_uid).expect("open destination directory");
        let error = rename_at(
            &source_directory,
            "candidate",
            &destination_directory,
            "candidate",
        )
        .expect_err("quarantine rename refuses a collision");
        assert_eq!(
            error.kind(),
            if HAS_ATOMIC_NOREPLACE_RENAME {
                io::ErrorKind::AlreadyExists
            } else {
                io::ErrorKind::Unsupported
            }
        );
        assert_eq!(
            fs::read(source.join("candidate")).expect("source stays in place"),
            b"source bytes"
        );
        assert_eq!(
            fs::read(destination.join("candidate")).expect("destination stays intact"),
            b"existing bytes"
        );

        fs::remove_file(source.join("candidate")).expect("remove test source");
        fs::remove_file(destination.join("candidate")).expect("remove test destination");
        fs::remove_dir(source).expect("remove source directory");
        fs::remove_dir(destination).expect("remove destination directory");
        fs::remove_dir(root).expect("remove test root");
    }

    #[test]
    fn descriptor_relative_helpers_reject_non_leaf_names_before_mutation() {
        let temporary = std::env::temp_dir().join(format!("ferric-dirfd-{}", Uuid::new_v4()));
        fs::create_dir(&temporary).expect("create isolated test root");
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o700))
            .expect("make test root private");
        let root = fs::canonicalize(&temporary).expect("canonicalize isolated test root");
        let parent = root.join("parent");
        let outside = root.join("outside");
        for directory in [&parent, &outside] {
            fs::create_dir(directory).expect("create private test directory");
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
                .expect("make test directory private");
        }
        fs::write(parent.join("candidate"), b"profile-owned bytes")
            .expect("create source candidate");
        fs::set_permissions(parent.join("candidate"), fs::Permissions::from_mode(0o600))
            .expect("make source candidate private");
        fs::write(outside.join("sentinel"), b"user data").expect("create outside sentinel");
        fs::set_permissions(outside.join("sentinel"), fs::Permissions::from_mode(0o600))
            .expect("make outside sentinel private");

        let owner_uid = fs::metadata(&root).expect("root metadata").uid();
        let parent_directory =
            open_private_directory(&parent, owner_uid).expect("pin private parent");
        let outside_directory =
            open_private_directory(&outside, owner_uid).expect("pin outside test directory");
        let invalid_names = ["", ".", "..", "../outside", "nested/name", "nul\0name"];

        for invalid_name in invalid_names {
            assert!(matches!(
                regular_entry_identity_at(&parent_directory, invalid_name, owner_uid),
                Err(ObjectPathError::UnsafeEntry)
            ));
            assert!(matches!(
                open_private_directory_at(&parent_directory, invalid_name, owner_uid),
                Err(ObjectPathError::UnsafeEntry)
            ));
            assert!(matches!(
                ensure_private_directory_at(&parent_directory, invalid_name, owner_uid),
                Err(ObjectPathError::UnsafeEntry)
            ));
            assert!(matches!(
                create_private_regular_file_at(&parent_directory, invalid_name, owner_uid),
                Err(ObjectPathError::UnsafeEntry)
            ));
            assert!(matches!(
                open_private_regular_file_at(&parent_directory, invalid_name, owner_uid),
                Err(ObjectPathError::UnsafeEntry)
            ));
            assert!(matches!(
                open_private_regular_file_update_at(&parent_directory, invalid_name, owner_uid),
                Err(ObjectPathError::UnsafeEntry)
            ));
            assert!(matches!(
                open_private_regular_file_at_with_flags(
                    &parent_directory,
                    invalid_name,
                    owner_uid,
                    OFlags::RDONLY,
                ),
                Err(ObjectPathError::UnsafeEntry)
            ));

            assert_eq!(
                mkdir_at(&parent_directory, invalid_name)
                    .expect_err("mkdir rejects a non-leaf name")
                    .kind(),
                io::ErrorKind::InvalidInput
            );
            assert_eq!(
                unlink_at(&parent_directory, invalid_name, false)
                    .expect_err("unlink rejects a non-leaf name")
                    .kind(),
                io::ErrorKind::InvalidInput
            );
            assert_eq!(
                link_at(
                    &parent_directory,
                    "candidate",
                    &outside_directory,
                    invalid_name,
                )
                .expect_err("link rejects a non-leaf destination")
                .kind(),
                io::ErrorKind::InvalidInput
            );
            assert_eq!(
                link_at(
                    &parent_directory,
                    invalid_name,
                    &outside_directory,
                    "linked",
                )
                .expect_err("link rejects a non-leaf source")
                .kind(),
                io::ErrorKind::InvalidInput
            );
            assert_eq!(
                rename_at(
                    &parent_directory,
                    invalid_name,
                    &outside_directory,
                    "renamed",
                )
                .expect_err("rename rejects a non-leaf source")
                .kind(),
                io::ErrorKind::InvalidInput
            );
            assert_eq!(
                rename_at(
                    &parent_directory,
                    "candidate",
                    &outside_directory,
                    invalid_name,
                )
                .expect_err("rename rejects a non-leaf destination")
                .kind(),
                io::ErrorKind::InvalidInput
            );
        }

        assert_eq!(
            fs::read(outside.join("sentinel")).expect("outside data remains"),
            b"user data"
        );
        assert_eq!(
            fs::read(parent.join("candidate")).expect("source data remains"),
            b"profile-owned bytes"
        );
        assert_eq!(
            fs::read_dir(&outside)
                .expect("outside directory remains readable")
                .count(),
            1,
            "invalid leaf names must not create outside entries"
        );

        fs::remove_file(parent.join("candidate")).expect("remove source candidate");
        fs::remove_file(outside.join("sentinel")).expect("remove test sentinel");
        fs::remove_dir(parent).expect("remove parent directory");
        fs::remove_dir(outside).expect("remove outside directory");
        fs::remove_dir(root).expect("remove test root");
    }
}
