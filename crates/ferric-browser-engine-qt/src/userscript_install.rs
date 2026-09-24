//! Safe local installation of userscript manifests and copied assets.

use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::userscript::{
    MAX_INSTALL_ASSET_BYTES, MAX_MANIFEST_BYTES, Manifest, set_private_directory_permissions,
    set_private_file_permissions, validate, validate_executable, validate_name,
    validate_page_source,
};

/// Installs one explicitly selected local manifest into the profile's
/// userscript directory. Relative executable and page-source assets are copied
/// into a private, script-specific asset directory and the installed manifest
/// is rewritten to refer to those copies. Existing manifests and asset
/// directories are never replaced.
///
/// # Errors
///
/// Returns an error when the selected file, manifest, dependency, or target
/// violates the bounded installation policy.
pub fn install_manifest(root: &Path, source_path: &Path) -> Result<String, String> {
    let source_metadata = fs::symlink_metadata(source_path)
        .map_err(|error| format!("userscript manifest is unavailable: {error}"))?;
    if !source_metadata.is_file() {
        return Err("selected userscript manifest is not a regular file".into());
    }
    if source_path.extension().and_then(|value| value.to_str()) != Some("toml") {
        return Err("userscript manifest must use the .toml extension".into());
    }
    if source_metadata.len() > MAX_MANIFEST_BYTES {
        return Err("userscript manifest exceeds 64 KiB".into());
    }
    let name = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "userscript manifest name is not valid UTF-8".to_owned())?;
    validate_name(name)?;
    let source = fs::read_to_string(source_path)
        .map_err(|error| format!("could not read userscript manifest: {error}"))?;
    let mut manifest: Manifest =
        toml::from_str(&source).map_err(|error| format!("invalid userscript manifest: {error}"))?;
    validate(&manifest, name)?;

    let userscripts = root.join("userscripts");
    fs::create_dir_all(&userscripts)
        .map_err(|error| format!("could not create userscript directory: {error}"))?;
    let userscripts_metadata = fs::symlink_metadata(&userscripts)
        .map_err(|error| format!("could not inspect userscript directory: {error}"))?;
    if userscripts_metadata.file_type().is_symlink() || !userscripts_metadata.is_dir() {
        return Err("userscript directory must be a real directory".into());
    }
    set_private_directory_permissions(&userscripts)?;
    let manifest_path = userscripts.join(format!("{name}.toml"));
    if fs::symlink_metadata(&manifest_path).is_ok() {
        return Err(format!("userscript {name} is already installed"));
    }
    let asset_directory = userscripts.join(format!("{name}.assets"));
    if fs::symlink_metadata(&asset_directory).is_ok() {
        return Err(format!(
            "userscript asset directory already exists for {name}"
        ));
    }

    let mut copied_assets = BTreeMap::new();
    let mut asset_directory_created = false;
    let install_result = (|| {
        let manifest_parent = source_path.parent().unwrap_or_else(|| Path::new("."));
        if Path::new(&manifest.executable).is_absolute() {
            validate_executable(Path::new(&manifest.executable))?;
        } else {
            asset_directory_created = true;
            let relative = install_relative_asset(
                manifest_parent,
                &manifest.executable,
                &asset_directory,
                &mut copied_assets,
                true,
            )?;
            manifest.executable = relative;
        }
        if let Some(source) = manifest.source.clone() {
            if Path::new(&source).is_absolute() {
                validate_page_source(Path::new(&source))?;
            } else {
                asset_directory_created = true;
                let relative = install_relative_asset(
                    manifest_parent,
                    &source,
                    &asset_directory,
                    &mut copied_assets,
                    false,
                )?;
                manifest.source = Some(relative);
            }
        }
        validate(&manifest, name)?;
        let installed = toml::to_string(&manifest)
            .map_err(|error| format!("could not encode installed userscript: {error}"))?;
        if installed.len() > MAX_MANIFEST_BYTES as usize {
            return Err("installed userscript manifest exceeds 64 KiB".into());
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&manifest_path)
            .map_err(|error| format!("could not create installed userscript: {error}"))?;
        file.write_all(installed.as_bytes())
            .map_err(|error| format!("could not write installed userscript: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("could not sync installed userscript: {error}"))?;
        set_private_file_permissions(&manifest_path)?;
        Ok::<(), String>(())
    })();
    if let Err(error) = install_result {
        let _ = fs::remove_file(&manifest_path);
        if asset_directory_created {
            let _ = fs::remove_dir_all(&asset_directory);
        }
        return Err(error);
    }
    Ok(name.to_owned())
}

fn install_relative_asset(
    manifest_parent: &Path,
    value: &str,
    asset_directory: &Path,
    copied_assets: &mut BTreeMap<PathBuf, PathBuf>,
    executable: bool,
) -> Result<String, String> {
    let relative = crate::userscript::safe_relative_asset(value)?;
    let source = manifest_parent.join(&relative);
    let destination = asset_directory.join(&relative);
    if !copied_assets.contains_key(&relative) {
        ensure_asset_directory(asset_directory)?;
        copy_install_asset(&source, &destination, executable)?;
        copied_assets.insert(relative.clone(), destination.clone());
    }
    if executable {
        validate_executable(&destination)?;
    } else {
        validate_page_source(&destination)?;
    }
    Ok(format!(
        "{}/{}",
        asset_directory
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "userscript asset directory name is invalid".to_owned())?,
        relative.to_string_lossy().replace('\\', "/")
    ))
}

fn ensure_asset_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("userscript asset directory must be a real directory".into());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path)
                .map_err(|error| format!("could not create userscript asset directory: {error}"))?;
        }
        Err(error) => {
            return Err(format!(
                "could not inspect userscript asset directory: {error}"
            ));
        }
    }
    set_private_directory_permissions(path)
}

fn copy_install_asset(source: &Path, destination: &Path, executable: bool) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| format!("userscript asset is unavailable: {error}"))?;
    if !metadata.is_file() {
        return Err("userscript asset is not a regular file".into());
    }
    if metadata.len() > MAX_INSTALL_ASSET_BYTES {
        return Err("userscript asset exceeds 16 MiB".into());
    }
    if executable {
        validate_executable(source)?;
    } else {
        validate_page_source(source)?;
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create userscript asset directory: {error}"))?;
        set_private_directory_permissions(parent)?;
    }
    fs::copy(source, destination)
        .map_err(|error| format!("could not copy userscript asset: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            destination,
            fs::Permissions::from_mode(if executable {
                metadata.permissions().mode() | 0o111
            } else {
                0o600
            }),
        )
        .map_err(|error| format!("could not set userscript asset permissions: {error}"))?;
    }
    Ok(())
}
