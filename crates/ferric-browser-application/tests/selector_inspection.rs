use ferric_browser_application::{BrowserApplication, ConfigurationLayers};
use ferric_browser_config::{
    Config, HintSelectorFamily, HintSelectorInputField, HintSelectorMode, HintSelectorPolicy,
    PolicySourceIdentity, SiteRule, load,
};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const URL: &str = "https://example.test/page";
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ferric-selector-inspection-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("child.toml"),
            "[hints.selectors.buttons]\nmode='replace'\nselectors=['#included']\n\
            [[site_rules]]\nid='same'\npattern='https://example.test/*'\npriority=9999\n\
            [site_rules.set]\n'hints.selectors.buttons'={mode='replace',selectors=['#authored-site']}\n",
        )
        .unwrap();
        fs::write(path.join("root.toml"), "schema_version=3\ninclude=['child.toml']\n[hints.selectors.buttons]\nselectors=['#base']\n").unwrap();
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

fn rule(id: &str, pattern: &str, selector: &str) -> SiteRule {
    SiteRule {
        id: id.into(),
        pattern: pattern.into(),
        priority: -1000,
        interaction: None,
        set: BTreeMap::from([(
            "hints.selectors.buttons".into(),
            toml::Value::try_from(HintSelectorPolicy {
                mode: HintSelectorMode::Extend,
                selectors: vec![selector.into()],
            })
            .unwrap(),
        )]),
    }
}

fn interleaved_layers(fixture: &Fixture) -> ConfigurationLayers {
    let mut layers = fixture.layers();
    layers
        .profile
        .set_literal(
            "hints.selectors.buttons",
            "{mode='extend',selectors=['#base','#profile']}",
        )
        .unwrap();
    layers
        .runtime
        .site_rules
        .push(rule("same", "https://other.test/*", "#never"));
    layers
        .runtime
        .set_literal(
            "hints.selectors.buttons",
            "{mode='extend',selectors=['#runtime']}",
        )
        .unwrap();
    layers
        .command_line
        .set_literal(
            "hints.selectors.buttons",
            "{mode='extend',selectors=['#cli']}",
        )
        .unwrap();
    layers
        .temporary
        .site_rules
        .push(rule("active", "https://example.test/*", "#temp-site"));
    layers
        .temporary
        .set_literal(
            "hints.selectors.buttons",
            "{mode='extend',selectors=['#temp-global']}",
        )
        .unwrap();
    layers
}

#[test]
fn inspection_observes_include_leaves_layer_order_and_nonmatching_id_shadow() {
    let fixture = Fixture::new();
    let layers = interleaved_layers(&fixture);
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers).unwrap();
    let sources = app.configuration_policy_sources();
    let before = app.configuration_snapshot();
    let count = app.interaction_compilation_count();
    let inspected = sources.inspect_hint_selectors(URL, "default").unwrap();
    assert_eq!(
        inspected.policies,
        sources.compile_hint_selectors(URL).unwrap()
    );
    let family = HintSelectorFamily::Buttons;
    assert_eq!(
        inspected.policies[&family]
            .policy
            .as_ref()
            .unwrap()
            .selectors,
        [
            "#base",
            "#profile",
            "#runtime",
            "#cli",
            "#temp-site",
            "#temp-global"
        ]
    );
    let fields = &inspected.trace[&family].fields;
    assert_eq!(
        fields.mode.as_ref().unwrap().source.source(),
        &PolicySourceIdentity::Authored { document_id: 0 }
    );
    assert_eq!(
        fields.selectors[0].source.source(),
        &PolicySourceIdentity::Authored { document_id: 1 }
    );
    assert_eq!(
        fields.selectors[1].source.source(),
        &PolicySourceIdentity::Profile {
            name: "default".into()
        }
    );
    assert_eq!(
        fields.selectors[1].input,
        HintSelectorInputField::Selector(1)
    );
    assert_eq!(
        fields.selectors[2].source.source(),
        &PolicySourceIdentity::Generated
    );
    assert_eq!(
        fields.selectors[3].source.source(),
        &PolicySourceIdentity::CommandLine
    );
    assert_eq!(fields.selectors[4].source.rule().unwrap().id(), "active");
    assert_eq!(
        fields.selectors[4].source.rule().unwrap().authored_index(),
        0
    );
    assert_eq!(fields.selectors[4].source.rule().unwrap().priority(), -1000);
    assert!(fields.selectors[5].source.rule().is_none());
    assert!(
        fields
            .selectors
            .iter()
            .all(|field| field.source.source_revision() == sources.reload_revision())
    );
    let shadow = &inspected.selection.shadowed[0];
    assert_eq!(
        shadow.replaced.source(),
        &PolicySourceIdentity::Authored { document_id: 0 }
    );
    assert_eq!(
        shadow.replacement.source(),
        &PolicySourceIdentity::Generated
    );
    assert_eq!(shadow.replaced.rule().unwrap().priority(), 9999);
    assert_eq!(inspected.selection.survivors.len(), 2);
    assert!(!inspected.selection.survivors[0].matched);
    assert_eq!(
        inspected.selection.survivors[0].origin.rule().unwrap().id(),
        "same"
    );
    assert!(inspected.selection.survivors[1].matched);
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    assert_eq!(app.interaction_compilation_count(), count);
}

