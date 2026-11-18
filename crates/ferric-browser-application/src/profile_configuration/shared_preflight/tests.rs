use super::*;
use crate::ProfileActivation;
use ferric_browser_config::{Config, ContextsConfig, SiteRule};
use ferric_browser_core::{
    Event, IdSource, PrivacyKind, SharedIdAllocator, ValidatedUrl, WindowId,
};
use ferric_browser_runtime::{BrowserRuntime, RuntimeInput};
use ferric_browser_storage::{ContextRegistry, ProfileRegistry, RootSpec, StorageRoots};
use std::{cell::Cell, collections::BTreeMap};

const URL: &str = "https://example.test/page";
const ANCHOR: &str =
    "[[anchors]]\nid='main'\ncode='m'\nfamilies=['links']\ntarget_selector='#main'";
const COLLECTION: &str = "[[collections]]\nid='list'\nfamilies=['links']\ncodes=['m']\ncontainer_self=true\nitem_selector='a.item'\nprimary_self=true\norder='dom'";

fn rule(id: &str, patch: &str) -> SiteRule {
    SiteRule {
        id: id.into(),
        pattern: "https://example.test/*".into(),
        priority: 0,
        set: BTreeMap::new(),
        interaction: Some(toml::from_str(patch).unwrap()),
    }
}

fn fixture() -> (BrowserApplication, BrowserApplication, WindowId) {
    let ids = SharedIdAllocator::default();
    let (runtime, _, _) = BrowserRuntime::bootstrap_with_ids(
        PrivacyKind::Private,
        "work",
        IdSource::from_shared(ids.clone()),
    )
    .unwrap();
    let mut source = BrowserApplication::from_runtime(runtime, Config::default());
    source
        .activate_profile(ProfileActivation {
            name: "work".into(),
            id: None,
            privacy: PrivacyKind::Private,
            roots: None,
            lock: None,
            contexts: None,
            storage_path: None,
        })
        .unwrap();
    let (runtime, window, _) = BrowserRuntime::bootstrap_with_ids(
        PrivacyKind::Private,
        "work",
        IdSource::from_shared(ids),
    )
    .unwrap();
    let peer = source.inherit_profile(runtime).unwrap();
    (source, peer, window)
}

fn known_page(peer: &mut BrowserApplication, window: WindowId) {
    let tab = peer.windows()[&window].active_tab.unwrap();
    peer.dispatch_runtime(RuntimeInput::Lifecycle(Event::RestoreTabAddress {
        target: peer.capture_target(tab).unwrap(),
        url: ValidatedUrl::parse(URL).unwrap(),
    }))
    .unwrap();
    assert_eq!(peer.tabs()[&tab].url.as_deref(), Some(URL));
}

fn own_anchor(peer: &mut BrowserApplication) {
    let mut layers = peer.configuration_layers().clone();
    layers.base.site_rules.push(rule("anchor", ANCHOR));
    peer.activate_configuration(layers).unwrap();
}

fn collection_candidate(source: &BrowserApplication) -> PreparedConfigurationUpdate {
    let mut layers = source.configuration_layers().clone();
    layers
        .runtime
        .site_rules
        .push(rule("collection", COLLECTION));
    source.prepare_configuration_update(layers).unwrap()
}

fn zoom_candidate(source: &BrowserApplication) -> PreparedConfigurationUpdate {
    let mut layers = source.configuration_layers().clone();
    layers.runtime.set_literal("content.zoom", "1.25").unwrap();
    source.prepare_configuration_update(layers).unwrap()
}

fn group(source: &BrowserApplication) -> Arc<SharedConfiguration> {
    Arc::clone(
        &source
            .active_profile
            .as_ref()
            .unwrap()
            .session
            .0
            .configuration,
    )
}

