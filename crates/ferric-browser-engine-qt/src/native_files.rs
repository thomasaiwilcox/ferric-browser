//! Native filesystem and Qt-text boundary helpers.
//!
//! All helpers here are deterministic and operate on explicit arguments; they
//! do not inspect or mutate browser state.

use cxx_qt_lib::QString;
use ferric_browser_storage::{RootSpec, StorageRoots};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub(super) fn resolve_browser_roots(storage_base: &str) -> Result<StorageRoots, String> {
    if storage_base.is_empty() {
        StorageRoots::resolve(RootSpec::Xdg)
    } else {
        StorageRoots::resolve(RootSpec::Base(PathBuf::from(storage_base)))
    }
    .map_err(|error| error.to_string())
}

pub(super) fn set_private_directory_permissions(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[must_use]
pub(super) fn bounded_header(value: &QString) -> String {
    let value = value.to_string();
    if value.len() <= 4096 && !value.chars().any(char::is_control) {
        value
    } else {
        String::new()
    }
}

pub(super) fn atomic_write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "target has no parent directory".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    set_private_directory_permissions(parent).map_err(|error| error.to_string())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "target filename is invalid".to_owned())?;
    let temporary = parent.join(format!(".{file_name}.tmp-{}", Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
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
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        fs::rename(&temporary, path).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[must_use]
pub(super) fn utf16_cursor_to_scalar(input: &str, cursor: usize) -> usize {
    let mut units: usize = 0;
    for (scalar, character) in input.chars().enumerate() {
        let width = character.len_utf16();
        if units.saturating_add(width) > cursor {
            return scalar;
        }
        units = units.saturating_add(width);
    }
    input.chars().count()
}

#[must_use]
pub(super) fn scalar_cursor_to_utf16(input: &str, cursor: usize) -> usize {
    input.chars().take(cursor).map(char::len_utf16).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_conversion_is_stable_for_non_bmp_characters() {
        let value = "a😀b";
        assert_eq!(utf16_cursor_to_scalar(value, 0), 0);
        assert_eq!(utf16_cursor_to_scalar(value, 2), 1);
        assert_eq!(utf16_cursor_to_scalar(value, 3), 2);
        assert_eq!(scalar_cursor_to_utf16(value, 2), 3);
    }

    #[test]
    fn headers_are_bounded_and_control_free() {
        assert_eq!(bounded_header(&QString::from("etag-1")), "etag-1");
        assert!(bounded_header(&QString::from("bad\nheader")).is_empty());
    }
}
