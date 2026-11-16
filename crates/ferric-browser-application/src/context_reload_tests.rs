use super::*;
use crate::profile::ProfileActivation;
use ferric_browser_config::{Config, ContextDefinition, ContextRoute};
use ferric_browser_runtime::BrowserRuntime;
use ferric_browser_storage::{RootSpec, StorageRoots};

fn application(roots: &StorageRoots, privacy: PrivacyKind) -> BrowserApplication {
    application_with_storage(roots, privacy, true)
}

fn application_with_storage(
    roots: &StorageRoots,
    privacy: PrivacyKind,
    writable: bool,
) -> BrowserApplication {
    let id = ProfileRegistry::open(roots)
        .unwrap()
        .get_or_create_default()
        .unwrap()
        .id;
    let (runtime, _, _) = BrowserRuntime::bootstrap(privacy, "Context tests").unwrap();
    let mut app = BrowserApplication::from_runtime(runtime, Config::default());
    app.activate_profile(ProfileActivation {
        name: "default".into(),
        id: Some(id),
        privacy,
        roots: Some(roots.clone()),
        lock: None,
        contexts: Some(ContextRegistry::open(roots).unwrap()),
        storage_path: (privacy == PrivacyKind::Normal && writable)
            .then(|| roots.data.join("browser.sqlite")),
    })
    .unwrap();
    app
}
fn configuration(profile: &str) -> ContextsConfig {
    ContextsConfig {
        schema_version: 3,
        contexts: vec![ContextDefinition {
            name: "work".into(),
            label: "Work".into(),
            profile: profile.into(),
            sessions: Vec::new(),
            workspace: Some("3".into()),
            accent: None,
            default_target: "reuse".into(),
        }],
        routes: Vec::new(),
    }
}

fn conflicting_interaction_preparation(
    app: &BrowserApplication,
) -> crate::PreparedConfigurationUpdate {
    let rule = |id: &str, patch: &str| ferric_browser_config::SiteRule {
        id: id.into(),
        pattern: "https://example.test/*".into(),
        priority: 0,
        set: std::collections::BTreeMap::new(),
        interaction: Some(toml::from_str(patch).unwrap()),
    };
    let mut layers = app.configuration_layers().clone();
    layers.base.site_rules.push(rule(
        "anchor",
        "[[anchors]]\nid='main'\ncode='m'\nfamilies=['links']\ntarget_selector='#main'",
    ));
    layers.runtime.site_rules.push(rule("collection", "[[collections]]\nid='list'\nfamilies=['links']\ncodes=['m']\ncontainer_self=true\nitem_selector='a.item'\nprimary_self=true\norder='dom'"));
    app.prepare_configuration_reload(layers).unwrap()
}

fn restore_interaction_page(app: &mut BrowserApplication) -> ferric_browser_core::WindowId {
    let window = app.active_window().unwrap();
    app.dispatch_runtime(ferric_browser_runtime::RuntimeInput::Lifecycle(
        ferric_browser_core::Event::RestoreTabAddress {
            target: app.capture_target(app.active_tab().unwrap().id).unwrap(),
            url: ferric_browser_core::ValidatedUrl::parse("https://example.test/page").unwrap(),
        },
    ))
    .unwrap();
    window
}

#[test]
fn interaction_conflict_rejects_context_admission_before_registry_io() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let mut app = application(&roots, PrivacyKind::Normal);
    let prepared = conflicting_interaction_preparation(&app);
    restore_interaction_page(&mut app);
    let job = app
        .prepare_context_reload(&ContextsConfig::default(), configuration("default"))
        .unwrap()
        .unwrap();
    let before = app.configuration_snapshot();
    assert!(
        app.admit_context_reload(&prepared, job)
            .unwrap_err()
            .to_string()
            .contains("Interaction active policy conflicts")
    );
    assert!(!roots.state.join("contexts.json").exists());
    assert!(app.contexts().unwrap().is_empty());
    assert_eq!(app.configuration_snapshot(), before);
    assert!(app.reserve_configuration_reload().is_ok());
}

