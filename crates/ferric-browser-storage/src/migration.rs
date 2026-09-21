//! Explicit, copy-only migration from the pre-alpha `RustBrowser` identity.
//!
//! Legacy names are intentionally confined to this module so production code
//! cannot accidentally resume writing to the retired roots.

use std::{
    env, fmt, fs,
    io::{self, BufReader, Read},
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{RootSpec, StorageRoots, inspect_store};

const LEGACY_XDG_NAME: &str = "rustbrowser";
pub const LEGACY_MIGRATION_FLAG: &str = "--from-rustbrowser";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationReport {
    pub files_copied: u64,
    pub bytes_copied: u64,
    pub databases_validated: u64,
}

#[derive(Debug)]
pub enum MigrationError {
    SourceMissing,
    DestinationPopulated(PathBuf),
    UnsafeEntry(PathBuf),
    InvalidDatabase { path: PathBuf, status: String },
    VerificationFailed(PathBuf),
    Io { path: PathBuf, source: io::Error },
}

impl fmt::Display for MigrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceMissing => formatter.write_str("no legacy RustBrowser data was found"),
            Self::DestinationPopulated(path) => write!(
                formatter,
                "Ferric Browser destination is already populated: {}",
                path.display()
            ),
            Self::UnsafeEntry(path) => {
                write!(
                    formatter,
                    "legacy data contains an unsafe entry: {}",
                    path.display()
                )
            }
            Self::InvalidDatabase { path, status } => write!(
                formatter,
                "legacy database failed read-only validation ({status}): {}",
                path.display()
            ),
            Self::VerificationFailed(path) => write!(
                formatter,
                "copied data failed checksum verification: {}",
                path.display()
            ),
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
        }
    }
}

impl std::error::Error for MigrationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Copies all detected legacy XDG roots into the Ferric Browser roots.
///
/// The legacy roots are never renamed, modified, or deleted. Every copied
/// regular file is compared byte-for-byte before any destination is committed,
/// and profile SQLite databases receive a read-only schema/integrity check.
///
/// # Errors
///
/// Returns an error if no legacy data exists, a destination contains data, an
/// entry is not a regular file/directory, validation fails, or I/O fails.
pub fn migrate_legacy_xdg() -> Result<MigrationReport, MigrationError> {
    let source = legacy_xdg_roots();
    let destination = StorageRoots::resolve(RootSpec::Xdg).map_err(|error| MigrationError::Io {
        path: PathBuf::from("XDG roots"),
        source: io::Error::other(error),
    })?;
    migrate_roots(&source, &destination)
}

/// Copy-migrates an explicit root set. This is public to support isolated
/// migration qualification without mutating process-global XDG variables.
///
/// # Errors
///
/// See [`migrate_legacy_xdg`].
pub fn migrate_roots(
    source: &StorageRoots,
    destination: &StorageRoots,
) -> Result<MigrationReport, MigrationError> {
    let pairs = root_pairs(source, destination);
    if !pairs
        .iter()
        .any(|(path, _)| is_populated(path).unwrap_or(false))
    {
        return Err(MigrationError::SourceMissing);
    }
    for (_, path) in &pairs {
        if is_populated(path).map_err(|source| io_at(path, source))? {
            return Err(MigrationError::DestinationPopulated((*path).clone()));
        }
    }

    let nonce = Uuid::new_v4().simple().to_string();
    let mut stages = Vec::with_capacity(pairs.len());
    let mut report = MigrationReport {
        files_copied: 0,
        bytes_copied: 0,
        databases_validated: 0,
    };
    let prepared = (|| {
        for (index, (source_root, destination_root)) in pairs.iter().enumerate() {
            let parent = destination_root
                .parent()
                .ok_or_else(|| MigrationError::UnsafeEntry((*destination_root).clone()))?;
            fs::create_dir_all(parent).map_err(|source| io_at(parent, source))?;
            let stage = parent.join(format!(".ferric-browser-migrate-{nonce}-{index}"));
            if stage.exists() {
                return Err(MigrationError::DestinationPopulated(stage));
            }
            fs::create_dir(&stage).map_err(|source| io_at(&stage, source))?;
            if source_root.exists() {
                copy_tree(source_root, &stage, &mut report)?;
                verify_tree(source_root, &stage)?;
                validate_databases(&stage, &mut report)?;
            }
            stages.push((stage, (*destination_root).clone()));
        }
        Ok(())
    })();
    if let Err(error) = prepared {
        cleanup_stages(&stages);
        return Err(error);
    }

    let mut committed = Vec::new();
    for (stage, destination_root) in &stages {
        if destination_root.exists() {
            fs::remove_dir(destination_root).map_err(|source| io_at(destination_root, source))?;
        }
        if let Err(source) = fs::rename(stage, destination_root) {
            for committed_root in committed.iter().rev() {
                let _ = fs::remove_dir_all(committed_root);
            }
            cleanup_stages(&stages);
            return Err(io_at(destination_root, source));
        }
        committed.push(destination_root.clone());
    }
    Ok(report)
}