#[test]
fn sharing_peers_use_their_own_layers_and_actual_open_pages_before_publication() {
    let (mut source, mut peer, window) = fixture();
    own_anchor(&mut peer);
    known_page(&mut peer, window);
    let before = source.configuration_snapshot();
    let revision = source.profile_configuration_revision().unwrap();
    let prepared = collection_candidate(&source);
    let error = source
        .validate_configuration_interaction_contexts(&prepared)
        .unwrap_err()
        .to_string();
    assert!(error.contains("prospective reload"), "{error}");
    let called = Cell::new(false);
    let error = source
        .publish_configuration_update(prepared, || {
            called.set(true);
            Ok(())
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("conflicts for open tab"), "{error}");
    assert!(!called.get());
    assert_eq!(source.configuration_snapshot(), before);
    assert_eq!(source.profile_configuration_revision().unwrap(), revision);
}

#[test]
fn a_context_opened_after_capture_invalidates_the_witness_and_is_revalidated() {
    let (mut source, mut peer, window) = fixture();
    own_anchor(&mut peer);
    let prepared = collection_candidate(&source);
    let captured = source
        .preflight_shared_policy_contexts(&prepared)
        .unwrap()
        .unwrap();
    known_page(&mut peer, window);
    assert!(captured.check_current(&group(&source)).is_err());
    assert!(
        source
            .publish_configuration_update(prepared, || Ok(()))
            .is_err()
    );
}

#[test]
fn context_and_source_aba_cannot_revive_a_captured_preflight() {
    let (source, mut peer, window) = fixture();
    known_page(&mut peer, window);
    let prepared = zoom_candidate(&source);
    let captured = source
        .preflight_shared_policy_contexts(&prepared)
        .unwrap()
        .unwrap();
    let tab = peer.windows()[&window].active_tab.unwrap();
    let target = peer.capture_target(tab).unwrap();
    for address in ["https://example.test/other", URL] {
        peer.dispatch_runtime(RuntimeInput::EngineFact(Event::SameDocumentNavigation {
            target,
            url: ValidatedUrl::parse(address).unwrap(),
        }))
        .unwrap();
    }
    assert!(captured.check_current(&group(&source)).is_err());
    let captured = source
        .preflight_shared_policy_contexts(&prepared)
        .unwrap()
        .unwrap();
    let original = peer.configuration_layers().clone();
    let mut changed = original.clone();
    changed
        .temporary
        .set_literal("content.zoom", "1.5")
        .unwrap();
    peer.activate_configuration(changed).unwrap();
    peer.activate_configuration(original).unwrap();
    assert!(captured.check_current(&group(&source)).is_err());
}

#[test]
fn stale_navigation_facts_and_focus_noops_do_not_invalidate_context_inputs() {
    let (source, mut peer, window) = fixture();
    known_page(&mut peer, window);
    let prepared = zoom_candidate(&source);
    let captured = source
        .preflight_shared_policy_contexts(&prepared)
        .unwrap()
        .unwrap();
    let tab = peer.windows()[&window].active_tab.unwrap();
    let mut stale = peer.capture_target(tab).unwrap();
    stale.generation += 1;
    let effects = peer
        .dispatch_runtime(RuntimeInput::EngineFact(Event::SameDocumentNavigation {
            target: stale,
            url: ValidatedUrl::parse("https://other.test/").unwrap(),
        }))
        .unwrap();
    assert!(effects.iter().any(|effect| matches!(
        effect,
        ferric_browser_runtime::RuntimeEffect::AppliedEvent(Event::SameDocumentNavigation { .. })
    )));
    peer.dispatch_runtime(RuntimeInput::Lifecycle(Event::FocusWindow { window }))
        .unwrap();
    peer.dispatch_runtime(RuntimeInput::Lifecycle(Event::SameDocumentNavigation {
        target: peer.capture_target(tab).unwrap(),
        url: ValidatedUrl::parse(URL).unwrap(),
    }))
    .unwrap();
    assert_eq!(peer.tabs()[&tab].url.as_deref(), Some(URL));
    captured.check_current(&group(&source)).unwrap();
}

#[test]
fn preflight_does_not_keep_a_closed_owner_or_its_shared_authority_alive() {
    let (source, peer, _) = fixture();
    let captured = source
        .preflight_shared_policy_contexts(&zoom_candidate(&source))
        .unwrap()
        .unwrap();
    let authority = Arc::downgrade(&group(&source));
    drop(peer);
    assert!(captured.check_current(&group(&source)).is_err());
    let fresh = source
        .preflight_shared_policy_contexts(&zoom_candidate(&source))
        .unwrap()
        .unwrap();
    fresh.check_current(&group(&source)).unwrap();
    drop(source);
    assert!(authority.upgrade().is_none());
}

#[test]
fn inherited_owner_membership_invalidates_an_older_capture() {
    let (source, peer, _) = fixture();
    let captured = source
        .preflight_shared_policy_contexts(&zoom_candidate(&source))
        .unwrap()
        .unwrap();
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Private, "work").unwrap();
    let newcomer = peer.inherit_profile(runtime).unwrap();
    assert!(captured.check_current(&group(&source)).is_err());
    drop(newcomer);
    // Returning to the same live owner set cannot revive a pre-join witness.
    assert!(captured.check_current(&group(&source)).is_err());
}

