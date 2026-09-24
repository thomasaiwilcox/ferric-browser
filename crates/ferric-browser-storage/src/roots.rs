use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// The storage-root contract for this clean-break pre-alpha release.
pub const CURRENT_ROOT_SCHEMA_VERSION: u32 = 3;
const ROOT_SCHEMA_FILE: &str = ".ferric-root-schema";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RootSpec {
    Xdg,
    Base(PathBuf),
    Temporary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageRoots {
    pub config: PathBuf,
    pub data: PathBuf,
    pub state: PathBuf,
    pub cache: PathBuf,
    pub runtime: PathBuf,
    pub(crate) temporary_root: Option<PathBuf>,
}

/// Summary of an explicit destructive reset of Ferric-owned storage roots.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResetReport {
    /// Number of root directories from which at least one entry was removed.
    pub roots_cleared: u8,
    /// Number of direct root entries removed. Nested entries are removed as
    /// part of their containing directory and are deliberately not counted.
    pub entries_removed: u64,
}

#[derive(Debug)]
pub enum StorageRootsError {
    Io { path: PathBuf, message: String },
    UnsafeBase(PathBuf),
    UnsafeRoot(PathBuf),
    UnsafeEntry(PathBuf),
    NotTemporary,
}

impl std::fmt::Display for StorageRootsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => write!(formatter, "{}: {message}", path.display()),
            Self::UnsafeBase(path) => write!(
                formatter,
                "storage base is not an absolute directory: {}",
                path.display()
            ),
            Self::UnsafeRoot(path) => write!(
                formatter,
                "storage root is not a real directory: {}",
                path.display()
            ),
            Self::UnsafeEntry(path) => write!(
                formatter,
                "storage reset found an unsupported filesystem entry: {}",
                path.display()
            ),
            Self::NotTemporary => formatter.write_str("only temporary roots can be cleaned up"),
        }
    }
}

impl std::error::Error for StorageRootsError {}

