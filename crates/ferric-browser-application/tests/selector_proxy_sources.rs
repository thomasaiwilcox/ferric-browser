use ferric_browser_application::{BrowserApplication, ConfigurationLayers};
use ferric_browser_config::{
    Config, EffectiveHintSelectorFamily, HintProxyTargets, HintSelectorFamily, HintSelectorMode,
    HintSelectorPolicy, SiteRule,
};
use std::collections::BTreeMap;

const URL: &str = "https://example.test/list";
const SELECTORS: &str = "hints.selectors.buttons";
const PROXY: &str = "hints.proxy_targets";

fn policy(selectors: &[&str], mode: HintSelectorMode) -> HintSelectorPolicy {
    HintSelectorPolicy {
        mode,
        selectors: selectors.iter().map(|value| (*value).into()).collect(),
    }
}

fn rule(id: &str, priority: i32, selectors: &[&str], proxy: &str) -> SiteRule {
    SiteRule {
        id: id.into(),
        pattern: "https://example.test/*".into(),
        priority,
        set: BTreeMap::from([
            (
                SELECTORS.into(),
                toml::Value::try_from(policy(selectors, HintSelectorMode::Replace)).unwrap(),
            ),
            (PROXY.into(), toml::Value::String(proxy.into())),
        ]),
        interaction: None,
    }
}

fn selectors(app: &BrowserApplication) -> EffectiveHintSelectorFamily {
    app.configuration_policy_sources()
        .compile_hint_selectors(URL)
        .unwrap()
        .remove(&HintSelectorFamily::Buttons)
        .unwrap()
}

fn proxy(app: &BrowserApplication) -> HintProxyTargets {
    app.configuration_policy_sources()
        .compile_hint_proxy_targets(URL)
        .unwrap()
}

fn assert_policy(app: &BrowserApplication, expected: &[&str], expected_proxy: &HintProxyTargets) {
    assert_eq!(
        selectors(app).policy,
        Some(policy(expected, HintSelectorMode::Replace))
    );
    assert_eq!(&proxy(app), expected_proxy);
}

#[test]
fn each_sources_explicit_globals_follow_its_sites_without_interaction_tables() {
    let mut base = Config::default();
    base.hints.selectors.insert(
        HintSelectorFamily::Buttons,
        policy(&[".base"], HintSelectorMode::Replace),
    );
    base.site_rules
        .push(rule("authored", i32::MAX, &[".authored"], "validated"));
    let mut layers = ConfigurationLayers::new(base);
    let mut app = BrowserApplication::new(Config::default());
    for (stage, expected, expected_proxy) in [
        (0, ".authored", HintProxyTargets::Validated),
        (1, ".authored", HintProxyTargets::Validated),
        (2, ".generated", HintProxyTargets::Off),
        (3, ".generated-global", HintProxyTargets::Validated),
        (4, ".cli", HintProxyTargets::Off),
        (5, ".temporary", HintProxyTargets::Validated),
        (6, ".temporary-global", HintProxyTargets::Off),
    ] {
        match stage {
            0 => {}
            1 => {
                layers
                    .profile
                    .set_literal(SELECTORS, "{mode='replace', selectors=['.profile']}")
                    .unwrap();
                layers.profile.set_literal(PROXY, "'off'").unwrap();
            }
            2 => {
                layers
                    .runtime
                    .site_rules
                    .push(rule("generated", i32::MIN, &[".generated"], "off"));
            }
            3 => {
                layers
                    .runtime
                    .set_literal(
                        SELECTORS,
                        "{mode='replace', selectors=['.generated-global']}",
                    )
                    .unwrap();
                layers.runtime.set_literal(PROXY, "'validated'").unwrap();
            }
            4 => {
                layers
                    .command_line
                    .set_literal(SELECTORS, "{mode='replace', selectors=['.cli']}")
                    .unwrap();
                layers.command_line.set_literal(PROXY, "'off'").unwrap();
            }
            5 => layers.temporary.site_rules.push(rule(
                "temporary",
                i32::MIN,
                &[".temporary"],
                "validated",
            )),
            6 => {
                layers
                    .temporary
                    .set_literal(
                        SELECTORS,
                        "{mode='replace', selectors=['.temporary-global']}",
                    )
                    .unwrap();
                layers.temporary.set_literal(PROXY, "'off'").unwrap();
            }
            _ => unreachable!(),
        }
        app.activate_configuration(layers.clone()).unwrap();
        assert_policy(&app, &[expected], &expected_proxy);
    }
}

