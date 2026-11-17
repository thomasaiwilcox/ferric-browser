use super::*;
use crate::{
    ConfigurationLayers, InteractionCompilationState, InteractionInspectionRequest,
    InteractionRuleEdit, ProfileActivation, configuration::ConfigurationAdmission,
};
use ferric_browser_config::{Config, ContextsConfig, PolicySourceIdentity, SiteRule};
use ferric_browser_core::{
    Event, IdSource, PrivacyKind, SharedIdAllocator, ValidatedUrl, WindowId,
};
use ferric_browser_runtime::{BrowserRuntime, RuntimeInput};
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

fn shared_zoom(source: &mut BrowserApplication, value: &str) {
    let mut layers = source.configuration_layers().clone();
    layers.runtime.set_literal("content.zoom", value).unwrap();
    source.update_configuration(layers).unwrap();
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

#[test]
fn committed_adoption_preserves_peer_layers_and_unblocks_repair_of_late_conflicts() {
    let (mut source, mut peer, window) = fixture();
    let mut own = peer.configuration_layers().clone();
    own.base.site_rules.push(rule("anchor", ANCHOR));
    own.profile.set_literal("hints.chars", "'ab'").unwrap();
    own.command_line
        .set_literal("hints.min_chars", "2")
        .unwrap();
    own.temporary.set_literal("content.zoom", "1.75").unwrap();
    peer.activate_configuration(own.clone()).unwrap();
    let mut incoming = source.configuration_layers().clone();
    incoming
        .runtime
        .site_rules
        .push(rule("collection", COLLECTION));
    source.update_configuration(incoming).unwrap();
    known_page(&mut peer, window);
    assert!(peer.capture_interaction_edits().is_err());
    let before = peer.configuration_snapshot();
    let sources = peer.configuration_policy_sources();
    let count = peer.interaction_compilation_count();
    let prepared = peer
        .prepare_committed_peer_configuration()
        .unwrap()
        .unwrap();
    assert!(prepared.is_committed_peer_update());
    assert_eq!(peer.configuration_snapshot(), before);
    assert_eq!(peer.configuration_policy_sources(), sources);
    assert_eq!(peer.interaction_compilation_count(), count);
    // The ordinary proposal path would reject this new prospective conflict;
    // receiving committed bytes is not another attempt to write those bytes.
    assert!(
        peer.validate_configuration_interaction_contexts(&prepared)
            .is_err()
    );
    let shared_revision = peer.profile_configuration_revision().unwrap();
    peer.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    assert_eq!(
        peer.profile_configuration_revision().unwrap(),
        shared_revision
    );
    let adopted = peer.configuration_layers();
    assert_eq!(adopted.base, own.base);
    assert_eq!(adopted.profile, own.profile);
    assert_eq!(adopted.command_line, own.command_line);
    assert_eq!(adopted.temporary, own.temporary);
    assert_eq!(adopted.runtime, source.configuration_layers().runtime);
    let report = peer
        .configuration_inspection_inputs()
        .inspect_interaction(&InteractionInspectionRequest::new(URL, false).unwrap())
        .unwrap();
    assert_eq!(report.active_state(), InteractionCompilationState::Ready);
    assert_eq!(
        report.desired_state(),
        InteractionCompilationState::Conflicted
    );
    assert!(
        peer.prepare_committed_peer_configuration()
            .unwrap()
            .is_none()
    );
    let capture = peer.capture_interaction_edits().unwrap();
    let repair = peer
        .prepare_interaction_edits(
            &capture,
            &PolicySourceIdentity::Generated,
            &[InteractionRuleEdit::Unset {
                id: "collection".into(),
                expected_pattern: "https://example.test/*".into(),
            }],
        )
        .unwrap();
    peer.publish_configuration_update(repair.into_prepared(), || Ok(()))
        .unwrap();
    assert_eq!(peer.configuration_layers().runtime.site_rules.len(), 0);
}

#[test]
fn equal_restored_shared_values_cannot_revive_a_captured_adoption() {
    let (mut source, mut peer, _) = fixture();
    shared_zoom(&mut source, "1.25");
    let prepared = peer
        .prepare_committed_peer_configuration()
        .unwrap()
        .unwrap();
    shared_zoom(&mut source, "1.5");
    shared_zoom(&mut source, "1.25");
    let before = peer.configuration_snapshot();
    let called = Cell::new(false);
    assert!(
        peer.publish_configuration_update(prepared, || {
            called.set(true);
            Ok(())
        })
        .is_err()
    );
    assert!(!called.get());
    assert_eq!(peer.configuration_snapshot(), before);
    let current = peer
        .prepare_committed_peer_configuration()
        .unwrap()
        .unwrap();
    peer.publish_configuration_update(current, || Ok(()))
        .unwrap();
    assert_eq!(
        peer.configuration().content.zoom.to_bits(),
        1.25_f64.to_bits()
    );
}

#[test]
fn receiving_alongside_a_new_local_edit_is_not_a_committed_adoption_exemption() {
    let (mut source, mut peer, _) = fixture();
    shared_zoom(&mut source, "1.25");
    let old = peer
        .prepare_committed_peer_configuration()
        .unwrap()
        .unwrap();
    let mut current = peer.peer_configuration_layers().unwrap().unwrap();
    current
        .temporary
        .set_literal("content.zoom", "1.5")
        .unwrap();
    peer.update_configuration(current).unwrap();
    assert!(
        peer.publish_configuration_update(old, || panic!("stale local revision must reject"))
            .is_err()
    );
    assert_eq!(
        peer.configuration().content.zoom.to_bits(),
        1.5_f64.to_bits()
    );
}

#[test]
fn foreign_owner_and_wrong_actual_sharing_identity_reject_before_publication() {
    let (mut source, mut peer, _) = fixture();
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Private, "work").unwrap();
    let mut other = source.inherit_profile(runtime).unwrap();
    shared_zoom(&mut source, "1.25");
    let prepared = peer
        .prepare_committed_peer_configuration()
        .unwrap()
        .unwrap();
    let before = other.configuration_snapshot();
    assert!(
        other
            .publish_configuration_update(prepared, || panic!("foreign owner"))
            .is_err()
    );
    assert_eq!(other.configuration_snapshot(), before);
    let mut wrong = peer
        .prepare_committed_peer_configuration()
        .unwrap()
        .unwrap();
    // Private fault injection proves the actual Arc guard, independently of
    // owner/revision checks. Public callers cannot forge this history.
    wrong.admission = ConfigurationAdmission::CommittedPeer(std::sync::Weak::new());
    let before = peer.configuration_snapshot();
    assert!(
        peer.publish_configuration_update(wrong, || panic!("wrong sharing identity"))
            .is_err()
    );
    assert_eq!(peer.configuration_snapshot(), before);
}