#[test]
fn retiring_a_retained_preparing_owner_unblocks_peers_and_invalidates_old_membership() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let ids = SharedIdAllocator::default();
    let (mut source, _) = durable_owner(&roots, "default", ids.clone());
    let (mut peer, _) = durable_owner(&roots, "other", ids);
    let original_profile = peer.profile_snapshot().unwrap();
    let backend = peer
        .active_profile
        .as_ref()
        .unwrap()
        .storage_path()
        .unwrap()
        .to_owned();
    let authority = group(&peer);
    let captured = source
        .preflight_shared_policy_contexts(&zoom_candidate(&source))
        .unwrap()
        .unwrap();
    peer.set_policy_owner_preparing(true);
    assert!(
        source
            .preflight_shared_policy_contexts(&zoom_candidate(&source))
            .is_err()
    );
    let membership = Arc::clone(&authority.owners.lock().unwrap().membership);

    peer.retire_shared_policy_observer();
    assert!(peer.shared_policy_owner.is_none());
    let retired_membership = Arc::clone(&authority.owners.lock().unwrap().membership);
    assert!(!Arc::ptr_eq(&membership, &retired_membership));
    peer.retire_shared_policy_observer();
    assert!(Arc::ptr_eq(
        &retired_membership,
        &authority.owners.lock().unwrap().membership
    ));
    assert_eq!(peer.profile_snapshot().unwrap(), original_profile);
    assert_eq!(
        peer.active_profile.as_ref().unwrap().storage_path(),
        Some(backend.as_path())
    );
    assert!(Arc::ptr_eq(&authority, &group(&peer)));
    assert!(captured.check_current(&group(&source)).is_err());
    source
        .publish_configuration_update(zoom_candidate(&source), || Ok(()))
        .unwrap();
    assert_eq!(
        source.configuration().content.zoom.to_bits(),
        1.25_f64.to_bits()
    );
}

fn durable_owner(
    roots: &StorageRoots,
    name: &str,
    ids: SharedIdAllocator,
) -> (BrowserApplication, WindowId) {
    let id = ProfileRegistry::open(roots)
        .unwrap()
        .get_or_create(name, name)
        .unwrap()
        .id;
    let (runtime, window, _) =
        BrowserRuntime::bootstrap_with_ids(PrivacyKind::Normal, name, IdSource::from_shared(ids))
            .unwrap();
    let mut app = BrowserApplication::from_runtime(runtime, Config::default());
    app.activate_profile(ProfileActivation {
        name: name.into(),
        id: Some(id),
        privacy: PrivacyKind::Normal,
        roots: Some(roots.clone()),
        lock: None,
        contexts: Some(ContextRegistry::open(roots).unwrap()),
        storage_path: Some(roots.data.join(format!("{id}-browser.sqlite"))),
    })
    .unwrap();
    (app, window)
}

#[test]
fn distinct_named_profiles_share_root_validation_before_writer_or_context_io() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let ids = SharedIdAllocator::default();
    let (mut source, _) = durable_owner(&roots, "default", ids.clone());
    let (mut peer, window) = durable_owner(&roots, "other", ids);
    assert!(Arc::ptr_eq(&group(&source), &group(&peer)));
    assert_ne!(source.profile_instance_id(), peer.profile_instance_id());
    own_anchor(&mut peer);
    known_page(&mut peer, window);
    let path = roots.state.join("runtime-overrides.toml");
    assert!(!path.exists());
    let prepared = collection_candidate(&source);
    let error = source
        .request_configuration_write(prepared)
        .unwrap_err()
        .to_string();
    assert!(error.contains("conflicts for open tab"), "{error}");
    assert!(!source.configuration_write_pending());
    assert!(!path.exists());
    let prepared = collection_candidate(&source);
    let job = source
        .prepare_context_reload(&ContextsConfig::default(), ContextsConfig::default())
        .unwrap()
        .unwrap();
    let error = source
        .admit_context_reload(&prepared, job)
        .unwrap_err()
        .to_string();
    assert!(error.contains("conflicts for open tab"), "{error}");
    assert!(source.reserve_configuration_reload().is_ok());
    assert!(!path.exists());
}

