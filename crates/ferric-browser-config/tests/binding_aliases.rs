use ferric_browser_config::{
    Config, RuntimeOverrides, apply_runtime_overrides, load, resolve_runtime_override_layers,
};
use std::{
    fmt::Write as _,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn layer(key: &str, command: &str) -> RuntimeOverrides {
    let mut layer = RuntimeOverrides::default();
    layer.set_binding("normal", key, command).unwrap();
    layer
}

fn assert_winner(config: &Config, key: &str, command: &str) {
    let bindings = &config.bindings["normal"];
    assert_eq!(bindings.len(), 1, "equivalent lower spelling survived");
    assert_eq!(bindings.get(key).map(String::as_str), Some(command));
}

struct Documents(PathBuf);
impl Documents {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ferric-binding-alias-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Documents {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn higher_runtime_binding_replaces_equivalent_lower_spelling() {
    for (lower, higher) in [("Plus", "+"), ("+", "Plus"), ("g,Plus", "g+")] {
        for command in ["zoom out", ""] {
            let mut base = Config::default();
            base.bindings
                .entry("normal".into())
                .or_default()
                .insert(lower.into(), "zoom in".into());
            let effective = apply_runtime_overrides(&base, &layer(higher, command)).unwrap();
            assert_winner(
                &effective,
                higher,
                if command.is_empty() {
                    "unbound"
                } else {
                    command
                },
            );
        }
    }
}

#[test]
fn all_runtime_layers_use_source_precedence_for_aliases() {
    for (lower, higher) in [("Plus", "+"), ("+", "Plus")] {
        let effective = resolve_runtime_override_layers(
            &Config::default(),
            &layer(lower, "zoom in"),
            &layer(higher, "zoom out"),
            &layer(lower, "zoom in"),
            &layer(higher, "zoom reset"),
        )
        .unwrap();
        assert_winner(&effective, higher, "zoom reset");
    }
}

#[test]
fn binding_setter_replaces_aliases_and_rejection_is_atomic() {
    for (old, incoming) in [("Plus", "+"), ("+", "Plus")] {
        let mut overrides = layer(old, "zoom in");
        overrides.set_binding("normal", incoming, "").unwrap();
        assert_eq!(overrides.bindings["normal"].len(), 1);
        assert_eq!(overrides.bindings["normal"][incoming], "");
        let before = overrides.clone();
        assert!(
            overrides
                .set_binding("normal", old, &"x".repeat(257))
                .is_err()
        );
        assert_eq!(overrides, before);
    }
}

#[test]
fn same_source_alias_duplicates_reject_before_layer_application() {
    for (left, right) in [("Plus", "+"), ("Return", "Enter"), ("g,Plus", "g+")] {
        let mut base = Config::default();
        base.bindings.insert(
            "normal".into(),
            [
                (left.into(), "zoom in".into()),
                (right.into(), "zoom out".into()),
            ]
            .into(),
        );
        assert!(apply_runtime_overrides(&base, &RuntimeOverrides::default()).is_err());
        let source = RuntimeOverrides {
            bindings: base.bindings.clone(),
            ..RuntimeOverrides::default()
        };
        assert!(apply_runtime_overrides(&Config::default(), &source).is_err());
    }
}

#[test]
fn include_binding_precedence_preserves_the_higher_authored_spelling() {
    for (lower, higher) in [("Plus", "+"), ("+", "Plus")] {
        for command in ["zoom out", "unbound"] {
            let docs = Documents::new();
            fs::write(
                docs.0.join("base.toml"),
                format!("schema_version=3\n[bindings.normal]\n'{lower}'='zoom in'\n"),
            )
            .unwrap();
            fs::write(docs.0.join("config.toml"), format!(
                "schema_version=3\ninclude=['base.toml']\n[bindings.normal]\n'{higher}'='{command}'\n")).unwrap();
            assert_winner(
                &load(docs.0.join("config.toml")).unwrap().config,
                higher,
                command,
            );
        }
    }
}

#[test]
fn an_include_cannot_hide_same_source_alias_duplicates() {
    let docs = Documents::new();
    fs::write(
        docs.0.join("base.toml"),
        "schema_version=3\n[bindings.normal]\nPlus='zoom in'\n'+'='zoom out'\n",
    )
    .unwrap();
    fs::write(
        docs.0.join("config.toml"),
        "schema_version=3\ninclude=['base.toml']\n[bindings.normal]\nPlus='zoom reset'\n",
    )
    .unwrap();
    assert!(load(docs.0.join("config.toml")).is_err());
}

#[test]
fn a_post_mutation_binding_limit_error_preserves_the_previous_layer() {
    let mut full = RuntimeOverrides::default();
    for index in 0..256 {
        full.set_binding("normal", &format!("g{index:03}"), "zoom in")
            .unwrap();
    }
    apply_runtime_overrides(&Config::default(), &full).unwrap();
    let before = full.clone();
    assert!(full.set_binding("normal", "z", "zoom out").is_err());
    assert_eq!(full, before);
}

#[test]
fn generic_binding_table_settings_keep_replacement_semantics() {
    let mut base = Config::default();
    base.bindings.insert(
        "normal".into(),
        [
            ("Plus".into(), "zoom in".into()),
            ("j".into(), "scroll down".into()),
        ]
        .into(),
    );
    let mut overrides = RuntimeOverrides::default();
    assert!(
        overrides
            .set_literal("bindings.normal", "{ '+' = 'zoom out' }")
            .is_err()
    );
    overrides.settings.insert(
        "bindings.normal".into(),
        toml::Value::Table(
            [("+".into(), toml::Value::String("zoom out".into()))]
                .into_iter()
                .collect(),
        ),
    );
    assert_winner(
        &apply_runtime_overrides(&base, &overrides).unwrap(),
        "+",
        "zoom out",
    );
    // A later explicit assignment must not hide an invalid raw table.
    overrides.settings.insert(
        "bindings.normal".into(),
        toml::Value::Table(
            [
                ("+".into(), toml::Value::String("zoom in".into())),
                ("Plus".into(), toml::Value::String("zoom out".into())),
            ]
            .into_iter()
            .collect(),
        ),
    );
    overrides.bindings = layer("+", "zoom reset").bindings;
    assert!(
        apply_runtime_overrides(&base, &overrides).is_err(),
        "same-source aliases in a raw setting must reject"
    );
}

#[test]
fn equivalent_bindings_in_different_modes_remain_independent() {
    let mut overrides = layer("Plus", "zoom in");
    overrides
        .set_binding("insert", "+", "mode-enter normal")
        .unwrap();
    let effective = apply_runtime_overrides(&Config::default(), &overrides).unwrap();
    assert_eq!(effective.bindings["normal"]["Plus"], "zoom in");
    assert_eq!(effective.bindings["insert"]["+"], "mode-enter normal");
}

#[test]
fn large_authored_binding_tables_keep_their_existing_capacity() {
    let docs = Documents::new();
    let mut lower = String::from("schema_version=3\n[bindings.normal]\n");
    let mut higher = String::from("schema_version=3\ninclude=['base.toml']\n[bindings.normal]\n");
    for index in 0..4_000 {
        writeln!(lower, "'g{index:04}+'='zoom in'").unwrap();
        let chain = format!("g{index:04}")
            .chars()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(",");
        writeln!(higher, "'{chain},Plus'='zoom out'").unwrap();
    }
    fs::write(docs.0.join("base.toml"), lower).unwrap();
    fs::write(docs.0.join("config.toml"), higher).unwrap();
    let loaded = load(docs.0.join("config.toml")).unwrap();
    let bindings = &loaded.config.bindings["normal"];
    assert_eq!(bindings.len(), 4_000);
    for index in [0, 1_999, 3_999] {
        let chain = format!("g{index:04}")
            .chars()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(bindings[&format!("{chain},Plus")], "zoom out");
        assert!(!bindings.contains_key(&format!("g{index:04}+")));
    }
}
