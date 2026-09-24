use super::*;

#[test]
fn defaults_are_the_starter_configuration() {
    let config = Config::default();
    validate(&config).expect("defaults validate");
    assert_eq!(config.navigation.default_search, "ddg");
    assert_eq!(config.tabs.undo_limit, 100);
}

#[test]
fn v3_documents_require_an_explicit_schema_marker() {
    assert!(toml::from_str::<Config>("[ui]\nfont_size_pt = 12.0\n").is_err());
    assert!(
        toml::from_str::<ProfilesConfig>("[[profiles]]\nname = 'work'\nlabel = 'Work'\n").is_err()
    );
    assert!(toml::from_str::<RuntimeOverrides>("[settings]\n").is_err());
}

#[test]
fn remote_suggestions_are_rejected_without_a_provider() {
    let mut config = Config::default();
    config.privacy.remote_suggestions = true;
    let error = validate(&config).expect_err("unsupported remote suggestions rejected");
    assert!(error.to_string().contains("remote suggestion provider"));
}

#[test]
fn rejects_unknown_keys_and_unsafe_search_templates() {
    let unknown = toml::from_str::<toml::Value>("[ui]\nnot_a_setting = true").expect("toml");
    let error = unknown
        .try_into::<Config>()
        .expect_err("unknown key rejected");
    assert!(error.to_string().contains("unknown field"));

    let mut config = Config::default();
    config
        .search_engines
        .insert("bad".into(), "https://example.test/{query}/{query}".into());
    assert!(validate(&config).is_err());
}

#[test]
fn tools_editor_requires_one_complete_file_argument() {
    let mut config = Config::default();
    validate(&config).expect("empty editor is valid but unavailable");

    config.tools.editor = vec!["nvim".into(), "{file}".into()];
    validate(&config).expect("direct editor argv validates");

    config.tools.editor = vec!["nvim".into()];
    assert!(validate(&config).is_err());
    config.tools.editor = vec!["nvim".into(), "--cmd={file}".into()];
    assert!(validate(&config).is_err());
    config.tools.editor = vec!["-nvim".into(), "{file}".into()];
    assert!(validate(&config).is_err());
}

#[test]
fn action_targets_are_typed_bounded_and_placeholder_safe() {
    let mut config = Config::default();
    config.action_targets.insert(
        "mpv".into(),
        ActionTargetConfig {
            subject_types: vec!["url".into(), "link".into()],
            executable: "mpv".into(),
            argv: vec!["--force-window".into(), "{url}".into()],
            detach: true,
            allow_private: false,
        },
    );
    validate(&config).expect("valid action target");

    config.action_targets.get_mut("mpv").unwrap().argv = vec!["--url={url}".into()];
    assert!(validate(&config).is_err());
    config.action_targets.get_mut("mpv").unwrap().argv = vec!["{selection}".into()];
    assert!(validate(&config).is_err());
    config.action_targets.get_mut("mpv").unwrap().argv = vec!["{url}".into()];
    config.action_targets.get_mut("mpv").unwrap().executable = "-sh".into();
    assert!(validate(&config).is_err());

    let mut tab_target = Config::default();
    tab_target.action_targets.insert(
        "tab-tool".into(),
        ActionTargetConfig {
            subject_types: vec!["tab".into()],
            executable: "tool".into(),
            argv: vec!["{title}".into(), "{url}".into()],
            detach: false,
            allow_private: true,
        },
    );
    validate(&tab_target).expect("tab targets may use tab URL/title fields");
}

#[test]
fn validates_binding_layers_before_commit() {
    let mut config = Config::default();
    config
        .bindings
        .entry("normal".into())
        .or_default()
        .insert("g,t".into(), "tab-next".into());
    validate(&config).expect("valid binding layer");

    config
        .bindings
        .insert("unsupported".into(), BTreeMap::new());
    assert!(validate(&config).is_err());
}

#[test]
fn validates_exact_profile_permission_rules() {
    let mut config = Config::default();
    config.permission_rules.push(PermissionRuleConfig {
        id: "mail-notifications".into(),
        profile: "work".into(),
        origin: "https://mail.google.com".into(),
        permission: "notifications".into(),
        decision: PermissionDecision::Allow,
    });
    validate(&config).expect("permission rule");

    config.permission_rules[0].origin = "http://example.test".into();
    assert!(validate(&config).is_err());
    config.permission_rules[0].origin = "https://mail.google.com".into();
    config.permission_rules[0].decision = PermissionDecision::Ask;
    validate(&config).expect("ask permission rule");
}