#[test]
fn independent_private_groups_are_not_joined_by_profile_name() {
    let (mut source, _, _) = fixture();
    let (_, mut unrelated, window) = fixture();
    own_anchor(&mut unrelated);
    known_page(&mut unrelated, window);
    assert!(!Arc::ptr_eq(&group(&source), &group(&unrelated)));
    source
        .publish_configuration_update(collection_candidate(&source), || Ok(()))
        .unwrap();
}

#[test]
fn accepted_shutdown_retires_membership_before_another_admission() {
    let (source, mut peer, _) = fixture();
    let captured = source
        .preflight_shared_policy_contexts(&zoom_candidate(&source))
        .unwrap()
        .unwrap();
    peer.dispatch_runtime(RuntimeInput::Lifecycle(Event::RequestShutdown))
        .unwrap();
    assert!(captured.check_current(&group(&source)).is_err());
    assert!(peer.shared_policy_owner.is_none());
}

#[test]
fn pending_profile_intent_cannot_supply_ready_peer_validation() {
    let (source, mut peer, _) = fixture();
    let captured = source
        .preflight_shared_policy_contexts(&zoom_candidate(&source))
        .unwrap()
        .unwrap();
    peer.request_profile_open(crate::ProfileOpenRequest {
        name: "pending".into(),
        label: "Pending".into(),
        privacy: PrivacyKind::Private,
        storage: crate::ProfileStorage::Transient,
        base_configuration: Config::default(),
        base_authored: Arc::default(),
        initial_profile_overrides: ferric_browser_config::RuntimeOverrides::default(),
        command_line_overrides: ferric_browser_config::RuntimeOverrides::default(),
        config_path: None,
        script_catalog_root: None,
        contexts: ContextsConfig::default(),
        context_configuration_error: None,
    })
    .unwrap();
    assert!(captured.check_current(&group(&source)).is_err());
    let error = match source.preflight_shared_policy_contexts(&zoom_candidate(&source)) {
        Ok(_) => panic!("a pending peer cannot qualify current ready inputs"),
        Err(error) => error.to_string(),
    };
    assert!(error.contains("opening"), "{error}");
}

#[test]
fn an_in_progress_mutation_closes_admission_without_invalidating_a_noop() {
    let (source, peer, _) = fixture();
    let prepared = zoom_candidate(&source);
    let captured = source
        .preflight_shared_policy_contexts(&prepared)
        .unwrap()
        .unwrap();
    let guard = peer.begin_shared_policy_mutation().unwrap();
    assert!(captured.check_current(&group(&source)).is_err());
    assert!(source.preflight_shared_policy_contexts(&prepared).is_err());
    BrowserApplication::finish_shared_source_mutation(Some(guard));
    captured.check_current(&group(&source)).unwrap();
}

#[test]
fn runtime_error_wakes_its_owner_and_stays_closed_until_cold_repair() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (source, mut peer, _) = fixture();
    let count = Arc::new(AtomicUsize::new(0));
    let wake_count = Arc::clone(&count);
    peer.set_storage_waker(Arc::new(move || {
        wake_count.fetch_add(1, Ordering::Relaxed);
    }));
    let prepared = zoom_candidate(&source);
    let captured = source
        .preflight_shared_policy_contexts(&prepared)
        .unwrap()
        .unwrap();
    let window = source.active_window().unwrap();
    let foreign = source.windows()[&window].active_tab.unwrap();
    assert!(
        peer.dispatch_runtime(RuntimeInput::Lifecycle(Event::CloseTab { tab: foreign }))
            .is_err()
    );
    assert!(peer.shared_policy_contexts_pending());
    assert!(count.load(Ordering::Relaxed) > 0);
    assert!(captured.check_current(&group(&source)).is_err());
    assert!(source.preflight_shared_policy_contexts(&prepared).is_err());
    peer.prepare_shared_policy_contexts();
    assert!(!peer.shared_policy_contexts_pending());
    // Repair is a new capture even when an early error left Core unchanged.
    assert!(captured.check_current(&group(&source)).is_err());
    source
        .preflight_shared_policy_contexts(&prepared)
        .unwrap()
        .unwrap()
        .check_current(&group(&source))
        .unwrap();
}

