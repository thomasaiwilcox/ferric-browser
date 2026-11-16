use super::*;
use ferric_browser_config::{ContextsConfig, InteractionPolicyPatch, SiteRule};
use ferric_browser_core::{Event, ValidatedUrl};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

const URL: &str = "https://example.test/page";
const ANCHOR: &str =
    "[[anchors]]\nid='main'\ncode='m'\nfamilies=['links']\ntarget_selector='#main'";
const COLLECTION: &str = "[[collections]]\nid='list'\nfamilies=['links']\ncodes=['m']\ncontainer_self=true\nitem_selector='a.item'\nprimary_self=true\norder='dom'";

fn activation(name: &str, privacy: PrivacyKind) -> ProfileActivation {
    ProfileActivation {
        name: name.into(),
        id: None,
        privacy,
        roots: None,
        lock: None,
        contexts: None,
        storage_path: None,
    }
}

fn layers(conflict: bool) -> ConfigurationLayers {
    let rule = |id: &str, patch: &str| SiteRule {
        id: id.into(),
        pattern: "https://example.test/*".into(),
        priority: 0,
        set: BTreeMap::new(),
        interaction: Some(toml::from_str::<InteractionPolicyPatch>(patch).unwrap()),
    };
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.base.site_rules.push(rule("anchor", ANCHOR));
    if conflict {
        layers.base.site_rules.push(rule("collection", COLLECTION));
    }
    layers
}

fn application(privacy: PrivacyKind, conflict: bool) -> (BrowserApplication, WindowId) {
    let (mut app, window, _) =
        BrowserApplication::bootstrap(Config::default(), privacy, "old").unwrap();
    app.activate_profile(activation("old", privacy)).unwrap();
    app.activate_configuration(layers(conflict)).unwrap();
    let target = app.capture_target(app.active_tab().unwrap().id).unwrap();
    app.dispatch_runtime(RuntimeInput::Lifecycle(Event::RestoreTabAddress {
        target,
        url: ValidatedUrl::parse(URL).unwrap(),
    }))
    .unwrap();
    (app, window)
}

fn prepared(
    candidate: ConfigurationLayers,
    fallback: ConfigurationLayers,
    name: &str,
    privacy: PrivacyKind,
) -> PreparedProfile {
    PreparedProfile {
        durable: None,
        preparation_error: None,
        pending_policy_owner: None,
        layers: candidate,
        saved_runtime_overrides: None,
        fallback_layers: fallback,
        activation: activation(name, privacy),
        userscript_root: None,
        page_scripts: Ok(Arc::default()),
        config_sources: Vec::new(),
        session_path: None,
        session_state_root: None,
        recovery_paths: Vec::new(),
        storage_error: None,
        configuration_error: None,
        context_error: None,
    }
}

#[test]
fn direct_activation_and_replacement_reject_open_conflicts_without_touching_the_cache() {
    let (mut app, window) = application(PrivacyKind::Normal, false);
    let installed = app.refresh_interaction_snapshot(window, URL).unwrap();
    let before = app.configuration_snapshot();
    let sources = app.configuration_policy_sources();
    let profile = app.profile_snapshot();
    let count = app.interaction_compilation_count();
    assert!(app.activate_configuration(layers(true)).is_err());
    assert!(app.replace_configuration(layers(true).base).is_err());
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    assert_eq!(app.profile_snapshot(), profile);
    assert_eq!(app.interaction_compilation_count(), count);
    assert!(Arc::ptr_eq(
        &installed,
        &app.refresh_interaction_snapshot(window, URL).unwrap()
    ));
    app.replace_configuration(Config::default()).unwrap();
    assert!(
        app.configuration_layers()
            .base_authored
            .documents()
            .is_empty()
    );
    assert_eq!(
        app.refresh_interaction_snapshot(window, URL)
            .unwrap()
            .state(),
        InteractionCompilationState::NoOp
    );
}

#[test]
fn an_incoming_profile_cannot_reuse_an_outgoing_closed_conflict_exemption() {
    let (mut app, window) = application(PrivacyKind::Normal, true);
    let closed = app.refresh_interaction_snapshot(window, URL).unwrap();
    assert_eq!(closed.state(), InteractionCompilationState::Conflicted);
    let before = app.configuration_snapshot();
    let sources = app.configuration_policy_sources();
    let profile = app.profile_snapshot();
    let generation = app.next_profile_generation;
    let candidate = prepared(layers(true), layers(true), "next", PrivacyKind::Normal);
    assert!(app.install_prepared_profile(candidate).is_err());
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    assert_eq!(app.profile_snapshot(), profile);
    assert_eq!(app.next_profile_generation, generation);
    assert!(Arc::ptr_eq(
        &closed,
        &app.refresh_interaction_snapshot(window, URL).unwrap()
    ));
}