#[test]
fn site_rules_match_in_priority_and_source_order() {
    let config = Config {
        site_rules: vec![
            SiteRule {
                id: "later".into(),
                pattern: "https://*.example.test/*".into(),
                priority: 10,
                set: BTreeMap::from([("content.zoom".into(), toml::Value::Float(1.25))]),
            },
            SiteRule {
                id: "base".into(),
                pattern: "https://example.test/*".into(),
                priority: 0,
                set: BTreeMap::from([("content.zoom".into(), toml::Value::Float(1.1))]),
            },
            SiteRule {
                id: "override".into(),
                pattern: "https://example.test/docs/*".into(),
                priority: 0,
                set: BTreeMap::from([("content.zoom".into(), toml::Value::Float(1.5))]),
            },
        ],
        ..Config::default()
    };
    validate(&config).expect("site rules validate");
    let rules = matching_site_rules(&config, "https://example.test/docs/start");
    assert_eq!(
        rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        vec!["base", "override", "later"]
    );
    assert_eq!(
        rules
            .last()
            .and_then(|rule| rule.set.get("content.zoom"))
            .and_then(toml::Value::as_float),
        Some(1.25)
    );
    assert!(matching_site_rules(&config, "file:///tmp/page.html").is_empty());
}

#[test]
fn site_patterns_normalize_scheme_host_port_and_path() {
    let mut config = Config {
        site_rules: vec![
            SiteRule {
                id: "wildcard".into(),
                pattern: "*://*.Example.test/*".into(),
                priority: 0,
                set: BTreeMap::new(),
            },
            SiteRule {
                id: "port".into(),
                pattern: "https://example.test:8443/docs/*".into(),
                priority: 0,
                set: BTreeMap::new(),
            },
        ],
        ..Config::default()
    };
    validate(&config).expect("site pattern syntax");
    assert_eq!(
        matching_site_rules(&config, "https://EXAMPLE.TEST/docs/start?x=1#part")
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        vec!["wildcard"]
    );
    assert_eq!(
        matching_site_rules(&config, "https://sub.example.test/other")
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        vec!["wildcard"]
    );
    assert!(matching_site_rules(&config, "https://badexample.test/other").is_empty());
    assert_eq!(
        matching_site_rules(&config, "https://example.test:8443/docs/page")
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        vec!["port"]
    );
    assert!(matching_site_rules(&config, "https://example.test:9443/docs/page").is_empty());
    config.site_rules[0].pattern = "ftp://example.test/*".into();
    assert!(validate(&config).is_err());
}

#[test]
fn site_rules_reject_unsafe_or_unknown_settings() {
    let mut config = Config::default();
    config.site_rules.push(SiteRule {
        id: "docs".into(),
        pattern: "https://docs.example.test/*".into(),
        priority: 0,
        set: BTreeMap::from([("unknown.setting".into(), toml::Value::Boolean(true))]),
    });
    assert!(validate(&config).is_err());

    config.site_rules[0].set = BTreeMap::from([("content.zoom".into(), toml::Value::Float(9.0))]);
    assert!(validate(&config).is_err());
}

#[test]
fn setting_registry_resolves_static_and_dynamic_settings() {
    let registry = SettingRegistry::default_v1();
    assert!(
        registry
            .definitions()
            .iter()
            .any(|setting| setting.key == "content.zoom")
    );
    assert_eq!(
        registry
            .resolve("content.zoom")
            .map(|setting| setting.value_type),
        Some("number")
    );
    assert_eq!(
        registry
            .resolve("search_engines.docs")
            .map(|setting| setting.value_type),
        Some("dynamic")
    );
    assert!(registry.resolve("unrecognized.setting").is_none());
}