#[test]
fn a_partial_core_mutation_cannot_reopen_with_the_pre_error_facts() {
    let (source, mut peer, window) = fixture();
    own_anchor(&mut peer);
    let prepared = collection_candidate(&source);
    let guard = peer.begin_shared_policy_mutation().unwrap();
    let tab = peer.windows()[&window].active_tab.unwrap();
    // Inject an actual earlier accepted Core event under the same production
    // guard, then simulate the dispatcher's later error/unwinding boundary.
    peer.runtime
        .handle(RuntimeInput::Lifecycle(Event::RestoreTabAddress {
            target: peer.capture_target(tab).unwrap(),
            url: ValidatedUrl::parse(URL).unwrap(),
        }))
        .unwrap();
    drop(guard);
    assert!(peer.shared_policy_contexts_pending());
    assert!(source.preflight_shared_policy_contexts(&prepared).is_err());
    peer.prepare_shared_policy_contexts();
    let error = match source.preflight_shared_policy_contexts(&prepared) {
        Ok(_) => panic!("new conflicting context must be included after repair"),
        Err(error) => error.to_string(),
    };
    assert!(error.contains("conflicts for open tab"), "{error}");
}

#[test]
fn explicit_adapter_preparation_and_resolved_worker_roots_block_new_admission() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let (source, _) = durable_owner(&roots, "default", SharedIdAllocator::default());
    let prepared = zoom_candidate(&source);
    let captured = source
        .preflight_shared_policy_contexts(&prepared)
        .unwrap()
        .unwrap();
    let pending = super::PendingRootOwner::for_root(&roots.state);
    assert!(captured.check_current(&group(&source)).is_err());
    assert!(source.preflight_shared_policy_contexts(&prepared).is_err());
    drop(pending);
    assert!(captured.check_current(&group(&source)).is_err());
    source
        .preflight_shared_policy_contexts(&prepared)
        .unwrap()
        .unwrap()
        .check_current(&group(&source))
        .unwrap();
    let (_, mut peer, _) = fixture();
    peer.set_policy_owner_preparing(true);
    assert!(
        peer.shared_policy_owner
            .as_ref()
            .unwrap()
            .state
            .lock()
            .unwrap()
            .inputs
            .pending
    );
    peer.set_policy_owner_preparing(false);
    assert!(
        !peer
            .shared_policy_owner
            .as_ref()
            .unwrap()
            .state
            .lock()
            .unwrap()
            .inputs
            .pending
    );
}

#[test]
fn memory_publication_wakes_only_actual_sharing_owners_outside_locks() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (mut source, mut peer, _) = fixture();
    let (_, mut unrelated, _) = fixture();
    let count = Arc::new(AtomicUsize::new(0));
    let peer_count = Arc::clone(&count);
    let authority = Arc::downgrade(&group(&source));
    peer.set_storage_waker(Arc::new(move || {
        let shared = authority.upgrade().unwrap();
        // This would deadlock if publication invoked callbacks under either lock.
        let _state = shared.state.lock().unwrap();
        let _owners = shared.owners.lock().unwrap();
        peer_count.fetch_add(1, Ordering::Relaxed);
    }));
    let unrelated_count = Arc::new(AtomicUsize::new(0));
    let unrelated_wake = Arc::clone(&unrelated_count);
    unrelated.set_storage_waker(Arc::new(move || {
        unrelated_wake.fetch_add(1, Ordering::Relaxed);
    }));
    source
        .publish_configuration_update(zoom_candidate(&source), || Ok(()))
        .unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 1);
    assert_eq!(unrelated_count.load(Ordering::Relaxed), 0);
    let prepared = peer
        .prepare_committed_peer_configuration()
        .unwrap()
        .unwrap();
    peer.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 1);
}

