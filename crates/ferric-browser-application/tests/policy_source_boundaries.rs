use ferric_browser_application::{BrowserApplication, ConfigurationLayers};
use ferric_browser_config::{
    Config, ConfigError, HintProxyTargets, HintSelectorFamily, HintSelectorMode,
    HintSelectorPolicy, InteractionLabelsPatch, InteractionPolicyPatch, RuntimeOverrides,
    TabsPresentation,
};
use std::{cell::Cell, fs, path::PathBuf, sync::Arc};

const PATTERN: &str = "https://example.test/*";

fn inputs(id: &str, chars: &str) -> ConfigurationLayers {
    let mut rules = RuntimeOverrides::default();
    rules
        .create_site_interaction(
            id,
            PATTERN,
            0,
            InteractionPolicyPatch {
                labels: Some(InteractionLabelsPatch {
                    alphabet: Some(if id == "old" { "jk" } else { "mn" }.into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .unwrap();
    rules.site_rules[0].set.insert(
        "hints.proxy_targets".into(),
        toml::Value::String("off".into()),
    );
    let mut config = Config {
        site_rules: rules.site_rules,
        ..Config::default()
    };
    config.hints.chars = chars.into();
    config.hints.selectors.insert(
        HintSelectorFamily::Links,
        HintSelectorPolicy {
            mode: HintSelectorMode::Replace,
            selectors: vec![format!("#{id}")],
        },
    );
    ConfigurationLayers::new(config)
}

#[test]
fn runtime_updates_retain_reload_rules_and_selectors_while_live_inputs_advance() {
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(inputs("old", "ab")).unwrap();
    let original = app.configuration_policy_sources();
    let mut desired = inputs("new", "cd");
    desired.base.hints.proxy_targets = HintProxyTargets::Validated;
    app.update_configuration(desired.clone()).unwrap();
    let sources = app.configuration_policy_sources();
    assert_eq!(sources.desired_layers(), &desired);
    assert_eq!(sources.reload_layers(), original.reload_layers());
    assert_eq!(sources.reload_revision(), original.reload_revision());
    assert_eq!(
        sources.reload_layers().base.site_rules[0]
            .interaction
            .as_ref()
            .unwrap()
            .labels
            .as_ref()
            .unwrap()
            .alphabet
            .as_deref(),
        Some("jk")
    );
    assert_eq!(
        sources.reload_layers().base.site_rules[0].set["hints.proxy_targets"].as_str(),
        Some("off")
    );
    assert_eq!(sources.revision(), app.configuration_revision());
    assert!(sources.revision() > sources.reload_revision());
    assert_eq!(app.configuration().site_rules[0].id, "old");
    assert_eq!(app.configuration().hints.chars, "cd");
    assert_eq!(
        app.configuration().hints.proxy_targets,
        HintProxyTargets::Validated
    );
    assert_eq!(
        app.configuration().hints.selectors[&HintSelectorFamily::Links].selectors,
        ["#old"]
    );
    assert_eq!(original.desired_layers().base.site_rules[0].id, "old");
    assert_eq!(original.desired_layers().base.hints.chars, "ab");
}

#[test]
fn reload_advances_source_identity_even_when_effective_values_and_pending_changes_are_equal() {
    let base = inputs("same", "ab");
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(base.clone()).unwrap();
    let old = app.configuration_policy_sources();
    let effective = app.configuration().clone();
    let mut desired = base;
    desired.runtime.site_rules = desired.base.site_rules.clone();
    desired.runtime.set_literal("hints.chars", "'ab'").unwrap();
    app.update_configuration(desired.clone()).unwrap();
    assert_eq!(app.configuration(), &effective);
    assert!(app.configuration_snapshot().pending_changes.is_empty());
    let pending = app.configuration_policy_sources();
    assert!(pending.reload_layers().runtime.site_rules.is_empty());
    assert_eq!(pending.reload_revision(), old.reload_revision());
    assert_eq!(
        pending.desired_layers().runtime.settings["hints.chars"].as_str(),
        Some("ab")
    );
    let prepared = app.prepare_configuration_reload(desired.clone()).unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    let reloaded = app.configuration_policy_sources();
    assert_eq!(reloaded.reload_layers(), &desired);
    assert_eq!(reloaded.reload_revision(), reloaded.revision());
    assert!(reloaded.reload_revision() > pending.reload_revision());
    assert_eq!(pending.reload_layers(), old.reload_layers());
}

#[test]
fn rejected_preparation_persistence_and_stale_publication_preserve_both_captures() {
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(inputs("old", "ab")).unwrap();
    let original = app.configuration_policy_sources();
    let mut invalid = inputs("new", "cd");
    invalid.base.ui.font_size_pt = 5.0;
    assert!(app.prepare_configuration_reload(invalid).is_err());
    assert_eq!(app.configuration_policy_sources(), original);

    let prepared = app
        .prepare_configuration_reload(inputs("new", "cd"))
        .unwrap()
        .prepare_policy_consumer(|_, sources| Ok(sources.clone()))
        .unwrap();
    assert!(
        app.publish_configuration_consumer(prepared, || Err(ConfigError::Validation(
            "write failed".into()
        )))
        .is_err()
    );
    assert_eq!(app.configuration_policy_sources(), original);

    let stale = app
        .prepare_configuration_reload(inputs("stale", "ef"))
        .unwrap()
        .prepare_policy_consumer(|_, sources| Ok(sources.clone()))
        .unwrap();
    app.update_configuration(inputs("current", "gh")).unwrap();
    let current = app.configuration_policy_sources();
    let wrote = Cell::new(false);
    assert!(
        app.publish_configuration_consumer(stale, || {
            wrote.set(true);
            Ok(())
        })
        .is_err()
    );
    assert!(!wrote.get());
    assert_eq!(app.configuration_policy_sources(), current);
}

#[test]
fn prepared_consumer_uses_the_candidate_captures_and_existing_publisher() {
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(inputs("old", "ab")).unwrap();
    let original = app.configuration_policy_sources();
    let prepared = app
        .prepare_configuration_update(inputs("new", "cd"))
        .unwrap()
        .prepare_policy_consumer(|effective, sources| {
            assert_eq!(effective.site_rules[0].id, "old");
            assert_eq!(sources.reload_layers().base.site_rules[0].id, "old");
            assert_eq!(sources.desired_layers().base.site_rules[0].id, "new");
            assert_eq!(sources.desired_layers().base.hints.chars, "cd");
            Ok(sources.clone())
        })
        .unwrap();
    assert_eq!(app.configuration_policy_sources(), original);
    let (snapshot, sources) = app
        .publish_configuration_consumer(prepared, || Ok(()))
        .unwrap();
    assert_eq!(snapshot.revision, sources.revision());
    assert_eq!(sources, app.configuration_policy_sources());
    app.update_configuration(inputs("later", "ef")).unwrap();
    assert_eq!(sources.desired_layers().base.site_rules[0].id, "new");
    assert_eq!(sources.reload_layers().base.site_rules[0].id, "old");
}

#[test]
fn activation_and_standalone_replacement_reset_both_source_captures() {
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(inputs("old", "ab")).unwrap();
    app.update_configuration(inputs("pending", "cd")).unwrap();
    let pending = app.configuration_policy_sources();
    app.activate_configuration(inputs("other", "ef")).unwrap();
    let activated = app.configuration_policy_sources();
    assert_eq!(activated.desired_layers(), activated.reload_layers());
    assert_eq!(activated.revision(), activated.reload_revision());
    assert_eq!(activated.reload_layers().base.site_rules[0].id, "other");
    app.replace_configuration(Config::default()).unwrap();
    let replaced = app.configuration_policy_sources();
    assert_eq!(replaced.desired_layers(), replaced.reload_layers());
    assert_eq!(replaced.revision(), replaced.reload_revision());
    assert!(replaced.reload_layers().base.site_rules.is_empty());
    assert_eq!(pending.reload_layers().base.site_rules[0].id, "old");
}

#[test]
fn cancelling_pending_rules_does_not_promote_the_pending_source_epoch() {
    let original = inputs("old", "ab");
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(original.clone()).unwrap();
    let active = app.configuration_policy_sources();
    app.update_configuration(inputs("pending", "cd")).unwrap();
    let pending = app.configuration_policy_sources();
    app.update_configuration(original.clone()).unwrap();
    let cancelled = app.configuration_policy_sources();
    assert_eq!(cancelled.desired_layers(), &original);
    assert_eq!(cancelled.reload_layers(), active.reload_layers());
    assert_eq!(cancelled.reload_revision(), active.reload_revision());
    assert!(app.configuration_snapshot().pending_changes.is_empty());
    assert_eq!(pending.desired_layers().base.site_rules[0].id, "pending");
}

#[test]
fn neither_source_capture_overrides_the_effective_startup_restart_projection() {
    let mut app = BrowserApplication::new(Config::default());
    let mut desired = inputs("new", "ab");
    desired.base.tabs.presentation = TabsPresentation::Windows;
    let prepared = app
        .prepare_configuration_reload(desired)
        .unwrap()
        .prepare_policy_consumer(|effective, sources| {
            assert_eq!(effective.tabs.presentation, TabsPresentation::Internal);
            assert_eq!(
                sources.desired_layers().base.tabs.presentation,
                TabsPresentation::Windows
            );
            assert_eq!(
                sources.reload_layers().base.tabs.presentation,
                TabsPresentation::Windows
            );
            Ok(sources.clone())
        })
        .unwrap();
    app.publish_configuration_consumer(prepared, || Ok(()))
        .unwrap();
    assert_eq!(
        app.configuration().tabs.presentation,
        TabsPresentation::Internal
    );
    assert!(
        app.configuration_snapshot()
            .pending_changes
            .iter()
            .any(|change| change.key == "tabs.presentation")
    );
}

struct Files(PathBuf);
impl Drop for Files {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn pending_replaced_includes_keep_each_catalogue_paired_with_its_own_rules() {
    let files =
        Files(std::env::temp_dir().join(format!("ferric-policy-capture-{}", uuid::Uuid::new_v4())));
    fs::create_dir_all(&files.0).unwrap();
    for (id, chars) in [("old", "ab"), ("new", "cd")] {
        fs::write(files.0.join(format!("{id}.toml")), format!(
            "[hints]\nchars='{chars}'\n[[site_rules]]\nid='{id}'\npattern='{PATTERN}'\n[site_rules.interaction]\nenabled=false\n"
        )).unwrap();
    }
    let root = files.0.join("config.toml");
    let load = |id| {
        fs::write(&root, format!("schema_version=3\ninclude=['{id}.toml']\n")).unwrap();
        let loaded = ferric_browser_config::load(&root).unwrap();
        let mut layers = ConfigurationLayers::new(loaded.config);
        layers.base_authored = Arc::clone(&loaded.authored);
        layers
    };
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(load("old")).unwrap();
    app.update_configuration(load("new")).unwrap();
    let sources = app.configuration_policy_sources();
    assert_eq!(sources.reload_layers().base.site_rules[0].id, "old");
    assert_eq!(sources.desired_layers().base.site_rules[0].id, "new");
    assert!(
        sources.reload_layers().base_authored.documents()[0]
            .path()
            .ends_with("old.toml")
    );
    assert!(
        sources.desired_layers().base_authored.documents()[0]
            .path()
            .ends_with("new.toml")
    );
    assert_eq!(app.configuration().hints.chars, "cd");
    assert_eq!(app.configuration().site_rules[0].id, "old");
}