#[test]
fn later_interaction_context_does_not_revoke_the_exact_committed_context_effect() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let mut app = application(&roots, PrivacyKind::Normal);
    let prepared = conflicting_interaction_preparation(&app);
    let job = app
        .prepare_context_reload(&ContextsConfig::default(), configuration("default"))
        .unwrap()
        .unwrap();
    let job = app.admit_context_reload(&prepared, job).unwrap();
    let window = restore_interaction_page(&mut app);
    let effect = job.run().unwrap();
    assert_eq!(
        ContextRegistry::open(&roots).unwrap().contexts()[0].label,
        "Work"
    );
    app.publish_context_configuration(prepared, effect).unwrap();
    assert_eq!(app.contexts().unwrap()[0].label, "Work");
    assert_eq!(
        app.refresh_interaction_snapshot(window, "https://example.test/page")
            .unwrap()
            .state(),
        crate::InteractionCompilationState::Conflicted
    );
}

#[test]
fn committed_context_effect_cannot_authorize_another_preparation_with_equal_revisions() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let mut app = application(&roots, PrivacyKind::Normal);
    let original = app
        .prepare_configuration_reload(app.configuration_layers().clone())
        .unwrap();
    let different = conflicting_interaction_preparation(&app);
    assert_eq!(original.expected_revision, different.expected_revision);
    assert_eq!(original.profile_revision, different.profile_revision);
    let effect = completed_context_reload_for(
        &app,
        &original,
        &ContextsConfig::default(),
        configuration("default"),
    );
    restore_interaction_page(&mut app);
    let before = app.configuration_snapshot();
    assert!(
        app.publish_context_configuration(different, effect)
            .is_err()
    );
    assert_eq!(app.configuration_snapshot(), before);
    assert!(app.contexts().unwrap().is_empty());
    assert_eq!(
        ContextRegistry::open(&roots).unwrap().contexts()[0].label,
        "Work"
    );
}

// Exercise final freshness checks independently of admission: these fixtures
// retain completed metadata after explicitly releasing its completed reservation.
fn completed_context_reload(
    app: &BrowserApplication,
    previous: &ContextsConfig,
    next: ContextsConfig,
) -> ContextReloadEffect {
    let prepared = app
        .prepare_configuration_reload(app.configuration_layers().clone())
        .unwrap();
    completed_context_reload_for(app, &prepared, previous, next)
}

fn completed_context_reload_for(
    app: &BrowserApplication,
    prepared: &crate::PreparedConfigurationUpdate,
    previous: &ContextsConfig,
    next: ContextsConfig,
) -> ContextReloadEffect {
    let job = app.prepare_context_reload(previous, next).unwrap().unwrap();
    let mut effect = app
        .admit_context_reload(prepared, job)
        .unwrap()
        .run()
        .unwrap();
    app.release_configuration_reload(effect.reservation.take().unwrap())
        .unwrap();
    effect
}

#[test]
fn unadmitted_context_job_rejects_before_any_io_even_with_a_competing_reservation() {
    for busy in [false, true] {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        let app = application(&roots, PrivacyKind::Normal);
        let job = app
            .prepare_context_reload(&ContextsConfig::default(), configuration("default"))
            .unwrap()
            .unwrap();
        let reservation = busy.then(|| app.reserve_configuration_reload().unwrap());
        assert!(matches!(job.run(), Err(ContextReloadError::NotAdmitted)));
        assert!(!roots.state.join("contexts.json").exists());
        assert!(app.contexts().unwrap().is_empty());
        if busy {
            assert!(app.reserve_configuration_reload().is_err());
        }
        drop(reservation);
        assert!(app.reserve_configuration_reload().is_ok());
    }
}

#[test]
fn context_admission_preflights_shared_revision_exhaustion_before_io() {
    for changes_runtime in [true, false] {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        let mut app = application(&roots, PrivacyKind::Normal);
        app.active_profile
            .as_ref()
            .unwrap()
            .session
            .0
            .configuration
            .state
            .lock()
            .unwrap()
            .revision = u64::MAX;
        let mut layers = app.configuration_layers().clone();
        if changes_runtime {
            layers.runtime.set_literal("content.zoom", "1.75").unwrap();
        }
        let prepared = app.prepare_configuration_reload(layers).unwrap();
        let job = app
            .prepare_context_reload(&ContextsConfig::default(), configuration("default"))
            .unwrap()
            .unwrap();
        let before = app.configuration_snapshot();
        let admission = app.admit_context_reload(&prepared, job);
        if changes_runtime {
            assert!(admission.is_err());
            assert!(!roots.state.join("contexts.json").exists());
            assert_eq!(app.configuration_snapshot(), before);
            assert!(app.contexts().unwrap().is_empty());
        } else {
            let effect = admission.unwrap().run().unwrap();
            app.publish_context_configuration(prepared, effect).unwrap();
            assert_eq!(app.contexts().unwrap()[0].label, "Work");
        }
        assert_eq!(app.profile_configuration_revision().unwrap(), u64::MAX);
        assert!(app.reserve_configuration_reload().is_ok());
    }
}

