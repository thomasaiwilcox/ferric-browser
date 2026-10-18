use crate::*;

const FOLDER: &str = "/tmp/Ferric files/résumé λ";

fn config_with(value: &str) -> Result<Config, toml::de::Error> {
    toml::from_str(&format!(
        "schema_version=3\n[downloads]\ndirectory={value}\n"
    ))
}
fn path_value(path: &str) -> toml::Value {
    let mut table = toml::Table::new();
    table.insert("path".into(), toml::Value::String(path.into()));
    toml::Value::Table(table)
}

#[test]
fn startup_and_runtime_paths_share_one_type_and_preserve_the_canonical_format() {
    let quoted = toml::Value::String(FOLDER.into()).to_string();
    for literal in [&quoted, &format!("{{path={quoted}}}")] {
        let config = config_with(literal).unwrap();
        validate(&config).unwrap();
        assert_eq!(
            config.downloads.directory,
            DownloadDirectory::Path(FOLDER.into())
        );
        let encoded = toml::to_string(&config).unwrap();
        assert_eq!(toml::from_str::<Config>(&encoded).unwrap(), config);
        let value = toml::Value::try_from(config).unwrap();
        assert_eq!(value["downloads"]["directory"], path_value(FOLDER));
    }
    for literal in [&quoted, FOLDER, &format!("{{path={quoted}}}")] {
        let mut overrides = RuntimeOverrides::default();
        overrides
            .set_literal("downloads.directory", literal)
            .unwrap();
        assert_eq!(
            overrides.settings["downloads.directory"],
            path_value(FOLDER)
        );
        let config = apply_runtime_overrides(&Config::default(), &overrides).unwrap();
        assert_eq!(
            config.downloads.directory,
            DownloadDirectory::Path(FOLDER.into())
        );
    }
}

#[test]
fn invalid_folder_edits_are_transactional_and_do_not_enable_arbitrary_tables() {
    let mut overrides = RuntimeOverrides::default();
    overrides
        .set_literal("downloads.directory", FOLDER)
        .unwrap();
    let previous = overrides.clone();
    for invalid in [
        toml::Value::String("relative/folder".into()),
        toml::Value::String("~/Downloads".into()),
        toml::Value::String("file:///tmp/folder".into()),
        toml::Value::String(String::new()),
        toml::Value::String("/tmp/bad\nname".into()),
        toml::Value::String("/tmp/bad\0name".into()),
        toml::Value::String(format!("/{}", "a".repeat(16 * 1024))),
        toml::Value::Boolean(true),
        toml::Value::Array(vec![]),
        path_value("relative/folder"),
        toml::Value::Table(toml::Table::from_iter([
            ("path".into(), toml::Value::String(FOLDER.into())),
            ("extra".into(), toml::Value::Boolean(true)),
        ])),
    ] {
        assert!(
            overrides
                .set_setting("downloads.directory", invalid.clone())
                .is_err(),
            "accepted {invalid:?}"
        );
        assert_eq!(overrides, previous);
        let mut manual = RuntimeOverrides::default();
        manual
            .settings
            .insert("downloads.directory".into(), invalid);
        assert!(apply_runtime_overrides(&Config::default(), &manual).is_err());
    }
    assert!(
        overrides
            .set_setting("ui.font_size_pt", path_value(FOLDER))
            .is_err()
    );
    assert_eq!(overrides, previous);
}

#[test]
fn defaults_reset_and_loaded_override_strings_resolve_without_falling_back() {
    let mut overrides = RuntimeOverrides::default();
    overrides
        .set_literal("downloads.directory", FOLDER)
        .unwrap();
    overrides
        .set_literal("downloads.directory", "xdg-downloads")
        .unwrap();
    let config = apply_runtime_overrides(&Config::default(), &overrides).unwrap();
    assert_eq!(config.downloads.directory, DownloadDirectory::XdgDownloads);
    assert_eq!(
        overrides.settings["downloads.directory"],
        toml::Value::String("xdg-downloads".into())
    );
    overrides.settings.insert(
        "downloads.directory".into(),
        toml::Value::String(FOLDER.into()),
    );
    let encoded = toml::to_string(&overrides).unwrap();
    let loaded = toml::from_str::<RuntimeOverrides>(&encoded).unwrap();
    let config = apply_runtime_overrides(&Config::default(), &loaded).unwrap();
    assert_eq!(
        config.downloads.directory,
        DownloadDirectory::Path(FOLDER.into())
    );
    overrides.unset_setting("downloads.directory").unwrap();
    assert_eq!(
        apply_runtime_overrides(&Config::default(), &overrides)
            .unwrap()
            .downloads
            .directory,
        DownloadDirectory::XdgDownloads
    );
}

#[test]
fn invalid_startup_paths_and_directly_constructed_candidates_are_rejected() {
    for path in [
        "relative",
        "~/Downloads",
        "file:///tmp/folder",
        "",
        "/tmp/bad\nname",
        "/tmp/bad\0name",
    ] {
        let quoted = toml::Value::String(path.into()).to_string();
        assert!(config_with(&quoted).is_err());
        assert!(config_with(&format!("{{path={quoted}}}")).is_err());
        let mut config = Config::default();
        config.downloads.directory = DownloadDirectory::Path(path.into());
        assert!(validate(&config).is_err());
    }
    assert!(config_with("{path=\"/tmp/folder\",unknown=true}").is_err());
    assert_eq!(
        config_with("\"xdg-downloads\"")
            .unwrap()
            .downloads
            .directory,
        DownloadDirectory::XdgDownloads
    );
}

#[test]
fn persisted_override_round_trip_preserves_the_selected_folder() {
    let root = std::env::temp_dir().join(format!(
        "ferric-download-directory-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("overrides.toml");
    let mut overrides = RuntimeOverrides::default();
    overrides
        .set_literal("downloads.directory", FOLDER)
        .unwrap();
    save_runtime_overrides_atomic(&file, &overrides).unwrap();
    let loaded = load_runtime_overrides(&file).unwrap();
    assert_eq!(loaded, overrides);
    assert_eq!(
        apply_runtime_overrides(&Config::default(), &loaded)
            .unwrap()
            .downloads
            .directory,
        DownloadDirectory::Path(FOLDER.into())
    );
    std::fs::remove_file(file).unwrap();
    std::fs::remove_dir(root).unwrap();
}
