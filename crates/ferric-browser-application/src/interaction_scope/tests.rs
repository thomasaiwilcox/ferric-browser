use super::*;
use crate::{ConfigurationLayers, InteractionInspectionRequest, SettingInspectionRequest};
use ferric_browser_config::Config;
use ferric_browser_core::{Event, IdSource, SharedIdAllocator, WindowId};
use ferric_browser_runtime::{BrowserRuntime, RuntimeInput};

const URL: &str = "https://example.test/";

fn activate(app: &mut BrowserApplication) {
    app.activate_profile(crate::profile::ProfileActivation {
        name: "work".into(),
        id: None,
        privacy: PrivacyKind::Normal,
        roots: None,
        lock: None,
        contexts: None,
        storage_path: None,
    })
    .unwrap();
}

fn application(ids: &SharedIdAllocator) -> (BrowserApplication, WindowId) {
    let (runtime, window, _) = BrowserRuntime::bootstrap_with_ids(
        PrivacyKind::Normal,
        "work",
        IdSource::from_shared(ids.clone()),
    )
    .unwrap();
    let mut app = BrowserApplication::from_runtime(runtime, Config::default());
    activate(&mut app);
    (app, window)
}

fn dispatch(app: &mut BrowserApplication, event: Event) {
    app.dispatch_runtime(RuntimeInput::Lifecycle(event))
        .unwrap();
}

fn foreign_window(app: &mut BrowserApplication) -> WindowId {
    let before = app.profiles().keys().copied().collect::<BTreeSet<_>>();
    dispatch(
        app,
        Event::CreateProfile {
            label: "work".into(),
            privacy: PrivacyKind::Normal,
        },
    );
    let profile = *app
        .profiles()
        .keys()
        .find(|id| !before.contains(id))
        .unwrap();
    dispatch(app, Event::CreateWindow { profile });
    let window = app.active_window().unwrap();
    dispatch(app, Event::OpenTab { window });
    window
}

fn inspected_profile(app: &BrowserApplication) -> Result<ProfileId, String> {
    app.configuration_inspection_inputs()
        .inspect_interaction(&InteractionInspectionRequest::new(URL, false).unwrap())
        .map(|report| report.core_profile)
}

#[test]
fn foreign_focus_cannot_rebind_an_existing_configuration_owner() {
    let (mut app, original) = application(&SharedIdAllocator::default());
    let profile = app.windows()[&original].profile;
    let installed = app.refresh_interaction_snapshot(original, URL).unwrap();
    assert_eq!(installed.revision().profile, profile);
    let foreign = foreign_window(&mut app);
    assert_eq!(app.active_window(), Some(foreign));
    assert_ne!(app.windows()[&foreign].profile, profile);
    let sources = app.configuration_policy_sources();
    let snapshot = app.configuration_snapshot();
    let count = app.interaction_compilation_count();
    assert_eq!(inspected_profile(&app).unwrap(), profile);
    assert_eq!(app.configuration_policy_sources(), sources);
    assert_eq!(app.configuration_snapshot(), snapshot);
    assert_eq!(app.interaction_compilation_count(), count);
    assert!(app.refresh_interaction_snapshot(foreign, URL).is_none());
    dispatch(&mut app, Event::SetActiveWindow { window: original });
    assert_eq!(
        app.refresh_interaction_snapshot(original, URL)
            .unwrap()
            .revision()
            .profile,
        profile
    );
}

#[test]
fn ambiguous_activation_stays_unbound_until_an_explicit_activation() {
    let (mut app, original) = application(&SharedIdAllocator::default());
    foreign_window(&mut app);
    activate(&mut app);
    assert!(app.interaction_core_profile().is_none());
    dispatch(&mut app, Event::SetActiveWindow { window: original });
    assert!(app.refresh_interaction_snapshot(original, URL).is_none());
    assert!(inspected_profile(&app).is_err());
    // This association is contextual, never a global configuration gate.
    app.update_configuration(ConfigurationLayers::new(Config::default()))
        .unwrap();
    assert!(
        app.configuration_inspection_inputs()
            .inspect_setting(&SettingInspectionRequest::new("content.zoom", None, false).unwrap())
            .is_ok()
    );
}

#[test]
fn disappearance_cannot_adopt_the_only_remaining_same_privacy_domain() {
    let ids = SharedIdAllocator::default();
    let (mut app, original) = application(&ids);
    let profile = app.windows()[&original].profile;
    // Fault injection: replace the whole Core domain with another real
    // bootstrapped runtime without changing the application/profile owner.
    // Public adapters cannot mutate private registries this way.
    let (replacement, window, _) =
        BrowserRuntime::bootstrap_with_ids(PrivacyKind::Normal, "work", IdSource::from_shared(ids))
            .unwrap();
    assert_ne!(replacement.view().windows()[&window].profile, profile);
    app.runtime = replacement;
    assert!(app.interaction_core_profile().is_none());
    assert!(app.refresh_interaction_snapshot(window, URL).is_none());
    assert!(inspected_profile(&app).is_err());
    activate(&mut app);
    assert_eq!(
        inspected_profile(&app).unwrap(),
        app.windows()[&window].profile
    );
    app.close_profile();
    assert!(app.core_profile_scope.is_none());
}

#[test]
fn inherited_owners_capture_their_own_distinct_core_domain() {
    let ids = SharedIdAllocator::default();
    let (source, source_window) = application(&ids);
    let (runtime, child_window, _) =
        BrowserRuntime::bootstrap_with_ids(PrivacyKind::Normal, "work", IdSource::from_shared(ids))
            .unwrap();
    let mut child = source.inherit_profile(runtime).unwrap();
    let source_profile = source.windows()[&source_window].profile;
    let child_profile = child.windows()[&child_window].profile;
    assert_ne!(source_profile, child_profile);
    assert_eq!(inspected_profile(&source).unwrap(), source_profile);
    assert_eq!(inspected_profile(&child).unwrap(), child_profile);
    assert_eq!(source.profile_instance_id(), child.profile_instance_id());
    assert_eq!(
        child
            .refresh_interaction_snapshot(child_window, URL)
            .unwrap()
            .revision()
            .profile,
        child_profile
    );
}

#[test]
fn window_without_an_active_tab_still_has_a_context_domain() {
    let (mut app, window) = application(&SharedIdAllocator::default());
    let tab = app.windows()[&window].active_tab.unwrap();
    dispatch(&mut app, Event::CloseTab { tab });
    assert!(app.windows()[&window].active_tab.is_none());
    activate(&mut app);
    assert_eq!(
        inspected_profile(&app).unwrap(),
        app.windows()[&window].profile
    );
    assert!(app.refresh_interaction_snapshot(window, URL).is_none());
}

#[test]
fn a_privacy_mismatch_cannot_establish_a_scope() {
    let (runtime, window, _) = BrowserRuntime::bootstrap(PrivacyKind::Private, "work").unwrap();
    let mut app = BrowserApplication::from_runtime(runtime, Config::default());
    activate(&mut app);
    assert!(app.interaction_core_profile().is_none());
    assert!(app.refresh_interaction_snapshot(window, URL).is_none());
    assert!(inspected_profile(&app).is_err());
}