#[test]
fn site_scope_capability_is_explicit_and_bounded() {
    assert_eq!(setting_supported_scopes("ui.reduced_motion"), &["global"]);
    assert_eq!(
        setting_supported_scopes("spellcheck.languages"),
        &["profile"]
    );
    assert_eq!(
        setting_supported_scopes("content.javascript"),
        &["global", "profile", "site"]
    );
    assert!(setting_supports_site_scope("content.javascript"));
    assert!(setting_supports_site_scope("content.zoom"));
    assert!(setting_supports_site_scope("input.entry_mode"));
    assert!(!setting_supports_site_scope("blocking.enabled"));
    assert!(!setting_supports_site_scope("ui.font_family"));
    assert!(!setting_supports_site_scope("proxy.global"));
}

#[test]
fn spellcheck_languages_are_bounded_bcp47_tags() {
    let mut config = Config::default();
    validate(&config).expect("default system language");

    config.spellcheck.languages = vec![
        "en-US".into(),
        "zh-Hant-TW".into(),
        "en-US-u-hc-h12".into(),
        "de-1996".into(),
        "x-private".into(),
        "i-klingon".into(),
        "zh-min-nan".into(),
    ];
    validate(&config).expect("valid BCP 47 language tags");

    for invalid in ["", "e", "x", "en_US", "en--US", "en-", "en-a"] {
        config.spellcheck.languages = vec![invalid.into()];
        assert!(
            validate(&config).is_err(),
            "accepted invalid tag {invalid:?}"
        );
    }

    config.spellcheck.languages = vec!["en".into(), "EN".into()];
    assert!(
        validate(&config).is_err(),
        "accepted duplicate language tags"
    );
    config.spellcheck.languages = vec!["system".into(), "SYSTEM".into()];
    assert!(validate(&config).is_err(), "accepted duplicate system tags");
    config.spellcheck.languages = vec!["SYSTEM".into()];
    assert!(
        validate(&config).is_err(),
        "accepted an incorrectly cased system token"
    );
    config.spellcheck.languages = vec!["en".into(); 17];
    assert!(
        validate(&config).is_err(),
        "accepted an oversized language list"
    );
}

#[test]
fn setting_registry_declares_metadata_for_static_and_dynamic_keys() {
    let metadata = setting_metadata("content.zoom").expect("content.zoom metadata");
    assert_eq!(metadata.value_type, "number");
    assert_eq!(metadata.default_value, "1.0");
    assert_eq!(metadata.supported_scopes, &["global", "profile", "site"]);
    assert_eq!(metadata.apply_time, "live");
    assert_eq!(metadata.prerequisite, "qt-webengine");

    let entry_mode = setting_metadata("input.entry_mode").expect("entry mode metadata");
    assert_eq!(entry_mode.apply_time, "navigation");
    assert_eq!(entry_mode.supported_scopes, &["global", "profile", "site"]);

    let dynamic = setting_metadata("search_engines.work").expect("dynamic metadata");
    assert_eq!(dynamic.value_type, "dynamic");
    assert_eq!(
        setting_supported_scopes("search_engines.work"),
        &["global", "profile"]
    );
    assert!(setting_metadata("not.a.setting").is_none());
}

#[test]
fn setting_registry_entries_are_unique_and_complete() {
    let mut keys = BTreeSet::new();
    for metadata in SETTING_METADATA {
        assert!(keys.insert(metadata.key), "duplicate metadata key");
        assert!(!metadata.value_type.is_empty());
        assert!(!metadata.supported_scopes.is_empty());
        assert!(!metadata.apply_time.is_empty());
        assert!(!metadata.prerequisite.is_empty());
        assert!(!metadata.sensitivity.is_empty());
    }
    assert!(keys.len() >= 70);
}

#[test]
fn reload_transaction_applies_immediate_values_and_reports_pending_values() {
    let current = toml::Value::try_from(Config::default()).expect("default TOML");
    let mut candidate = current.clone();
    candidate
        .get_mut("input")
        .and_then(toml::Value::as_table_mut)
        .expect("input table")
        .insert("entry_mode".into(), toml::Value::String("insert".into()));
    candidate
        .get_mut("session")
        .and_then(toml::Value::as_table_mut)
        .expect("session table")
        .insert("restore".into(), toml::Value::String("always".into()));
    candidate
        .get_mut("search_engines")
        .and_then(toml::Value::as_table_mut)
        .expect("search engines table")
        .remove("ddg");

    let active = apply_immediate_config_changes(&current, &candidate);
    assert_eq!(
        active
            .get("input")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("entry_mode")),
        Some(&toml::Value::String("insert".into()))
    );
    assert_eq!(
        active
            .get("session")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("restore")),
        current
            .get("session")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("restore"))
    );
    assert!(
        active
            .get("search_engines")
            .and_then(toml::Value::as_table)
            .is_some_and(|engines| !engines.contains_key("ddg"))
    );
    assert_eq!(
        pending_config_changes(&current, &candidate),
        vec![PendingSettingChange {
            key: "session.restore".into(),
            current: toml::Value::String("ask-after-crash".into()),
            pending: toml::Value::String("always".into()),
            apply_time: "startup",
        }]
    );
}

