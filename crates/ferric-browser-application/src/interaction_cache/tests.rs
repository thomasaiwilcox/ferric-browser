use super::*;
use crate::{ConfigurationLayers, profile::ProfileActivation};
use ferric_browser_config::{Config, InteractionField, InteractionPolicyPatch, SiteRule};
use ferric_browser_core::{Event, PrivacyKind};
use ferric_browser_runtime::{BrowserRuntime, RuntimeInput};
use std::collections::BTreeMap;

const URL: &str = "https://example.test/ready";
const ANCHOR: &str =
    "[[anchors]]\nid='main'\ncode='m'\nfamilies=['links']\ntarget_selector='#main'";

fn rule(id: &str, pattern: &str, patch: &str) -> SiteRule {
    SiteRule {
        id: id.into(),
        pattern: pattern.into(),
        priority: 0,
        set: BTreeMap::new(),
        interaction: Some(toml::from_str::<InteractionPolicyPatch>(patch).unwrap()),
    }
}

fn layers(patch: &str) -> ConfigurationLayers {
    let mut config = Config::default();
    config
        .site_rules
        .push(rule("site", "https://example.test/ready*", patch));
    ConfigurationLayers::new(config)
}

fn activation() -> ProfileActivation {
    ProfileActivation {
        name: "default".into(),
        id: None,
        privacy: PrivacyKind::Normal,
        roots: None,
        lock: None,
        contexts: None,
        storage_path: None,
    }
}

fn application(layers: ConfigurationLayers) -> (BrowserApplication, WindowId) {
    let (mut app, window, _) =
        BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default").unwrap();
    // Install bootstrap inputs before this owner initializes the shared
    // authority. Ready direct activation must retain that committed Runtime.
    app.activate_configuration(layers).unwrap();
    app.activate_profile(activation()).unwrap();
    (app, window)
}

fn context(url: &str) -> SiteMatchContext {
    SiteMatchContext::for_url(&ValidatedUrl::parse(url).unwrap()).unwrap()
}

fn reload(app: &mut BrowserApplication, layers: ConfigurationLayers) {
    let prepared = app.prepare_configuration_reload(layers).unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
}

#[test]
fn no_profile_never_fabricates_a_runtime_policy_owner() {
    let (mut app, window, _) =
        BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default").unwrap();
    assert!(app.refresh_interaction_snapshot(window, URL).is_none());
    assert_eq!(
        app.interaction_cache_unavailable(),
        Some(InteractionCacheUnavailable::OwnerNotReady)
    );
    app.activate_profile(activation()).unwrap();
    assert_eq!(
        app.refresh_interaction_snapshot(window, URL)
            .unwrap()
            .state(),
        InteractionCompilationState::NoOp
    );
}

#[test]
fn states_distinguish_inert_disabled_and_actionable_sources() {
    for (patch, expected) in [
        ("", InteractionCompilationState::NoOp),
        ("enabled=true", InteractionCompilationState::NoOp),
        ("[labels]", InteractionCompilationState::NoOp),
        (
            "anchors=[]\ncollections=[]\nhint_page_keys=[]\nremaps=[]",
            InteractionCompilationState::NoOp,
        ),
        (
            "[page_receiver]\nscope='top-document'",
            InteractionCompilationState::NoOp,
        ),
        (ANCHOR, InteractionCompilationState::Ready),
        (
            "[[remaps]]\nid='play'\nfrom='p'\nto='k'\ncontexts=['passthrough']",
            InteractionCompilationState::Ready,
        ),
        (
            &format!("enabled=false\n{ANCHOR}"),
            InteractionCompilationState::Disabled,
        ),
    ] {
        let (mut app, window) = application(layers(patch));
        let snapshot = app.refresh_interaction_snapshot(window, URL).unwrap();
        assert_eq!(snapshot.state(), expected, "{patch}");
        assert_eq!(snapshot.policy().unwrap().revision(), snapshot.revision());
        if expected == InteractionCompilationState::Disabled {
            assert_eq!(snapshot.policy().unwrap().anchors().len(), 1);
        }
    }
}

#[test]
fn equivalent_match_contexts_and_readers_do_not_recompile() {
    let (mut app, window) = application(layers(ANCHOR));
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    for url in [
        URL,
        "HTTPS://EXAMPLE.TEST:443/ready?sort=other#hash",
        "https://example.test/ready?q=two",
    ] {
        let snapshot = app.refresh_interaction_snapshot(window, url).unwrap();
        assert!(Arc::ptr_eq(&initial, &snapshot));
        for _ in 0..32 {
            assert!(Arc::ptr_eq(
                &initial,
                app.cached_interaction_snapshot(window, &context(url))
                    .unwrap()
            ));
        }
    }
    assert_eq!(app.interaction_compilation_count(), 1);
}

#[test]
fn routes_and_unsupported_or_invalid_urls_replace_the_previous_site() {
    let (mut app, window) = application(layers(ANCHOR));
    let first = app.refresh_interaction_snapshot(window, URL).unwrap();
    assert!(
        app.cached_interaction_snapshot(window, &context("https://example.test/other"))
            .is_none()
    );
    let other = app
        .refresh_interaction_snapshot(window, "https://example.test/other")
        .unwrap();
    assert_eq!(other.state(), InteractionCompilationState::NoOp);
    assert!(other.revision().policy_revision > first.revision().policy_revision);
    assert!(
        app.cached_interaction_snapshot(window, &context(URL))
            .is_none()
    );
    for url in [
        "about:blank",
        "file:///tmp/local.html",
        "https://user@example.test/ready",
        &format!("https://example.test/{}", "a".repeat(16 * 1024)),
    ] {
        assert!(app.refresh_interaction_snapshot(window, url).is_none());
        assert!(
            app.cached_interaction_snapshot(window, &context(URL))
                .is_none()
        );
        assert_eq!(
            app.interaction_cache_unavailable(),
            Some(InteractionCacheUnavailable::UnsupportedContext)
        );
    }
    let restored = app.refresh_interaction_snapshot(window, URL).unwrap();
    assert!(restored.revision().policy_revision > other.revision().policy_revision);
}

