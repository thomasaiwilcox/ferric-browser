//! Deliberately inconsistent old captures are test-only fault injection. Public
//! activation must never accept them merely to support inspection regressions.
use super::*;
use crate::{
    BrowserApplication, ConfigurationLayers, InteractionCompilationState,
    profile::ProfileActivation,
};
use ferric_browser_config::{Config, ConfigurationApplyBoundary, InteractionPolicyRevision};
use ferric_browser_core::PrivacyKind;

const URL: &str = "https://example.test/page";

fn historical_application() -> (BrowserApplication, ConfigurationLayers) {
    let loaded = ferric_browser_config::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ferric-browser-config/tests/fixtures/site-policy/includes/root.toml"
    ))
    .unwrap();
    let mut good = ConfigurationLayers::new(loaded.config);
    good.base_authored = loaded.authored;
    let (mut app, _, _) =
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
    app.activate_configuration(good.clone()).unwrap();
    let mut bad = good.clone();
    bad.base.ui.font_size_pt = 20.0;
    app.configuration.activate_layers(bad).unwrap();
    (app, good)
}

#[test]
fn invalid_historical_active_catalogue_does_not_erase_repaired_prospective_inspection() {
    let (app, good) = historical_application();
    let before = app.configuration_snapshot();
    let count = app.interaction_compilation_count();
    let candidate = app
        .capture_configuration_inspection_candidate(
            &app.capture_configuration_request().unwrap(),
            &good,
            ConfigurationApplyBoundary::RuntimeUpdate,
        )
        .unwrap();
    let report = app
        .configuration_inspection_inputs()
        .inspect_interaction_with_candidate(
            &InteractionInspectionRequest::new(URL, true).unwrap(),
            &candidate,
        )
        .unwrap();
    assert!(report.active.as_ref().unwrap_err().contains("catalogue"));
    assert_eq!(
        report.active_state(),
        InteractionCompilationState::Conflicted
    );
    assert_eq!(
        report.active_validation().raw_static,
        InspectionValidationStage::Unknown
    );
    assert_eq!(report.desired_state(), InteractionCompilationState::NoOp);
    assert_eq!(
        report.desired_validation().effective_combination,
        InspectionValidationStage::Passed
    );
    assert_eq!(report.pending, SettingInspectionPending::Unknown);
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.interaction_compilation_count(), count);
}

#[test]
fn known_historical_values_survive_unavailable_catalogue_provenance() {
    let (app, _) = historical_application();
    let before = app.configuration_snapshot();
    let report = app
        .configuration_inspection_inputs()
        .inspect_setting(&SettingInspectionRequest::new("ui.font_size_pt", None, true).unwrap())
        .unwrap();
    let active = report.active.unwrap();
    assert_eq!(
        active.value.unwrap().as_float().map(f64::to_bits),
        Some(20.0_f64.to_bits())
    );
    assert!(
        active
            .provenance_diagnostic
            .unwrap()
            .contains("ui.font_size_pt")
    );
    assert!(active.explanation.is_none());
    assert_eq!(
        active.validation.raw_static,
        InspectionValidationStage::Unknown
    );
    assert!(report.desired.is_err());
    assert_eq!(report.pending, SettingInspectionPending::Unknown);
    assert_eq!(app.configuration_snapshot(), before);
}

#[test]
fn invalid_historical_reload_catalogue_rejects_until_actual_reload_repair() {
    let (mut app, good) = historical_application();
    let mut corrupt = app.configuration_layers().clone();
    corrupt.base.hints.chars = "cd".into();
    app.configuration.activate_layers(corrupt).unwrap();
    let before = app.configuration_snapshot();
    let profile = app.profile_snapshot().unwrap();
    let core_profile = app.interaction_core_profile().unwrap();
    let compile = |sources: &ConfigurationPolicySources| {
        sources.compile_interaction(
            &profile.name,
            InteractionPolicyRevision {
                profile: core_profile,
                profile_revision: profile.generation,
                configuration_revision: sources.revision(),
                policy_revision: 0,
            },
            URL,
        )
    };
    let pending = app.prepare_configuration_update(good.clone()).unwrap();
    let sources = pending.policy_sources();
    assert_eq!(
        sources.reload_layers().base.ui.font_size_pt.to_bits(),
        20.0_f64.to_bits()
    );
    assert_eq!(sources.desired_layers(), &good);
    assert!(
        compile(&sources)
            .unwrap_err()
            .to_string()
            .contains("catalogue")
    );
    assert!(
        app.publish_configuration_update(pending, || panic!("invalid reload must not persist"))
            .is_err()
    );
    assert_eq!(app.configuration_snapshot(), before);
    let repaired = app
        .prepare_configuration_reload(good.clone())
        .unwrap()
        .prepare_policy_consumer(|_, sources| compile(sources))
        .unwrap();
    app.publish_configuration_consumer(repaired, || Ok(()))
        .unwrap();
    assert_eq!(
        compile(&app.configuration_policy_sources())
            .unwrap()
            .labels()
            .alphabet(),
        good.base.hints.chars
    );
}

#[test]
fn historical_discovery_inspection_rejects_unprovable_global_origins() {
    let (app, _) = historical_application();
    let before = app.configuration_snapshot();
    let sources = app.configuration_policy_sources();
    let count = app.interaction_compilation_count();
    assert!(
        sources
            .inspect_hint_selectors(URL, "work")
            .unwrap_err()
            .to_string()
            .contains("ui.font_size_pt")
    );
    assert!(
        sources
            .inspect_hint_proxy_targets(URL, "work")
            .unwrap_err()
            .to_string()
            .contains("ui.font_size_pt")
    );
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    assert_eq!(app.interaction_compilation_count(), count);
}

#[test]
fn historical_provenance_failure_keeps_the_typed_ordinary_site_winner() {
    let (mut app, _) = historical_application();
    let mut corrupt = app.configuration_layers().clone();
    corrupt
        .base
        .site_rules
        .push(ferric_browser_config::SiteRule {
            id: "active-site".into(),
            pattern: "https://example.test/*".into(),
            priority: 0,
            set: std::collections::BTreeMap::from([(
                "content.zoom".into(),
                toml::Value::Integer(3),
            )]),
            interaction: None,
        });
    app.configuration.activate_layers(corrupt).unwrap();
    let before = app.configuration_snapshot();
    let report = app
        .configuration_inspection_inputs()
        .inspect_setting(&SettingInspectionRequest::new("content.zoom", Some(URL), true).unwrap())
        .unwrap();
    let active = report.active.unwrap();
    assert_eq!(
        active.value.unwrap().as_float().map(f64::to_bits),
        Some(3.0_f64.to_bits())
    );
    assert!(active.provenance_diagnostic.is_some());
    assert!(active.explanation.is_none());
    assert_eq!(
        active.validation.effective_combination,
        InspectionValidationStage::Unknown
    );
    assert!(report.desired.is_err());
    assert_eq!(report.pending, SettingInspectionPending::Unknown);
    assert_eq!(app.configuration_snapshot(), before);
}