#[test]
fn theme_palette_import_is_data_only_and_uses_deterministic_fallbacks() {
    let palette = parse_theme_palette(
        r##"
mode = "light"
accent = "#123456"
background = "#010203"
foreground = "#fefefe"
selection_background = "#223344"
red = "#aa0000"
"##,
        "omarchy",
        Path::new("/theme/colors.toml"),
    )
    .expect("palette");
    assert_eq!(palette.mode, "light");
    assert_eq!(palette.accent, "#123456");
    assert_eq!(palette.selection, "#223344");
    assert_eq!(palette.selection_foreground, "#fefefe");
    assert_eq!(palette.yellow, ThemePalette::default().yellow);
    assert_eq!(palette.green, ThemePalette::default().green);
}

#[test]
fn omarchy_commit_metadata_accepts_only_bounded_git_refs() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-omarchy-git-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let git = directory.join(".git");
    fs::create_dir_all(git.join("refs/heads")).expect("git metadata directory");
    let commit = "0123456789abcdef0123456789abcdef01234567";
    fs::write(git.join("HEAD"), "ref: refs/heads/main\n").expect("symbolic HEAD");
    fs::write(git.join("refs/heads/main"), format!("{commit}\n")).expect("ref");
    assert_eq!(git_commit_at(&directory).as_deref(), Some(commit));

    fs::write(git.join("HEAD"), format!("{commit}\n")).expect("detached HEAD");
    assert_eq!(git_commit_at(&directory).as_deref(), Some(commit));

    fs::write(git.join("HEAD"), "ref: refs/../outside\n").expect("unsafe HEAD");
    assert!(git_commit_at(&directory).is_none());
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn semantic_theme_tokens_use_spec_defaults_and_accept_alpha() {
    let defaults = ThemePalette::default();
    assert_eq!(defaults.background, "#1e1e2e");
    assert_eq!(defaults.foreground, "#cdd6f4");
    assert_eq!(defaults.accent, "#89b4fa");
    assert_eq!(defaults.error, "#f38ba8");
    assert_eq!(defaults.warning, "#f9e2af");
    assert_eq!(defaults.success, "#a6e3a1");

    let palette = parse_theme_palette(
        "surface = \"#313244cc\"\nborder = \"#585b70\"\nprivate = \"#cba6f7\"\nmode_insert = \"#f9e2af\"\n",
        "file",
        Path::new("/theme/colors.toml"),
    )
    .expect("semantic palette");
    assert_eq!(palette.surface, "#313244cc");
    assert_eq!(palette.border, "#585b70");
    assert_eq!(palette.private, "#cba6f7");
    assert_eq!(palette.mode_insert, "#f9e2af");
    assert_eq!(palette.lighter_background, "#313244cc");
}

#[test]
fn builtin_theme_meets_wcag_aa_for_assessed_chrome_pairs() {
    let report = theme_contrast_report(&ThemePalette::default());
    assert_eq!(report.status, "pass");
    assert!(report.failing.is_empty());
    assert!(report.checks.iter().all(|check| check.passes_aa));
    assert!(contrast_ratio("#ffffff", "#000000").is_some_and(|ratio| ratio > 21.0 - 0.01));
}

#[test]
fn imperfect_user_theme_is_reported_without_being_rejected() {
    let palette = ThemePalette {
        foreground: "#222222".into(),
        background: "#111111".into(),
        ..ThemePalette::default()
    };
    let report = theme_contrast_report(&palette);
    assert_eq!(report.status, "warning");
    assert!(
        report
            .failing
            .iter()
            .any(|name| name == "foreground-on-background")
    );
}

