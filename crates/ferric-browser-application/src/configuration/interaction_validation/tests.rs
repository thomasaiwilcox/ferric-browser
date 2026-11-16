use super::*;
use crate::{InteractionCompilationState, profile::ProfileActivation};
use ferric_browser_config::{Config, SiteRule};
use ferric_browser_core::{Event, PrivacyKind, TabId, WindowId};
use ferric_browser_runtime::RuntimeInput;
use std::{cell::Cell, collections::BTreeMap, sync::Arc};

const URL: &str = "https://example.test/known?secret=value#private";
const ANCHOR: &str =
    "[[anchors]]\nid='main'\ncode='m'\nfamilies=['links']\ntarget_selector='#main'";
const COLLECTION: &str = "[[collections]]\nid='list'\nfamilies=['links']\ncodes=['m']\ncontainer_self=true\nitem_selector='a.item'\nprimary_self=true\norder='dom'";

pub(super) fn rule(id: &str, patch: &str) -> SiteRule {
    SiteRule {
        id: id.into(),
        pattern: "https://example.test/*".into(),
        priority: 0,
        set: BTreeMap::new(),
        interaction: Some(toml::from_str(patch).unwrap()),
    }
}

fn application() -> (BrowserApplication, WindowId) {
    let (mut app, window, _) =
        BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "work").unwrap();
    app.activate_profile(ProfileActivation {
        name: "work".into(),
        id: None,
        privacy: PrivacyKind::Normal,
        roots: None,
        lock: None,
        contexts: None,
        storage_path: None,
    })
    .unwrap();
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.base.site_rules.push(rule("anchor", ANCHOR));
    app.activate_configuration(layers).unwrap();
    (app, window)
}

fn dispatch(app: &mut BrowserApplication, event: Event) {
    app.dispatch_runtime(RuntimeInput::Lifecycle(event))
        .unwrap();
}

pub(super) fn background_page(app: &mut BrowserApplication, window: WindowId) -> TabId {
    let before = app.tabs().keys().copied().collect::<BTreeSet<_>>();
    dispatch(
        app,
        Event::OpenTabWithNavigation {
            window,
            url: ValidatedUrl::parse(URL).unwrap(),
            background: true,
        },
    );
    let tab = *app.tabs().keys().find(|id| !before.contains(id)).unwrap();
    dispatch(
        app,
        Event::CommitNavigation {
            target: app.capture_target(tab).unwrap(),
            url: ValidatedUrl::parse(URL).unwrap(),
            title: "private".into(),
        },
    );
    tab
}

fn candidate(app: &BrowserApplication, reload: bool) -> PreparedConfigurationUpdate {
    let mut layers = app.configuration_layers().clone();
    layers
        .runtime
        .site_rules
        .push(rule("collection", COLLECTION));
    if reload {
        app.prepare_configuration_reload(layers).unwrap()
    } else {
        app.prepare_configuration_update(layers).unwrap()
    }
}

#[test]
fn background_conflict_rejects_before_callback_without_changing_sources_or_cache() {
    for reload in [false, true] {
        let (mut app, window) = application();
        let active = app.windows()[&window].active_tab.unwrap();
        background_page(&mut app, window);
        assert_eq!(app.windows()[&window].active_tab, Some(active));
        assert!(app.tabs()[&active].url.is_none());
        let cached = app
            .refresh_site_policy_snapshot(window, "rb://blank")
            .unwrap();
        let sources = app.configuration_policy_sources();
        let snapshot = app.configuration_snapshot();
        let count = app.interaction_compilation_count();
        let prepared = candidate(&app, reload);
        let diagnostic = app
            .validate_configuration_interaction_contexts(&prepared)
            .unwrap_err()
            .to_string();
        assert!(
            diagnostic.contains(if reload {
                "active"
            } else {
                "prospective reload"
            }),
            "{diagnostic}"
        );
        assert!(!diagnostic.contains("secret"));
        assert!(!diagnostic.contains("private"));
        let persisted = Cell::new(false);
        assert!(
            app.publish_configuration_update(prepared, || {
                persisted.set(true);
                Ok(())
            })
            .is_err()
        );
        assert!(!persisted.get());
        assert_eq!(app.configuration_policy_sources(), sources);
        assert_eq!(app.configuration_snapshot(), snapshot);
        assert_eq!(app.interaction_compilation_count(), count);
        assert!(Arc::ptr_eq(
            &cached,
            &app.refresh_site_policy_snapshot(window, "rb://blank")
                .unwrap()
        ));
    }
}