#[test]
fn composed_conflict_replaces_ready_and_is_cached_until_repair() {
    let mut inputs = layers(ANCHOR);
    let (mut app, window) = application(inputs.clone());
    let ready = app.refresh_interaction_snapshot(window, URL).unwrap();
    inputs.runtime.site_rules.push(rule("collection", "https://example.test/ready*",
        "[[collections]]\nid='list'\nfamilies=['links']\ncodes=['m']\ncontainer_self=true\nitem_selector='a.item'\nprimary_self=true\norder='dom'"));
    reload(&mut app, inputs.clone());
    let conflicted = Arc::clone(
        app.cached_interaction_snapshot(window, &context(URL))
            .unwrap(),
    );
    assert_eq!(conflicted.state(), InteractionCompilationState::Conflicted);
    assert!(conflicted.policy().is_none());
    assert!(conflicted.diagnostic().unwrap().contains("duplicate"));
    assert!(conflicted.revision().policy_revision > ready.revision().policy_revision);
    let attempts = app.interaction_compilation_count();
    assert!(Arc::ptr_eq(
        &conflicted,
        &app.refresh_interaction_snapshot(window, URL).unwrap()
    ));
    assert_eq!(app.interaction_compilation_count(), attempts);
    inputs
        .temporary
        .site_rules
        .push(rule("repair", "https://example.test/ready*", "anchors=[]"));
    reload(&mut app, inputs);
    assert_eq!(
        app.cached_interaction_snapshot(window, &context(URL))
            .unwrap()
            .state(),
        InteractionCompilationState::Ready
    );
}

#[test]
fn no_op_reload_rebinds_all_source_revisions_without_changing_meaning() {
    let inputs = layers(ANCHOR);
    let (mut app, window) = application(inputs.clone());
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    reload(&mut app, inputs);
    let rebound = app
        .cached_interaction_snapshot(window, &context(URL))
        .unwrap();
    assert!(!Arc::ptr_eq(&initial, rebound));
    assert_eq!(
        rebound.revision().policy_revision,
        initial.revision().policy_revision
    );
    assert!(rebound.revision().configuration_revision > initial.revision().configuration_revision);
    assert_eq!(
        rebound
            .policy()
            .unwrap()
            .origin(InteractionField::Anchors)
            .unwrap()
            .source_revision(),
        rebound.sources().reload_revision()
    );
    assert_eq!(
        initial
            .policy()
            .unwrap()
            .origin(InteractionField::Anchors)
            .unwrap()
            .source_revision(),
        initial.sources().reload_revision()
    );
    assert_ne!(
        initial.sources().reload_revision(),
        rebound.sources().reload_revision()
    );
    assert_eq!(app.interaction_compilation_count(), 2);
}

#[test]
fn pending_rule_edits_rebind_capture_but_wait_for_reload_to_change_meaning() {
    let mut inputs = layers(ANCHOR);
    let (mut app, window) = application(inputs.clone());
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    inputs.base.site_rules[0]
        .interaction
        .as_mut()
        .unwrap()
        .anchors
        .as_mut()
        .unwrap()[0]
        .target_selector = Some("#changed".into());
    app.update_configuration(inputs.clone()).unwrap();
    let pending = Arc::clone(
        app.cached_interaction_snapshot(window, &context(URL))
            .unwrap(),
    );
    assert_eq!(
        pending.revision().policy_revision,
        initial.revision().policy_revision
    );
    assert_eq!(
        pending.policy().unwrap().anchors()[0]
            .target_selector
            .as_deref(),
        Some("#main")
    );
    assert_eq!(
        pending
            .policy()
            .unwrap()
            .origin(InteractionField::Anchors)
            .unwrap()
            .source_revision(),
        initial.sources().reload_revision()
    );
    reload(&mut app, inputs);
    let active = app
        .cached_interaction_snapshot(window, &context(URL))
        .unwrap();
    assert!(active.revision().policy_revision > pending.revision().policy_revision);
    assert_eq!(
        active.policy().unwrap().anchors()[0]
            .target_selector
            .as_deref(),
        Some("#changed")
    );
}

#[test]
fn winner_identity_changes_even_when_values_are_equal() {
    let mut inputs = layers(ANCHOR);
    let (mut app, window) = application(inputs.clone());
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    inputs.base.site_rules[0].id = "renamed".into();
    reload(&mut app, inputs);
    let changed = app
        .cached_interaction_snapshot(window, &context(URL))
        .unwrap();
    assert_eq!(
        changed.policy().unwrap().anchors(),
        initial.policy().unwrap().anchors()
    );
    assert!(changed.revision().policy_revision > initial.revision().policy_revision);
}

#[test]
fn rejected_persistence_keeps_configuration_and_cache_atomic() {
    let mut inputs = layers(ANCHOR);
    let (mut app, window) = application(inputs.clone());
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    let before = app.configuration_snapshot();
    inputs.base.site_rules[0].id = "changed".into();
    let candidate = app.prepare_configuration_reload(inputs).unwrap();
    assert!(
        app.publish_configuration_update(candidate, || Err(
            ferric_browser_config::ConfigError::Validation("disk error".into())
        ))
        .is_err()
    );
    assert_eq!(app.configuration_snapshot(), before);
    assert!(Arc::ptr_eq(
        &initial,
        app.cached_interaction_snapshot(window, &context(URL))
            .unwrap()
    ));
}

#[test]
fn inherited_and_reactivated_owners_never_adopt_another_capture() {
    let (mut app, window) = application(layers(ANCHOR));
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    let (runtime, child_window, _) =
        BrowserRuntime::bootstrap(PrivacyKind::Normal, "default").unwrap();
    let mut child = app.inherit_profile(runtime).unwrap();
    assert!(
        child
            .cached_interaction_snapshot(child_window, &context(URL))
            .is_none()
    );
    let own = child
        .refresh_interaction_snapshot(child_window, URL)
        .unwrap();
    assert!(!Arc::ptr_eq(&initial, &own));
    app.close_profile();
    assert!(
        app.cached_interaction_snapshot(window, &context(URL))
            .is_none()
    );
    assert!(
        child
            .cached_interaction_snapshot(child_window, &context(URL))
            .is_some()
    );
    app.activate_profile(activation()).unwrap();
    assert!(
        app.cached_interaction_snapshot(window, &context(URL))
            .is_none()
    );
    let renewed = app.refresh_interaction_snapshot(window, URL).unwrap();
    assert_ne!(
        renewed.revision().profile_revision,
        initial.revision().profile_revision
    );
    assert!(renewed.revision().policy_revision > initial.revision().policy_revision);
}