#[test]
fn auto_theme_prefers_current_omarchy_path_and_explicit_path_wins() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-theme-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let current = directory.join(".local/state/omarchy/current/theme");
    let old = directory.join(".config/omarchy/current/theme");
    fs::create_dir_all(&current).expect("current theme directory");
    fs::create_dir_all(&old).expect("old theme directory");
    fs::write(current.join("colors.toml"), "accent = \"#112233\"\n").expect("current palette");
    fs::write(old.join("colors.toml"), "accent = \"#445566\"\n").expect("old palette");

    let auto = load_theme_palette_at(
        &ThemeConfig {
            source: ThemeSource::Auto,
            path: String::new(),
        },
        Some(&directory),
    )
    .expect("auto palette");
    assert_eq!(auto.accent, "#112233");
    assert_eq!(auto.source, "omarchy");
    assert_eq!(auto.provider_layout, "current");
    assert!(!auto.provider_version.contains('\n'));
    assert!(!auto.provider_commit.contains('\n'));

    let explicit = load_theme_palette_at(
        &ThemeConfig {
            source: ThemeSource::Builtin,
            path: old.join("colors.toml").to_string_lossy().into_owned(),
        },
        Some(&directory),
    )
    .expect("explicit palette");
    assert_eq!(explicit.accent, "#445566");
    assert_eq!(explicit.source, "file");
    assert!(explicit.provider_layout.is_empty());
    assert!(explicit.provider_version.is_empty());
    assert!(explicit.provider_commit.is_empty());
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn theme_palette_rejects_invalid_colors() {
    let result = parse_theme_palette(
        "accent = \"javascript:bad\"\n",
        "file",
        Path::new("/theme/colors.toml"),
    );
    assert!(matches!(result, Err(ThemePaletteError::InvalidColor(_))));
}

#[test]
fn reload_keeps_last_good_candidate_on_failure() {
    let directory =
        std::env::temp_dir().join(format!("ferric-browser-config-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("temp directory");
    let path = directory.join("config.toml");
    fs::write(&path, "schema_version = 3\n[ui]\nfont_size_pt = 12.0\n").expect("valid config");
    let mut store = ConfigStore::new();
    store.reload(&path).expect("first reload");
    fs::write(&path, "schema_version = 3\n[ui]\nfont_size_pt = 99.0\n").expect("invalid config");
    assert!(store.reload(&path).is_err());
    assert_eq!(store.revision(), 1);
    assert!(
        (store.current().expect("last good").config.ui.font_size_pt - 12.0).abs() < f64::EPSILON
    );
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&directory);
}

#[test]
fn contexts_save_round_trips_validated_routes_atomically() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-context-save-{}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).expect("temp directory");
    let path = directory.join("contexts.toml");
    let config = ContextsConfig {
        contexts: vec![ContextDefinition {
            name: "work".into(),
            label: "Work".into(),
            profile: "work".into(),
            sessions: Vec::new(),
            workspace: None,
            accent: None,
            default_target: "reuse-or-window".into(),
        }],
        routes: vec![ContextRoute {
            id: "company-work".into(),
            pattern: "https://*.company.test/*".into(),
            context: "work".into(),
            priority: 0,
            behavior: "prompt".into(),
            entry_points: vec!["explicit-open".into()],
        }],
        ..ContextsConfig::default()
    };
    save_contexts_atomic(&path, &config).expect("atomic contexts save");
    assert_eq!(load_contexts(&path).expect("saved contexts"), config);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&directory);
}

#[test]
fn profiles_load_selects_one_default_and_applies_typed_overrides() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-config-profiles-{}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).expect("temp directory");
    let path = directory.join("profiles.toml");
    fs::write(
        &path,
        r#"
schema_version = 3

[[profiles]]
name = "work"
label = "Work"
default = true
overrides = { "content.zoom" = 1.25, "input.entry_mode" = "insert" }
"#,
    )
    .expect("profiles");
    let profiles = load_profiles(&path).expect("valid profiles");
    let profile = profiles.default_profile().expect("default profile");
    let layer = profile_override_layer(profile).expect("profile layer");
    let config = apply_runtime_overrides(&Config::default(), &layer).expect("apply profile");
    assert_eq!(profile.name, "work");
    assert!((config.content.zoom - 1.25).abs() < f64::EPSILON);
    assert_eq!(config.input.entry_mode, EntryMode::Insert);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&directory);
}