#[test]
fn generated_global_extend_keeps_site_replace_mode_and_stable_unique_order() {
    let mut base = Config::default();
    base.site_rules
        .push(rule("authored", 0, &[".old", ".shared"], "validated"));
    let mut layers = ConfigurationLayers::new(base);
    layers
        .runtime
        .set_literal(SELECTORS, "{mode='extend',selectors=['.shared','.new']}")
        .unwrap();
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers).unwrap();
    assert_policy(
        &app,
        &[".old", ".shared", ".new"],
        &HintProxyTargets::Validated,
    );
}

#[test]
fn explicit_empty_global_replace_clears_a_matching_site_policy() {
    let mut base = Config::default();
    base.site_rules
        .push(rule("authored", 0, &[".old"], "validated"));
    let mut layers = ConfigurationLayers::new(base);
    layers
        .runtime
        .set_literal(SELECTORS, "{mode='replace',selectors=[]}")
        .unwrap();
    layers.runtime.set_literal(PROXY, "'off'").unwrap();
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers).unwrap();
    assert_policy(&app, &[], &HintProxyTargets::Off);
}

#[test]
fn pending_rules_and_selectors_stay_reload_scoped_while_proxy_globals_are_live() {
    let mut base = Config::default();
    base.site_rules
        .push(rule("shared-id", 0, &[".active"], "validated"));
    let mut layers = ConfigurationLayers::new(base);
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers.clone()).unwrap();
    let original = selectors(&app);
    let mut replacement = rule("shared-id", 0, &[".replacement"], "off");
    replacement.pattern = "https://different.test/*".into();
    layers.runtime.site_rules.push(replacement);
    layers
        .runtime
        .set_literal(SELECTORS, "{mode='replace',selectors=['.pending-global']}")
        .unwrap();
    layers.runtime.set_literal(PROXY, "'off'").unwrap();
    app.update_configuration(layers.clone()).unwrap();
    assert_eq!(selectors(&app), original);
    assert_eq!(proxy(&app), HintProxyTargets::Off);
    layers
        .temporary
        .set_literal("ui.font_size_pt", "18")
        .unwrap();
    app.update_configuration(layers.clone()).unwrap();
    assert_eq!(selectors(&app), original);
    assert_eq!(proxy(&app), HintProxyTargets::Off);
    let prepared = app.prepare_configuration_reload(layers).unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    assert_policy(&app, &[".pending-global"], &HintProxyTargets::Off);
}

#[test]
fn nonmatching_same_id_replacement_shadows_lower_rules_but_other_ids_still_apply() {
    let mut base = Config::default();
    base.site_rules
        .push(rule("lower", 0, &[".lower"], "validated"));
    base.site_rules.push(rule("other", 1, &[".other"], "off"));
    let mut layers = ConfigurationLayers::new(base);
    let mut replacement = rule("lower", i32::MIN, &[".foreign"], "validated");
    replacement.pattern = "https://different.test/*".into();
    layers.runtime.site_rules.push(replacement);
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers).unwrap();
    assert_policy(&app, &[".other"], &HintProxyTargets::Off);
}

#[test]
fn same_pattern_distinct_rules_contribute_in_authored_tie_order() {
    let mut base = Config::default();
    base.site_rules.push(rule("one", 0, &[".one"], "off"));
    let mut second = rule("two", 0, &[".two"], "validated");
    second.set.insert(
        SELECTORS.into(),
        toml::Value::try_from(policy(&[".two"], HintSelectorMode::Extend)).unwrap(),
    );
    base.site_rules.push(second);
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(ConfigurationLayers::new(base))
        .unwrap();
    assert_policy(&app, &[".one", ".two"], &HintProxyTargets::Validated);
}

#[test]
fn split_include_selector_leaves_merge_once_and_replaced_rule_arrays_stay_replaced() {
    let roots =
        ferric_browser_storage::StorageRoots::resolve(ferric_browser_storage::RootSpec::Temporary)
            .unwrap();
    roots.ensure().unwrap();
    let include = roots.config.join("included.toml");
    let root = roots.config.join("root.toml");
    std::fs::write(&include, "schema_version=3\n[hints.selectors.buttons]\nselectors=['.included']\n[[site_rules]]\nid='removed'\npattern='https://example.test/*'\n[site_rules.set]\n'hints.proxy_targets'='validated'\n").unwrap();
    std::fs::write(&root, "schema_version=3\ninclude=['included.toml']\nsite_rules=[]\n[hints.selectors.buttons]\nmode='replace'\n").unwrap();
    let loaded = ferric_browser_config::load(&root).unwrap();
    let mut layers = ConfigurationLayers::new(loaded.config);
    layers.base_authored = loaded.authored;
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers).unwrap();
    assert_policy(&app, &[".included"], &HintProxyTargets::Off);
}