fn worker_request(roots: &StorageRoots, name: &str) -> crate::ProfileOpenRequest {
    crate::ProfileOpenRequest {
        name: name.into(),
        label: name.into(),
        privacy: PrivacyKind::Normal,
        storage: crate::ProfileStorage::Durable(RootSpec::Base(
            roots.temporary_root().unwrap().to_owned(),
        )),
        base_configuration: Config::default(),
        base_authored: Arc::default(),
        initial_profile_overrides: ferric_browser_config::RuntimeOverrides::default(),
        command_line_overrides: ferric_browser_config::RuntimeOverrides::default(),
        config_path: None,
        script_catalog_root: None,
        contexts: ContextsConfig::default(),
        context_configuration_error: None,
    }
}

fn worker_result(worker: &mut crate::ProfileOpenCoordinator) -> crate::PreparedProfile {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Some(result) = worker.poll() {
            return result.unwrap();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "profile worker timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn actual_durable_worker_retains_pending_membership_until_installed_sources_are_ready() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let ids = SharedIdAllocator::default();
    let (source, _) = durable_owner(&roots, "default", ids.clone());
    let candidate = zoom_candidate(&source);
    let old = source
        .preflight_shared_policy_contexts(&candidate)
        .unwrap()
        .unwrap();
    let mut worker = crate::ProfileOpenCoordinator::spawn().unwrap();
    worker
        .submit(worker_request(&roots, "worker-profile"))
        .unwrap();
    let prepared = worker_result(&mut worker);
    assert!(prepared.storage_error.is_none());
    assert!(prepared.pending_policy_owner.is_some());
    drop(worker);
    assert!(old.check_current(&group(&source)).is_err());
    assert!(source.preflight_shared_policy_contexts(&candidate).is_err());
    let (runtime, _, _) = BrowserRuntime::bootstrap_with_ids(
        PrivacyKind::Normal,
        "worker-profile",
        IdSource::from_shared(ids),
    )
    .unwrap();
    let mut incoming = BrowserApplication::from_runtime(runtime, Config::default());
    incoming.set_policy_owner_preparing(true);
    incoming.install_prepared_profile(prepared).unwrap();
    assert!(Arc::ptr_eq(&group(&source), &group(&incoming)));
    assert!(source.preflight_shared_policy_contexts(&candidate).is_err());
    incoming.set_policy_owner_preparing(false);
    let current = source
        .preflight_shared_policy_contexts(&candidate)
        .unwrap()
        .unwrap();
    current.check_current(&group(&source)).unwrap();
    drop(incoming);
    assert!(current.check_current(&group(&source)).is_err());
    source
        .preflight_shared_policy_contexts(&candidate)
        .unwrap()
        .unwrap()
        .check_current(&group(&source))
        .unwrap();
}

#[test]
fn dropping_an_actual_prepared_worker_result_retires_its_pending_root() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let (source, _) = durable_owner(&roots, "default", SharedIdAllocator::default());
    let candidate = zoom_candidate(&source);
    let mut worker = crate::ProfileOpenCoordinator::spawn().unwrap();
    worker
        .submit(worker_request(&roots, "abandoned-profile"))
        .unwrap();
    let prepared = worker_result(&mut worker);
    assert!(prepared.pending_policy_owner.is_some());
    assert!(source.preflight_shared_policy_contexts(&candidate).is_err());
    drop(prepared);
    source
        .preflight_shared_policy_contexts(&candidate)
        .unwrap()
        .unwrap()
        .check_current(&group(&source))
        .unwrap();
    drop(worker);
    source
        .preflight_shared_policy_contexts(&candidate)
        .unwrap()
        .unwrap()
        .check_current(&group(&source))
        .unwrap();
}

