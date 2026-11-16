use ferric_browser_application::{BrowserApplication, ConfigurationLayers};
use ferric_browser_config::{Config, HintProxyTargets, PolicySourceIdentity, SiteRule, load};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const URL: &str = "https://example.test/page";
const KEY: &str = "hints.proxy_targets";
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new(child: &str, root: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "ferric-proxy-inspection-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("child.toml"), child).unwrap();
        fs::write(
            path.join("root.toml"),
            format!("schema_version=3\ninclude=['child.toml']\n{root}"),
        )
        .unwrap();
        Self(path)
    }

    fn layers(&self) -> ConfigurationLayers {
        let loaded = load(self.0.join("root.toml")).unwrap();
        let mut layers = ConfigurationLayers::new(loaded.config);
        layers.base_authored = loaded.authored;
        layers
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn rule(id: &str, pattern: &str, proxy: &str) -> SiteRule {
    SiteRule {
        id: id.into(),
        pattern: pattern.into(),
        priority: -1000,
        interaction: None,
        set: BTreeMap::from([(KEY.into(), toml::Value::String(proxy.into()))]),
    }
}

#[test]
fn authored_absence_and_explicit_default_have_distinct_seed_origins() {
    for (child, expected_source) in [
        ("[hints]\nchars='ab'\n", None),
        (
            "[hints]\nproxy_targets='off'\n",
            Some(PolicySourceIdentity::Authored { document_id: 0 }),
        ),
    ] {
        let fixture = Fixture::new(child, "[hints]\nmin_chars=2\n");
        let mut app = BrowserApplication::new(Config::default());
        app.activate_configuration(fixture.layers()).unwrap();
        let sources = app.configuration_policy_sources();
        let inspected = sources.inspect_hint_proxy_targets(URL, "default").unwrap();
        assert_eq!(inspected.effective.policy, HintProxyTargets::Off);
        assert_eq!(
            inspected.effective,
            sources.compile_hint_proxy_policy(URL).unwrap()
        );
        assert_eq!(
            inspected
                .trace
                .winning_origin()
                .map(ferric_browser_config::PolicyFieldOrigin::source),
            expected_source.as_ref()
        );
        assert_eq!(
            inspected.trace.seed.len(),
            usize::from(expected_source.is_some())
        );
        assert!(inspected.trace.contributions.is_empty());
    }
    let app = BrowserApplication::new(Config::default());
    let inspected = app
        .configuration_policy_sources()
        .inspect_hint_proxy_targets(URL, "default")
        .unwrap();
    assert_eq!(
        inspected.trace.winning_origin().unwrap().source(),
        &PolicySourceIdentity::InMemory
    );
}

#[test]
fn scalar_trace_preserves_include_replacement_explicit_off_and_shadowed_rules() {
    let fixture = Fixture::new(
        "[hints]\nproxy_targets='validated'\n",
        "[hints]\nproxy_targets='off'\n",
    );
    fs::write(fixture.0.join("root.toml"), "schema_version=3\ninclude=['child.toml']\n[hints]\nproxy_targets='off'\n[[site_rules]]\nid='same'\npattern='https://example.test/*'\n[site_rules.set]\n'hints.proxy_targets'='validated'\n").unwrap();
    let mut layers = fixture.layers();
    layers.profile.set_literal(KEY, "'validated'").unwrap();
    layers
        .runtime
        .site_rules
        .push(rule("same", "https://other.test/*", "validated"));
    layers.runtime.set_literal(KEY, "'off'").unwrap();
    layers.command_line.set_literal(KEY, "'off'").unwrap();
    layers
        .temporary
        .site_rules
        .push(rule("temp", "https://example.test/*", "validated"));
    layers.temporary.set_literal(KEY, "'off'").unwrap();
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers).unwrap();
    let before = app.configuration_snapshot();
    let sources = app.configuration_policy_sources();
    let count = app.interaction_compilation_count();
    let inspected = sources.inspect_hint_proxy_targets(URL, "work").unwrap();
    assert_eq!(
        inspected.effective,
        sources.compile_hint_proxy_policy(URL).unwrap()
    );
    assert_eq!(inspected.trace.seed.len(), 2);
    assert_eq!(inspected.trace.seed[0].value, HintProxyTargets::Validated);
    assert_eq!(inspected.trace.seed[1].value, HintProxyTargets::Off);
    assert_eq!(
        inspected.trace.seed[1].origin.source(),
        &PolicySourceIdentity::Authored { document_id: 1 }
    );
    let contributions = &inspected.trace.contributions;
    assert_eq!(contributions.len(), 5);
    assert_eq!(
        contributions[0].origin.source(),
        &PolicySourceIdentity::Profile {
            name: "work".into()
        }
    );
    assert_eq!(
        contributions[1].origin.source(),
        &PolicySourceIdentity::Generated
    );
    assert_eq!(
        contributions[2].origin.source(),
        &PolicySourceIdentity::CommandLine
    );
    assert_eq!(contributions[3].origin.rule().unwrap().id(), "temp");
    assert_eq!(contributions[3].origin.rule().unwrap().priority(), -1000);
    assert!(contributions[4].origin.rule().is_none());
    assert_eq!(
        inspected.trace.winning_origin().unwrap().source(),
        &PolicySourceIdentity::Temporary
    );
    assert_eq!(inspected.effective.policy, HintProxyTargets::Off);
    assert_eq!(inspected.selection.shadowed.len(), 1);
    assert!(
        inspected
            .selection
            .survivors
            .iter()
            .any(|entry| entry.origin.rule().unwrap().id() == "same" && !entry.matched)
    );
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    assert_eq!(app.interaction_compilation_count(), count);
}

