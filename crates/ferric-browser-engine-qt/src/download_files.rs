//! Secure file-system helpers for downloads and exported page artifacts.
//!
//! Qt owns download transport; this module owns only local path selection,
//! staging, finalization, and file-URL conversion. No Qt object or policy
//! state crosses this boundary.

use crate::MAX_USER_DIRS_FILE_BYTES;
use ferric_browser_storage::{CollisionPolicy, StorageRoots, choose_download_path};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(super) struct StagedDownload {
    staging_directory: PathBuf,
    staging_path: PathBuf,
    final_path: PathBuf,
}

impl StagedDownload {
    #[must_use]
    pub(super) fn staging_directory(&self) -> &Path {
        &self.staging_directory
    }

    #[cfg(test)]
    #[must_use]
    pub(super) fn staging_path(&self) -> &Path {
        &self.staging_path
    }
}

#[must_use]
pub(super) fn default_download_directory_path() -> PathBuf {
    if let Some(path) = std::env::var_os("XDG_DOWNLOAD_DIR").map(PathBuf::from)
        && path.is_absolute()
    {
        return path;
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let config_home = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| home.join(".config"));
        let user_dirs = config_home.join("user-dirs.dirs");
        if let Ok(metadata) = fs::metadata(&user_dirs)
            && metadata.is_file()
            && metadata.len() <= MAX_USER_DIRS_FILE_BYTES
            && let Ok(contents) = fs::read_to_string(user_dirs)
            && let Some(path) = parse_user_dirs_download(&contents, &home)
        {
            return path;
        }
        return home.join("Downloads");
    }
    std::env::temp_dir().join("ferric-browser-downloads")
}

pub(super) fn parse_user_dirs_download(contents: &str, home: &Path) -> Option<PathBuf> {
    for line in contents.lines().take(256) {
        let line = line.trim();
        let Some(value) = line.strip_prefix("XDG_DOWNLOAD_DIR=") else {
            continue;
        };
        let value = value.trim();
        let Some(value) = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
        else {
            continue;
        };
        let mut expanded = String::with_capacity(value.len());
        let mut characters = value.chars();
        while let Some(character) = characters.next() {
            if character != '\\' {
                expanded.push(character);
                continue;
            }
            match characters.next() {
                Some('x') if characters.next() == Some('2') && characters.next() == Some('0') => {
                    expanded.push(' ');
                }
                Some('\\') => expanded.push('\\'),
                Some('"') => expanded.push('"'),
                _ => {
                    expanded.clear();
                    break;
                }
            }
        }
        if expanded.is_empty() || expanded.chars().any(char::is_control) {
            continue;
        }
        let path = if let Some(suffix) = expanded.strip_prefix("$HOME/") {
            home.join(suffix)
        } else if expanded == "$HOME" {
            home.to_owned()
        } else {
            PathBuf::from(expanded)
        };
        if path.is_absolute() {
            return Some(path);
        }
    }
    None
}

#[must_use]
pub(super) fn configured_download_directory_path(config: &Value) -> PathBuf {
    let directory = config
        .get("downloads")
        .and_then(|downloads| downloads.get("directory"));
    if let Some(path) = directory
        .and_then(|directory| directory.get("path").or_else(|| directory.get("Path")))
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return path;
    }
    default_download_directory_path()
}

pub(super) fn validate_pdf_output_path(input: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(input);
    if !path.is_absolute() || input.chars().any(char::is_control) {
        return Err("PDF path must be an absolute path without control characters".into());
    }
    let directory = path
        .parent()
        .ok_or_else(|| "PDF path has no containing directory".to_owned())?;
    let metadata = fs::symlink_metadata(directory)
        .map_err(|error| format!("PDF containing directory is unavailable: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("PDF containing path is not a real directory".into());
    }
    Ok(path)
}

pub(super) fn validate_print_pdf_path(input: &str) -> Result<PathBuf, String> {
    let path = validate_pdf_output_path(input)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "PDF path has no valid filename".to_owned())?;
    let directory = path
        .parent()
        .ok_or_else(|| "PDF path has no containing directory".to_owned())?;
    let chosen = choose_download_path(directory, name, CollisionPolicy::Ask)
        .map_err(|error| format!("PDF destination rejected: {error}"))?;
    if chosen != path {
        return Err("PDF destination is not a safe non-colliding path".into());
    }
    Ok(path)
}

pub(super) fn cleanup_print_artifact(path: &Path, private_temporary: bool) {
    let _ = fs::remove_file(path);
    if private_temporary && let Some(parent) = path.parent() {
        let _ = fs::remove_dir(parent);
    }
}

#[must_use]
pub(super) fn download_staging_root(roots: Option<&StorageRoots>, session_id: Uuid) -> PathBuf {
    roots.map_or_else(
        || {
            std::env::temp_dir()
                .join("ferric-browser-download-staging")
                .join(session_id.to_string())
        },
        |roots| roots.runtime.join("download-staging"),
    )
}

