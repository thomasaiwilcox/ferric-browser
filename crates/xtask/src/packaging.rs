use super::{BTreeSet, Path, fs, run_from_directory};

pub(super) fn run_package(arguments: &[String]) -> Result<(), String> {
    let task = arguments
        .first()
        .ok_or_else(|| "package requires arch or artifacts".to_owned())?;
    if arguments.len() > 1 {
        return Err(format!(
            "package {task} does not accept additional arguments"
        ));
    }
    match task.as_str() {
        "arch" => validate_arch_package(),
        "artifacts" => validate_release_artifacts().and_then(|()| validate_license_policy()),
        other => Err(format!("unknown package task: {other}")),
    }
}

pub(super) fn validate_release_artifacts() -> Result<(), String> {
    let manifest_path = Path::new("packaging/release-artifacts.toml");
    let manifest = fs::read_to_string(manifest_path)
        .map_err(|error| format!("could not read {}: {error}", manifest_path.display()))?;
    let parsed = manifest
        .parse::<toml::Table>()
        .map_err(|error| format!("could not parse {}: {error}", manifest_path.display()))?;
    let entries = parsed
        .get("artifact")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "release artifact manifest must contain an [[artifact]] array".to_owned())?;
    if entries.is_empty() || entries.len() > 64 {
        return Err("release artifact manifest must contain 1..=64 entries".to_owned());
    }
    let mut paths = BTreeSet::new();
    for entry in entries {
        let table = entry
            .as_table()
            .ok_or_else(|| "release artifact entries must be tables".to_owned())?;
        let name = table
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| "release artifact entry is missing a name".to_owned())?;
        let path = table
            .get("path")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("release artifact {name} is missing a path"))?;
        if name.is_empty() || name.len() > 96 || path.is_empty() || path.len() > 256 {
            return Err(format!(
                "release artifact {name:?} has an invalid bounded name/path"
            ));
        }
        let relative = Path::new(path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| component == std::path::Component::ParentDir)
        {
            return Err(format!(
                "release artifact {name} has an unsafe path {path:?}"
            ));
        }
        if !paths.insert(path.to_owned()) {
            return Err(format!("release artifact manifest duplicates {path}"));
        }
        if !relative.is_file() {
            return Err(format!("release artifact {name} is missing: {path}"));
        }
    }
    println!(
        "release artifact manifest passed ({} checked-in inputs; source archives, signatures, and generated checksums remain release outputs)",
        entries.len()
    );
    Ok(())
}

pub(super) fn validate_license_policy() -> Result<(), String> {
    let policy_path = Path::new("packaging/license-policy.toml");
    let policy = fs::read_to_string(policy_path)
        .map_err(|error| format!("could not read {}: {error}", policy_path.display()))?;
    let parsed = policy
        .parse::<toml::Table>()
        .map_err(|error| format!("could not parse {}: {error}", policy_path.display()))?;
    let entries = parsed
        .get("dependency")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "license policy must contain a [[dependency]] array".to_owned())?;
    if entries.is_empty() || entries.len() > 32 {
        return Err("license policy must contain 1..=32 dependencies".to_owned());
    }
    let lockfile = fs::read_to_string("Cargo.lock")
        .map_err(|error| format!("could not read Cargo.lock: {error}"))?;
    let mut names = BTreeSet::new();
    for entry in entries {
        let table = entry
            .as_table()
            .ok_or_else(|| "license policy entries must be tables".to_owned())?;
        let name = table
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| "license policy dependency is missing a name".to_owned())?;
        let license = table
            .get("license")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("license policy dependency {name} is missing a license"))?;
        let provenance = table
            .get("provenance")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("license policy dependency {name} is missing provenance"))?;
        if name.is_empty()
            || name.len() > 96
            || license.is_empty()
            || license.len() > 128
            || provenance.is_empty()
            || provenance.len() > 512
            || name.chars().any(char::is_control)
            || license.chars().any(char::is_control)
            || provenance.chars().any(char::is_control)
        {
            return Err(format!(
                "license policy dependency {name:?} has invalid bounded metadata"
            ));
        }
        if !names.insert(name.to_owned()) {
            return Err(format!("license policy duplicates dependency {name}"));
        }
        let lock_name = format!(r#"name = "{name}""#);
        if !lockfile.lines().any(|line| line == lock_name) {
            return Err(format!(
                "license policy dependency {name} is absent from Cargo.lock"
            ));
        }
    }
    println!(
        "license policy passed ({} locked dependencies with bounded provenance)",
        entries.len()
    );
    Ok(())
}

pub(super) fn validate_arch_package() -> Result<(), String> {
    let required_files = [
        "packaging/PKGBUILD",
        "packaging/omarchy/PKGBUILD",
        "packaging/io.github.ferricbrowser.FerricBrowser.desktop",
        "packaging/io.github.ferricbrowser.FerricBrowser.svg",
        "packaging/dependencies.toml",
    ];
    for file in required_files {
        if !Path::new(file).is_file() {
            return Err(format!("Arch package input is missing: {file}"));
        }
    }
    validate_arch_install_layout()?;

    run_from_directory(
        "desktop-file-validate",
        &["io.github.ferricbrowser.FerricBrowser.desktop"],
        Path::new("packaging"),
    )?;
    run_from_directory(
        "makepkg",
        &["--printsrcinfo", "-p", "PKGBUILD"],
        Path::new("packaging"),
    )?;
    run_from_directory(
        "makepkg",
        &["--printsrcinfo", "-p", "PKGBUILD"],
        Path::new("packaging/omarchy"),
    )?;
    println!("Arch package metadata and desktop integration validation passed");
    Ok(())
}