#[test]
fn proxy_trace_keeps_live_globals_and_reload_rules_at_their_captured_revisions() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers
        .base
        .site_rules
        .push(rule("base", "https://example.test/*", "validated"));
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers.clone()).unwrap();
    let original_revision = app.configuration_revision();
    layers.base.site_rules.clear();
    layers.runtime.set_literal(KEY, "'off'").unwrap();
    app.update_configuration(layers.clone()).unwrap();
    let captured = app.configuration_policy_sources();
    let inspected = captured.inspect_hint_proxy_targets(URL, "default").unwrap();
    assert_eq!(inspected.trace.contributions.len(), 2);
    assert_eq!(
        inspected.trace.contributions[0].origin.source_revision(),
        original_revision
    );
    assert_eq!(
        inspected.trace.contributions[1].origin.source_revision(),
        captured.revision()
    );
    assert_eq!(inspected.effective.policy, HintProxyTargets::Off);
    layers.runtime.unset_setting(KEY).unwrap();
    app.update_configuration(layers.clone()).unwrap();
    let inspected = app
        .configuration_policy_sources()
        .inspect_hint_proxy_targets(URL, "default")
        .unwrap();
    assert_eq!(inspected.effective.policy, HintProxyTargets::Validated);
    assert_eq!(
        inspected
            .trace
            .winning_origin()
            .unwrap()
            .rule()
            .unwrap()
            .id(),
        "base"
    );
    let prepared = app.prepare_configuration_reload(layers).unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    let inspected = app
        .configuration_policy_sources()
        .inspect_hint_proxy_targets(URL, "default")
        .unwrap();
    assert_eq!(inspected.effective.policy, HintProxyTargets::Off);
    assert!(inspected.trace.contributions.is_empty());
    assert_eq!(
        captured
            .inspect_hint_proxy_targets(URL, "default")
            .unwrap()
            .trace
            .contributions
            .len(),
        2
    );
}

#[test]
fn proxy_inspection_rejects_inconsistent_global_metadata_without_effects() {
    let fixture = Fixture::new("[hints]\nproxy_targets='validated'\n", "");
    let good = fixture.layers();
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(good.clone()).unwrap();
    let before = app.configuration_snapshot();
    let before_sources = app.configuration_policy_sources();
    let mut bad = good;
    bad.base.ui.font_size_pt = 24.0;
    assert!(app.activate_configuration(bad.clone()).is_err());
    let prepared = app.prepare_configuration_update(bad).unwrap();
    assert!(
        prepared
            .policy_sources()
            .inspect_hint_proxy_targets(URL, "default")
            .unwrap_err()
            .to_string()
            .contains("ui.font_size_pt")
    );
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), before_sources);
    assert!(
        before_sources
            .inspect_hint_proxy_targets(URL, "default")
            .is_ok()
    );
}