#[test]
fn profiles_reject_duplicate_defaults_unknown_keys_and_site_rules() {
    let duplicate_defaults = ProfilesConfig {
        profiles: vec![
            ProfileDefinition {
                name: "one".into(),
                label: "One".into(),
                default: true,
                overrides: BTreeMap::new(),
            },
            ProfileDefinition {
                name: "two".into(),
                label: "Two".into(),
                default: true,
                overrides: BTreeMap::new(),
            },
        ],
        ..ProfilesConfig::default()
    };
    assert!(validate_profiles(&duplicate_defaults).is_err());

    let mut unknown = ProfileDefinition {
        name: "work".into(),
        label: "Work".into(),
        default: false,
        overrides: BTreeMap::new(),
    };
    unknown
        .overrides
        .insert("not.a_setting".into(), toml::Value::Boolean(true));
    assert!(validate_profile_definition(&unknown).is_err());
    unknown.overrides.clear();
    unknown
        .overrides
        .insert("site_rules".into(), toml::Value::Array(Vec::new()));
    assert!(validate_profile_definition(&unknown).is_err());
}

#[test]
fn includes_merge_in_order_and_reject_cycles() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-config-include-{}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).expect("temp directory");
    fs::write(
        directory.join("base.toml"),
        "[ui]\nfont_size_pt = 11.0\nfont_family = \"serif\"\n",
    )
    .expect("base");
    fs::write(
        directory.join("config.toml"),
        "schema_version = 3\ninclude = [\"base.toml\"]\n[ui]\nfont_size_pt = 13.0\n",
    )
    .expect("root");
    let loaded = load(directory.join("config.toml")).expect("include");
    assert!((loaded.config.ui.font_size_pt - 13.0).abs() < f64::EPSILON);
    assert_eq!(loaded.config.ui.font_family, "serif");
    fs::write(directory.join("base.toml"), "include = [\"config.toml\"]\n").expect("cycle");
    assert!(matches!(
        load(directory.join("config.toml")),
        Err(ConfigError::IncludeCycle { .. })
    ));
    let _ = fs::remove_file(directory.join("base.toml"));
    let _ = fs::remove_file(directory.join("config.toml"));
    let _ = fs::remove_dir(&directory);
}

#[test]
fn runtime_overrides_round_trip_atomically_and_apply_by_key() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-runtime-overrides-{}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).expect("temp directory");
    let path = directory.join("runtime-overrides.toml");
    let mut overrides = RuntimeOverrides::default();
    overrides
        .set_literal("content.zoom", "1.25")
        .expect("numeric override");
    overrides
        .set_literal("input.entry_mode", "insert")
        .expect("string override");
    overrides
        .set_binding("normal", "g,g", "")
        .expect("explicit unbound binding");
    save_runtime_overrides_atomic(&path, &overrides).expect("atomic save");

    let loaded = load_runtime_overrides(&path).expect("load");
    assert_eq!(loaded.settings, overrides.settings);
    assert_eq!(loaded.bindings, overrides.bindings);
    assert_eq!(loaded.site_rules, overrides.site_rules);
    let config = apply_runtime_overrides(&Config::default(), &loaded).expect("apply");
    assert!((config.content.zoom - 1.25).abs() < f64::EPSILON);
    assert_eq!(config.input.entry_mode, EntryMode::Insert);
    assert_eq!(
        config
            .bindings
            .get("normal")
            .and_then(|bindings| bindings.get("g,g"))
            .map(String::as_str),
        Some("unbound")
    );

    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&directory);
}

#[test]
fn runtime_override_layers_apply_in_documented_precedence_order() {
    let mut profile = RuntimeOverrides::default();
    profile
        .set_literal("content.zoom", "1.1")
        .expect("profile override");
    let mut runtime = RuntimeOverrides::default();
    runtime
        .set_literal("content.zoom", "1.2")
        .expect("runtime override");
    let mut command_line = RuntimeOverrides::default();
    command_line
        .set_literal("content.zoom", "1.3")
        .expect("command-line override");
    let mut temporary = RuntimeOverrides::default();
    temporary
        .set_literal("content.zoom", "1.4")
        .expect("temporary override");

    let effective = resolve_runtime_override_layers(
        &Config::default(),
        &profile,
        &runtime,
        &command_line,
        &temporary,
    )
    .expect("resolve layers");

    assert!((effective.content.zoom - 1.4).abs() < f64::EPSILON);
}

