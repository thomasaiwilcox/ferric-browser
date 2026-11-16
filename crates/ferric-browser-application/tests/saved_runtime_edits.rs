//! Saved typed sources can be repaired without substituting the engine's safe
//! fallback or granting an engine-validation receipt. Actual CSS lives in Qt tests.
use ferric_browser_application::{
    BrowserApplication, InteractionRuleEdit, ProfileOpenRequest, ProfileStorage,
};
use ferric_browser_config::{
    Config, ContextsConfig, PolicySourceIdentity, RuntimeOverrideWriteOutcome, RuntimeOverrides,
    load_runtime_overrides, save_runtime_overrides_atomic,
};
use ferric_browser_core::PrivacyKind;
use ferric_browser_storage::{RootSpec, StorageRoots};
use std::{
    cell::Cell,
    sync::Arc,
    time::{Duration, Instant},
};

fn saved() -> RuntimeOverrides {
    toml::from_str("schema_version=3\n[settings]\n'content.zoom'=1.25\n'hints.selectors.links'={mode='replace',selectors=['a[']}\n[bindings.normal]\nx='scroll down'\n[[site_rules]]\nid='preserved-first'\npattern='https://elsewhere.test/*'\npriority=7\n[site_rules.set]\n'content.images'=false\n[site_rules.interaction]\nenabled=false\nhint_page_keys=['p']\n[[site_rules]]\nid='preserved-second'\npattern='https://second.test/*'\npriority=-4\n[site_rules.interaction]\nenabled=false\n").unwrap()
}
fn open(roots: &StorageRoots) -> BrowserApplication {
    let (mut app, _, _) =
        BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default").unwrap();
    app.request_profile_open(ProfileOpenRequest {
        name: "default".into(),
        label: "Default".into(),
        privacy: PrivacyKind::Normal,
        storage: ProfileStorage::Durable(RootSpec::Base(roots.data.parent().unwrap().to_owned())),
        base_configuration: Config::default(),
        base_authored: Arc::default(),
        initial_profile_overrides: RuntimeOverrides::default(),
        command_line_overrides: RuntimeOverrides::default(),
        config_path: None,
        script_catalog_root: None,
        contexts: ContextsConfig::default(),
        context_configuration_error: None,
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = app.poll_profile_open() {
            assert!(result.unwrap().configuration_error.is_none());
            break;
        }
        assert!(Instant::now() < deadline, "profile open timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
    app
}
fn fixture() -> (BrowserApplication, StorageRoots, RuntimeOverrides) {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let runtime = saved();
    save_runtime_overrides_atomic(roots.state.join("runtime-overrides.toml"), &runtime).unwrap();
    let mut app = open(&roots);
    assert_eq!(app.configuration_layers().runtime, runtime);
    // Exactly the application-side safe selection used after engine CSS
    // rejection; this test does not pretend to run WebEngine.
    app.set_policy_owner_preparing(true);
    app.replace_configuration(Config::default()).unwrap();
    app.set_policy_owner_preparing(false);
    assert_eq!(
        app.configuration_layers().runtime,
        RuntimeOverrides::default()
    );
    (app, roots, runtime)
}
#[test]
fn saved_capture_repairs_the_full_graph_and_retains_activation_boundaries() {
    let (mut app, roots, runtime) = fixture();
    let before = app.configuration_snapshot();
    assert!(app.capture_interaction_edits().is_err());
    let capture = app.capture_saved_runtime_edits().unwrap();
    assert_eq!(capture.layers().runtime, runtime);
    assert_eq!(app.configuration_snapshot(), before);
    let mut fixed = runtime.clone();
    fixed
        .set_literal(
            "hints.selectors.links",
            "{mode='replace',selectors=['#repaired']}",
        )
        .unwrap();
    let prepared = app
        .prepare_saved_runtime_edits(&capture, fixed.clone())
        .unwrap();
    assert!(!prepared.is_committed_peer_update());
    assert_eq!(prepared.policy_sources().desired_layers().runtime, fixed);
    assert!(prepared.snapshot().effective.hints.selectors.is_empty());
    assert!(prepared.snapshot().effective.site_rules.is_empty());
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(
        load_runtime_overrides(roots.state.join("runtime-overrides.toml")).unwrap(),
        runtime
    );
    app.request_configuration_write(prepared).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let receipt = loop {
        if let Some(result) = app.poll_configuration_write() {
            break result.unwrap();
        }
        assert!(Instant::now() < deadline, "write timed out");
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(receipt.outcome, RuntimeOverrideWriteOutcome::Durable);
    assert!(receipt.publication_error.is_none());
    app.publish_configuration_update(receipt.prepared, || Ok(()))
        .unwrap();
    assert_eq!(
        load_runtime_overrides(roots.state.join("runtime-overrides.toml")).unwrap(),
        fixed
    );
    assert_eq!(app.configuration_layers().runtime, fixed);
    assert!(app.configuration().hints.selectors.is_empty());
    assert!(app.configuration().site_rules.is_empty());
    assert!((app.configuration().content.zoom - 1.25).abs() < f64::EPSILON);
    let reloaded = app
        .prepare_configuration_reload(app.configuration_layers().clone())
        .unwrap();
    app.publish_configuration_update(reloaded, || Ok(()))
        .unwrap();
    assert_eq!(app.configuration().site_rules, fixed.site_rules);
    assert_eq!(app.configuration().bindings["normal"]["x"], "scroll down");
    assert_eq!(
        load_runtime_overrides(roots.state.join("runtime-overrides.toml")).unwrap(),
        fixed
    );
}
#[test]
fn saved_interaction_capture_preserves_invalid_legacy_css_and_rejects_other_sources() {
    let (app, roots, runtime) = fixture();
    let capture = app.capture_saved_interaction_edits().unwrap();
    let operation = InteractionRuleEdit::Replace {
        id: "preserved-first".into(),
        expected_pattern: "https://elsewhere.test/*".into(),
        interaction: toml::from_str("enabled=true").unwrap(),
    };
    assert!(
        app.prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Temporary,
            std::slice::from_ref(&operation)
        )
        .is_err()
    );
    let prepared = app
        .prepare_interaction_edits(&capture, &PolicySourceIdentity::Generated, &[operation])
        .unwrap();
    let desired = prepared.prepared().policy_sources();
    let candidate = &desired.desired_layers().runtime;
    assert_eq!(candidate.settings, runtime.settings);
    assert_eq!(candidate.bindings, runtime.bindings);
    assert_eq!(candidate.site_rules[0].set, runtime.site_rules[0].set);
    assert_eq!(candidate.site_rules[1], runtime.site_rules[1]);
    assert_eq!(
        load_runtime_overrides(roots.state.join("runtime-overrides.toml")).unwrap(),
        runtime
    );
}
#[test]
fn local_changes_and_shared_value_aba_retire_captures_before_persistence() {
    let (mut app, roots, runtime) = fixture();
    let capture = app.capture_saved_runtime_edits().unwrap();
    let stale_prepared = app
        .prepare_saved_runtime_edits(&capture, runtime.clone())
        .unwrap();
    let (peer_runtime, _, _) =
        ferric_browser_runtime::BrowserRuntime::bootstrap(PrivacyKind::Normal, "default").unwrap();
    let mut peer = app.inherit_profile(peer_runtime).unwrap();
    for zoom in ["1.5", "1.25"] {
        let current = peer.capture_saved_runtime_edits().unwrap();
        let mut candidate = current.layers().runtime.clone();
        candidate.set_literal("content.zoom", zoom).unwrap();
        let prepared = peer
            .prepare_saved_runtime_edits(&current, candidate)
            .unwrap();
        peer.publish_configuration_update(prepared, || Ok(()))
            .unwrap();
    }
    assert_eq!(
        app.capture_saved_runtime_edits().unwrap().layers().runtime,
        runtime
    );
    assert!(
        app.prepare_saved_runtime_edits(&capture, runtime.clone())
            .is_err()
    );
    let called = Cell::new(false);
    assert!(
        app.publish_configuration_update(stale_prepared, || {
            called.set(true);
            Ok(())
        })
        .is_err()
    );
    assert!(!called.get());
    let local = app.capture_saved_runtime_edits().unwrap();
    app.set_policy_owner_preparing(true);
    app.replace_configuration(Config::default()).unwrap();
    app.set_policy_owner_preparing(false);
    assert!(
        app.prepare_saved_runtime_edits(&local, runtime.clone())
            .is_err()
    );
    assert_eq!(
        load_runtime_overrides(roots.state.join("runtime-overrides.toml")).unwrap(),
        runtime
    );
}
#[test]
fn unadmitted_capture_does_not_keep_a_retired_same_path_root_alive() {
    let (app, roots, runtime) = fixture();
    let capture = app.capture_saved_runtime_edits().unwrap();
    let prepared = app
        .prepare_saved_runtime_edits(&capture, runtime.clone())
        .unwrap();
    drop(app);
    let mut changed = runtime;
    changed.set_literal("content.zoom", "1.75").unwrap();
    save_runtime_overrides_atomic(roots.state.join("runtime-overrides.toml"), &changed).unwrap();
    let mut replacement = open(&roots);
    assert_eq!(
        replacement
            .capture_saved_runtime_edits()
            .unwrap()
            .layers()
            .runtime,
        changed
    );
    assert!(
        replacement
            .prepare_saved_runtime_edits(&capture, changed.clone())
            .is_err()
    );
    let called = Cell::new(false);
    assert!(
        replacement
            .publish_configuration_update(prepared, || {
                called.set(true);
                Ok(())
            })
            .is_err()
    );
    assert!(!called.get());
    assert_eq!(
        load_runtime_overrides(roots.state.join("runtime-overrides.toml")).unwrap(),
        changed
    );
}

#[test]
fn saved_repair_remains_a_fresh_proposal_to_every_shared_owner() {
    use ferric_browser_config::SiteRule;
    use ferric_browser_core::{Event, ValidatedUrl};
    use ferric_browser_runtime::{BrowserRuntime, RuntimeInput};
    use std::collections::BTreeMap;
    let (mut app, roots, runtime) = fixture();
    let (peer_runtime, window, _) =
        BrowserRuntime::bootstrap(PrivacyKind::Normal, "default").unwrap();
    let mut peer = app.inherit_profile(peer_runtime).unwrap();
    let rule = |id: &str, patch: &str| SiteRule {
        id: id.into(),
        pattern: "https://example.test/*".into(),
        priority: 0,
        set: BTreeMap::new(),
        interaction: Some(toml::from_str(patch).unwrap()),
    };
    let mut own = peer.configuration_layers().clone();
    own.runtime = runtime;
    own.base.site_rules.push(rule(
        "peer-anchor",
        "[[anchors]]\nid='main'\ncode='m'\nfamilies=['links']\ntarget_selector='#main'",
    ));
    peer.set_policy_owner_preparing(true);
    peer.activate_configuration(own).unwrap();
    peer.set_policy_owner_preparing(false);
    let tab = peer.windows()[&window].active_tab.unwrap();
    peer.dispatch_runtime(RuntimeInput::Lifecycle(Event::RestoreTabAddress {
        target: peer.capture_target(tab).unwrap(),
        url: ValidatedUrl::parse("https://example.test/page").unwrap(),
    }))
    .unwrap();
    let capture = app.capture_saved_runtime_edits().unwrap();
    let mut fixed = capture.layers().runtime.clone();
    fixed
        .set_literal(
            "hints.selectors.links",
            "{mode='replace',selectors=['#repaired']}",
        )
        .unwrap();
    fixed.site_rules.push(rule("proposed-collection", "[[collections]]\nid='list'\nfamilies=['links']\ncodes=['m']\ncontainer_self=true\nitem_selector='a.item'\nprimary_self=true\norder='dom'"));
    let prepared = app.prepare_saved_runtime_edits(&capture, fixed).unwrap();
    let before = app.configuration_snapshot();
    let path = roots.state.join("runtime-overrides.toml");
    let bytes = std::fs::read(&path).unwrap();
    let error = app.request_configuration_write(prepared).unwrap_err();
    assert!(error.to_string().contains("duplicate"), "{error}");
    assert!(!app.configuration_write_pending());
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}