#[test]
fn admitted_context_work_keeps_peers_blocked_after_observer_retirement() {
    use ferric_browser_runtime::worker::SingleFlightWorker;
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };

    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let app = application(&roots, PrivacyKind::Normal);
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "peer").unwrap();
    let peer = app.inherit_profile(runtime).unwrap();
    let prepared = app
        .prepare_configuration_reload(app.configuration_layers().clone())
        .unwrap();
    let job = app
        .prepare_context_reload(&ContextsConfig::default(), configuration("default"))
        .unwrap()
        .unwrap();
    let job = app.admit_context_reload(&prepared, job).unwrap();
    let (started, running) = mpsc::channel();
    let (release, proceed) = mpsc::channel();
    let mut worker =
        SingleFlightWorker::spawn("paused-context-reload", move |job: ContextReloadJob| {
            started.send(()).unwrap();
            proceed.recv_timeout(Duration::from_secs(3)).unwrap();
            job.run()
        })
        .unwrap();
    worker.submit(job).unwrap();
    running.recv_timeout(Duration::from_secs(3)).unwrap();
    drop(prepared);
    drop(worker);
    drop(app);
    assert!(peer.reserve_configuration_reload().is_err());
    assert!(
        peer.prepare_configuration_reload(peer.configuration_layers().clone())
            .is_err()
    );
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Ok(reservation) = peer.reserve_configuration_reload() {
            drop(reservation);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "retired context completion retained its gate"
        );
        std::thread::yield_now();
    }
    assert_eq!(
        ContextRegistry::open(&roots).unwrap().contexts()[0].label,
        "Work"
    );
}

#[test]
fn admitted_context_completion_retains_its_gate_until_publication() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let mut app = application(&roots, PrivacyKind::Normal);
    let prepared = app
        .prepare_configuration_reload(app.configuration_layers().clone())
        .unwrap();
    let job = app
        .prepare_context_reload(&ContextsConfig::default(), configuration("default"))
        .unwrap()
        .unwrap();
    let effect = app
        .admit_context_reload(&prepared, job)
        .unwrap()
        .run()
        .unwrap();
    assert!(app.reserve_configuration_reload().is_err());
    app.publish_context_configuration(prepared, effect).unwrap();
    assert_eq!(app.contexts().unwrap()[0].label, "Work");
    assert!(app.reserve_configuration_reload().is_ok());
}

#[test]
fn rejected_context_admission_never_writes_or_strands_the_gate() {
    for (foreign, peer_update) in [(true, false), (false, false), (false, true)] {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        let mut app = application(&roots, PrivacyKind::Normal);
        let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "peer").unwrap();
        let mut peer = app.inherit_profile(runtime).unwrap();
        let prepared = app
            .prepare_configuration_reload(app.configuration_layers().clone())
            .unwrap();
        let job = app
            .prepare_context_reload(&ContextsConfig::default(), configuration("default"))
            .unwrap()
            .unwrap();
        let before = app.configuration_snapshot();
        if foreign {
            assert!(peer.admit_context_reload(&prepared, job).is_err());
            assert_eq!(app.configuration_snapshot(), before);
        } else if peer_update {
            let mut layers = peer.configuration_layers().clone();
            layers.runtime.set_literal("content.zoom", "1.75").unwrap();
            peer.update_configuration(layers).unwrap();
            assert!(app.admit_context_reload(&prepared, job).is_err());
            assert_eq!(app.configuration_snapshot(), before);
        } else {
            let mut layers = app.configuration_layers().clone();
            layers
                .temporary
                .set_literal("content.zoom", "1.75")
                .unwrap();
            app.update_configuration(layers).unwrap();
            let after = app.configuration_snapshot();
            assert!(app.admit_context_reload(&prepared, job).is_err());
            assert_eq!(app.configuration_snapshot(), after);
        }
        assert!(app.reserve_configuration_reload().is_ok());
        assert!(ContextRegistry::open(&roots).unwrap().contexts().is_empty());
        assert!(app.contexts().unwrap().is_empty());
    }
}

