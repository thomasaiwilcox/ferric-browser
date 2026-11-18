use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use ferric_browser_config::{ConfigError, load, load_runtime_overrides};

const LIMIT: usize = 1024;
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "ferric-policy-limits-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }
    fn write(&self, file: &str, source: &str) -> PathBuf {
        let path = self.0.join(file);
        fs::write(&path, source).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn bounded(error: &ConfigError) {
    assert!(
        error.to_string().len() <= LIMIT,
        "display diagnostic has {} bytes",
        error.to_string().len()
    );
    match error {
        ConfigError::Parse { message, .. } | ConfigError::Validation(message) => {
            assert!(
                message.len() <= LIMIT,
                "stored diagnostic has {} bytes",
                message.len()
            );
        }
        _ => panic!("expected a precise parse/validation error"),
    }
}

#[test]
fn syntax_errors_do_not_copy_long_source_lines() {
    let fixture = Fixture::new();
    let source = format!("schema_version=3\nvalue='{}'?", "é".repeat(50_000));
    let path = fixture.write("config.toml", &source);
    let error = load(&path).unwrap_err();
    bounded(&error);
    assert!(error.to_string().contains("config.toml"));
    let ConfigError::Parse { message, .. } = error else {
        panic!("syntax error must be precise")
    };
    assert!(
        message.contains("byte"),
        "retain the original source position"
    );
}

#[test]
fn huge_unknown_fields_reject_with_bounded_stored_errors_in_each_input() {
    let fixture = Fixture::new();
    let unknown = "é".repeat(50_000);
    let rule = format!(
        "[[site_rules]]\nid='valid'\npattern='https://example.test/*'\n[site_rules.interaction]\nenabled=false\n'{unknown}'=true\n"
    );
    let root = fixture.write("config.toml", &format!("schema_version=3\n{rule}"));
    bounded(&load(&root).unwrap_err());
    let runtime = fixture.write("runtime.toml", &format!("schema_version=3\n{rule}"));
    bounded(&load_runtime_overrides(runtime).unwrap_err());
    fixture.write("included.toml", &rule);
    fixture.write(
        "config.toml",
        "schema_version=3\ninclude=['included.toml']\nsite_rules=[]\n",
    );
    let error = load(root).unwrap_err();
    bounded(&error);
    assert!(error.to_string().contains("included.toml"));
}

#[test]
fn huge_unsupported_legacy_setting_does_not_amplify_diagnostics() {
    let fixture = Fixture::new();
    let path = fixture.write("config.toml", &format!("schema_version=3\n[[site_rules]]\nid='valid'\npattern='https://example.test/*'\n[site_rules.set]\n'{}'=true\n", "é".repeat(50_000)));
    let error = load(path).unwrap_err();
    bounded(&error);
    assert!(error.to_string().contains("unsupported setting"));
}

#[test]
fn public_error_display_is_utf8_bounded_even_for_caller_constructed_errors() {
    for message in ["é".repeat(10_000), "\u{1b}[31m".repeat(10_000)] {
        for error in [
            ConfigError::Validation(message.clone()),
            ConfigError::Parse {
                path: PathBuf::from("config.toml"),
                message: message.clone(),
            },
            ConfigError::Io {
                path: PathBuf::from("é".repeat(10_000)),
                message,
            },
        ] {
            let display = error.to_string();
            assert!(display.len() <= LIMIT);
            assert!(!display.chars().any(char::is_control));
        }
    }
}

fn padded_source(prefix: &str, length: usize) -> String {
    let mut text = format!("{prefix}\n#");
    assert!(text.len() <= length);
    let remaining = length - text.len();
    text.push_str(&"é".repeat(remaining / 2));
    if !remaining.is_multiple_of(2) {
        text.push(' ');
    }
    assert_eq!(text.len(), length);
    text
}

#[test]
fn outer_file_and_aggregate_input_caps_preserve_exact_utf8_byte_boundaries() {
    use ferric_browser_config::MAX_CONFIGURATION_BYTES;
    assert_eq!(MAX_CONFIGURATION_BYTES, 2 * 1024 * 1024);
    let cap = usize::try_from(MAX_CONFIGURATION_BYTES).unwrap();
    let fixture = Fixture::new();
    for length in [cap - 1, cap, cap + 1] {
        let path = fixture.write("config.toml", &padded_source("schema_version=3", length));
        assert_eq!(load(&path).is_ok(), length <= cap);
        assert_eq!(load_runtime_overrides(&path).is_ok(), length <= cap);
        if length > cap {
            assert_eq!(load(&path).unwrap_err(), ConfigError::SizeLimit);
        }
    }
    let root = "schema_version=3\ninclude=['included.toml']\n";
    let root_path = fixture.write("config.toml", root);
    for delta in [-1_isize, 0, 1] {
        let child_size = cap.checked_add_signed(delta).unwrap() - root.len();
        fixture.write("included.toml", &padded_source("", child_size));
        assert_eq!(load(&root_path).is_ok(), delta <= 0);
        if delta > 0 {
            assert_eq!(load(&root_path).unwrap_err(), ConfigError::SizeLimit);
        }
    }
    // A zero-byte child still fits when the root consumed the entire budget.
    fixture.write("config.toml", &padded_source(root, cap));
    fixture.write("included.toml", "");
    assert!(load(&root_path).is_ok());
    fixture.write("included.toml", " ");
    assert_eq!(load(root_path).unwrap_err(), ConfigError::SizeLimit);
}

#[test]
fn include_depth_and_adversarial_nesting_rejections_preserve_the_last_publication() {
    use ferric_browser_config::{ConfigStore, MAX_CONFIGURATION_INCLUDE_DEPTH};
    assert_eq!(MAX_CONFIGURATION_INCLUDE_DEPTH, 8);
    let fixture = Fixture::new();
    for index in 0..8 {
        let text = if index == 7 {
            "schema_version=3\n".into()
        } else {
            format!("schema_version=3\ninclude=['level{}.toml']\n", index + 1)
        };
        fixture.write(&format!("level{index}.toml"), &text);
    }
    let root = fixture.0.join("level0.toml");
    let mut store = ConfigStore::new();
    let previous = store.reload(&root).unwrap().clone();
    let revision = store.revision();
    fixture.write("level7.toml", "schema_version=3\ninclude=['level8.toml']\n");
    fixture.write("level8.toml", "");
    assert!(matches!(
        store.reload(&root),
        Err(ConfigError::IncludeDepth { .. })
    ));
    assert_eq!(store.revision(), revision);
    assert_eq!(store.current(), Some(&previous));
    for source in [
        format!(
            "schema_version=3\nvalue={}0{}",
            "[".repeat(512),
            "]".repeat(512)
        ),
        format!(
            "schema_version=3\n[{}]\nx=1\n",
            vec!["nested"; 512].join(".")
        ),
    ] {
        fixture.write("level0.toml", &source);
        let error = store.reload(&root).unwrap_err();
        bounded(&error);
        assert_eq!(store.revision(), revision);
        assert_eq!(store.current(), Some(&previous));
    }
}

#[test]
fn legacy_validation_messages_bound_stored_context_without_losing_the_reason() {
    use ferric_browser_config::{Config, validate};
    use std::collections::BTreeMap;
    let huge = "é".repeat(50_000);
    let search = Config {
        search_engines: BTreeMap::from([(huge.clone(), "http://invalid.test".into())]),
        ..Default::default()
    };
    let bindings = Config {
        bindings: BTreeMap::from([(huge.clone(), BTreeMap::new())]),
        ..Default::default()
    };
    let mut spellcheck = Config::default();
    spellcheck.spellcheck.languages = vec![huge.clone()];
    for (config, reason) in [
        (search, "HTTPS template"),
        (bindings, "not supported"),
        (spellcheck, "valid BCP 47"),
    ] {
        let error = validate(&config).unwrap_err();
        bounded(&error);
        assert!(
            error.to_string().contains(reason),
            "reason was hidden by context"
        );
    }
    let mut mixed = Config::default();
    mixed.input.page_keys = vec![huge];
    let error = validate(&mixed).unwrap_err();
    bounded(&error);
    assert!(error.to_string().contains("64 UTF-8 bytes"));
}

#[test]
fn huge_site_keys_and_runtime_binding_modes_reject_without_mutating_overrides() {
    use ferric_browser_config::{Config, RuntimeOverrides, apply_runtime_overrides};
    use std::collections::BTreeMap;
    let huge = "é".repeat(50_000);
    let mut overrides = RuntimeOverrides::default();
    let before = overrides.clone();
    let error = overrides
        .set_site_literal("https://example.test/*", &huge, "true")
        .unwrap_err();
    bounded(&error);
    assert!(error.to_string().contains("does not support site scope"));
    assert_eq!(overrides, before);
    let error = overrides
        .unset_site_setting("https://example.test/*", &huge)
        .unwrap_err();
    bounded(&error);
    assert!(error.to_string().contains("does not support site scope"));
    assert_eq!(overrides, before);
    let overrides = RuntimeOverrides {
        bindings: BTreeMap::from([(huge, BTreeMap::from([("j".into(), "scroll-down".into())]))]),
        ..Default::default()
    };
    let error = apply_runtime_overrides(&Config::default(), &overrides).unwrap_err();
    bounded(&error);
    assert!(error.to_string().contains("not supported"));
}

#[test]
fn context_route_errors_have_bounded_stored_messages() {
    use ferric_browser_config::load_contexts;
    let fixture = Fixture::new();
    let huge = "é".repeat(50_000);
    let path = fixture.write("contexts.toml", &format!("schema_version=3\n[[routes]]\nid='route'\npattern='https://example.test/*'\ncontext='{huge}'\n"));
    let error = load_contexts(path).unwrap_err();
    bounded(&error);
    assert!(error.to_string().contains("unknown context"));
}

#[test]
fn selector_deserialization_errors_bound_allowed_size_but_malformed_context() {
    use ferric_browser_config::{Config, RuntimeOverrides, apply_runtime_overrides};
    let mut overrides = RuntimeOverrides::default();
    overrides
        .set_literal("hints.selectors.links", "{mode='replace', selectors=[]}")
        .unwrap();
    assert!(apply_runtime_overrides(&Config::default(), &overrides).is_ok());
    overrides.settings.insert(
        "hints.selectors.links".into(),
        toml::Value::Table(toml::map::Map::from_iter([(
            "é".repeat(2000),
            toml::Value::Boolean(true),
        )])),
    );
    let error = apply_runtime_overrides(&Config::default(), &overrides).unwrap_err();
    bounded(&error);
    assert!(error.to_string().contains("runtime selector policy"));
}

#[test]
fn debug_rendering_preserves_error_kind_without_echoing_unbounded_owned_fields() {
    let path = PathBuf::from("é".repeat(50_000));
    let message = format!("reason{}\u{1b}\u{2028}", "é".repeat(50_000));
    for (error, kind) in [
        (ConfigError::Validation(message.clone()), "Validation"),
        (
            ConfigError::Parse {
                path: path.clone(),
                message: message.clone(),
            },
            "Parse",
        ),
        (
            ConfigError::Io {
                path: path.clone(),
                message,
            },
            "Io",
        ),
        (
            ConfigError::IncludeDepth { path: path.clone() },
            "IncludeDepth",
        ),
        (
            ConfigError::IncludeCycle { path: path.clone() },
            "IncludeCycle",
        ),
        (
            ConfigError::IncludeRepeated { path: path.clone() },
            "IncludeRepeated",
        ),
        (ConfigError::SizeLimit, "SizeLimit"),
    ] {
        let debug = format!("{error:?}");
        assert!(debug.len() <= LIMIT, "unbounded {kind} Debug");
        assert!(debug.contains(kind));
        assert!(!debug.chars().any(char::is_control));
        assert!(!debug.contains(['\u{2028}', '\u{2029}']));
        if let ConfigError::Io { path: stored, .. }
        | ConfigError::Parse { path: stored, .. }
        | ConfigError::IncludeDepth { path: stored }
        | ConfigError::IncludeCycle { path: stored }
        | ConfigError::IncludeRepeated { path: stored } = error
        {
            assert_eq!(stored, path);
        }
    }
}

#[test]
fn long_real_source_paths_do_not_hide_include_validation_reasons() {
    let fixture = Fixture::new();
    let mut directory = fixture.0.clone();
    for _ in 0..9 {
        directory.push("nested".repeat(24));
    }
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("config.toml");
    assert!(path.to_string_lossy().len() > 1024);
    for (include, reason) in [
        ("[false]", "include entries must be strings"),
        (
            "['$HOME/config.toml']",
            "include must be a trusted literal local path",
        ),
    ] {
        fs::write(&path, format!("schema_version=3\ninclude={include}\n")).unwrap();
        let error = load(&path).unwrap_err();
        bounded(&error);
        assert!(
            error.to_string().contains(reason),
            "include reason was hidden by source path"
        );
    }
}

#[test]
fn non_array_include_values_cannot_disappear_before_typed_deserialization() {
    use ferric_browser_config::{Config, ConfigStore};
    let fixture = Fixture::new();
    let path = fixture.write("config.toml", "schema_version=3\n");
    let mut store = ConfigStore::new();
    let previous = store.reload(&path).unwrap().clone();
    let revision = store.revision();
    for include in ["false", "42", "'included.toml'", "{}"] {
        let source = format!("schema_version=3\ninclude={include}\n");
        assert!(toml::from_str::<Config>(&source).is_err());
        fs::write(&path, source).unwrap();
        assert!(
            store.reload(&path).is_err(),
            "include={include} was silently dropped"
        );
        assert_eq!(store.current(), Some(&previous));
        assert_eq!(store.revision(), revision);
    }
}