#[test]
fn suspended_background_pages_still_validate_but_closing_pages_do_not() {
    let (mut app, window) = application();
    let tab = background_page(&mut app, window);
    dispatch(&mut app, Event::SuspendTab { tab });
    assert!(
        app.validate_configuration_interaction_contexts(&candidate(&app, false))
            .is_err()
    );
    dispatch(&mut app, Event::CloseTab { tab });
    let prepared = candidate(&app, false);
    app.validate_configuration_interaction_contexts(&prepared)
        .unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
}

#[test]
fn a_new_context_after_preparation_is_checked_at_publication() {
    let (mut app, window) = application();
    let prepared = candidate(&app, false);
    app.validate_configuration_interaction_contexts(&prepared)
        .unwrap();
    background_page(&mut app, window);
    assert!(
        app.publish_configuration_update(prepared, || panic!("must reject before persistence"))
            .is_err()
    );
}

#[test]
fn a_later_unseen_context_is_explicitly_conflicted_without_previous_site_fallback() {
    let (mut app, window) = application();
    let prepared = candidate(&app, false);
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    let layers = app.configuration_layers().clone();
    let reload = app.prepare_configuration_reload(layers).unwrap();
    app.publish_configuration_update(reload, || Ok(())).unwrap();
    let initial = app.active_tab().unwrap().id;
    let target = app.capture_target(initial).unwrap();
    dispatch(
        &mut app,
        Event::RestoreTabAddress {
            target,
            url: ValidatedUrl::parse("https://elsewhere.test/").unwrap(),
        },
    );
    assert_eq!(
        app.tabs()[&initial].url.as_deref(),
        Some("https://elsewhere.test/")
    );
    let ready = app
        .refresh_interaction_snapshot(window, "https://elsewhere.test/")
        .unwrap();
    assert_eq!(ready.state(), InteractionCompilationState::NoOp);
    let tab = background_page(&mut app, window);
    dispatch(&mut app, Event::ActivateTab { window, tab });
    let conflicted = app.refresh_interaction_snapshot(window, URL).unwrap();
    assert_eq!(conflicted.state(), InteractionCompilationState::Conflicted);
    assert!(conflicted.policy().is_none());
    assert!(conflicted.diagnostic().unwrap().contains("duplicate"));
    assert_ne!(
        ready.revision().policy_revision,
        conflicted.revision().policy_revision
    );
}

#[test]
fn foreign_open_domains_do_not_use_this_owners_sources() {
    let (mut app, _) = application();
    let before = app.profiles().keys().copied().collect::<BTreeSet<_>>();
    dispatch(
        &mut app,
        Event::CreateProfile {
            label: "foreign".into(),
            privacy: PrivacyKind::Normal,
        },
    );
    let foreign = *app
        .profiles()
        .keys()
        .find(|id| !before.contains(id))
        .unwrap();
    dispatch(&mut app, Event::CreateWindow { profile: foreign });
    let window = app.active_window().unwrap();
    background_page(&mut app, window);
    let prepared = candidate(&app, false);
    app.validate_configuration_interaction_contexts(&prepared)
        .unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    assert!(app.refresh_interaction_snapshot(window, URL).is_none());
}