#[test]
fn runtime_site_rules_round_trip_override_and_unset() {
    let mut overrides = RuntimeOverrides::default();
    overrides
        .set_site_literal("https://docs.example/*", "input.entry_mode", "insert")
        .expect("site setting");
    overrides
        .set_site_literal("https://docs.example/*", "content.zoom", "1.25")
        .expect("second site setting");
    let config = apply_runtime_overrides(&Config::default(), &overrides).expect("apply");
    let rules = matching_site_rules(&config, "https://docs.example/editor");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].priority, i32::MAX);
    assert_eq!(rules[0].set["input.entry_mode"].as_str(), Some("insert"));
    assert_eq!(rules[0].set["content.zoom"].as_float(), Some(1.25));

    assert!(
        overrides
            .unset_site_setting("https://docs.example/*", "input.entry_mode")
            .expect("unset site setting")
    );
    assert!(
        !overrides
            .unset_site_setting("https://docs.example/*", "input.entry_mode")
            .expect("repeat unset")
    );
    assert!(
        overrides
            .unset_site_setting("https://docs.example/*", "content.zoom")
            .expect("unset final site setting")
    );
    assert!(overrides.site_rules.is_empty());
    assert!(
        overrides
            .set_site_literal("https://docs.example/*", "ui.font_size_pt", "12")
            .is_err()
    );
}

#[test]
fn runtime_override_unset_preserves_lower_layer_and_rejects_bad_types() {
    let mut overrides = RuntimeOverrides::default();
    overrides
        .set_literal("content.zoom", "1.5")
        .expect("override");
    assert!(overrides.unset_setting("content.zoom").expect("unset"));
    assert!(
        !overrides
            .unset_setting("content.zoom")
            .expect("second unset")
    );

    overrides
        .set_literal("content.zoom", "not-a-number")
        .expect("string is stored until typed application");
    let error =
        apply_runtime_overrides(&Config::default(), &overrides).expect_err("wrong type rejected");
    assert!(error.to_string().contains("wrong type"));
}

#[test]
fn blocking_bypass_sites_are_validated_and_runtime_overridable() {
    let mut config = Config::default();
    config.blocking.bypass_sites = vec!["example.test".into(), "*.sub.test".into()];
    validate(&config).expect("valid host bypasses");

    config.blocking.bypass_sites = vec!["https://example.test".into()];
    assert!(validate(&config).is_err());

    let mut overrides = RuntimeOverrides::default();
    overrides
        .set_literal("blocking.bypass_sites", "[\"example.test\"]")
        .expect("array override");
    let effective = apply_runtime_overrides(&Config::default(), &overrides).expect("apply");
    assert_eq!(effective.blocking.bypass_sites, ["example.test"]);
}

#[test]
fn link_cleaning_updates_require_explicit_https_source_and_pinned_checksum() {
    let mut config = Config::default();
    config.links.cleaning.update_source =
        Some("https://updates.example.test/ferric-browser-links.toml".into());
    config.links.cleaning.update_sha256 = Some("a".repeat(64));
    validate(&config).expect("pinned HTTPS link source");

    config.links.cleaning.update_source = Some("http://updates.example.test/rules".into());
    assert!(validate(&config).is_err());

    config.links.cleaning.update_source = None;
    assert!(validate(&config).is_err());
}

#[test]
fn context_document_validates_metadata_and_route_references() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-context-config-{}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).expect("temp directory");
    let path = directory.join("contexts.toml");
    fs::write(
        &path,
        r##"schema_version = 3

[[contexts]]
name = "work"
label = "Work"
profile = "work"
workspace = "3"
accent = "#7aa2f7"

[[routes]]
id = "company-work"
pattern = "https://*.company.test/*"
context = "work"
entry_points = ["explicit-open"]
"##,
    )
    .expect("contexts");
    let loaded = load_contexts(&path).expect("valid contexts");
    assert_eq!(loaded.contexts[0].default_target, "reuse-or-window");
    assert_eq!(loaded.routes[0].behavior, "prompt");
    fs::write(
        &path,
        "[[routes]]\nid = \"bad\"\npattern = \"https://example.test\"\ncontext = \"missing\"\n",
    )
    .expect("invalid contexts");
    assert!(load_contexts(&path).is_err());
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&directory);
}