#[test]
fn failed_admitted_context_work_releases_its_gate() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let app = application(&roots, PrivacyKind::Normal);
    let prepared = app
        .prepare_configuration_reload(app.configuration_layers().clone())
        .unwrap();
    let job = app
        .prepare_context_reload(&ContextsConfig::default(), configuration("missing"))
        .unwrap()
        .unwrap();
    let job = app.admit_context_reload(&prepared, job).unwrap();
    assert!(app.reserve_configuration_reload().is_err());
    assert!(job.run().is_err());
    assert!(app.reserve_configuration_reload().is_ok());
    assert!(ContextRegistry::open(&roots).unwrap().contexts().is_empty());
}

#[test]
fn context_job_and_committed_effect_retain_the_profile_lock_after_observer_close() {
    use ferric_browser_storage::ProfileLock;

    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let id = ProfileRegistry::open(&roots)
        .unwrap()
        .get_or_create_default()
        .unwrap()
        .id;
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "lock owner").unwrap();
    let mut app = BrowserApplication::from_runtime(runtime, Config::default());
    app.activate_profile(ProfileActivation {
        name: "default".into(),
        id: Some(id),
        privacy: PrivacyKind::Normal,
        roots: Some(roots.clone()),
        lock: Some(ProfileLock::acquire(&roots, id).unwrap()),
        contexts: Some(ContextRegistry::open(&roots).unwrap()),
        storage_path: Some(roots.data.join("browser.sqlite")),
    })
    .unwrap();
    let job = app
        .prepare_context_reload(&ContextsConfig::default(), configuration("default"))
        .unwrap()
        .unwrap();
    let prepared = app
        .prepare_configuration_reload(app.configuration_layers().clone())
        .unwrap();
    let job = app.admit_context_reload(&prepared, job).unwrap();
    drop(prepared);
    drop(app);
    assert!(ProfileLock::is_held(&roots, id));
    let effect = job.run().unwrap();
    assert!(ProfileLock::is_held(&roots, id));
    assert_eq!(
        ContextRegistry::open(&roots).unwrap().contexts()[0].label,
        "Work"
    );
    drop(effect);
    assert!(!ProfileLock::is_held(&roots, id));
}

#[test]
fn rejected_context_refresh_preserves_configuration_sources_runtime_and_shared_layer() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let mut app = application(&roots, PrivacyKind::Normal);
    let older = configuration("default");
    let mut layers = app.configuration_layers().clone();
    layers.runtime.set_literal("content.zoom", "1.5").unwrap();
    let prepared = app.prepare_configuration_reload(layers).unwrap();
    let old_effect =
        completed_context_reload_for(&app, &prepared, &ContextsConfig::default(), older.clone());
    let mut newer = older.clone();
    newer.contexts[0].label = "Newer work".into();
    let new_effect = completed_context_reload(&app, &older, newer.clone());
    app.apply_context_reload(new_effect).unwrap();
    let before = app.configuration_snapshot();
    let sources = app.configuration_policy_sources();
    let runtime = app.snapshot();
    let shared = app
        .active_profile
        .as_ref()
        .unwrap()
        .session
        .0
        .configuration
        .clone();
    let revision = shared.state.lock().unwrap().revision;
    let persistent = shared.state.lock().unwrap().runtime.clone();
    assert!(
        app.publish_context_configuration(prepared, old_effect)
            .is_err()
    );
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    assert_eq!(app.snapshot(), runtime);
    assert_eq!(app.contexts().unwrap()[0].label, "Newer work");
    assert_eq!(shared.state.lock().unwrap().revision, revision);
    assert_eq!(shared.state.lock().unwrap().runtime, persistent);
}

#[test]
fn old_context_effect_cannot_pair_with_a_fresh_preparation_after_a_local_update() {
    reject_old_context_effect(false, true);
}

#[test]
fn old_context_effect_cannot_pair_with_a_fresh_preparation_after_a_peer_update() {
    reject_old_context_effect(true, true);
}

#[test]
fn standalone_context_effect_rejects_a_later_local_update() {
    reject_old_context_effect(false, false);
}

#[test]
fn standalone_context_effect_rejects_a_later_peer_update() {
    reject_old_context_effect(true, false);
}