#[test]
fn committed_adoption_still_rejects_invalid_raw_catalogues() {
    let (mut source, mut peer, _) = fixture();
    let loaded = ferric_browser_config::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ferric-browser-config/tests/fixtures/site-policy/includes/root.toml"
    ))
    .unwrap();
    let mut own = ConfigurationLayers::new(loaded.config);
    own.base_authored = loaded.authored;
    peer.activate_configuration(own.clone()).unwrap();
    own.base.ui.font_size_pt = 20.0;
    peer.configuration.activate_layers(own).unwrap();
    shared_zoom(&mut source, "1.25");
    let before = peer.configuration_snapshot();
    assert!(
        peer.prepare_committed_peer_configuration()
            .unwrap_err()
            .to_string()
            .contains("catalogue")
    );
    assert_eq!(peer.configuration_snapshot(), before);
}

#[test]
fn unavailable_busy_or_current_owners_produce_no_adoption() {
    let empty = BrowserApplication::new(Config::default());
    assert!(
        empty
            .prepare_committed_peer_configuration()
            .unwrap()
            .is_none()
    );
    let (mut source, peer, _) = fixture();
    assert!(
        peer.prepare_committed_peer_configuration()
            .unwrap()
            .is_none()
    );
    shared_zoom(&mut source, "1.25");
    let reservation = source.reserve_configuration_reload().unwrap();
    assert!(
        peer.prepare_committed_peer_configuration()
            .unwrap()
            .is_none()
    );
    drop(reservation);
    assert!(
        peer.prepare_committed_peer_configuration()
            .unwrap()
            .is_some()
    );
}