#[test]
fn context_routes_use_narrow_host_port_path_and_entry_point_matching() {
    let config = ContextsConfig {
        routes: vec![
            ContextRoute {
                id: "broad".into(),
                pattern: "*://*.example.test/*".into(),
                context: "broad".into(),
                priority: 1,
                behavior: "prompt".into(),
                entry_points: vec!["explicit-open".into()],
            },
            ContextRoute {
                id: "specific".into(),
                pattern: "https://app.example.test:8443/docs/*".into(),
                context: "specific".into(),
                priority: 1,
                behavior: "prompt".into(),
                entry_points: vec!["explicit-open".into()],
            },
            ContextRoute {
                id: "higher-priority".into(),
                pattern: "https://app.example.test:8443/docs/*".into(),
                context: "higher".into(),
                priority: 4,
                behavior: "suggest".into(),
                entry_points: vec!["typed-initial-url".into()],
            },
        ],
        ..ContextsConfig::default()
    };
    let matches = matching_context_routes(
        &config,
        "https://app.example.test:8443/docs/start?token=ignored",
        "explicit-open",
    );
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].id, "specific");
    assert!(
        matching_context_routes(
            &config,
            "https://badexample.test/docs/start",
            "explicit-open"
        )
        .is_empty()
    );
    assert!(
        matching_context_routes(
            &config,
            "https://app.example.test/docs/start",
            "explicit-open"
        )
        .iter()
        .any(|route| route.id == "broad")
    );
    assert!(
        matching_context_routes(
            &config,
            "https://app.example.test:8443/docs/start",
            "typed-initial-url"
        )
        .iter()
        .any(|route| route.id == "higher-priority")
    );
    assert!(matching_context_routes(&config, "file:///tmp/page.html", "explicit-open").is_empty());
}

#[test]
fn context_route_validation_rejects_ambiguous_patterns() {
    for pattern in [
        "ftp://example.test/*",
        "https://bad*example.test/*",
        "https://*.example.test/?token=secret",
        "https://example.test:0/*",
        "https://example.test/*#fragment",
    ] {
        let config = ContextsConfig {
            contexts: vec![ContextDefinition {
                name: "work".into(),
                label: "Work".into(),
                profile: "work".into(),
                sessions: Vec::new(),
                workspace: None,
                accent: None,
                default_target: "reuse-or-window".into(),
            }],
            routes: vec![ContextRoute {
                id: "route".into(),
                pattern: pattern.into(),
                context: "work".into(),
                priority: 0,
                behavior: "prompt".into(),
                entry_points: vec!["explicit-open".into()],
            }],
            ..ContextsConfig::default()
        };
        assert!(validate_contexts(&config).is_err(), "pattern {pattern}");
    }
}

#[test]
fn context_route_validation_rejects_unknown_entry_points_and_conflicts() {
    let context = |name: &str| ContextDefinition {
        name: name.into(),
        label: name.into(),
        profile: name.into(),
        sessions: Vec::new(),
        workspace: None,
        accent: None,
        default_target: "reuse-or-window".into(),
    };
    let route = |id: &str, target: &str| ContextRoute {
        id: id.into(),
        pattern: "https://example.test/*".into(),
        context: target.into(),
        priority: 2,
        behavior: "prompt".into(),
        entry_points: vec!["explicit-open".into()],
    };
    let mut invalid_entry_point = ContextsConfig {
        contexts: vec![context("work")],
        routes: vec![route("route", "work")],
        ..ContextsConfig::default()
    };
    invalid_entry_point.routes[0].entry_points = vec!["address-bar".into()];
    assert!(validate_contexts(&invalid_entry_point).is_err());

    let ambiguous = ContextsConfig {
        contexts: vec![context("work"), context("personal")],
        routes: vec![
            route("work-route", "work"),
            route("personal-route", "personal"),
        ],
        ..ContextsConfig::default()
    };
    assert!(validate_contexts(&ambiguous).is_err());
}