#[test]
fn peer_durable_revision_does_not_relabel_stale_local_inputs() {
    let (mut app, window) = application(layers(ANCHOR));
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "default").unwrap();
    let mut peer = app.inherit_profile(runtime).unwrap();
    let mut inputs = peer.configuration_layers().clone();
    inputs.runtime.set_literal("hints.chars", "'ab'").unwrap();
    peer.update_configuration(inputs).unwrap();
    assert!(app.profile_configuration_revision().unwrap() > 0);
    assert!(Arc::ptr_eq(
        &initial,
        &app.refresh_interaction_snapshot(window, URL).unwrap()
    ));
    let local_inputs = app.peer_configuration_layers().unwrap().unwrap();
    app.update_configuration(local_inputs).unwrap();
    let synchronized = app
        .cached_interaction_snapshot(window, &context(URL))
        .unwrap();
    assert_eq!(
        synchronized.policy().unwrap().ordinary_labels().alphabet(),
        "ab"
    );
    assert!(
        synchronized.revision().configuration_revision > initial.revision().configuration_revision
    );
}

#[test]
fn active_tab_scope_is_checked_even_with_the_same_url() {
    let (mut app, window) = application(layers(ANCHOR));
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    app.dispatch_runtime(RuntimeInput::Lifecycle(Event::OpenTabWithNavigation {
        window,
        url: ValidatedUrl::parse(URL).unwrap(),
        background: false,
    }))
    .unwrap();
    assert!(
        app.cached_interaction_snapshot(window, &context(URL))
            .is_none()
    );
    let new_tab = app.refresh_interaction_snapshot(window, URL).unwrap();
    assert!(new_tab.revision().policy_revision > initial.revision().policy_revision);
}

#[test]
fn semantic_revision_exhaustion_preserves_rebinding_but_closes_changed_policy() {
    let mut inputs = layers(ANCHOR);
    let (mut app, window) = application(inputs.clone());
    app.interaction_cache.semantic_revision = u64::MAX - 1;
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    assert_eq!(initial.revision().policy_revision, u64::MAX);
    reload(&mut app, inputs.clone());
    assert_eq!(
        app.cached_interaction_snapshot(window, &context(URL))
            .unwrap()
            .revision()
            .policy_revision,
        u64::MAX
    );
    inputs.base.site_rules[0].id = "changed".into();
    reload(&mut app, inputs);
    assert!(
        app.cached_interaction_snapshot(window, &context(URL))
            .is_none()
    );
    assert_eq!(
        app.interaction_cache_unavailable(),
        Some(InteractionCacheUnavailable::RevisionExhausted)
    );
    let attempts = app.interaction_compilation_count();
    assert!(app.refresh_interaction_snapshot(window, URL).is_none());
    assert_eq!(app.interaction_compilation_count(), attempts);
}

#[test]
fn background_navigation_does_not_replace_the_active_capture() {
    let (mut app, window) = application(layers(ANCHOR));
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    app.dispatch_runtime(RuntimeInput::Lifecycle(Event::OpenTabWithNavigation {
        window,
        url: ValidatedUrl::parse("https://other.test/background").unwrap(),
        background: true,
    }))
    .unwrap();
    assert!(Arc::ptr_eq(
        &initial,
        app.cached_interaction_snapshot(window, &context(URL))
            .unwrap()
    ));
}

#[test]
fn changing_active_window_rejects_the_previous_window_capture() {
    let (mut app, window) = application(layers(ANCHOR));
    app.refresh_interaction_snapshot(window, URL).unwrap();
    let profile = app.windows()[&window].profile;
    app.dispatch_runtime(RuntimeInput::Lifecycle(Event::CreateWindow { profile }))
        .unwrap();
    assert_ne!(app.active_window(), Some(window));
    assert!(
        app.cached_interaction_snapshot(window, &context(URL))
            .is_none()
    );
    assert!(app.refresh_interaction_snapshot(window, URL).is_none());
}

#[test]
fn standalone_replacement_and_activation_publish_their_consumer_together() {
    let inputs = layers(ANCHOR);
    let (mut app, window) = application(inputs.clone());
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    app.replace_configuration(Config::default()).unwrap();
    let replaced = Arc::clone(
        app.cached_interaction_snapshot(window, &context(URL))
            .unwrap(),
    );
    assert_eq!(replaced.state(), InteractionCompilationState::NoOp);
    assert!(replaced.revision().policy_revision > initial.revision().policy_revision);
    app.activate_configuration(inputs).unwrap();
    let restored = app
        .cached_interaction_snapshot(window, &context(URL))
        .unwrap();
    assert_eq!(restored.state(), InteractionCompilationState::Ready);
    assert!(restored.revision().policy_revision > replaced.revision().policy_revision);
}

