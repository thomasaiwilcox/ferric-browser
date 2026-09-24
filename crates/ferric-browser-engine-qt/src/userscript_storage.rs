//! Atomic userscript enablement updates and safe removal.

use std::{fs, io::Write, path::Path};

use crate::userscript::{MAX_MANIFEST_BYTES, load, validate, validate_name};

pub(crate) fn set_private_file_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("could not set userscript permissions: {error}"))?;
    }
    Ok(())
}

pub(crate) fn set_private_directory_permissions(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("could not set userscript directory permissions: {error}"))?;
    }
    Ok(())
}

/// Changes one installed manifest's enabled state using an atomic replacement.
///
/// # Errors
///
/// Returns an error when the name or manifest is invalid, the replacement file
/// cannot be created safely, or the durable replacement cannot be installed.
pub fn set_enabled(root: &Path, name: &str, enabled: bool) -> Result<(), String> {
    validate_name(name)?;
    let loaded = load(root, name)?;
    let metadata = fs::symlink_metadata(&loaded.path)
        .map_err(|error| format!("could not inspect userscript manifest: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("userscript manifest must be a real file".into());
    }
    let mut manifest = loaded.manifest;
    manifest.enabled = enabled;
    validate(&manifest, name)?;
    let encoded = toml::to_string(&manifest)
        .map_err(|error| format!("could not encode userscript manifest: {error}"))?;
    if encoded.len() > MAX_MANIFEST_BYTES as usize {
        return Err("userscript manifest exceeds 64 KiB".into());
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "userscript clock is before the Unix epoch".to_owned())?
        .as_nanos();
    let temporary = loaded
        .path
        .with_extension(format!("toml.tmp-{}-{stamp}", std::process::id()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("could not create userscript update: {error}"))?;
        file.write_all(encoded.as_bytes())
            .map_err(|error| format!("could not write userscript update: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("could not sync userscript update: {error}"))?;
        set_private_file_permissions(&temporary)?;
        fs::rename(&temporary, &loaded.path)
            .map_err(|error| format!("could not install userscript update: {error}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Removes one installed userscript and its private copied assets.
///
/// The manifest is renamed out of the live namespace before the asset
/// directory is removed, so a failed asset deletion can restore the manifest.
/// Symlinked manifests and asset directories are refused.
///
/// # Errors
///
/// Returns an error when the manifest or asset directory is invalid, the
/// removal cannot be completed, or the directory cannot be synchronized.
pub fn remove(root: &Path, name: &str) -> Result<(), String> {
    validate_name(name)?;
    let loaded = load(root, name)?;
    let manifest_metadata = fs::symlink_metadata(&loaded.path)
        .map_err(|error| format!("could not inspect userscript manifest: {error}"))?;
    if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
        return Err("userscript manifest must be a real file".into());
    }
    let assets = root.join("userscripts").join(format!("{name}.assets"));
    inspect_asset_directory(&assets)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "userscript clock is before the Unix epoch".to_owned())?
        .as_nanos();
    let tombstone = loaded
        .path
        .with_extension(format!("toml.remove-{}-{stamp}", std::process::id()));
    if fs::symlink_metadata(&tombstone).is_ok() {
        return Err("userscript removal marker already exists".into());
    }
    fs::rename(&loaded.path, &tombstone)
        .map_err(|error| format!("could not stage userscript removal: {error}"))?;
    let cleanup = (|| {
        if inspect_asset_directory(&assets)? {
            fs::remove_dir_all(&assets)
                .map_err(|error| format!("could not remove userscript assets: {error}"))?;
        }
        fs::remove_file(&tombstone)
            .map_err(|error| format!("could not remove userscript manifest: {error}"))?;
        sync_directory(loaded.path.parent().unwrap_or_else(|| Path::new(".")))
    })();
    if let Err(error) = cleanup {
        if fs::symlink_metadata(&tombstone).is_ok() {
            let _ = fs::rename(&tombstone, &loaded.path);
        }
        return Err(error);
    }
    Ok(())
}

fn inspect_asset_directory(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("userscript asset directory must be a real directory".into());
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("could not inspect userscript assets: {error}")),
    }
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("could not sync userscript directory: {error}"))
}
