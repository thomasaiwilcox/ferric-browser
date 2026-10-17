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
    let result = std::thread::spawn(move || job.run())
        .join()
        .unwrap()
        .unwrap();
    assert!(app.contexts().unwrap().is_empty());
    assert_eq!(
        ContextRegistry::open(&roots).unwrap().contexts()[0].label,
        "Work"
    );
    assert_eq!(app.apply_context_reload(result).unwrap(), next);
    assert_eq!(app.contexts().unwrap()[0].label, "Work");
    assert_eq!(app.profile_snapshot(), identity);
}

#[test]
fn unknown_profiles_are_rejected_without_creating_metadata_or_profiles() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let app = application(&roots, PrivacyKind::Normal);
    let job = app
        .prepare_context_reload(&ContextsConfig::default(), configuration("missing"))
        .unwrap()
        .unwrap();
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
    let result = app
        .prepare_context_reload(&ContextsConfig::default(), configuration("default"))
        .unwrap()
        .unwrap()
        .run()
        .unwrap();
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
    let result = source
        .prepare_context_reload(&ContextsConfig::default(), configuration("default"))
        .unwrap()
        .unwrap()
        .run()
        .unwrap();
    assert!(matches!(
        destination.apply_context_reload(result),
        Err(ContextReloadError::Stale)
    ));
    assert_eq!(destination.profile_snapshot(), identity);
    assert!(destination.contexts().unwrap().is_empty());
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