#[test]
fn inspection_stays_at_reload_boundary_and_old_capture_cannot_promote_pending_rules() {
    let mut base = Config::default();
    base.hints.selectors.insert(
        HintSelectorFamily::Buttons,
        HintSelectorPolicy {
            mode: HintSelectorMode::Replace,
            selectors: vec!["#base".into()],
        },
    );
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(ConfigurationLayers::new(base))
        .unwrap();
    let mut layers = app.configuration_layers().clone();
    layers
        .runtime
        .set_literal(
            "hints.selectors.buttons",
            "{mode='extend',selectors=['#pending']}",
        )
        .unwrap();
    layers
        .runtime
        .site_rules
        .push(rule("pending", "https://example.test/*", "#pending-site"));
    app.update_configuration(layers.clone()).unwrap();
    let old = app.configuration_policy_sources();
    let active = old.inspect_hint_selectors(URL, "default").unwrap();
    let family = HintSelectorFamily::Buttons;
    assert_eq!(
        active.policies[&family].policy.as_ref().unwrap().selectors,
        ["#base"]
    );
    assert_eq!(active.trace[&family].contributions.len(), 0);
    assert_eq!(active.selection.survivors.len(), 0);
    let prepared = app.prepare_configuration_reload(layers).unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    let current = app.configuration_policy_sources();
    let active = current.inspect_hint_selectors(URL, "default").unwrap();
    assert_eq!(
        active.policies[&family].policy.as_ref().unwrap().selectors,
        ["#base", "#pending-site", "#pending"]
    );
    assert_eq!(
        active.trace[&family].fields.selectors[1]
            .source
            .source_revision(),
        current.reload_revision()
    );
    assert_eq!(
        old.inspect_hint_selectors(URL, "default").unwrap().policies[&family]
            .policy
            .as_ref()
            .unwrap()
            .selectors,
        ["#base"]
    );
}

#[test]
fn inspection_rejects_inconsistent_global_catalogue_before_claiming_origins() {
    let fixture = Fixture::new();
    let good = fixture.layers();
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(good.clone()).unwrap();
    let before = app.configuration_snapshot();
    let before_sources = app.configuration_policy_sources();
    let mut bad = good;
    bad.base.ui.font_size_pt = 22.0;
    assert!(app.activate_configuration(bad.clone()).is_err());
    let prepared = app.prepare_configuration_update(bad).unwrap();
    assert!(
        prepared
            .policy_sources()
            .inspect_hint_selectors(URL, "default")
            .unwrap_err()
            .to_string()
            .contains("ui.font_size_pt")
    );
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), before_sources);
    assert!(
        before_sources
            .inspect_hint_selectors(URL, "default")
            .is_ok()
    );
}

#[test]
fn authored_site_replacement_has_its_own_origin_distinct_from_same_document_seed() {
    let fixture = Fixture::new();
    let mut layers = fixture.layers();
    layers
        .profile
        .set_literal(
            "hints.selectors.buttons",
            "{mode='extend',selectors=['#profile']}",
        )
        .unwrap();
    layers
        .runtime
        .set_literal(
            "hints.selectors.buttons",
            "{mode='extend',selectors=['#runtime']}",
        )
        .unwrap();
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers).unwrap();
    let sources = app.configuration_policy_sources();
    let inspected = sources.inspect_hint_selectors(URL, "default").unwrap();
    let family = HintSelectorFamily::Buttons;
    assert_eq!(
        inspected.policies,
        sources.compile_hint_selectors(URL).unwrap()
    );
    assert_eq!(
        inspected.policies[&family]
            .policy
            .as_ref()
            .unwrap()
            .selectors,
        ["#authored-site", "#runtime"]
    );
    let trace = &inspected.trace[&family];
    let seed = &trace.seed.mode.as_ref().unwrap().source;
    let mode = &trace.fields.mode.as_ref().unwrap().source;
    assert_eq!(mode.source(), seed.source());
    assert!(seed.rule().is_none());
    assert_eq!(mode.rule().unwrap().id(), "same");
    assert_eq!(mode.rule().unwrap().authored_index(), 0);
    assert_eq!(mode.rule().unwrap().priority(), 9999);
    assert_eq!(trace.fields.selectors[0].source, *mode);
    assert_eq!(
        trace.fields.selectors[1].source.source(),
        &PolicySourceIdentity::Generated
    );
    assert_eq!(trace.contributions.len(), 3);
    assert_eq!(inspected.selection.shadowed.len(), 0);
    assert!(inspected.selection.survivors[0].matched);
}