#[test]
fn capture_local_authored_ids_do_not_hide_a_source_document_change() {
    let directory =
        std::env::temp_dir().join(format!("ferric-cache-catalogue-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let text = format!(
        "schema_version=3\n[[site_rules]]\nid='site'\npattern='https://example.test/ready*'\n[site_rules.interaction]\n{ANCHOR}"
    );
    // TOML nested entries require their actual site-rule table path.
    let text = text.replace("[[anchors]]", "[[site_rules.interaction.anchors]]");
    let mut captures = Vec::new();
    for name in ["first.toml", "second.toml"] {
        let path = directory.join(name);
        std::fs::write(&path, &text).unwrap();
        let loaded = ferric_browser_config::load(&path).unwrap();
        let mut inputs = ConfigurationLayers::new(loaded.config);
        inputs.base_authored = loaded.authored;
        captures.push(inputs);
    }
    let (mut app, window) = application(captures.remove(0));
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    reload(&mut app, captures.remove(0));
    let changed = app
        .cached_interaction_snapshot(window, &context(URL))
        .unwrap();
    assert!(
        initial
            .policy()
            .unwrap()
            .same_meaning(changed.policy().unwrap())
    );
    assert!(changed.revision().policy_revision > initial.revision().policy_revision);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn reducer_navigation_rejects_old_lookup_before_adapter_refresh_or_publication() {
    let inputs = layers(ANCHOR);
    let (mut app, window) = application(inputs.clone());
    app.refresh_interaction_snapshot(window, URL).unwrap();
    let tab = app.windows()[&window].active_tab.unwrap();
    let target = app.capture_target(tab).unwrap();
    app.dispatch_runtime(RuntimeInput::Command(
        ferric_browser_runtime::RuntimeCommand::Navigate {
            target,
            url: ValidatedUrl::parse("https://example.test/other").unwrap(),
        },
    ))
    .unwrap();
    // Even a caller holding the old context cannot obtain the old result.
    assert!(
        app.cached_interaction_snapshot(window, &context(URL))
            .is_none()
    );
    reload(&mut app, inputs);
    assert!(
        app.cached_interaction_snapshot(window, &context(URL))
            .is_none()
    );
    assert!(
        app.cached_interaction_snapshot(window, &context("https://example.test/other"))
            .is_none()
    );
    assert_eq!(
        app.refresh_interaction_snapshot(window, "https://example.test/other")
            .unwrap()
            .state(),
        InteractionCompilationState::NoOp
    );
}

#[test]
fn same_url_document_replacement_requires_refresh_without_recompiling_policy() {
    let (mut app, window) = application(layers(ANCHOR));
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    let tab = app.windows()[&window].active_tab.unwrap();
    let target = app.capture_target(tab).unwrap();
    app.dispatch_runtime(RuntimeInput::Command(
        ferric_browser_runtime::RuntimeCommand::Navigate {
            target,
            url: ValidatedUrl::parse(URL).unwrap(),
        },
    ))
    .unwrap();
    assert_ne!(app.capture_target(tab), Some(target));
    assert!(
        app.cached_interaction_snapshot(window, &context(URL))
            .is_none()
    );
    let rebound = app.refresh_interaction_snapshot(window, URL).unwrap();
    assert!(Arc::ptr_eq(&initial, &rebound));
    assert_eq!(app.interaction_compilation_count(), 1);
    assert!(
        app.cached_interaction_snapshot(window, &context(URL))
            .is_some()
    );
}

#[test]
fn invalidation_closes_reads_without_compilation_and_preserves_a_no_op_comparator() {
    let inputs = layers(ANCHOR);
    let (mut app, window) = application(inputs.clone());
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    let attempts = app.interaction_compilation_count();
    app.invalidate_interaction_snapshot();
    assert!(
        app.cached_interaction_snapshot(window, &context(URL))
            .is_none()
    );
    assert_eq!(
        app.interaction_cache_unavailable(),
        Some(InteractionCacheUnavailable::PendingRefresh)
    );
    reload(&mut app, inputs.clone());
    reload(&mut app, inputs);
    assert_eq!(app.interaction_compilation_count(), attempts);
    assert!(
        app.cached_interaction_snapshot(window, &context(URL))
            .is_none()
    );
    let refreshed = app.refresh_interaction_snapshot(window, URL).unwrap();
    assert_eq!(app.interaction_compilation_count(), attempts + 1);
    assert_eq!(
        refreshed.revision().policy_revision,
        initial.revision().policy_revision
    );
    assert_eq!(
        refreshed.revision().configuration_revision,
        app.configuration_revision()
    );
    assert_eq!(
        refreshed
            .policy()
            .unwrap()
            .origin(InteractionField::Anchors)
            .unwrap()
            .source_revision(),
        refreshed.sources().reload_revision()
    );
}

#[test]
fn dirty_publication_compiles_only_the_latest_changed_sources_on_refresh() {
    let mut inputs = layers(ANCHOR);
    let (mut app, window) = application(inputs.clone());
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    app.invalidate_interaction_snapshot();
    inputs.base.site_rules[0].id = "new-first".into();
    reload(&mut app, inputs.clone());
    inputs.base.site_rules[0].id = "new-last".into();
    reload(&mut app, inputs);
    assert_eq!(app.interaction_compilation_count(), 1);
    assert!(
        app.cached_interaction_snapshot(window, &context(URL))
            .is_none()
    );
    let refreshed = app.refresh_interaction_snapshot(window, URL).unwrap();
    assert_eq!(app.interaction_compilation_count(), 2);
    assert!(refreshed.revision().policy_revision > initial.revision().policy_revision);
    assert_eq!(
        refreshed
            .policy()
            .unwrap()
            .origin(InteractionField::Anchors)
            .unwrap()
            .rule()
            .unwrap()
            .id(),
        "new-last"
    );
}

#[test]
fn rejected_dirty_publication_can_resume_the_original_capture_without_compilation() {
    let mut inputs = layers(ANCHOR);
    let (mut app, window) = application(inputs.clone());
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    app.invalidate_interaction_snapshot();
    inputs.base.site_rules[0].id = "rejected".into();
    let prepared = app.prepare_configuration_reload(inputs).unwrap();
    assert!(
        app.publish_configuration_update(prepared, || Err(
            ferric_browser_config::ConfigError::Validation("disk error".into())
        ))
        .is_err()
    );
    assert!(
        app.cached_interaction_snapshot(window, &context(URL))
            .is_none()
    );
    let resumed = app.refresh_interaction_snapshot(window, URL).unwrap();
    assert!(Arc::ptr_eq(&initial, &resumed));
    assert_eq!(app.interaction_compilation_count(), 1);
}

#[test]
fn invalidation_and_same_context_refresh_never_recompile() {
    let (mut app, window) = application(layers(ANCHOR));
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    for _ in 0..32 {
        app.invalidate_interaction_snapshot();
        assert!(
            app.cached_interaction_snapshot(window, &context(URL))
                .is_none()
        );
        let current = app.refresh_interaction_snapshot(window, URL).unwrap();
        assert!(Arc::ptr_eq(&initial, &current));
    }
    assert_eq!(app.interaction_compilation_count(), 1);
}

#[test]
fn rejected_engine_observation_stays_closed_through_refresh_and_publication() {
    let inputs = layers(ANCHOR);
    let (mut app, window) = application(inputs.clone());
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    let tab = app.windows()[&window].active_tab.unwrap();
    let target = app.capture_target(tab).unwrap();
    let observation = app.capture_site_policy_observation(window, target).unwrap();
    assert!(app.reject_observed_interaction_context(&observation));
    for _ in 0..3 {
        app.invalidate_interaction_snapshot();
        reload(&mut app, inputs.clone());
        assert!(app.refresh_interaction_snapshot(window, URL).is_none());
        assert!(
            app.cached_interaction_snapshot(window, &context(URL))
                .is_none()
        );
        assert_eq!(
            app.interaction_cache_unavailable(),
            Some(InteractionCacheUnavailable::RejectedEngineContext)
        );
    }
    assert_eq!(app.interaction_compilation_count(), 1);
    assert!(app.accept_observed_interaction_context(&observation));
    let recovered = app.refresh_interaction_snapshot(window, URL).unwrap();
    assert_eq!(app.interaction_compilation_count(), 2);
    assert!(recovered.revision().policy_revision > initial.revision().policy_revision);
    assert_eq!(
        recovered.revision().configuration_revision,
        app.configuration_revision()
    );
}

#[test]
fn background_rejection_preserves_active_capture_and_survives_tab_selection() {
    let (mut app, window) = application(layers(ANCHOR));
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    let original_tab = app.windows()[&window].active_tab.unwrap();
    app.dispatch_runtime(RuntimeInput::Lifecycle(Event::OpenTabWithNavigation {
        window,
        url: ValidatedUrl::parse(URL).unwrap(),
        background: true,
    }))
    .unwrap();
    let background = app.windows()[&window]
        .tabs
        .iter()
        .copied()
        .find(|tab| *tab != original_tab)
        .unwrap();
    let target = app.capture_target(background).unwrap();
    let observation = app.capture_site_policy_observation(window, target).unwrap();
    assert!(!app.reject_observed_interaction_context(&observation));
    assert!(Arc::ptr_eq(
        &initial,
        app.cached_interaction_snapshot(window, &context(URL))
            .unwrap()
    ));
    app.dispatch_runtime(RuntimeInput::Lifecycle(Event::ActivateTab {
        window,
        tab: background,
    }))
    .unwrap();
    assert!(app.refresh_interaction_snapshot(window, URL).is_none());
    assert_eq!(app.interaction_compilation_count(), 1);
    assert!(app.accept_observed_interaction_context(&observation));
    assert!(app.refresh_interaction_snapshot(window, URL).is_some());
}

#[test]
fn stale_observations_cannot_close_or_recover_a_new_document() {
    let (mut app, window) = application(layers(ANCHOR));
    app.refresh_interaction_snapshot(window, URL).unwrap();
    let tab = app.windows()[&window].active_tab.unwrap();
    let old = app.capture_target(tab).unwrap();
    let old_observation = app.capture_site_policy_observation(window, old).unwrap();
    app.dispatch_runtime(RuntimeInput::Command(
        ferric_browser_runtime::RuntimeCommand::Navigate {
            target: old,
            url: ValidatedUrl::parse(URL).unwrap(),
        },
    ))
    .unwrap();
    let current = app.capture_target(tab).unwrap();
    let current_observation = app
        .capture_site_policy_observation(window, current)
        .unwrap();
    assert!(!app.reject_observed_interaction_context(&old_observation));
    let ready = app.refresh_interaction_snapshot(window, URL).unwrap();
    assert!(app.reject_observed_interaction_context(&current_observation));
    assert!(!app.accept_observed_interaction_context(&old_observation));
    assert!(app.refresh_interaction_snapshot(window, URL).is_none());
    assert!(app.accept_observed_interaction_context(&current_observation));
    assert!(
        app.refresh_interaction_snapshot(window, URL)
            .unwrap()
            .revision()
            .policy_revision
            > ready.revision().policy_revision
    );
}

#[test]
fn navigation_request_cannot_release_a_rejected_observation() {
    let (mut app, window) = application(layers(ANCHOR));
    app.refresh_interaction_snapshot(window, URL).unwrap();
    let tab = app.windows()[&window].active_tab.unwrap();
    let old = app.capture_target(tab).unwrap();
    let old_observation = app.capture_site_policy_observation(window, old).unwrap();
    assert!(app.reject_observed_interaction_context(&old_observation));
    app.dispatch_runtime(RuntimeInput::Command(
        ferric_browser_runtime::RuntimeCommand::Navigate {
            target: old,
            url: ValidatedUrl::parse("https://example.test/other").unwrap(),
        },
    ))
    .unwrap();
    assert!(
        app.refresh_interaction_snapshot(window, "https://example.test/other")
            .is_none()
    );
    assert!(!app.accept_observed_interaction_context(&old_observation));
    let current = app.capture_target(tab).unwrap();
    let current_observation = app
        .capture_site_policy_observation(window, current)
        .unwrap();
    assert!(app.accept_observed_interaction_context(&current_observation));
    assert_eq!(
        app.refresh_interaction_snapshot(window, "https://example.test/other")
            .unwrap()
            .state(),
        InteractionCompilationState::NoOp
    );
}

#[test]
fn retiring_profile_drops_its_observed_contexts() {
    let (mut app, window) = application(layers(ANCHOR));
    app.refresh_interaction_snapshot(window, URL).unwrap();
    let target = app
        .capture_target(app.windows()[&window].active_tab.unwrap())
        .unwrap();
    let observation = app.capture_site_policy_observation(window, target).unwrap();
    assert!(app.reject_observed_interaction_context(&observation));
    app.close_profile();
    assert!(app.interaction_cache.observed_contexts.is_empty());
    app.activate_profile(activation()).unwrap();
    assert!(app.refresh_interaction_snapshot(window, URL).is_some());
    // Runtime IDs can survive profile activation; the captured owner cannot.
    assert_eq!(app.capture_target(target.tab), Some(target));
    let new_observation = app.capture_site_policy_observation(window, target).unwrap();
    assert!(app.reject_observed_interaction_context(&new_observation));
    assert!(!app.accept_observed_interaction_context(&observation));
    assert!(!app.reject_observed_interaction_context(&observation));
    assert!(app.refresh_interaction_snapshot(window, URL).is_none());
    assert!(app.accept_observed_interaction_context(&new_observation));
    assert!(app.refresh_interaction_snapshot(window, URL).is_some());
}

#[test]
fn foreign_application_observation_cannot_release_or_reject_another_owner() {
    let (mut app, window) = application(layers(ANCHOR));
    let (mut foreign, foreign_window) = application(layers(ANCHOR));
    let own = app
        .capture_site_policy_observation(
            window,
            app.capture_target(app.windows()[&window].active_tab.unwrap())
                .unwrap(),
        )
        .unwrap();
    let other = foreign
        .capture_site_policy_observation(
            foreign_window,
            foreign
                .capture_target(foreign.windows()[&foreign_window].active_tab.unwrap())
                .unwrap(),
        )
        .unwrap();
    app.refresh_interaction_snapshot(window, URL).unwrap();
    assert!(app.reject_observed_interaction_context(&own));
    assert!(!app.accept_observed_interaction_context(&other));
    assert!(!app.reject_observed_interaction_context(&other));
    assert!(app.refresh_interaction_snapshot(window, URL).is_none());
    foreign
        .refresh_interaction_snapshot(foreign_window, URL)
        .unwrap();
    assert!(!foreign.reject_observed_interaction_context(&own));
    assert!(
        foreign
            .cached_interaction_snapshot(foreign_window, &context(URL))
            .is_some()
    );
}

#[test]
fn closing_a_rejected_tab_prunes_its_retained_observation_without_another_rejection() {
    let (mut app, window) = application(layers(ANCHOR));
    let tab = app.windows()[&window].active_tab.unwrap();
    let observation = app
        .capture_site_policy_observation(window, app.capture_target(tab).unwrap())
        .unwrap();
    assert!(app.reject_observed_interaction_context(&observation));
    assert_eq!(app.interaction_cache.observed_contexts.len(), 1);
    app.dispatch_runtime(RuntimeInput::Command(
        ferric_browser_runtime::RuntimeCommand::CloseTab { tab },
    ))
    .unwrap();
    assert!(app.interaction_cache.observed_contexts.is_empty());
    assert!(!app.accept_observed_interaction_context(&observation));
}

#[test]
fn transfer_out_and_shutdown_retire_observation_records() {
    for transfer in [true, false] {
        let (mut app, window) = application(layers(ANCHOR));
        let tab = app.windows()[&window].active_tab.unwrap();
        let observation = app
            .capture_site_policy_observation(window, app.capture_target(tab).unwrap())
            .unwrap();
        assert!(app.reject_observed_interaction_context(&observation));
        let event = if transfer {
            Event::TransferTabOut { tab }
        } else {
            Event::RequestShutdown
        };
        app.dispatch_runtime(RuntimeInput::Lifecycle(event))
            .unwrap();
        assert!(app.interaction_cache.observed_contexts.is_empty());
        assert!(!app.accept_observed_interaction_context(&observation));
    }
}

fn shared_context(url: &str) -> SitePolicyMatchContext {
    SitePolicyMatchContext::for_url(&ValidatedUrl::parse(url).unwrap())
}

fn configured_buttons(snapshot: &CompiledSitePolicySnapshot) -> &[String] {
    &snapshot.discovery().selectors().unwrap()[&ferric_browser_config::HintSelectorFamily::Buttons]
        .policy
        .as_ref()
        .unwrap()
        .selectors
}

fn discovery_inputs() -> ConfigurationLayers {
    let mut inputs = layers(ANCHOR);
    inputs.base.hints.proxy_targets = ferric_browser_config::HintProxyTargets::Validated;
    inputs.base.hints.selectors.insert(
        ferric_browser_config::HintSelectorFamily::Buttons,
        toml::from_str("mode='replace'\nselectors=['.global']").unwrap(),
    );
    inputs.base.site_rules[0].set.insert(
        "hints.selectors.buttons".into(),
        toml::from_str::<toml::Table>("mode='replace'\nselectors=['.site']")
            .unwrap()
            .into(),
    );
    inputs.base.site_rules[0]
        .set
        .insert("hints.proxy_targets".into(), "off".into());
    inputs
}

#[test]
fn supported_non_http_contexts_keep_global_discovery_without_previous_site_values() {
    let (mut app, window) = application(discovery_inputs());
    let site = app.refresh_site_policy_snapshot(window, URL).unwrap();
    assert_eq!(configured_buttons(&site), &[".site"]);
    assert_eq!(
        site.discovery().proxy_targets().unwrap(),
        &ferric_browser_config::HintProxyTargets::Off
    );
    let mut global = None;
    for url in [
        "file:///tmp/page.html",
        "about:blank",
        "rb://start",
        "rb://blank",
    ] {
        let snapshot = app.refresh_site_policy_snapshot(window, url).unwrap();
        assert!(snapshot.interaction().is_none());
        assert_eq!(configured_buttons(&snapshot), &[".global"]);
        assert_eq!(
            snapshot.discovery().proxy_targets().unwrap(),
            &ferric_browser_config::HintProxyTargets::Validated
        );
        assert_eq!(
            app.interaction_cache_unavailable(),
            Some(InteractionCacheUnavailable::UnsupportedContext)
        );
        assert!(
            app.cached_interaction_snapshot(window, &context(URL))
                .is_none()
        );
        assert!(
            app.cached_site_policy_snapshot(window, &shared_context(URL))
                .is_none()
        );
        if let Some(previous) = &global {
            assert!(Arc::ptr_eq(previous, &snapshot));
        }
        global = Some(snapshot);
    }
    assert_eq!(app.interaction_compilation_count(), 2);
    assert!(
        app.refresh_site_policy_snapshot(window, "data:text/html,unmodelled")
            .is_none()
    );
    assert!(
        app.cached_site_policy_snapshot(window, &shared_context("about:blank"))
            .is_none()
    );
}

#[test]
fn equal_proxy_values_keep_winning_rule_identity_and_no_op_reload_stability() {
    let mut inputs = discovery_inputs();
    // Isolate ordinary discovery: no Interaction field can cause retirement.
    inputs.base.site_rules[0].interaction = None;
    inputs.base.site_rules[0]
        .set
        .remove("hints.selectors.buttons");
    let (mut app, window) = application(inputs.clone());
    let initial = app.refresh_site_policy_snapshot(window, URL).unwrap();
    assert_eq!(
        initial.discovery().proxy_policy().unwrap().provenance,
        ["base", "in-memory:site-rule:site"]
    );
    reload(&mut app, inputs.clone());
    let unchanged = app.refresh_site_policy_snapshot(window, URL).unwrap();
    assert_eq!(
        initial.revision().policy_revision,
        unchanged.revision().policy_revision
    );
    inputs.base.site_rules[0].id = "replacement".into();
    reload(&mut app, inputs);
    let replacement = app.refresh_site_policy_snapshot(window, URL).unwrap();
    assert_eq!(
        initial.discovery().proxy_targets(),
        replacement.discovery().proxy_targets()
    );
    assert!(replacement.revision().policy_revision > initial.revision().policy_revision);
    assert_eq!(
        replacement.discovery().proxy_policy().unwrap().provenance,
        ["base", "in-memory:site-rule:replacement"]
    );
}

#[test]
fn interaction_conflict_does_not_replace_valid_discovery_with_defaults_or_failure() {
    let mut inputs = discovery_inputs();
    inputs.runtime.site_rules.push(rule("collision", "https://example.test/ready*",
        "[[collections]]\nid='list'\nfamilies=['links']\ncodes=['m']\ncontainer_self=true\nitem_selector='a.item'\nprimary_self=true\norder='dom'"));
    let (mut app, window) = application(inputs);
    let snapshot = app.refresh_site_policy_snapshot(window, URL).unwrap();
    assert_eq!(
        snapshot.interaction().unwrap().state(),
        InteractionCompilationState::Conflicted
    );
    assert_eq!(configured_buttons(&snapshot), &[".site"]);
    assert_eq!(
        snapshot.discovery().proxy_targets().unwrap(),
        &ferric_browser_config::HintProxyTargets::Off
    );
    assert_eq!(
        snapshot.revision(),
        snapshot.interaction().unwrap().revision()
    );
}

#[test]
fn selector_composition_failure_keeps_independent_interaction_and_proxy_results() {
    let mut inputs = discovery_inputs();
    inputs
        .base
        .hints
        .selectors
        .get_mut(&ferric_browser_config::HintSelectorFamily::Buttons)
        .unwrap()
        .selectors = (0..32).map(|index| format!(".global{index}")).collect();
    inputs.base.site_rules[0].set.insert(
        "hints.selectors.buttons".into(),
        toml::from_str::<toml::Table>("mode='extend'\nselectors=['.site']")
            .unwrap()
            .into(),
    );
    let (mut app, window) = application(inputs);
    let snapshot = app.refresh_site_policy_snapshot(window, URL).unwrap();
    assert!(snapshot.discovery().selectors().unwrap_err().contains("32"));
    assert_eq!(
        snapshot.discovery().proxy_targets().unwrap(),
        &ferric_browser_config::HintProxyTargets::Off
    );
    assert_eq!(
        snapshot.interaction().unwrap().state(),
        InteractionCompilationState::Ready
    );
}

#[test]
fn shared_capture_respects_pending_selectors_and_live_proxy_globals() {
    let mut inputs = discovery_inputs();
    let (mut app, window) = application(inputs.clone());
    app.refresh_site_policy_snapshot(window, URL).unwrap();
    inputs
        .runtime
        .set_literal("hints.proxy_targets", "'validated'")
        .unwrap();
    inputs
        .runtime
        .set_literal(
            "hints.selectors.buttons",
            "{mode='replace',selectors=['.pending']}",
        )
        .unwrap();
    app.update_configuration(inputs.clone()).unwrap();
    let pending = app
        .cached_site_policy_snapshot(window, &shared_context(URL))
        .unwrap();
    assert_eq!(configured_buttons(pending), &[".site"]);
    assert_eq!(
        pending.discovery().proxy_targets().unwrap(),
        &ferric_browser_config::HintProxyTargets::Validated
    );
    reload(&mut app, inputs);
    let active = app
        .cached_site_policy_snapshot(window, &shared_context(URL))
        .unwrap();
    assert_eq!(configured_buttons(active), &[".pending"]);
    assert_eq!(
        active.discovery().proxy_targets().unwrap(),
        &ferric_browser_config::HintProxyTargets::Validated
    );
}

#[test]
fn dirty_and_rejected_contexts_close_every_typed_consumer_without_compiling() {
    let (mut app, window) = application(discovery_inputs());
    let initial = app.refresh_site_policy_snapshot(window, URL).unwrap();
    for _ in 0..32 {
        assert!(Arc::ptr_eq(
            &initial,
            app.cached_site_policy_snapshot(window, &shared_context(URL))
                .unwrap()
        ));
    }
    app.invalidate_interaction_snapshot();
    assert!(
        app.cached_site_policy_snapshot(window, &shared_context(URL))
            .is_none()
    );
    let current = app.refresh_site_policy_snapshot(window, URL).unwrap();
    assert!(Arc::ptr_eq(&initial, &current));
    let target = app
        .capture_target(app.windows()[&window].active_tab.unwrap())
        .unwrap();
    let observation = app.capture_site_policy_observation(window, target).unwrap();
    assert!(app.reject_observed_interaction_context(&observation));
    assert!(app.refresh_site_policy_snapshot(window, URL).is_none());
    assert!(
        app.cached_site_policy_snapshot(window, &shared_context(URL))
            .is_none()
    );
    assert_eq!(app.interaction_compilation_count(), 1);
}

#[test]
fn browser_url_bound_is_preserved_at_the_exact_boundary() {
    let (mut app, window) = application(layers(ANCHOR));
    let prefix = format!("{URL}?q=");
    let at_limit = format!("{prefix}{}", "a".repeat(16 * 1024 - prefix.len()));
    assert_eq!(at_limit.len(), 16 * 1024);
    assert!(
        app.refresh_interaction_snapshot(window, &at_limit)
            .is_some()
    );
    assert!(
        app.refresh_interaction_snapshot(window, &(at_limit + "a"))
            .is_none()
    );
    assert!(
        app.cached_interaction_snapshot(window, &context(URL))
            .is_none()
    );
}

#[test]
fn assigning_semantic_revision_does_not_mutate_a_retained_policy() {
    let (mut app, window) = application(layers(ANCHOR));
    let initial = app.refresh_interaction_snapshot(window, URL).unwrap();
    let original = initial.policy().unwrap();
    let assigned = original.clone().with_policy_revision(42);
    assert_eq!(assigned.revision().policy_revision, 42);
    assert_ne!(original.revision().policy_revision, 42);
    assert_eq!(
        assigned.revision().configuration_revision,
        original.revision().configuration_revision
    );
    assert!(assigned.same_meaning(original));
}

fn transferred_page() -> ferric_browser_core::TabTransfer {
    ferric_browser_core::TabTransfer {
        identity: None,
        url: Some(URL.into()),
        title: "Transferred".into(),
        loading: ferric_browser_core::LoadingState::Idle,
        pinned: false,
        muted: false,
        zoom_hundredths: 100,
    }
}

#[test]
fn incoming_tabs_start_closed_and_old_placeholder_observations_cannot_release_them() {
    for reuse_placeholder in [true, false] {
        let (mut app, window) = application(layers(ANCHOR));
        let original = app.windows()[&window].active_tab.unwrap();
        if !reuse_placeholder {
            app.dispatch_runtime(RuntimeInput::Lifecycle(Event::SameDocumentNavigation {
                target: app.capture_target(original).unwrap(),
                url: ValidatedUrl::parse(URL).unwrap(),
            }))
            .unwrap();
        }
        let old = app
            .capture_site_policy_observation(window, app.capture_target(original).unwrap())
            .unwrap();
        app.dispatch_runtime(RuntimeInput::Lifecycle(Event::TransferTabIn {
            window,
            transfer: transferred_page(),
        }))
        .unwrap();
        let imported = app.windows()[&window].active_tab.unwrap();
        assert_eq!(imported == original, reuse_placeholder);
        assert_eq!(app.tabs()[&imported].url.as_deref(), Some(URL));
        assert!(app.refresh_site_policy_snapshot(window, URL).is_none());
        assert!(!app.accept_observed_interaction_context(&old));
        assert!(!app.reject_observed_interaction_context(&old));
        let observed = app
            .capture_site_policy_observation(window, app.capture_target(imported).unwrap())
            .unwrap();
        assert!(app.accept_observed_interaction_context(&observed));
        assert!(app.refresh_site_policy_snapshot(window, URL).is_some());
        assert!(!app.reject_observed_interaction_context(&old));
    }
}

#[test]
fn repeated_association_closes_identical_targets_and_fences_old_completions() {
    let (mut app, window) = application(layers(ANCHOR));
    let tab = app.windows()[&window].active_tab.unwrap();
    let target = app.capture_target(tab).unwrap();
    let first = app.begin_site_policy_association(window, target).unwrap();
    assert!(app.accept_observed_interaction_context(&first));
    let next = app.begin_site_policy_association(window, target).unwrap();
    assert!(!app.accept_observed_interaction_context(&first));
    assert!(!app.reject_observed_interaction_context(&first));
    assert!(app.refresh_site_policy_snapshot(window, URL).is_none());
    assert!(app.accept_observed_interaction_context(&next));
    assert!(app.refresh_site_policy_snapshot(window, URL).is_some());
}

#[test]
fn cross_window_move_and_return_require_fresh_observations() {
    let (mut app, window) = application(layers(ANCHOR));
    let tab = app.windows()[&window].active_tab.unwrap();
    let old = app
        .capture_site_policy_observation(window, app.capture_target(tab).unwrap())
        .unwrap();
    assert!(app.reject_observed_interaction_context(&old));
    let profile = app.windows()[&window].profile;
    app.dispatch_runtime(RuntimeInput::Lifecycle(Event::CreateWindow { profile }))
        .unwrap();
    let other = app.active_window().unwrap();
    app.dispatch_runtime(RuntimeInput::Lifecycle(Event::MoveTab {
        tab,
        to_window: other,
        index: None,
    }))
    .unwrap();
    assert!(app.refresh_site_policy_snapshot(other, URL).is_none());
    assert!(!app.accept_observed_interaction_context(&old));
    let moved = app
        .capture_site_policy_observation(other, app.capture_target(tab).unwrap())
        .unwrap();
    app.dispatch_runtime(RuntimeInput::Lifecycle(Event::MoveTab {
        tab,
        to_window: window,
        index: None,
    }))
    .unwrap();
    assert!(!app.accept_observed_interaction_context(&moved));
    assert!(!app.accept_observed_interaction_context(&old));
    let restored = app
        .capture_site_policy_observation(window, app.capture_target(tab).unwrap())
        .unwrap();
    assert!(app.accept_observed_interaction_context(&restored));
}

#[test]
fn exhausted_association_identity_never_opens_a_cache() {
    let (mut app, window) = application(layers(ANCHOR));
    app.refresh_site_policy_snapshot(window, URL).unwrap();
    let tab = app.windows()[&window].active_tab.unwrap();
    let target = app.capture_target(tab).unwrap();
    app.interaction_cache.observation_generation = u64::MAX;
    assert!(app.begin_site_policy_association(window, target).is_none());
    assert!(app.refresh_site_policy_snapshot(window, URL).is_none());
    assert_eq!(
        app.interaction_cache_unavailable(),
        Some(InteractionCacheUnavailable::RevisionExhausted)
    );
}

#[test]
fn same_window_reorder_and_same_index_move_keep_current_observations() {
    for index in [Some(0), Some(1), None] {
        let (mut app, window) = application(layers(ANCHOR));
        app.dispatch_runtime(RuntimeInput::Lifecycle(Event::OpenTabWithNavigation {
            window,
            url: ValidatedUrl::parse(URL).unwrap(),
            background: false,
        }))
        .unwrap();
        let tab = app.windows()[&window].active_tab.unwrap();
        let observed = app
            .capture_site_policy_observation(window, app.capture_target(tab).unwrap())
            .unwrap();
        let cached = app.refresh_site_policy_snapshot(window, URL).unwrap();
        app.dispatch_runtime(RuntimeInput::Lifecycle(Event::MoveTab {
            tab,
            to_window: window,
            index,
        }))
        .unwrap();
        assert!(Arc::ptr_eq(
            &cached,
            app.cached_site_policy_snapshot(window, &shared_context(URL))
                .unwrap()
        ));
        assert!(app.reject_observed_interaction_context(&observed));
        assert!(app.accept_observed_interaction_context(&observed));
    }
}

#[test]
fn publication_prepared_before_adoption_cannot_restore_old_observation_lifetime() {
    let (mut app, window) = application(layers(ANCHOR));
    let tab = app.windows()[&window].active_tab.unwrap();
    let before = app
        .capture_site_policy_observation(window, app.capture_target(tab).unwrap())
        .unwrap();
    app.refresh_site_policy_snapshot(window, URL).unwrap();
    let prepared = app.prepare_configuration_reload(layers(ANCHOR)).unwrap();
    app.dispatch_runtime(RuntimeInput::Lifecycle(Event::TransferTabIn {
        window,
        transfer: transferred_page(),
    }))
    .unwrap();
    let imported = app.windows()[&window].active_tab.unwrap();
    let current = app
        .capture_site_policy_observation(window, app.capture_target(imported).unwrap())
        .unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    assert!(!app.accept_observed_interaction_context(&before));
    assert!(app.refresh_site_policy_snapshot(window, URL).is_none());
    assert!(app.accept_observed_interaction_context(&current));
    assert!(app.refresh_site_policy_snapshot(window, URL).is_some());
}