fn reject_old_context_effect(peer_update: bool, paired: bool) {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let mut app = application(&roots, PrivacyKind::Normal);
    let effect =
        completed_context_reload(&app, &ContextsConfig::default(), configuration("default"));
    if peer_update {
        let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "peer").unwrap();
        let mut peer = app.inherit_profile(runtime).unwrap();
        let mut layers = peer.configuration_layers().clone();
        layers.runtime.set_literal("content.zoom", "1.75").unwrap();
        peer.update_configuration(layers).unwrap();
    } else {
        let mut layers = app.configuration_layers().clone();
        layers
            .temporary
            .set_literal("content.zoom", "1.75")
            .unwrap();
        app.update_configuration(layers).unwrap();
    }
    let before = app.configuration_snapshot();
    let sources = app.configuration_policy_sources();
    let runtime = app.snapshot();
    if paired {
        let layers = app
            .peer_configuration_layers()
            .unwrap()
            .unwrap_or_else(|| app.configuration_layers().clone());
        let prepared = app.prepare_configuration_reload(layers).unwrap();
        assert!(app.publish_context_configuration(prepared, effect).is_err());
    } else {
        assert!(app.apply_context_reload(effect).is_err());
    }
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    assert_eq!(app.snapshot(), runtime);
    assert!(app.contexts().unwrap().is_empty());
}

#[test]
fn context_publication_rejects_a_stale_shared_profile_revision() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let mut app = application(&roots, PrivacyKind::Normal);
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "peer").unwrap();
    let mut peer = app.inherit_profile(runtime).unwrap();
    let prepared = app
        .prepare_configuration_reload(app.configuration_layers().clone())
        .unwrap();
    let effect = completed_context_reload_for(
        &app,
        &prepared,
        &ContextsConfig::default(),
        configuration("default"),
    );
    let mut newer = peer.configuration_layers().clone();
    newer.runtime.set_literal("content.zoom", "1.5").unwrap();
    peer.update_configuration(newer).unwrap();
    let before = app.configuration_snapshot();
    let before_sources = app.configuration_policy_sources();
    assert!(app.publish_context_configuration(prepared, effect).is_err());
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), before_sources);
    assert!(app.contexts().unwrap().is_empty());
}

#[test]
fn context_publication_synchronizes_persistent_settings_without_overwriting_peer_temporary_values()
{
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let mut app = application(&roots, PrivacyKind::Normal);
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "peer").unwrap();
    let mut peer = app.inherit_profile(runtime).unwrap();
    let mut peer_layers = peer.configuration_layers().clone();
    peer_layers
        .temporary
        .set_literal("content.zoom", "1.75")
        .unwrap();
    peer.update_configuration(peer_layers).unwrap();
    let mut layers = app.configuration_layers().clone();
    layers.runtime.set_literal("content.zoom", "1.5").unwrap();
    let prepared = app.prepare_configuration_reload(layers).unwrap();
    let effect = completed_context_reload_for(
        &app,
        &prepared,
        &ContextsConfig::default(),
        configuration("default"),
    );
    app.publish_context_configuration(prepared, effect).unwrap();
    let synchronized = peer
        .peer_configuration_layers()
        .unwrap()
        .expect("persistent layer was not published to peers");
    assert_eq!(
        synchronized.runtime.settings["content.zoom"].as_float(),
        Some(1.5)
    );
    assert_eq!(
        synchronized.temporary.settings["content.zoom"].as_float(),
        Some(1.75)
    );
    peer.update_configuration(synchronized).unwrap();
    assert!((peer.configuration().content.zoom - 1.75).abs() < f64::EPSILON);
}

#[test]
fn inherited_peer_cannot_adopt_another_windows_context_effect() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let app = application(&roots, PrivacyKind::Normal);
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "peer").unwrap();
    let mut peer = app.inherit_profile(runtime).unwrap();
    assert_eq!(app.profile_snapshot(), peer.profile_snapshot());
    let effect =
        completed_context_reload(&app, &ContextsConfig::default(), configuration("default"));
    assert!(peer.apply_context_reload(effect).is_err());
    assert!(peer.contexts().unwrap().is_empty());
}