#[test]
fn incoming_conflict_uses_only_an_independently_validated_fallback() {
    let (mut app, window) = application(PrivacyKind::Normal, false);
    let old = app.profile_snapshot().unwrap();
    let mut rejected = layers(false);
    let collection = layers(true).base.site_rules.pop().unwrap();
    rejected.runtime.site_rules.push(collection);
    rejected
        .runtime
        .set_literal("content.zoom", "1.25")
        .unwrap();
    rejected.profile.set_literal("hints.chars", "'ab'").unwrap();
    let candidate = prepared(rejected, layers(false), "next", PrivacyKind::Normal);
    let effect = app.install_prepared_profile(candidate).unwrap();
    assert!(
        effect
            .configuration_error
            .unwrap()
            .contains("Interaction active policy conflicts")
    );
    assert_eq!(effect.profile.name, "next");
    assert!(effect.profile.generation > old.generation);
    assert_eq!(app.configuration_layers(), &layers(false));
    assert_eq!(effect.profile_overrides, app.configuration_layers().profile);
    assert_eq!(effect.runtime_overrides, app.configuration_layers().runtime);
    assert_eq!(effect.profile_overrides, RuntimeOverrides::default());
    assert_eq!(effect.runtime_overrides, RuntimeOverrides::default());
    assert_eq!(
        app.refresh_interaction_snapshot(window, URL)
            .unwrap()
            .state(),
        InteractionCompilationState::Ready
    );
}

#[test]
fn incoming_privacy_uses_its_own_scope_instead_of_the_outgoing_profile() {
    let (mut app, window) = application(PrivacyKind::Normal, false);
    let installed = app.refresh_interaction_snapshot(window, URL).unwrap();
    let before = app.configuration_snapshot();
    let profile = app.profile_snapshot();
    let candidate = prepared(layers(false), layers(false), "next", PrivacyKind::Private);
    let error = app
        .install_prepared_profile(candidate)
        .unwrap_err()
        .to_string();
    assert!(error.contains("associated runtime profile"), "{error}");
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.profile_snapshot(), profile);
    assert!(Arc::ptr_eq(
        &installed,
        &app.refresh_interaction_snapshot(window, URL).unwrap()
    ));
}

#[test]
fn profile_generation_exhaustion_rejects_before_installation_or_worker_admission() {
    let (mut app, _) = application(PrivacyKind::Private, false);
    let before = app.configuration_snapshot();
    let profile = app.profile_snapshot();
    app.next_profile_generation = u64::MAX;
    let input = ProfileOpenRequest {
        name: "next".into(),
        label: "Next".into(),
        privacy: PrivacyKind::Private,
        storage: ProfileStorage::Transient,
        base_configuration: Config::default(),
        base_authored: Arc::default(),
        initial_profile_overrides: RuntimeOverrides::default(),
        command_line_overrides: RuntimeOverrides::default(),
        config_path: None,
        script_catalog_root: None,
        contexts: ContextsConfig::default(),
        context_configuration_error: None,
    };
    assert!(
        app.request_profile_open(input)
            .unwrap_err()
            .to_string()
            .contains("exhausted")
    );
    assert!(app.profile_open.is_none());
    assert!(
        app.install_prepared_profile(prepared(
            layers(false),
            layers(false),
            "next",
            PrivacyKind::Private
        ))
        .unwrap_err()
        .to_string()
        .contains("exhausted")
    );
    assert!(
        app.activate_profile(activation("next", PrivacyKind::Private))
            .unwrap_err()
            .to_string()
            .contains("exhausted")
    );
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.profile_snapshot(), profile);
    assert_eq!(app.next_profile_generation, u64::MAX);
}

