//! Constrained file transfers on the existing configuration file worker.
use ferric_browser_config::{MAX_SITE_POLICY_INTERCHANGE_BYTES, SitePolicyExchange};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub(super) enum ReadFailure {
    Io(String),
    Format(String),
}

pub(super) fn read(path: &Path) -> Result<SitePolicyExchange, ReadFailure> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|error| ReadFailure::Io(error.to_string()))?;
    if !file
        .metadata()
        .map_err(|error| ReadFailure::Io(error.to_string()))?
        .is_file()
    {
        return Err(ReadFailure::Io(
            "import requires a regular local file".into(),
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_SITE_POLICY_INTERCHANGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ReadFailure::Io(error.to_string()))?;
    SitePolicyExchange::parse(&bytes).map_err(ReadFailure::Format)
}

/// Returns a durability warning after publication, never a false pre-write
/// failure that might encourage repeating an already completed export.
pub(super) fn write(
    path: &Path,
    bytes: &[u8],
    overwrite: bool,
    protected: &[PathBuf],
) -> Result<bool, String> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path.file_name().ok_or("export requires a filename")?;
    let mut directory_options = OpenOptions::new();
    directory_options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        directory_options.custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW);
    }
    let directory = directory_options
        .open(parent)
        .map_err(|error| error.to_string())?;
    #[cfg(target_os = "linux")]
    let anchor = {
        use std::os::fd::AsRawFd;
        PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()))
    };
    #[cfg(not(target_os = "linux"))]
    let anchor = fs::canonicalize(parent).map_err(|error| error.to_string())?;
    let target = anchor.join(name);
    let actual = fs::canonicalize(&anchor)
        .map_err(|error| error.to_string())?
        .join(name);
    for source in protected {
        let resolved = fs::canonicalize(source).ok().or_else(|| {
            Some(
                fs::canonicalize(source.parent()?)
                    .ok()?
                    .join(source.file_name()?),
            )
        });
        if resolved.as_ref() == Some(&actual) {
            return Err("policy export cannot replace a managed configuration source".into());
        }
    }
    match fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(
                "export destination must be a regular file, never a symlink or special file".into(),
            );
        }
        Ok(_) if !overwrite => {
            return Err("export destination already exists; use --overwrite explicitly".into());
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.to_string()),
        _ => {}
    }
    let temporary = anchor.join(format!(".ferric-policy-{}.tmp", uuid::Uuid::new_v4()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        let mut file = options
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| error.to_string())?;
        if overwrite {
            fs::rename(&temporary, &target)
        } else {
            fs::hard_link(&temporary, &target)
        }
        .map_err(|error| error.to_string())?;
        let _ = fs::remove_file(&temporary);
        Ok(directory.sync_all().is_err())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