#[test]
fn context_configuration_rejects_a_foreign_effect_even_with_local_preparation() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let app = application(&roots, PrivacyKind::Normal);
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "peer").unwrap();
    let mut peer = app.inherit_profile(runtime).unwrap();
    let prepared = peer
        .prepare_configuration_reload(peer.configuration_layers().clone())
        .unwrap();
    let effect =
        completed_context_reload(&app, &ContextsConfig::default(), configuration("default"));
    let before = peer.configuration_snapshot();
    assert!(
        peer.publish_context_configuration(prepared, effect)
            .is_err()
    );
    assert_eq!(peer.configuration_snapshot(), before);
    assert!(peer.contexts().unwrap().is_empty());
}

#[test]
fn context_configuration_rejects_a_foreign_preparation_even_with_local_effect() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let app = application(&roots, PrivacyKind::Normal);
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "peer").unwrap();
    let mut peer = app.inherit_profile(runtime).unwrap();
    let prepared = app
        .prepare_configuration_reload(app.configuration_layers().clone())
        .unwrap();
    let effect =
        completed_context_reload(&peer, &ContextsConfig::default(), configuration("default"));
    let before = peer.configuration_snapshot();
    assert!(
        peer.publish_context_configuration(prepared, effect)
            .is_err()
    );
    assert_eq!(peer.configuration_snapshot(), before);
    assert!(peer.contexts().unwrap().is_empty());
}

#[test]
fn durable_job_publishes_only_when_its_owner_accepts_it() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let mut app = application(&roots, PrivacyKind::Normal);
    let identity = app.profile_snapshot();
    let next = configuration("default");
    let job = app
        .prepare_context_reload(&ContextsConfig::default(), next.clone())
        .unwrap()
        .unwrap();
    let prepared = app
        .prepare_configuration_reload(app.configuration_layers().clone())
        .unwrap();
    let job = app.admit_context_reload(&prepared, job).unwrap();
    let result = std::thread::spawn(move || job.run())
        .join()
        .unwrap()
        .unwrap();
    assert_eq!(app.contexts().unwrap(), []);
    assert_eq!(
        ContextRegistry::open(&roots).unwrap().contexts()[0].label,
        "Work"
    );
    assert_eq!(app.apply_context_reload(result).unwrap().0, next);
    assert_eq!(app.contexts().unwrap()[0].label, "Work");
    assert_eq!(app.profile_snapshot(), identity);
}

#[test]
fn configuration_and_contexts_publish_together_after_the_reserved_job() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let mut app = application(&roots, PrivacyKind::Normal);
    let before = app.configuration_snapshot();
    let before_sources = app.configuration_policy_sources();
    let loaded = ferric_browser_config::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ferric-browser-config/tests/fixtures/site-policy/includes/root.toml"
    ))
    .unwrap();
    let authored = loaded.authored;
    let mut layers = crate::ConfigurationLayers::new(loaded.config);
    layers.base_authored = authored.clone();
    layers
        .runtime
        .set_literal("ui.font_size_pt", "18.0")
        .unwrap();
    let prepared = app.prepare_configuration_reload(layers).unwrap();
    let job = app
        .prepare_context_reload(&ContextsConfig::default(), configuration("default"))
        .unwrap()
        .unwrap();
    let job = app.admit_context_reload(&prepared, job).unwrap();
    let effect = std::thread::spawn(move || job.run())
        .join()
        .unwrap()
        .unwrap();
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), before_sources);
    assert_eq!(app.contexts().unwrap(), []);
    let (published, _) = app.publish_context_configuration(prepared, effect).unwrap();
    assert!((published.effective.ui.font_size_pt - 18.0).abs() < f64::EPSILON);
    assert_eq!(app.contexts().unwrap()[0].name, "work");
    let sources = app.configuration_policy_sources();
    assert_eq!(sources.desired_layers(), sources.reload_layers());
    assert_eq!(sources.revision(), published.revision);
    assert_eq!(sources.reload_revision(), published.revision);
    assert!(std::sync::Arc::ptr_eq(
        &sources.reload_layers().base_authored,
        &authored
    ));
}

#[test]
fn stale_configuration_rejects_the_catalog_without_partial_publication() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let mut app = application(&roots, PrivacyKind::Normal);
    let layers = crate::ConfigurationLayers::new(Config::default());
    let prepared = app.prepare_configuration_reload(layers.clone()).unwrap();
    let effect = completed_context_reload_for(
        &app,
        &prepared,
        &ContextsConfig::default(),
        configuration("default"),
    );
    app.update_configuration(layers).unwrap();
    let before = app.configuration_snapshot();
    let before_sources = app.configuration_policy_sources();
    assert!(app.publish_context_configuration(prepared, effect).is_err());
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), before_sources);
    assert_eq!(app.contexts().unwrap(), []);
}

