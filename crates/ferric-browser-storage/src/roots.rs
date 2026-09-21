use std::{
    env, fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

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

#[derive(Debug)]
pub enum StorageRootsError {
    Io { path: PathBuf, message: String },
    UnsafeBase(PathBuf),
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
}