fn legacy_xdg_roots() -> StorageRoots {
    let home = env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
    let state = env_path("XDG_STATE_HOME", home.join(".local/state")).join(LEGACY_XDG_NAME);
    StorageRoots {
        config: env_path("XDG_CONFIG_HOME", home.join(".config")).join(LEGACY_XDG_NAME),
        data: env_path("XDG_DATA_HOME", home.join(".local/share")).join(LEGACY_XDG_NAME),
        state: state.clone(),
        cache: env_path("XDG_CACHE_HOME", home.join(".cache")).join(LEGACY_XDG_NAME),
        runtime: env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .map_or_else(|| state.join("runtime"), |path| path.join(LEGACY_XDG_NAME)),
        temporary_root: None,
    }
}

fn root_pairs<'a>(
    source: &'a StorageRoots,
    destination: &'a StorageRoots,
) -> [(&'a PathBuf, &'a PathBuf); 5] {
    [
        (&source.config, &destination.config),
        (&source.data, &destination.data),
        (&source.state, &destination.state),
        (&source.cache, &destination.cache),
        (&source.runtime, &destination.runtime),
    ]
}

fn env_path(name: &str, fallback: PathBuf) -> PathBuf {
    env::var_os(name).map_or(fallback, PathBuf::from)
}

fn is_populated(path: &Path) -> io::Result<bool> {
    match fs::read_dir(path) {
        Ok(mut entries) => Ok(entries.next().transpose()?.is_some()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn copy_tree(
    source: &Path,
    destination: &Path,
    report: &mut MigrationReport,
) -> Result<(), MigrationError> {
    let metadata = fs::symlink_metadata(source).map_err(|error| io_at(source, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(MigrationError::UnsafeEntry(source.to_owned()));
    }
    for entry in fs::read_dir(source).map_err(|error| io_at(source, error))? {
        let entry = entry.map_err(|error| io_at(source, error))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| io_at(&source_path, error))?;
        if file_type.is_symlink() {
            return Err(MigrationError::UnsafeEntry(source_path));
        }
        if file_type.is_dir() {
            fs::create_dir(&destination_path).map_err(|error| io_at(&destination_path, error))?;
            copy_tree(&source_path, &destination_path, report)?;
        } else if file_type.is_file() {
            let bytes = fs::copy(&source_path, &destination_path)
                .map_err(|error| io_at(&destination_path, error))?;
            report.files_copied += 1;
            report.bytes_copied += bytes;
        } else {
            return Err(MigrationError::UnsafeEntry(source_path));
        }
    }
    Ok(())
}

fn verify_tree(source: &Path, destination: &Path) -> Result<(), MigrationError> {
    for entry in fs::read_dir(source).map_err(|error| io_at(source, error))? {
        let entry = entry.map_err(|error| io_at(source, error))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry
            .file_type()
            .map_err(|error| io_at(&source_path, error))?
            .is_dir()
        {
            verify_tree(&source_path, &destination_path)?;
        } else if !files_equal(&source_path, &destination_path)? {
            return Err(MigrationError::VerificationFailed(source_path));
        }
    }
    Ok(())
}

fn files_equal(left: &Path, right: &Path) -> Result<bool, MigrationError> {
    let mut left = BufReader::new(fs::File::open(left).map_err(|error| io_at(left, error))?);
    let mut right = BufReader::new(fs::File::open(right).map_err(|error| io_at(right, error))?);
    let mut left_buffer = vec![0_u8; 64 * 1024];
    let mut right_buffer = vec![0_u8; 64 * 1024];
    loop {
        let left_read = left
            .read(&mut left_buffer)
            .map_err(|error| MigrationError::Io {
                path: PathBuf::from("migration checksum source"),
                source: error,
            })?;
        let right_read = right
            .read(&mut right_buffer)
            .map_err(|error| MigrationError::Io {
                path: PathBuf::from("migration checksum destination"),
                source: error,
            })?;
        if left_read != right_read || left_buffer[..left_read] != right_buffer[..right_read] {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
    }
}

fn validate_databases(path: &Path, report: &mut MigrationReport) -> Result<(), MigrationError> {
    for entry in fs::read_dir(path).map_err(|error| io_at(path, error))? {
        let entry = entry.map_err(|error| io_at(path, error))?;
        let entry_path = entry.path();
        if entry
            .file_type()
            .map_err(|error| io_at(&entry_path, error))?
            .is_dir()
        {
            validate_databases(&entry_path, report)?;
        } else if entry.file_name() == "browser.sqlite" {
            let inspection = inspect_store(&entry_path);
            if inspection.status != "available" {
                return Err(MigrationError::InvalidDatabase {
                    path: entry_path,
                    status: inspection.status,
                });
            }
            report.databases_validated += 1;
        }
    }
    Ok(())
}

fn cleanup_stages(stages: &[(PathBuf, PathBuf)]) {
    for (stage, _) in stages {
        let _ = fs::remove_dir_all(stage);
    }
}

fn io_at(path: &Path, source: io::Error) -> MigrationError {
    MigrationError::Io {
        path: path.to_owned(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(base: &Path, name: &str) -> StorageRoots {
        StorageRoots::resolve(RootSpec::Base(base.join(name))).expect("roots")
    }

    #[test]
    fn migration_copies_and_verifies_without_changing_source() {
        let base = env::temp_dir().join(format!("ferric-browser-migration-{}", Uuid::new_v4()));
        let source = roots(&base, "legacy");
        let destination = roots(&base, "ferric");
        fs::create_dir_all(source.config.join("nested")).expect("source directory");
        fs::write(source.config.join("nested/config.toml"), b"[ui]\n").expect("source file");

        let report = migrate_roots(&source, &destination).expect("migration");
        assert_eq!(report.files_copied, 1);
        assert_eq!(
            fs::read(destination.config.join("nested/config.toml")).expect("destination"),
            b"[ui]\n"
        );
        assert_eq!(
            fs::read(source.config.join("nested/config.toml")).expect("source retained"),
            b"[ui]\n"
        );
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn populated_destination_is_never_overwritten() {
        let base = env::temp_dir().join(format!("ferric-browser-migration-{}", Uuid::new_v4()));
        let source = roots(&base, "legacy");
        let destination = roots(&base, "ferric");
        fs::create_dir_all(&source.config).expect("source");
        fs::write(source.config.join("config.toml"), b"legacy").expect("source file");
        fs::create_dir_all(&destination.config).expect("destination");
        fs::write(destination.config.join("config.toml"), b"current").expect("destination file");

        let error = migrate_roots(&source, &destination).expect_err("must refuse overwrite");
        assert!(matches!(error, MigrationError::DestinationPopulated(_)));
        assert_eq!(
            fs::read(destination.config.join("config.toml")).expect("destination retained"),
            b"current"
        );
        let _ = fs::remove_dir_all(base);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_failure_leaves_source_untouched() {
        use std::os::unix::fs::symlink;

        let base = env::temp_dir().join(format!("ferric-browser-migration-{}", Uuid::new_v4()));
        let source = roots(&base, "legacy");
        let destination = roots(&base, "ferric");
        fs::create_dir_all(&source.config).expect("source");
        fs::write(base.join("outside"), b"secret").expect("outside");
        symlink(base.join("outside"), source.config.join("link")).expect("symlink");

        let error = migrate_roots(&source, &destination).expect_err("unsafe entry");
        assert!(matches!(error, MigrationError::UnsafeEntry(_)));
        assert!(source.config.join("link").exists());
        assert!(!destination.config.join("link").exists());
        let _ = fs::remove_dir_all(base);
    }
}