impl StorageRoots {
    /// Resolves the five browser-owned roots without touching the filesystem.
    /// `Base` redirects all categories under one explicit absolute directory;
    /// `Xdg` uses standard environment variables with a home fallback.
    ///
    /// # Errors
    ///
    /// Returns an error when an explicit base is not absolute or temporary
    /// root creation fails.
    pub fn resolve(spec: RootSpec) -> Result<Self, StorageRootsError> {
        match spec {
            RootSpec::Base(base) => {
                if !base.is_absolute() {
                    return Err(StorageRootsError::UnsafeBase(base));
                }
                Ok(Self::from_base(&base, None))
            }
            RootSpec::Temporary => {
                let root = temporary_directory()?;
                Ok(Self::from_base(&root, Some(root.clone())))
            }
            RootSpec::Xdg => {
                let home = env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from);
                let config =
                    env_path("XDG_CONFIG_HOME", home.join(".config")).join("ferric-browser");
                let data =
                    env_path("XDG_DATA_HOME", home.join(".local/share")).join("ferric-browser");
                let state =
                    env_path("XDG_STATE_HOME", home.join(".local/state")).join("ferric-browser");
                let cache = env_path("XDG_CACHE_HOME", home.join(".cache")).join("ferric-browser");
                let runtime = env::var_os("XDG_RUNTIME_DIR")
                    .map(PathBuf::from)
                    .map_or_else(|| state.join("runtime"), |path| path.join("ferric-browser"));
                Ok(Self {
                    config,
                    data,
                    state,
                    cache,
                    runtime,
                    temporary_root: None,
                })
            }
        }
    }

    /// Creates all root directories with private permissions where supported.
    ///
    /// # Errors
    ///
    /// Returns an error if a root cannot be created or is not a directory.
    pub fn ensure(&self) -> Result<(), StorageRootsError> {
        for path in [
            &self.config,
            &self.data,
            &self.state,
            &self.cache,
            &self.runtime,
        ] {
            fs::create_dir_all(path).map_err(|error| io_error(path, &error))?;
            let metadata = fs::metadata(path).map_err(|error| io_error(path, &error))?;
            if !metadata.is_dir() {
                return Err(StorageRootsError::Io {
                    path: path.clone(),
                    message: "path is not a directory".into(),
                });
            }
            set_private_permissions(path).map_err(|error| io_error(path, &error))?;
        }
        Ok(())
    }

    #[must_use]
    pub fn temporary_root(&self) -> Option<&Path> {
        self.temporary_root.as_deref()
    }

    /// Returns whether any Ferric-owned root currently contains an entry.
    ///
    /// This does not create missing roots. It rejects symlink roots so callers
    /// can safely use the result before a destructive reset.
    ///
    /// # Errors
    ///
    /// Returns an error when an existing root is unreadable, is not a real
    /// directory, or contains an unsupported filesystem root type.
    pub fn is_populated(&self) -> Result<bool, StorageRootsError> {
        self.roots().into_iter().try_fold(false, |populated, root| {
            if populated {
                return Ok(true);
            }
            let metadata = match fs::symlink_metadata(root) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(io_error(root, &error)),
            };
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(StorageRootsError::UnsafeRoot(root.to_owned()));
            }
            fs::read_dir(root)
                .map_err(|error| io_error(root, &error))?
                .next()
                .transpose()
                .map(|entry| entry.is_some())
                .map_err(|error| io_error(root, &error))
        })
    }

    /// Returns whether populated roots belong to a prior incompatible schema.
    ///
    /// A missing or malformed marker is intentionally treated as incompatible:
    /// the caller must require an explicit reset rather than guessing how old
    /// files should be interpreted.
    ///
    /// # Errors
    ///
    /// Returns an error when the roots cannot be inspected safely.
    pub fn requires_schema_reset(&self) -> Result<bool, StorageRootsError> {
        if !self.is_populated()? {
            return Ok(false);
        }
        let marker = self.state.join(ROOT_SCHEMA_FILE);
        match fs::read_to_string(&marker) {
            Ok(value) => Ok(value.trim() != CURRENT_ROOT_SCHEMA_VERSION.to_string()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(true),
            Err(error) => Err(io_error(&marker, &error)),
        }
    }

    /// Creates the roots and records the current clean-break schema marker.
    ///
    /// # Errors
    ///
    /// Returns an error when the roots or schema marker cannot be written.
    pub fn initialize_current_schema(&self) -> Result<(), StorageRootsError> {
        self.ensure()?;
        let marker = self.state.join(ROOT_SCHEMA_FILE);
        let stage = self.state.join(format!(
            ".{ROOT_SCHEMA_FILE}.{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos())
        ));
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&stage)
                .map_err(|error| io_error(&stage, &error))?;
            file.write_all(format!("{CURRENT_ROOT_SCHEMA_VERSION}\n").as_bytes())
                .map_err(|error| io_error(&stage, &error))?;
            file.sync_all().map_err(|error| io_error(&stage, &error))?;
            set_private_permissions(&stage).map_err(|error| io_error(&stage, &error))?;
            fs::rename(&stage, &marker).map_err(|error| io_error(&marker, &error))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&stage);
        }
        result
    }

    /// Removes every entry beneath the resolved Ferric Browser roots.
    ///
    /// The roots themselves and their parents are retained. Existing root
    /// symlinks and unsupported special entries are rejected before removal;
    /// child symlinks are removed as links and are never traversed.
    ///
    /// # Errors
    ///
    /// Returns an error when a root is unsafe, an entry cannot be removed, or
    /// a special filesystem entry is encountered.
    pub fn reset_owned_contents(&self) -> Result<ResetReport, StorageRootsError> {
        let mut report = ResetReport::default();
        for root in self.roots() {
            let metadata = match fs::symlink_metadata(root) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(io_error(root, &error)),
            };
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(StorageRootsError::UnsafeRoot(root.to_owned()));
            }

            let mut cleared_root = false;
            for entry in fs::read_dir(root).map_err(|error| io_error(root, &error))? {
                let entry = entry.map_err(|error| io_error(root, &error))?;
                let path = entry.path();
                let file_type = entry.file_type().map_err(|error| io_error(&path, &error))?;
                if file_type.is_dir() {
                    fs::remove_dir_all(&path).map_err(|error| io_error(&path, &error))?;
                } else if file_type.is_file() || file_type.is_symlink() {
                    fs::remove_file(&path).map_err(|error| io_error(&path, &error))?;
                } else {
                    return Err(StorageRootsError::UnsafeEntry(path));
                }
                cleared_root = true;
                report.entries_removed = report.entries_removed.saturating_add(1);
            }
            report.roots_cleared += u8::from(cleared_root);
        }
        Ok(report)
    }

    /// Removes exactly the root created by [`RootSpec::Temporary`].
    ///
    /// # Errors
    ///
    /// Returns an error for a non-temporary root or a filesystem failure.
    pub fn cleanup(self) -> Result<(), StorageRootsError> {
        let Some(root) = self.temporary_root else {
            return Err(StorageRootsError::NotTemporary);
        };
        fs::remove_dir_all(&root).map_err(|error| io_error(&root, &error))
    }

    fn roots(&self) -> [&Path; 5] {
        [
            &self.config,
            &self.data,
            &self.state,
            &self.cache,
            &self.runtime,
        ]
    }

    fn from_base(base: &Path, temporary_root: Option<PathBuf>) -> Self {
        Self {
            config: base.join("config"),
            data: base.join("data"),
            state: base.join("state"),
            cache: base.join("cache"),
            runtime: base.join("runtime"),
            temporary_root,
        }
    }
}