#[test]
fn a_read_capture_cannot_keep_retired_shared_authority_alive() {
    let (mut source, peer, _) = fixture();
    shared_zoom(&mut source, "1.25");
    let authority = Arc::downgrade(
        &source
            .active_profile
            .as_ref()
            .unwrap()
            .session
            .0
            .configuration,
    );
    let captured = peer
        .prepare_committed_peer_configuration()
        .unwrap()
        .unwrap();
    drop(source);
    drop(peer);
    assert!(authority.upgrade().is_none());
    assert!(captured.is_committed_peer_update());
}

#[test]
fn receiving_a_committed_layer_cannot_submit_it_to_the_runtime_writer_again() {
    let (mut source, mut peer, _) = fixture();
    shared_zoom(&mut source, "1.25");
    let prepared = peer
        .prepare_committed_peer_configuration()
        .unwrap()
        .unwrap();
    let before = peer.configuration_snapshot();
    let error = peer
        .request_configuration_write(prepared)
        .unwrap_err()
        .to_string();
    assert!(error.contains("already belongs to admitted"), "{error}");
    assert!(!peer.configuration_write_pending());
    assert_eq!(peer.configuration_snapshot(), before);
    assert!(peer.profile_snapshot().unwrap().roots.is_none());
}

#[test]
fn durable_adoption_uses_the_saved_receipt_without_replacing_the_file() {
    use ferric_browser_storage::{ContextRegistry, ProfileRegistry, RootSpec, StorageRoots};
    use std::time::{Duration, Instant};
    let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
    let id = ProfileRegistry::open(&roots)
        .unwrap()
        .get_or_create_default()
        .unwrap()
        .id;
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "work").unwrap();
    let mut source = BrowserApplication::from_runtime(runtime, Config::default());
    source
        .activate_profile(ProfileActivation {
            name: "default".into(),
            id: Some(id),
            privacy: PrivacyKind::Normal,
            roots: Some(roots.clone()),
            lock: None,
            contexts: Some(ContextRegistry::open(&roots).unwrap()),
            storage_path: Some(roots.data.join("browser.sqlite")),
        })
        .unwrap();
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "work").unwrap();
    let mut peer = source.inherit_profile(runtime).unwrap();
    let mut incoming = source.configuration_layers().clone();
    incoming
        .runtime
        .set_literal("content.zoom", "1.25")
        .unwrap();
    let prepared = source.prepare_configuration_update(incoming).unwrap();
    source.request_configuration_write(prepared).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let receipt = loop {
        if let Some(receipt) = source.poll_configuration_write() {
            break receipt.unwrap();
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    };
    assert!(receipt.publication_error.is_none());
    let path = roots.state.join("runtime-overrides.toml");
    let bytes = std::fs::read(&path).unwrap();
    let metadata = std::fs::metadata(&path).unwrap();
    let prepared = peer
        .prepare_committed_peer_configuration()
        .unwrap()
        .unwrap();
    let job = peer
        .prepare_context_reload(&ContextsConfig::default(), ContextsConfig::default())
        .unwrap()
        .unwrap();
    assert!(
        peer.admit_context_reload(&prepared, job)
            .unwrap_err()
            .to_string()
            .contains("already belongs to admitted")
    );
    peer.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    assert_eq!(
        peer.configuration().content.zoom.to_bits(),
        1.25_f64.to_bits()
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let after = std::fs::metadata(&path).unwrap();
    assert_eq!(after.modified().unwrap(), metadata.modified().unwrap());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(after.ino(), metadata.ino());
    }
    assert!(!peer.configuration_write_pending());
    assert!(peer.reserve_configuration_reload().is_ok());
    // The original exact receipt still belongs to its writer and can publish.
    source
        .publish_configuration_update(receipt.prepared, || Ok(()))
        .unwrap();
}