#[test]
fn real_profile_worker_rejection_preserves_the_previous_profile_and_configuration() {
    let (mut app, window) = application(PrivacyKind::Private, false);
    let before = app.configuration_snapshot();
    let sources = app.configuration_policy_sources();
    let profile = app.profile_snapshot();
    let generation = app.next_profile_generation;
    app.request_profile_open(ProfileOpenRequest {
        name: "next".into(),
        label: "Next".into(),
        privacy: PrivacyKind::Private,
        storage: ProfileStorage::Transient,
        base_configuration: layers(true).base,
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
    let result = loop {
        if let Some(result) = app.poll_profile_open() {
            break result;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    };
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Interaction active policy conflicts")
    );
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    assert_eq!(app.profile_snapshot(), profile);
    assert_eq!(app.next_profile_generation, generation);
    assert!(app.profile_open.is_none());
    assert_eq!(
        app.refresh_interaction_snapshot(window, URL)
            .unwrap()
            .state(),
        InteractionCompilationState::Ready
    );
}

#[test]
fn rejected_durable_profile_worker_preserves_profile_and_context_metadata() {
    use ferric_browser_storage::{ContextRegistry, ProfileRegistry, RootSpec, StorageRoots};

    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let base = roots.temporary_root().unwrap().to_owned();
    let mut registry = ProfileRegistry::open(&roots).unwrap();
    registry.get_or_create("kept", "Kept").unwrap();
    let mut contexts = ContextRegistry::open(&roots).unwrap();
    contexts
        .create("kept-context", "Kept context", "kept")
        .unwrap();
    let metadata_before = profile_metadata_snapshot(&roots);
    let (mut app, window) = application(PrivacyKind::Normal, false);
    let configuration_before = app.configuration_snapshot();
    let profile_before = app.profile_snapshot();
    let generation_before = app.next_profile_generation;
    let contexts = toml::from_str::<ContextsConfig>(
        "schema_version=3\n[[contexts]]\nname='rejected-context'\nlabel='Rejected context'\nprofile='rejected-next'\n",
    )
    .unwrap();
    ferric_browser_config::validate_contexts_configuration(&contexts).unwrap();
    app.request_profile_open(ProfileOpenRequest {
        name: "rejected-next".into(),
        label: "Rejected next".into(),
        privacy: PrivacyKind::Normal,
        storage: ProfileStorage::Durable(RootSpec::Base(base)),
        // Both the complete candidate and its captured fallback conflict in
        // an actual open Core context; parsing alone accepts these sources.
        base_configuration: layers(true).base,
        base_authored: Arc::default(),
        initial_profile_overrides: RuntimeOverrides::default(),
        command_line_overrides: RuntimeOverrides::default(),
        config_path: None,
        script_catalog_root: None,
        contexts,
        context_configuration_error: None,
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let result = loop {
        if let Some(result) = app.poll_profile_open() {
            break result;
        }
        assert!(Instant::now() < deadline, "profile worker did not finish");
        std::thread::sleep(Duration::from_millis(1));
    };
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("Interaction active policy conflicts"),
        "{error}"
    );
    assert_eq!(app.configuration_snapshot(), configuration_before);
    assert_eq!(app.profile_snapshot(), profile_before);
    assert_eq!(app.next_profile_generation, generation_before);
    assert!(app.profile_open.is_none());
    assert_eq!(
        app.refresh_interaction_snapshot(window, URL)
            .unwrap()
            .state(),
        InteractionCompilationState::Ready
    );
    assert_eq!(
        profile_metadata_snapshot(&roots),
        metadata_before,
        "rejected profile configuration must not publish profile/context metadata ({})",
        roots.state.display()
    );
    assert!(
        ProfileRegistry::open(&roots)
            .unwrap()
            .profiles()
            .iter()
            .all(|profile| profile.name != "rejected-next")
    );
    assert!(
        ContextRegistry::open(&roots)
            .unwrap()
            .contexts()
            .iter()
            .all(|context| context.name != "rejected-context")
    );
    drop(app);
    roots.cleanup().unwrap();
}

#[test]
fn durable_profile_worker_commits_an_independently_validated_fallback() {
    use ferric_browser_storage::{ProfileRegistry, RootSpec, StorageRoots};

    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let base = roots.temporary_root().unwrap().to_owned();
    let (mut app, window) = application(PrivacyKind::Normal, false);
    let fallback = layers(false);
    let mut saved_runtime = RuntimeOverrides::default();
    saved_runtime
        .site_rules
        .push(layers(true).base.site_rules.pop().unwrap());
    saved_runtime.set_literal("content.zoom", "1.25").unwrap();
    roots.ensure().unwrap();
    let runtime_path = roots.state.join("runtime-overrides.toml");
    let runtime_source = toml::to_string(&saved_runtime).unwrap();
    std::fs::write(&runtime_path, &runtime_source).unwrap();
    app.request_profile_open(ProfileOpenRequest {
        name: "accepted-fallback".into(),
        label: "Accepted fallback".into(),
        privacy: PrivacyKind::Normal,
        storage: ProfileStorage::Durable(RootSpec::Base(base)),
        base_configuration: fallback.base.clone(),
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
    let effect = loop {
        if let Some(result) = app.poll_profile_open() {
            break result.unwrap();
        }
        assert!(Instant::now() < deadline, "profile worker did not finish");
        std::thread::sleep(Duration::from_millis(1));
    };
    let error = effect.configuration_error.as_deref().unwrap();
    assert!(
        error.contains("Interaction active policy conflicts"),
        "{error}"
    );
    assert!(effect.storage_error.is_none(), "{:?}", effect.storage_error);
    assert!(effect.profile.durable);
    assert_eq!(effect.profile.name, "accepted-fallback");
    assert_eq!(app.configuration_layers(), &fallback);
    assert_eq!(effect.profile_overrides, fallback.profile);
    assert_eq!(effect.runtime_overrides, fallback.runtime);
    assert_eq!(
        std::fs::read_to_string(runtime_path).unwrap(),
        runtime_source
    );
    assert_eq!(
        app.refresh_interaction_snapshot(window, URL)
            .unwrap()
            .state(),
        InteractionCompilationState::Ready
    );
    let registry = ProfileRegistry::open(&roots).unwrap();
    let profile = registry
        .profiles()
        .iter()
        .find(|profile| profile.name == "accepted-fallback")
        .unwrap();
    assert_eq!(effect.profile.id, Some(profile.id));
    assert!(
        roots
            .data
            .join("profiles")
            .join(profile.id.to_string())
            .join("browser.sqlite")
            .is_file()
    );
    app.close_profile();
    drop(app);
    roots.cleanup().unwrap();
}

pub(super) fn profile_metadata_snapshot(
    roots: &ferric_browser_storage::StorageRoots,
) -> [(Vec<u8>, std::time::SystemTime, Option<u64>); 2] {
    ["profiles.json", "contexts.json"].map(|name| {
        let path = roots.state.join(name);
        let metadata = std::fs::metadata(&path).unwrap();
        #[cfg(unix)]
        let inode = {
            use std::os::unix::fs::MetadataExt;
            Some(metadata.ino())
        };
        #[cfg(not(unix))]
        let inode = None;
        (
            std::fs::read(path).unwrap(),
            metadata.modified().unwrap(),
            inode,
        )
    })
}

pub(super) fn staged_profile_fixture() -> (
    BrowserApplication,
    WindowId,
    ferric_browser_storage::StorageRoots,
) {
    use ferric_browser_storage::{ContextRegistry, ProfileRegistry, RootSpec, StorageRoots};
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    ProfileRegistry::open(&roots)
        .unwrap()
        .get_or_create("kept", "Kept")
        .unwrap();
    ContextRegistry::open(&roots)
        .unwrap()
        .create("kept-context", "Kept context", "kept")
        .unwrap();
    let (app, window) = application(PrivacyKind::Normal, false);
    (app, window, roots)
}

pub(super) fn staged_profile_candidate(
    app: &mut BrowserApplication,
    roots: &ferric_browser_storage::StorageRoots,
) -> ProfileOpenCandidate {
    app.request_profile_open_for_validation(ProfileOpenRequest {
        name: "staged".into(),
        label: "Staged".into(),
        privacy: PrivacyKind::Normal,
        storage: ProfileStorage::Durable(ferric_browser_storage::RootSpec::Base(
            roots.temporary_root().unwrap().to_owned(),
        )),
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
        assert!(
            app.poll_profile_open().is_none(),
            "automatic polling must not admit external validation"
        );
        if let Some(candidate) = app.poll_profile_candidate() {
            return candidate.unwrap();
        }
        assert!(Instant::now() < deadline, "profile candidate timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn finish_staged_profile(app: &mut BrowserApplication) -> ProfileOpenEffect {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(result) = app.poll_profile_open() {
            return result.unwrap();
        }
        assert!(Instant::now() < deadline, "profile commit timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn staged_engine_fallback_retains_the_exact_saved_runtime_as_a_repair_source() {
    let (mut app, _, roots) = staged_profile_fixture();
    let mut saved = RuntimeOverrides::default();
    saved.set_literal("content.zoom", "1.25").unwrap();
    saved
        .set_literal("hints.selectors.links", "{mode='replace',selectors=['a[']}")
        .unwrap();
    saved.site_rules = layers(false).base.site_rules;
    let path = roots.state.join("runtime-overrides.toml");
    let bytes = toml::to_string(&saved).unwrap().into_bytes();
    std::fs::write(&path, &bytes).unwrap();
    let before = std::fs::metadata(&path).unwrap();
    let candidate = staged_profile_candidate(&mut app, &roots);
    assert_eq!(candidate.layers().runtime, saved);
    let candidate = app
        .prepare_profile_candidate_fallback(candidate, "Engine CSS rejection".into())
        .unwrap();
    assert_eq!(candidate.layers().runtime, RuntimeOverrides::default());
    app.admit_profile_candidate(candidate).unwrap();
    let effect = finish_staged_profile(&mut app);
    assert!(effect.profile.durable);
    assert_eq!(effect.runtime_overrides, RuntimeOverrides::default());
    assert_eq!(
        app.configuration_layers().runtime,
        RuntimeOverrides::default()
    );
    assert!((app.configuration().content.zoom - 1.0).abs() < f64::EPSILON);
    assert_eq!(
        app.capture_saved_runtime_edits().unwrap().layers().runtime,
        saved
    );
    let mut temporary = app.configuration_layers().clone();
    temporary
        .temporary
        .set_literal("content.zoom", "1.5")
        .unwrap();
    assert!(app.prepare_configuration_update(temporary).is_err());
    assert!((app.configuration().content.zoom - 1.0).abs() < f64::EPSILON);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let after = std::fs::metadata(&path).unwrap();
    assert_eq!(after.modified().unwrap(), before.modified().unwrap());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(after.dev(), before.dev());
        assert_eq!(after.ino(), before.ino());
    }
    app.close_profile();
    drop(app);
    roots.cleanup().unwrap();
}

#[test]
fn external_profile_validation_holds_all_metadata_until_exact_admission() {
    let (mut app, _, roots) = staged_profile_fixture();
    let metadata = profile_metadata_snapshot(&roots);
    let active = app.profile_snapshot();
    let candidate = staged_profile_candidate(&mut app, &roots);
    let capture = candidate.validation_capture();
    for _ in 0..16 {
        assert!(app.poll_profile_open().is_none());
    }
    assert_eq!(profile_metadata_snapshot(&roots), metadata);
    assert_eq!(app.profile_snapshot(), active);
    assert!(!app.profile_open_validation_is_current(&capture));
    app.admit_profile_candidate(candidate).unwrap();
    let effect = finish_staged_profile(&mut app);
    assert!(effect.profile.durable);
    assert_eq!(effect.profile.name, "staged");
    assert!(app.profile_open_validation_is_current(&capture));
    assert!(
        ferric_browser_storage::ProfileRegistry::open(&roots)
            .unwrap()
            .profiles()
            .iter()
            .any(|profile| Some(profile.id) == effect.profile.id)
    );
    app.close_profile();
    assert!(!app.profile_open_validation_is_current(&capture));
    drop(app);
    roots.cleanup().unwrap();
}

#[test]
fn dropping_external_profile_candidate_cannot_commit_metadata() {
    let (mut app, _, roots) = staged_profile_fixture();
    let metadata = profile_metadata_snapshot(&roots);
    let active = app.profile_snapshot();
    let candidate = staged_profile_candidate(&mut app, &roots);
    drop(candidate);
    assert!(app.poll_profile_open().unwrap().is_err());
    assert!(app.profile_open.is_none());
    assert_eq!(app.profile_snapshot(), active);
    assert_eq!(profile_metadata_snapshot(&roots), metadata);
    drop(app);
    roots.cleanup().unwrap();
}

#[test]
fn profile_candidate_rejects_open_context_aba_before_metadata_admission() {
    let (mut app, window, roots) = staged_profile_fixture();
    let metadata = profile_metadata_snapshot(&roots);
    let candidate = staged_profile_candidate(&mut app, &roots);
    let tab = app.windows()[&window].active_tab.unwrap();
    let target = app.capture_target(tab).unwrap();
    app.dispatch_runtime(RuntimeInput::Lifecycle(Event::RestoreTabAddress {
        target,
        url: ValidatedUrl::parse("https://example.test/ignored-restore").unwrap(),
    }))
    .unwrap();
    assert_eq!(app.tabs()[&tab].url.as_deref(), Some(URL));
    assert!(app.profile_candidate_is_current(&candidate));
    for url in ["https://example.test/changed", URL] {
        let target = app
            .capture_target(app.windows()[&window].active_tab.unwrap())
            .unwrap();
        app.dispatch_runtime(RuntimeInput::EngineFact(Event::SameDocumentNavigation {
            target,
            url: ValidatedUrl::parse(url).unwrap(),
        }))
        .unwrap();
        assert_eq!(app.tabs()[&tab].url.as_deref(), Some(url));
        assert!(!app.profile_candidate_is_current(&candidate));
    }
    assert!(!app.profile_candidate_is_current(&candidate));
    assert!(app.admit_profile_candidate(candidate).is_err());
    assert!(app.profile_open.is_none());
    assert_eq!(profile_metadata_snapshot(&roots), metadata);
    drop(app);
    roots.cleanup().unwrap();
}

#[test]
fn foreign_profile_candidate_cannot_cancel_another_owners_pending_open() {
    let (mut first, _, first_roots) = staged_profile_fixture();
    let (mut second, _, second_roots) = staged_profile_fixture();
    let foreign = staged_profile_candidate(&mut first, &first_roots);
    let own = staged_profile_candidate(&mut second, &second_roots);
    assert!(second.admit_profile_candidate(foreign).is_err());
    assert!(second.profile_candidate_is_current(&own));
    assert!(first.poll_profile_open().unwrap().is_err());
    second.admit_profile_candidate(own).unwrap();
    assert!(finish_staged_profile(&mut second).profile.durable);
    drop(first);
    second.close_profile();
    drop(second);
    first_roots.cleanup().unwrap();
    second_roots.cleanup().unwrap();
}

#[test]
fn explicit_invalid_profile_temporary_replacement_rejects_without_fallback_or_metadata() {
    let (mut app, _, roots) = staged_profile_fixture();
    let metadata = profile_metadata_snapshot(&roots);
    let configuration = app.configuration_snapshot();
    let candidate = staged_profile_candidate(&mut app, &roots);
    let mut temporary = RuntimeOverrides::default();
    temporary
        .settings
        .insert("content.zoom".into(), toml::Value::Float(99.0));
    assert!(
        app.prepare_profile_candidate_temporary(candidate, temporary)
            .is_err()
    );
    assert!(app.profile_open.is_none());
    assert_eq!(app.configuration_snapshot(), configuration);
    assert_eq!(profile_metadata_snapshot(&roots), metadata);
    drop(app);
    roots.cleanup().unwrap();
}

#[test]
fn replacing_profile_candidate_retires_old_evidence_before_admission() {
    let (mut app, _, roots) = staged_profile_fixture();
    let metadata = profile_metadata_snapshot(&roots);
    let candidate = staged_profile_candidate(&mut app, &roots);
    let old_capture = candidate.validation_capture();
    let mut temporary = RuntimeOverrides::default();
    temporary.set_literal("content.zoom", "1.25").unwrap();
    let candidate = app
        .prepare_profile_candidate_temporary(candidate, temporary.clone())
        .unwrap();
    let current_capture = candidate.validation_capture();
    assert_eq!(candidate.layers().temporary, temporary);
    assert_eq!(profile_metadata_snapshot(&roots), metadata);
    app.admit_profile_candidate(candidate).unwrap();
    finish_staged_profile(&mut app);
    assert!(!app.profile_open_validation_is_current(&old_capture));
    assert!(app.profile_open_validation_is_current(&current_capture));
    assert_eq!(app.configuration_layers().temporary, temporary);
    app.close_profile();
    drop(app);
    roots.cleanup().unwrap();
}

#[test]
fn profile_candidate_rejects_real_peer_context_aba_before_any_metadata_commit() {
    let (mut app, _, roots) = staged_profile_fixture();
    let candidate = staged_profile_candidate(&mut app, &roots);
    let mut peer = ferric_browser_storage::ContextRegistry::open(&roots).unwrap();
    peer.create("peer", "Peer", "kept").unwrap();
    peer.remove("peer").unwrap();
    let peer_metadata = profile_metadata_snapshot(&roots);
    assert!(app.admit_profile_candidate(candidate).is_err());
    assert_eq!(profile_metadata_snapshot(&roots), peer_metadata);
    assert_eq!(app.profile_snapshot().unwrap().name, "old");
    drop(app);
    roots.cleanup().unwrap();
}