#[test]
fn standalone_base_retains_resolved_32_selector_limit() {
    let mut base = Config::default();
    let selectors = (0..32).map(|i| format!(".selector{i}")).collect::<Vec<_>>();
    base.hints.selectors.insert(
        HintSelectorFamily::Buttons,
        HintSelectorPolicy {
            mode: HintSelectorMode::Replace,
            selectors: selectors.clone(),
        },
    );
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(ConfigurationLayers::new(base))
        .unwrap();
    assert_eq!(self::selectors(&app).policy.unwrap().selectors, selectors);
}

#[test]
fn live_proxy_global_changes_without_a_pending_site_replacement() {
    let mut base = Config::default();
    base.site_rules
        .push(rule("active", 0, &[".active"], "validated"));
    let mut layers = ConfigurationLayers::new(base);
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers.clone()).unwrap();
    let original = selectors(&app);
    layers.runtime.set_literal(PROXY, "'off'").unwrap();
    app.update_configuration(layers.clone()).unwrap();
    assert_eq!(proxy(&app), HintProxyTargets::Off);
    assert_eq!(selectors(&app), original);
    layers.runtime.unset_setting(PROXY).unwrap();
    app.update_configuration(layers).unwrap();
    assert_eq!(proxy(&app), HintProxyTargets::Validated);
    assert_eq!(selectors(&app), original);
}

#[test]
fn nonmatching_same_id_replacement_does_not_resurrect_the_only_lower_rule() {
    let mut base = Config::default();
    base.site_rules
        .push(rule("only", 0, &[".lower"], "validated"));
    let mut layers = ConfigurationLayers::new(base);
    let mut higher = rule("only", 0, &[".different"], "validated");
    higher.pattern = "https://different.test/*".into();
    layers.runtime.site_rules.push(higher);
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers).unwrap();
    assert_eq!(selectors(&app).policy, None);
    assert_eq!(proxy(&app), HintProxyTargets::Off);
}

#[test]
fn effective_selector_overflow_rejects_prepared_consumer_before_publication() {
    let mut app = BrowserApplication::new(Config::default());
    let before = app.configuration_snapshot();
    let sources = app.configuration_policy_sources();
    let mut base = Config::default();
    base.hints.selectors.insert(
        HintSelectorFamily::Buttons,
        HintSelectorPolicy {
            mode: HintSelectorMode::Replace,
            selectors: (0..32).map(|i| format!(".base{i}")).collect(),
        },
    );
    let mut extra = rule("extends", 0, &[".extra"], "validated");
    extra.set.insert(
        SELECTORS.into(),
        toml::Value::try_from(policy(&[".extra"], HintSelectorMode::Extend)).unwrap(),
    );
    base.site_rules.push(extra);
    let prepared = app
        .prepare_configuration_reload(ConfigurationLayers::new(base))
        .unwrap();
    let compiled =
        prepared.prepare_policy_consumer(|_, sources| sources.compile_hint_selectors(URL));
    assert!(compiled.is_err());
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    // The normal coordinator remains usable after rejected compilation.
    let prepared = app
        .prepare_configuration_update(app.configuration_layers().clone())
        .unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
}

#[test]
fn higher_global_replace_repairs_lower_site_effective_overflow() {
    let mut base = Config::default();
    base.hints.selectors.insert(
        HintSelectorFamily::Buttons,
        HintSelectorPolicy {
            mode: HintSelectorMode::Replace,
            selectors: (0..32).map(|i| format!(".base{i}")).collect(),
        },
    );
    let mut extends = rule("extends", 0, &[".extra"], "off");
    extends.set.insert(
        SELECTORS.into(),
        toml::Value::try_from(policy(&[".extra"], HintSelectorMode::Extend)).unwrap(),
    );
    base.site_rules.push(extends);
    let mut layers = ConfigurationLayers::new(base);
    layers
        .runtime
        .set_literal(SELECTORS, "{mode='replace',selectors=['.winner']}")
        .unwrap();
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers).unwrap();
    assert_policy(&app, &[".winner"], &HintProxyTargets::Off);
}

#[test]
fn later_replacement_cannot_hide_an_oversized_raw_selector_entry() {
    let mut base = Config::default();
    let mut invalid = rule("invalid", 0, &[], "off");
    invalid.set.insert(
        SELECTORS.into(),
        toml::Value::try_from(HintSelectorPolicy {
            mode: HintSelectorMode::Extend,
            selectors: (0..17).map(|i| format!(".raw{i}")).collect(),
        })
        .unwrap(),
    );
    base.site_rules.push(invalid);
    let mut layers = ConfigurationLayers::new(base);
    layers
        .runtime
        .set_literal(SELECTORS, "{mode='replace',selectors=['.winner']}")
        .unwrap();
    let app = BrowserApplication::new(Config::default());
    let before = app.configuration_snapshot();
    assert!(app.prepare_configuration_reload(layers).is_err());
    assert_eq!(app.configuration_snapshot(), before);
}