#[test]
fn unbound_contexts_use_canonical_selection_and_cannot_fabricate_identity() {
    let (mut app, window) = application();
    background_page(&mut app, window);
    app.core_profile_scope = None; // Inject lost association without changing sources.
    assert!(
        app.validate_configuration_interaction_contexts(&candidate(&app, false))
            .is_err()
    );
    let mut layers = app.configuration_layers().clone();
    // This complete higher replacement shadows the matching lower anchor,
    // even though its own pattern does not match. No selected section remains.
    let mut replacement = rule("anchor", ANCHOR);
    replacement.pattern = "https://elsewhere.test/*".into();
    layers.runtime.site_rules.push(replacement);
    let prepared = app.prepare_configuration_reload(layers).unwrap();
    app.validate_configuration_interaction_contexts(&prepared)
        .unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    assert!(app.refresh_interaction_snapshot(window, URL).is_none());
}

#[test]
fn ordinary_updates_remain_usable_without_an_interaction_association() {
    let (mut app, window) = application();
    background_page(&mut app, window);
    app.core_profile_scope = None;
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.runtime.set_literal("content.zoom", "1.25").unwrap();
    let prepared = app.prepare_configuration_reload(layers).unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    assert!((app.configuration().content.zoom - 1.25).abs() < f64::EPSILON);
}

#[test]
fn invalid_old_catalogue_cannot_veto_a_valid_replacement_with_interaction() {
    let loaded = ferric_browser_config::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ferric-browser-config/tests/fixtures/site-policy/includes/root.toml"
    ))
    .unwrap();
    let mut good = ConfigurationLayers::new(loaded.config);
    good.base_authored = loaded.authored;
    good.temporary
        .site_rules
        .push(rule("repaired-anchor", ANCHOR));
    let (mut app, window) = application();
    let mut bad = good.clone();
    bad.base.ui.font_size_pt = 26.0;
    // Historical corruption is injected through the private state, rather than
    // requiring any publication API to accept inconsistent source metadata.
    app.configuration.activate_layers(bad).unwrap();
    let tab = background_page(&mut app, window);
    let prepared = app.prepare_configuration_reload(good).unwrap();
    app.validate_configuration_interaction_contexts(&prepared)
        .unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    dispatch(&mut app, Event::ActivateTab { window, tab });
    assert_eq!(
        app.refresh_interaction_snapshot(window, URL)
            .unwrap()
            .state(),
        InteractionCompilationState::Ready
    );
}

#[test]
fn existing_conflicts_remain_closed_during_ordinary_edits_and_a_pending_repair() {
    let (mut app, window) = application();
    // Establish a valid admission with no known HTTP page, then encounter a
    // combination that could only be diagnosed when that later page appeared.
    let prepared = candidate(&app, true);
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    let tab = background_page(&mut app, window);
    dispatch(&mut app, Event::ActivateTab { window, tab });
    assert_eq!(
        app.refresh_interaction_snapshot(window, URL)
            .unwrap()
            .state(),
        InteractionCompilationState::Conflicted
    );
    let mut layers = app.configuration_layers().clone();
    layers.runtime.set_literal("content.zoom", "1.25").unwrap();
    app.update_configuration(layers.clone()).unwrap();
    assert_eq!(
        app.refresh_interaction_snapshot(window, URL)
            .unwrap()
            .state(),
        InteractionCompilationState::Conflicted
    );
    // A different bad policy must still reject, even when it emits a similar
    // diagnostic. Comparing the error text would not establish unchanged inputs.
    layers.runtime.site_rules[0].interaction =
        Some(toml::from_str(&COLLECTION.replace("['m']", "['ma']")).unwrap());
    assert!(app.update_configuration(layers).is_err());
    let mut repair = app.configuration_layers().clone();
    repair.runtime.site_rules[0].interaction = Some(toml::from_str("collections=[]").unwrap());
    app.update_configuration(repair.clone()).unwrap();
    // RuntimeUpdate saved the repair, retaining the old closed reload policy.
    assert_eq!(
        app.refresh_interaction_snapshot(window, URL)
            .unwrap()
            .state(),
        InteractionCompilationState::Conflicted
    );
    let reload = app.prepare_configuration_reload(repair).unwrap();
    app.publish_configuration_update(reload, || Ok(())).unwrap();
    assert_eq!(
        app.refresh_interaction_snapshot(window, URL)
            .unwrap()
            .state(),
        InteractionCompilationState::Ready
    );
}