pub(super) fn stage_download_path(
    roots: Option<&StorageRoots>,
    session_id: Uuid,
    final_path: &Path,
) -> Result<StagedDownload, String> {
    let root = download_staging_root(roots, session_id);
    fs::create_dir_all(&root)
        .map_err(|error| format!("download staging directory is unavailable: {error}"))?;
    let root_metadata = fs::symlink_metadata(&root)
        .map_err(|error| format!("download staging directory is unavailable: {error}"))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err("download staging path is not a real directory".into());
    }
    #[cfg(unix)]
    fs::set_permissions(&root, std::os::unix::fs::PermissionsExt::from_mode(0o700))
        .map_err(|error| format!("download staging permissions could not be secured: {error}"))?;

    let staging_directory = root.join(Uuid::new_v4().to_string());
    fs::create_dir(&staging_directory)
        .map_err(|error| format!("download staging directory could not be created: {error}"))?;
    #[cfg(unix)]
    if let Err(error) = fs::set_permissions(
        &staging_directory,
        std::os::unix::fs::PermissionsExt::from_mode(0o700),
    ) {
        let _ = fs::remove_dir(&staging_directory);
        return Err(format!(
            "download staging permissions could not be secured: {error}"
        ));
    }
    let file_name = final_path
        .file_name()
        .ok_or_else(|| "download destination has no filename".to_owned())?;
    let staging_path = staging_directory.join(file_name);
    Ok(StagedDownload {
        staging_directory,
        staging_path,
        final_path: final_path.to_owned(),
    })
}

pub(super) fn cleanup_staged_download(staged: &StagedDownload) {
    let _ = fs::remove_file(&staged.staging_path);
    let _ = fs::remove_dir(&staged.staging_directory);
}

pub(super) fn cleanup_staged_downloads(staged_downloads: &mut BTreeMap<String, StagedDownload>) {
    for staged in staged_downloads.values() {
        cleanup_staged_download(staged);
    }
    staged_downloads.clear();
}

pub(super) fn finalize_staged_download(staged: &StagedDownload) -> Result<(), String> {
    let source_metadata = fs::symlink_metadata(&staged.staging_path)
        .map_err(|error| format!("staged download is unavailable: {error}"))?;
    if source_metadata.file_type().is_symlink() || !source_metadata.is_file() {
        return Err("staged download is not a regular file".into());
    }
    let directory = staged
        .final_path
        .parent()
        .ok_or_else(|| "download destination has no containing directory".to_owned())?;
    let directory_metadata = fs::symlink_metadata(directory)
        .map_err(|error| format!("download destination directory is unavailable: {error}"))?;
    if directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir() {
        return Err("download destination directory is not a real directory".into());
    }
    let mut destination = fs::OpenOptions::new();
    destination.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut destination, 0o600);
    let mut destination = destination
        .open(&staged.final_path)
        .map_err(|error| format!("download destination cannot be created safely: {error}"))?;
    let copy_result = (|| {
        let mut source = fs::File::open(&staged.staging_path)
            .map_err(|error| format!("staged download cannot be opened: {error}"))?;
        std::io::copy(&mut source, &mut destination)
            .map_err(|error| format!("download finalization failed: {error}"))?;
        destination
            .sync_all()
            .map_err(|error| format!("download destination could not be synced: {error}"))
    })();
    if let Err(error) = copy_result {
        drop(destination);
        let _ = fs::remove_file(&staged.final_path);
        return Err(error);
    }
    drop(destination);
    cleanup_staged_download(staged);
    Ok(())
}

pub(super) fn validate_save_page_path(input: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(input);
    if !path.is_absolute() || input.chars().any(char::is_control) {
        return Err("save-page path must be an absolute path without control characters".into());
    }
    let directory = path
        .parent()
        .ok_or_else(|| "save-page path has no containing directory".to_owned())?;
    let metadata = fs::symlink_metadata(directory)
        .map_err(|error| format!("save-page containing directory is unavailable: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("save-page containing path is not a real directory".into());
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "save-page path has no valid filename".to_owned())?;
    let chosen = choose_download_path(directory, name, CollisionPolicy::Ask)
        .map_err(|error| format!("save-page destination rejected: {error}"))?;
    if chosen != path {
        return Err("save-page destination is not a safe non-colliding path".into());
    }
    Ok(path)
}

pub(super) fn path_to_file_url(path: &Path) -> Result<String, String> {
    let path = path
        .to_str()
        .ok_or_else(|| "download path is not valid Unicode".to_owned())?;
    if !Path::new(path).is_absolute() || path.chars().any(char::is_control) {
        return Err("download path is not a safe absolute path".into());
    }
    let mut url = String::from("file://");
    for byte in path.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            url.push(char::from(*byte));
        } else {
            use std::fmt::Write as _;
            write!(url, "%{byte:02X}").map_err(|_| "file URL encoding failed".to_owned())?;
        }
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_safe_xdg_download_paths() {
        let home = Path::new("/home/ferric");
        assert_eq!(
            parse_user_dirs_download("XDG_DOWNLOAD_DIR=\"$HOME/Work\\x20Files\"", home),
            Some(home.join("Work Files"))
        );
        assert!(parse_user_dirs_download("XDG_DOWNLOAD_DIR=\"relative\"", home).is_none());
        assert!(parse_user_dirs_download("XDG_DOWNLOAD_DIR=\"$HOME/Bad\\q\"", home).is_none());
    }

    #[test]
    fn file_urls_encode_untrusted_path_delimiters() {
        assert_eq!(
            path_to_file_url(Path::new("/tmp/a file#1.txt")).expect("URL"),
            "file:///tmp/a%20file%231.txt"
        );
        assert!(path_to_file_url(Path::new("relative.txt")).is_err());
    }
}