#[test]
fn a_durable_worker_failure_after_root_resolution_retires_pending_membership() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let (source, _) = durable_owner(&roots, "default", SharedIdAllocator::default());
    let candidate = zoom_candidate(&source);
    let old = source
        .preflight_shared_policy_contexts(&candidate)
        .unwrap()
        .unwrap();
    let id = ProfileRegistry::open(&roots)
        .unwrap()
        .get_or_create("failed-worker", "Failed")
        .unwrap()
        .id;
    // A real profile registry ID and an ordinary local I/O failure after the
    // worker resolves/registers its root; no simulated coordinator result.
    std::fs::create_dir_all(roots.runtime.join("profiles").join(format!("{id}.lock"))).unwrap();
    let mut incoming = BrowserApplication::new(Config::default());
    incoming
        .request_profile_open(worker_request(&roots, "failed-worker"))
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let effect = loop {
        if let Some(result) = incoming.poll_profile_open() {
            break result.unwrap();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "profile worker timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    assert!(effect.storage_error.is_some());
    assert!(!effect.profile.durable);
    assert!(old.check_current(&group(&source)).is_err());
    source
        .preflight_shared_policy_contexts(&candidate)
        .unwrap()
        .unwrap()
        .check_current(&group(&source))
        .unwrap();
    drop(incoming);
}

#[test]
fn direct_source_activation_rejects_an_unsynchronized_shared_layer() {
    let (mut source, mut peer, window) = fixture();
    known_page(&mut peer, window);
    source
        .publish_configuration_update(collection_candidate(&source), || Ok(()))
        .unwrap();
    let before = peer.configuration_snapshot();
    let before_sources = peer.configuration_policy_sources();
    let mut candidate = peer.configuration_layers().clone();
    candidate.base.site_rules.push(rule("anchor", ANCHOR));
    // This anchor is compatible with the old local Runtime but conflicts with
    // the already committed collection. A ready owner must synchronize first.
    assert!(peer.activate_configuration(candidate).is_err());
    assert_eq!(peer.configuration_snapshot(), before);
    assert_eq!(peer.configuration_policy_sources(), before_sources);
    let committed = peer
        .prepare_committed_peer_configuration()
        .unwrap()
        .unwrap();
    peer.publish_configuration_update(committed, || Ok(()))
        .unwrap();
    let mut candidate = peer.configuration_layers().clone();
    candidate.base.site_rules.push(rule("anchor", ANCHOR));
    assert!(peer.activate_configuration(candidate).is_err());
}

#[test]
fn preparing_source_activation_can_stage_fallback_until_committed_adoption() {
    let (mut source, mut peer, _) = fixture();
    source
        .publish_configuration_update(zoom_candidate(&source), || Ok(()))
        .unwrap();
    peer.set_policy_owner_preparing(true);
    let mut candidate = peer.configuration_layers().clone();
    candidate
        .temporary
        .set_literal("content.zoom", "1.75")
        .unwrap();
    peer.activate_configuration(candidate).unwrap();
    peer.set_policy_owner_preparing(false);
    let committed = peer
        .prepare_committed_peer_configuration()
        .unwrap()
        .unwrap();
    peer.publish_configuration_update(committed, || Ok(()))
        .unwrap();
    assert_eq!(
        peer.configuration().content.zoom.to_bits(),
        1.75_f64.to_bits()
    );
    assert_eq!(
        peer.configuration_layers().runtime,
        source.configuration_layers().runtime
    );
}

#[test]
fn ready_source_activation_cannot_discard_the_authoritative_runtime() {
    let (mut source, mut peer, window) = fixture();
    known_page(&mut peer, window);
    source
        .publish_configuration_update(collection_candidate(&source), || Ok(()))
        .unwrap();
    let committed = peer
        .prepare_committed_peer_configuration()
        .unwrap()
        .unwrap();
    peer.publish_configuration_update(committed, || Ok(()))
        .unwrap();
    let before = peer.configuration_snapshot();
    let mut candidate = peer.configuration_layers().clone();
    candidate.runtime = ferric_browser_config::RuntimeOverrides::default();
    candidate.base.site_rules.push(rule("anchor", ANCHOR));
    assert!(peer.activate_configuration(candidate).is_err());
    assert_eq!(peer.configuration_snapshot(), before);
    assert!(peer.replace_configuration(Config::default()).is_err());
    assert_eq!(peer.configuration_snapshot(), before);
    assert_eq!(
        peer.configuration_layers().runtime,
        source.configuration_layers().runtime
    );
}