pub(super) fn validate_arch_install_layout() -> Result<(), String> {
    let recipe = fs::read_to_string("packaging/PKGBUILD")
        .map_err(|error| format!("could not read packaging/PKGBUILD: {error}"))?;
    let required_recipe_fragments = [
        "cargo build --release --locked",
        "target/release/ferric-browser",
        "/usr/bin/ferric-browser",
        "/usr/share/applications/io.github.ferricbrowser.FerricBrowser.desktop",
        "/usr/share/icons/hicolor/scalable/apps/io.github.ferricbrowser.FerricBrowser.svg",
        "/usr/share/doc/${pkgname}/README.md",
        "/usr/share/doc/${pkgname}/DEVELOPMENT_SPEC.md",
        "/usr/share/doc/${pkgname}/grid-navigation.md",
        "/usr/share/doc/${pkgname}/dependencies.toml",
    ];
    for fragment in required_recipe_fragments {
        if !recipe.contains(fragment) {
            return Err(format!(
                "Arch package recipe does not declare the required installed layout fragment: {fragment}"
            ));
        }
    }
    validate_installed_file_manifest(
        Path::new("packaging/installed-files.txt"),
        &recipe,
        "ferric-browser",
    )?;
    let installed_manifest = fs::read_to_string("packaging/installed-files.txt")
        .map_err(|error| format!("could not read packaging/installed-files.txt: {error}"))?;
    for required in [
        "/usr/share/doc/ferric-browser/grid-navigation.md",
        "/usr/share/doc/ferric-browser/LICENSES.md",
        "/usr/share/doc/ferric-browser/license-policy.toml",
        "/usr/share/doc/ferric-browser/SUPPORT.md",
        "/usr/share/doc/ferric-browser/SECURITY.md",
        "/usr/share/doc/ferric-browser/RELEASE_POLICY.md",
        "/usr/share/doc/ferric-browser/PROVENANCE.md",
        "/usr/share/doc/ferric-browser/MAINTENANCE.md",
        "/usr/share/doc/ferric-browser/CONTRIBUTING.md",
        "/usr/share/doc/ferric-browser/CODE_OF_CONDUCT.md",
    ] {
        if !installed_manifest
            .lines()
            .any(|line| line.trim() == required)
        {
            return Err(format!(
                "primary installed-file manifest omits required release documentation: {required}"
            ));
        }
    }
    let omarchy_recipe = fs::read_to_string("packaging/omarchy/PKGBUILD")
        .map_err(|error| format!("could not read packaging/omarchy/PKGBUILD: {error}"))?;
    validate_installed_file_manifest(
        Path::new("packaging/omarchy/installed-files.txt"),
        &omarchy_recipe,
        "ferric-browser-omarchy",
    )?;
    let dependencies = fs::read_to_string("packaging/dependencies.toml")
        .map_err(|error| format!("could not read packaging/dependencies.toml: {error}"))?;
    for fragment in [
        "model = \"system-dynamic\"",
        "[required]",
        "xdg-utils",
        "[build]",
        "desktop-file-utils",
        "check_packages = [\"dbus\", \"weston\"]",
        "[optional.portals]",
    ] {
        if !dependencies.contains(fragment) {
            return Err(format!(
                "dependency manifest does not declare the distribution boundary: {fragment}"
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_installed_file_manifest(
    manifest_path: &Path,
    recipe: &str,
    package_name: &str,
) -> Result<(), String> {
    let manifest = fs::read_to_string(manifest_path)
        .map_err(|error| format!("could not read {}: {error}", manifest_path.display()))?;
    let expanded_recipe = recipe.replace("${pkgname}", package_name);
    let mut paths = BTreeSet::new();
    for (line_number, line) in manifest.lines().enumerate() {
        let path = line.trim();
        if path.is_empty() || path.starts_with('#') {
            continue;
        }
        let relative = Path::new(path);
        if !path.starts_with('/')
            || path.len() > 256
            || relative
                .components()
                .any(|component| component == std::path::Component::ParentDir)
        {
            return Err(format!(
                "installed-file manifest {} line {} has an unsafe path: {path:?}",
                manifest_path.display(),
                line_number + 1
            ));
        }
        if !paths.insert(path.to_owned()) {
            return Err(format!(
                "installed-file manifest {} duplicates {path}",
                manifest_path.display()
            ));
        }
        if !expanded_recipe.contains(path) {
            return Err(format!(
                "package recipe for {package_name} does not install manifest path {path}"
            ));
        }
    }
    if paths.is_empty() {
        return Err(format!(
            "installed-file manifest {} is empty",
            manifest_path.display()
        ));
    }
    Ok(())
}