fn env_path(name: &str, fallback: PathBuf) -> PathBuf {
    env::var_os(name).map_or(fallback, PathBuf::from)
}

fn temporary_directory() -> Result<PathBuf, StorageRootsError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path = env::temp_dir().join(format!("ferric-browser-{timestamp}-{}", std::process::id()));
    fs::create_dir(&path).map_err(|error| io_error(&path, &error))?;
    set_private_permissions(&path).map_err(|error| io_error(&path, &error))?;
    Ok(path)
}

fn set_private_permissions(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn io_error(path: &Path, error: &io::Error) -> StorageRootsError {
    StorageRootsError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn base_roots_are_absolute_and_private() {
        let base = env::temp_dir().join(format!("ferric-browser-roots-{}", std::process::id()));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("roots");
        roots.ensure().expect("ensure");
        assert_eq!(roots.config, base.join("config"));
        #[cfg(unix)]
        assert_eq!(
            std::fs::metadata(&roots.config)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn temporary_roots_record_and_clean_exact_target() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary");
        let target = roots.temporary_root().expect("target").to_owned();
        roots.ensure().expect("ensure");
        roots.cleanup().expect("cleanup");
        assert!(!target.exists());
    }

    #[test]
    fn reset_clears_only_owned_root_contents() {
        let base = env::temp_dir().join(format!(
            "ferric-browser-reset-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("roots");
        roots.ensure().expect("ensure");
        fs::write(roots.config.join("config.toml"), "old").expect("config");
        fs::create_dir_all(roots.data.join("profiles/default")).expect("profile directory");
        fs::write(roots.data.join("profiles/default/browser.sqlite"), "old").expect("profile");
        fs::write(base.join("outside"), "retained").expect("outside");

        assert!(roots.is_populated().expect("populated"));
        let report = roots.reset_owned_contents().expect("reset");

        assert_eq!(report.roots_cleared, 2);
        assert_eq!(report.entries_removed, 2);
        assert!(!roots.is_populated().expect("empty"));
        assert_eq!(
            fs::read_to_string(base.join("outside")).expect("outside retained"),
            "retained"
        );
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn schema_marker_distinguishes_a_fresh_root_from_prior_data() {
        let base = env::temp_dir().join(format!(
            "ferric-browser-schema-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("roots");
        roots.ensure().expect("ensure");
        fs::write(roots.data.join("legacy"), "old").expect("legacy data");
        assert!(roots.requires_schema_reset().expect("legacy schema"));

        roots.reset_owned_contents().expect("reset");
        roots.initialize_current_schema().expect("initialize");
        assert!(!roots.requires_schema_reset().expect("current schema"));
        assert_eq!(
            fs::read_to_string(roots.state.join(ROOT_SCHEMA_FILE)).expect("schema marker"),
            format!("{CURRENT_ROOT_SCHEMA_VERSION}\n")
        );
        let _ = fs::remove_dir_all(base);
    }

    #[cfg(unix)]
    #[test]
    fn reset_rejects_a_symlink_root_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let base = env::temp_dir().join(format!(
            "ferric-browser-reset-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        let roots = StorageRoots::resolve(RootSpec::Base(base.join("roots"))).expect("roots");
        let target = base.join("outside");
        fs::create_dir_all(&target).expect("outside");
        fs::write(target.join("retained"), "value").expect("outside value");
        fs::create_dir_all(roots.config.parent().expect("config parent")).expect("parent");
        symlink(&target, &roots.config).expect("root symlink");

        let error = roots.reset_owned_contents().expect_err("unsafe root");
        assert!(matches!(error, StorageRootsError::UnsafeRoot(path) if path == roots.config));
        assert_eq!(
            fs::read_to_string(target.join("retained")).expect("retained"),
            "value"
        );
        let _ = fs::remove_dir_all(base);
    }
}