#[test]
fn combined_publication_cannot_bypass_an_active_configuration_reservation() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let mut app = application(&roots, PrivacyKind::Normal);
    let prepared = app
        .prepare_configuration_reload(crate::ConfigurationLayers::new(Config::default()))
        .unwrap();
    let effect = completed_context_reload_for(
        &app,
        &prepared,
        &ContextsConfig::default(),
        configuration("default"),
    );
    let before = app.configuration_snapshot();
    let before_sources = app.configuration_policy_sources();
    let reservation = app.reserve_configuration_reload().unwrap();
    assert!(app.publish_context_configuration(prepared, effect).is_err());
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), before_sources);
    assert_eq!(app.contexts().unwrap(), []);
    drop(reservation);
}

#[test]
fn unknown_profiles_are_rejected_without_creating_metadata_or_profiles() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let app = application(&roots, PrivacyKind::Normal);
    let job = app
        .prepare_context_reload(&ContextsConfig::default(), configuration("missing"))
        .unwrap()
        .unwrap();
    let prepared = app
        .prepare_configuration_reload(app.configuration_layers().clone())
        .unwrap();
    let job = app.admit_context_reload(&prepared, job).unwrap();
    assert!(matches!(job.run(), Err(ContextReloadError::UnknownProfile(name)) if name=="missing"));
    assert!(!roots.state.join("contexts.json").exists());
    assert_eq!(ProfileRegistry::open(&roots).unwrap().profiles().len(), 1);
}

#[test]
fn private_and_temporary_sessions_never_create_durable_context_jobs() {
    for privacy in [PrivacyKind::Private, PrivacyKind::Ephemeral] {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        let app = application(&roots, privacy);
        assert!(
            app.prepare_context_reload(&ContextsConfig::default(), configuration("default"))
                .unwrap()
                .is_none()
        );
        assert!(!roots.state.join("contexts.json").exists());
    }
}

#[test]
fn closed_owner_cannot_publish_a_completed_job() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let mut app = application(&roots, PrivacyKind::Normal);
    let result =
        completed_context_reload(&app, &ContextsConfig::default(), configuration("default"));
    app.close_profile();
    assert!(matches!(
        app.apply_context_reload(result),
        Err(ContextReloadError::Stale)
    ));
    assert!(app.profile_snapshot().is_none());
}

#[test]
fn unrelated_storage_owner_cannot_adopt_the_result() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let other = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let source = application(&roots, PrivacyKind::Normal);
    let mut destination = application(&other, PrivacyKind::Normal);
    let identity = destination.profile_snapshot();
    let result = completed_context_reload(
        &source,
        &ContextsConfig::default(),
        configuration("default"),
    );
    assert!(matches!(
        destination.apply_context_reload(result),
        Err(ContextReloadError::Stale)
    ));
    assert_eq!(destination.profile_snapshot(), identity);
    assert_eq!(destination.contexts().unwrap(), []);
}

#[test]
fn in_memory_documents_use_the_canonical_route_validator() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let app = application(&roots, PrivacyKind::Normal);
    let before = app.configuration_snapshot();
    let mut next = configuration("default");
    next.routes.push(ContextRoute {
        id: "bad".into(),
        pattern: "https://example.test/*".into(),
        context: "missing".into(),
        priority: 0,
        behavior: "prompt".into(),
        entry_points: vec!["explicit-open".into()],
    });
    assert!(matches!(
        app.prepare_context_reload(&ContextsConfig::default(), next),
        Err(ContextReloadError::InvalidConfiguration(_))
    ));
    assert!(!roots.state.join("contexts.json").exists());
    assert_eq!(app.configuration_snapshot(), before);
}

#[test]
fn unavailable_normal_storage_is_not_reported_as_a_transient_success() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let app = application_with_storage(&roots, PrivacyKind::Normal, false);
    assert!(matches!(
        app.prepare_context_reload(&ContextsConfig::default(), configuration("default")),
        Err(ContextReloadError::ProfileUnavailable)
    ));
    assert!(!roots.state.join("contexts.json").exists());
}
