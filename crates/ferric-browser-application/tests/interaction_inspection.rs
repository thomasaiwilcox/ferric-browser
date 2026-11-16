use ferric_browser_application::{
    BrowserApplication, ConfigurationInspectionCandidateStage, ConfigurationLayers,
    InspectionBoundary, InspectionValidationStage, InteractionCompilationState,
    InteractionInspectionRequest, ProfileOpenRequest, ProfileStorage, SettingActivationBoundary,
    SettingInspectionPending, SettingInspectionRequest,
};
use ferric_browser_config::{
    Config, ConfigurationApplyBoundary, ContextsConfig, InteractionFamily, InteractionField,
    InteractionLabelField, InteractionPolicyPatch, PolicySourceIdentity, SiteRule,
};
use ferric_browser_core::PrivacyKind;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

const URL: &str = "https://example.test/page";
const ANCHOR: &str =
    "[[anchors]]\nid='main'\ncode='m'\nfamilies=['links']\ntarget_selector='#main'";

fn rule(id: &str, patch: &str) -> SiteRule {
    SiteRule {
        id: id.into(),
        pattern: "https://example.test/*".into(),
        priority: 0,
        set: BTreeMap::new(),
        interaction: Some(toml::from_str::<InteractionPolicyPatch>(patch).unwrap()),
    }
}

fn open(layers: ConfigurationLayers) -> (BrowserApplication, ferric_browser_core::WindowId) {
    let (mut app, window, _) =
        BrowserApplication::bootstrap(Config::default(), PrivacyKind::Private, "work").unwrap();
    activate(&mut app, layers, "work");
    (app, window)
}

fn activate(app: &mut BrowserApplication, layers: ConfigurationLayers, name: &str) {
    app.request_profile_open(ProfileOpenRequest {
        name: name.into(),
        label: name.into(),
        privacy: PrivacyKind::Private,
        storage: ProfileStorage::Transient,
        base_configuration: layers.base.clone(),
        base_authored: layers.base_authored.clone(),
        initial_profile_overrides: layers.profile.clone(),
        command_line_overrides: layers.command_line.clone(),
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
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    app.update_configuration(layers.clone()).unwrap();
    app.activate_configuration(layers).unwrap();
}

#[test]
fn inspection_is_a_captured_read_without_replacing_an_installed_context() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.base.site_rules.push(rule("site", ANCHOR));
    let (mut app, window) = open(layers);
    let installed = app.refresh_interaction_snapshot(window, URL).unwrap();
    let sources = app.configuration_policy_sources();
    let before = app.configuration_snapshot();
    let count = app.interaction_compilation_count();
    let inputs = app.configuration_inspection_inputs();
    let report = inputs
        .inspect_interaction(&InteractionInspectionRequest::new(URL, false).unwrap())
        .unwrap();
    assert_eq!(report.core_profile, app.windows()[&window].profile);
    assert_eq!(report.profile.name, "work");
    assert_eq!(report.active_state(), InteractionCompilationState::Ready);
    assert_eq!(report.active.as_ref().unwrap().anchor_count, 1);
    assert!(report.active.as_ref().unwrap().explanation.is_none());
    assert_eq!(report.pending, SettingInspectionPending::Unchanged);
    let other = inputs
        .inspect_interaction(
            &InteractionInspectionRequest::new("https://elsewhere.test/", true).unwrap(),
        )
        .unwrap();
    let explained = inputs
        .inspect_interaction(&InteractionInspectionRequest::new(URL, true).unwrap())
        .unwrap();
    let detail = explained
        .active
        .as_ref()
        .unwrap()
        .explanation
        .as_ref()
        .unwrap();
    let (_, codebook) = detail
        .codebooks
        .iter()
        .find(|(family, _)| *family == InteractionFamily::Links)
        .unwrap();
    assert_eq!(codebook.declarations().len(), 1);
    assert_eq!(codebook.declarations()[0].code, "m");
    assert_eq!(codebook.declarations()[0].rule_id, "main");
    assert!(codebook.capacity(2) > 0);
    assert_eq!(other.active_state(), InteractionCompilationState::NoOp);
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    assert_eq!(app.interaction_compilation_count(), count);
    let current = app
        .cached_interaction_snapshot(
            window,
            &ferric_browser_config::SiteMatchContext::for_url(
                &ferric_browser_core::ValidatedUrl::parse(URL).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(current.revision(), installed.revision());
    assert!(std::sync::Arc::ptr_eq(current, &installed));
}

#[test]
fn inspection_never_fabricates_identity_or_non_http_applicability() {
    assert!(InteractionInspectionRequest::new("rb://blank", true).is_err());
    assert!(InteractionInspectionRequest::new("file:///tmp/test", true).is_err());
    let app = BrowserApplication::new(Config::default());
    assert!(
        app.configuration_inspection_inputs()
            .inspect_interaction(&InteractionInspectionRequest::new(URL, true).unwrap())
            .is_err()
    );
    let mut without_runtime = BrowserApplication::new(Config::default());
    without_runtime
        .dispatch_runtime(ferric_browser_runtime::RuntimeInput::Lifecycle(
            ferric_browser_core::Event::CreateProfile {
                label: "unselected".into(),
                privacy: PrivacyKind::Private,
            },
        ))
        .unwrap();
    assert_eq!(without_runtime.profiles().len(), 1);
    assert!(without_runtime.active_window().is_none());
    activate(
        &mut without_runtime,
        ConfigurationLayers::new(Config::default()),
        "work",
    );
    let inputs = without_runtime.configuration_inspection_inputs();
    assert!(
        inputs
            .inspect_interaction(&InteractionInspectionRequest::new(URL, false).unwrap())
            .unwrap_err()
            .contains("runtime profile")
    );
    // This context requirement does not infect global setting ownership.
    assert!(
        inputs
            .inspect_setting(&SettingInspectionRequest::new("content.zoom", None, false).unwrap())
            .is_ok()
    );
}

#[test]
fn headless_activation_does_not_adopt_a_later_window_without_activation() {
    use ferric_browser_core::Event;
    use ferric_browser_runtime::RuntimeInput;
    let layers = ConfigurationLayers::new(Config::default());
    let mut app = BrowserApplication::new(Config::default());
    app.dispatch_runtime(RuntimeInput::Lifecycle(Event::CreateProfile {
        label: "work".into(),
        privacy: PrivacyKind::Private,
    }))
    .unwrap();
    let profile = *app.profiles().keys().next().unwrap();
    activate(&mut app, layers.clone(), "work");
    app.dispatch_runtime(RuntimeInput::Lifecycle(Event::CreateWindow { profile }))
        .unwrap();
    let window = app.active_window().unwrap();
    app.dispatch_runtime(RuntimeInput::Lifecycle(Event::OpenTab { window }))
        .unwrap();
    let request = InteractionInspectionRequest::new(URL, false).unwrap();
    assert!(
        app.configuration_inspection_inputs()
            .inspect_interaction(&request)
            .is_err()
    );
    assert!(app.refresh_interaction_snapshot(window, URL).is_none());
    activate(&mut app, layers, "work");
    assert_eq!(
        app.configuration_inspection_inputs()
            .inspect_interaction(&request)
            .unwrap()
            .core_profile,
        profile
    );
    assert!(app.refresh_interaction_snapshot(window, URL).is_some());
}

#[test]
fn unused_registry_entries_do_not_override_a_real_activation_domain() {
    use ferric_browser_core::Event;
    use ferric_browser_runtime::RuntimeInput;
    let (mut app, window, _) =
        BrowserApplication::bootstrap(Config::default(), PrivacyKind::Private, "work").unwrap();
    let profile = app.windows()[&window].profile;
    app.dispatch_runtime(RuntimeInput::Lifecycle(Event::CreateProfile {
        label: "unused".into(),
        privacy: PrivacyKind::Private,
    }))
    .unwrap();
    assert_eq!(app.profiles().len(), 2);
    activate(
        &mut app,
        ConfigurationLayers::new(Config::default()),
        "work",
    );
    assert_eq!(
        app.configuration_inspection_inputs()
            .inspect_interaction(&InteractionInspectionRequest::new(URL, false).unwrap())
            .unwrap()
            .core_profile,
        profile
    );
    assert!(app.refresh_interaction_snapshot(window, URL).is_some());
}

#[test]
fn all_fields_and_ordinary_labels_have_separate_opt_in_provenance() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.base.hints.chars = "ab".into();
    layers
        .base
        .site_rules
        .push(rule("site", &format!("{ANCHOR}\n[labels]\nalphabet='cd'")));
    layers.runtime.set_literal("hints.min_chars", "2").unwrap();
    let (app, _) = open(layers);
    let request = InteractionInspectionRequest::new(
        "https://example.test/page?token=private-query#private-fragment",
        true,
    )
    .unwrap();
    let inputs = app.configuration_inspection_inputs();
    let report = inputs.inspect_interaction(&request).unwrap();
    let active = report.active.as_ref().unwrap();
    assert_eq!(
        active.enhanced_hint_families.len(),
        InteractionFamily::ALL.len()
    );
    let detail = active.explanation.as_ref().unwrap();
    assert_eq!(detail.fields.len(), InteractionField::ALL.len());
    assert_eq!(
        detail
            .fields
            .keys()
            .map(|field| field.path())
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "enabled",
            "labels.alphabet",
            "labels.min_chars",
            "labels.fallback_prefix",
            "labels.excluded_chars",
            "labels.auto_follow",
            "anchors",
            "collections",
            "hint_page_keys",
            "page_receiver",
            "remaps"
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        detail.ordinary_labels.len(),
        InteractionLabelField::ALL.len()
    );
    assert_eq!(detail.values.labels.alphabet(), "cd");
    assert_eq!(detail.values.ordinary_labels.alphabet(), "ab");
    let alpha = detail.fields[&InteractionField::Labels(InteractionLabelField::Alphabet)]
        .as_ref()
        .unwrap();
    assert_eq!(alpha.boundary, InspectionBoundary::Reload);
    assert_eq!(alpha.origin.rule().unwrap().id(), "site");
    let ordinary = detail.ordinary_labels[&InteractionLabelField::Alphabet]
        .as_ref()
        .unwrap();
    assert_eq!(ordinary.boundary, InspectionBoundary::Desired);
    assert!(ordinary.origin.rule().is_none());
    assert_eq!(
        detail.fields[&InteractionField::Labels(InteractionLabelField::MinChars)]
            .as_ref()
            .unwrap()
            .origin
            .source(),
        &PolicySourceIdentity::Generated
    );
    assert!(detail.fields[&InteractionField::Enabled].is_none());
    assert_eq!(detail.selection.survivors.len(), 1);
    assert_eq!(
        active.validation.raw_static,
        InspectionValidationStage::Passed
    );
    assert_eq!(
        active.validation.effective_combination,
        InspectionValidationStage::Passed
    );
    assert_eq!(
        active.validation.engine_css,
        InspectionValidationStage::Unknown
    );
    assert_eq!(
        active.validation.live_document,
        InspectionValidationStage::Unknown
    );
    assert!(!format!("{report:?}").contains("private-query"));
    let simple = inputs
        .inspect_interaction(&InteractionInspectionRequest::new(URL, false).unwrap())
        .unwrap();
    assert!(simple.active.as_ref().unwrap().explanation.is_none());
    assert!(!format!("{simple:?}").contains("#main"));
}

#[test]
fn source_compilation_states_keep_disabled_data_and_remap_only_intent() {
    for (patch, state, anchors, remaps, families) in [
        ("", InteractionCompilationState::NoOp, 0, 0, 0),
        ("enabled=true", InteractionCompilationState::NoOp, 0, 0, 0),
        (
            "[page_receiver]\nscope='top-document'",
            InteractionCompilationState::NoOp,
            0,
            0,
            0,
        ),
        (
            "[[remaps]]\nid='play'\nfrom='p'\nto='k'\ncontexts=['passthrough']",
            InteractionCompilationState::Ready,
            0,
            1,
            0,
        ),
        (ANCHOR, InteractionCompilationState::Ready, 1, 0, 1),
        (
            &format!("enabled=false\n{ANCHOR}"),
            InteractionCompilationState::Disabled,
            1,
            0,
            0,
        ),
    ] {
        let mut layers = ConfigurationLayers::new(Config::default());
        layers.base.site_rules.push(rule("site", patch));
        let (app, _) = open(layers);
        let report = app
            .configuration_inspection_inputs()
            .inspect_interaction(&InteractionInspectionRequest::new(URL, true).unwrap())
            .unwrap();
        let active = report.active.unwrap();
        assert_eq!(active.state, state, "{patch}");
        assert_eq!(active.anchor_count, anchors);
        assert_eq!(active.remap_count, remaps);
        assert_eq!(active.enhanced_hint_families.len(), families);
        assert_eq!(
            active.explanation.as_ref().unwrap().values.anchors.len(),
            anchors
        );
    }
}

#[test]
fn live_shared_fields_do_not_rewrite_reload_scoped_interaction() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.base.hints.chars = "ab".into();
    layers
        .base
        .site_rules
        .push(rule("site", &format!("{ANCHOR}\n[labels]\nalphabet='cd'")));
    let (mut app, _) = open(layers.clone());
    let reload_revision = app.configuration_revision();
    layers.base.site_rules[0] = rule("site", "[labels]\nmin_chars=3");
    layers.runtime.set_literal("hints.min_chars", "2").unwrap();
    app.update_configuration(layers).unwrap();
    let report = app
        .configuration_inspection_inputs()
        .inspect_interaction(&InteractionInspectionRequest::new(URL, true).unwrap())
        .unwrap();
    let active = report.active.unwrap().explanation.unwrap();
    let desired = report.desired.unwrap().explanation.unwrap();
    assert_eq!(active.values.labels.alphabet(), "cd");
    assert_eq!(desired.values.labels.alphabet(), "ab");
    assert_eq!(active.values.labels.min_chars(), 2);
    assert_eq!(desired.values.labels.min_chars(), 2);
    assert_eq!(
        active.fields[&InteractionField::Anchors]
            .as_ref()
            .unwrap()
            .origin
            .source_revision(),
        reload_revision
    );
    assert_eq!(
        active.fields[&InteractionField::Labels(InteractionLabelField::MinChars)]
            .as_ref()
            .unwrap()
            .boundary,
        InspectionBoundary::Desired
    );
    assert!(matches!(
        report.pending,
        SettingInspectionPending::Pending {
            boundary: SettingActivationBoundary::ConfigurationReload,
            value_changed: true,
            ..
        }
    ));
}

#[test]
fn canonical_publication_projection_distinguishes_a_live_update_from_a_reload() {
    let layers = ConfigurationLayers::new(Config::default());
    let (app, _) = open(layers.clone());
    let captured = app.configuration_inspection_inputs();
    let owner = app.capture_configuration_request().unwrap();
    let mut staged = layers;
    staged.temporary.site_rules.push(rule("staged", ANCHOR));
    let request = InteractionInspectionRequest::new(URL, true).unwrap();
    for (boundary, expected) in [
        (
            ConfigurationApplyBoundary::RuntimeUpdate,
            SettingActivationBoundary::ConfigurationReload,
        ),
        (
            ConfigurationApplyBoundary::Reload,
            SettingActivationBoundary::Publication,
        ),
    ] {
        let candidate = app
            .capture_configuration_inspection_candidate(&owner, &staged, boundary)
            .unwrap();
        let report = captured
            .inspect_interaction_with_candidate(&request, &candidate)
            .unwrap();
        assert_eq!(
            report.candidate_stage,
            Some(ConfigurationInspectionCandidateStage::Unprepared)
        );
        assert_eq!(report.active_state(), InteractionCompilationState::NoOp);
        assert_eq!(report.desired_state(), InteractionCompilationState::Ready);
        assert!(
            matches!(report.pending, SettingInspectionPending::Pending { boundary, value_changed:true, .. } if boundary == expected)
        );
        assert_eq!(
            report.desired_validation().engine_css,
            InspectionValidationStage::Unknown
        );
    }
    let prepared = app.prepare_configuration_reload(staged).unwrap();
    let report = captured
        .inspect_interaction_with_candidate(&request, &prepared.inspection_candidate())
        .unwrap();
    assert_eq!(
        report.candidate_stage,
        Some(ConfigurationInspectionCandidateStage::Prepared)
    );
    assert_eq!(
        report.core_profile,
        app.windows()[&app.active_window().unwrap()].profile
    );
    assert!(matches!(
        report.pending,
        SettingInspectionPending::Pending {
            boundary: SettingActivationBoundary::Publication,
            ..
        }
    ));
}

#[test]
fn candidate_attachment_rejects_other_owners_and_newer_local_revisions() {
    let layers = ConfigurationLayers::new(Config::default());
    let (mut app, _) = open(layers.clone());
    let (other, _) = open(layers.clone());
    let old = app.configuration_inspection_inputs();
    let owner = app.capture_configuration_request().unwrap();
    let candidate = app
        .capture_configuration_inspection_candidate(
            &owner,
            &layers,
            ConfigurationApplyBoundary::Reload,
        )
        .unwrap();
    let request = InteractionInspectionRequest::new(URL, false).unwrap();
    assert_eq!(app.configuration_revision(), other.configuration_revision());
    assert!(
        other
            .configuration_inspection_inputs()
            .inspect_interaction_with_candidate(&request, &candidate)
            .is_err()
    );
    app.update_configuration(layers).unwrap();
    assert!(!app.configuration_inspection_candidate_is_current(&candidate));
    assert!(
        app.configuration_inspection_inputs()
            .inspect_interaction_with_candidate(&request, &candidate)
            .is_err()
    );
    assert!(
        old.inspect_interaction_with_candidate(&request, &candidate)
            .is_ok()
    );
    assert!(!app.configuration_inspection_is_current(&old));
}

#[test]
fn prospective_combination_failure_keeps_independently_valid_active_data() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.base.site_rules.push(rule("site", ANCHOR));
    let (app, window) = open(layers.clone());
    let mut app = app;
    let installed = app.refresh_interaction_snapshot(window, URL).unwrap();
    let count = app.interaction_compilation_count();
    let collections = (0..8)
        .map(|index| {
            serde_json::json!({
                "id":format!("c{index}"),"families":["links"],
                "codes":(0..64).map(|code|format!("b{index:02}{code:02}")).collect::<Vec<_>>(),
                "container_self":true,"item_selector":".item","primary_self":true,"order":"dom"
            })
        })
        .collect::<Vec<_>>();
    let mut extra = rule("collection", "");
    extra.interaction =
        Some(serde_json::from_value(serde_json::json!({"collections":collections})).unwrap());
    layers.temporary.site_rules.push(extra);
    let candidate = app
        .capture_configuration_inspection_candidate(
            &app.capture_configuration_request().unwrap(),
            &layers,
            ConfigurationApplyBoundary::Reload,
        )
        .unwrap();
    let report = app
        .configuration_inspection_inputs()
        .inspect_interaction_with_candidate(
            &InteractionInspectionRequest::new(URL, true).unwrap(),
            &candidate,
        )
        .unwrap();
    assert_eq!(report.active_state(), InteractionCompilationState::Ready);
    assert_eq!(report.active.as_ref().unwrap().anchor_count, 1);
    assert!(report.desired.as_ref().unwrap_err().contains("512"));
    assert_eq!(
        report.desired_state(),
        InteractionCompilationState::Conflicted
    );
    assert_eq!(
        report.desired_validation().raw_static,
        InspectionValidationStage::Unknown
    );
    assert_eq!(report.pending, SettingInspectionPending::Unknown);
    assert_eq!(app.interaction_compilation_count(), count);
    assert_eq!(
        app.cached_interaction_snapshot(
            window,
            &ferric_browser_config::SiteMatchContext::for_url(
                &ferric_browser_core::ValidatedUrl::parse(URL).unwrap()
            )
            .unwrap()
        )
        .unwrap()
        .revision(),
        installed.revision()
    );
}

struct Files(std::path::PathBuf);
impl Files {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ferric-interaction-inspection-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn layers(&self, raw: &str) -> ConfigurationLayers {
        let path = self.0.join("config.toml");
        std::fs::write(&path, format!("schema_version=3\n{raw}")).unwrap();
        let loaded = ferric_browser_config::load(path).unwrap();
        let mut layers = ConfigurationLayers::new(loaded.config);
        layers.base_authored = loaded.authored;
        layers
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn rejected_catalogue_keeps_active_inspection_separate_from_the_bad_candidate() {
    let files = Files::new();
    let good = files.layers("[hints]\nchars='ab'");
    let (mut app, _) = open(good.clone());
    let mut bad = good.clone();
    bad.base.ui.font_size_pt = 20.0;
    let before = app.configuration_snapshot();
    let sources = app.configuration_policy_sources();
    assert!(app.activate_configuration(bad.clone()).is_err());
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    let candidate = app
        .capture_configuration_inspection_candidate(
            &app.capture_configuration_request().unwrap(),
            &bad,
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
    assert_eq!(report.active_state(), InteractionCompilationState::NoOp);
    assert_eq!(
        report.active_validation().raw_static,
        InspectionValidationStage::Passed
    );
    assert!(report.desired.as_ref().unwrap_err().contains("catalogue"));
    assert_eq!(
        report.desired_state(),
        InteractionCompilationState::Conflicted
    );
    assert_eq!(
        report.desired_validation().effective_combination,
        InspectionValidationStage::Unknown
    );
    assert_eq!(report.pending, SettingInspectionPending::Unknown);
}

#[test]
fn identical_document_ids_and_values_do_not_hide_a_changed_authored_document() {
    let left = Files::new();
    let right = Files::new();
    let raw = format!(
        "[[site_rules]]\nid='site'\npattern='https://example.test/*'\n[site_rules.interaction]\n{}",
        ANCHOR.replace("[[anchors]]", "[[site_rules.interaction.anchors]]")
    );
    let (mut app, _) = open(left.layers(&raw));
    app.update_configuration(right.layers(&raw)).unwrap();
    let report = app
        .configuration_inspection_inputs()
        .inspect_interaction(&InteractionInspectionRequest::new(URL, true).unwrap())
        .unwrap();
    let active = report.active.unwrap().explanation.unwrap();
    let desired = report.desired.unwrap().explanation.unwrap();
    assert_eq!(active.values, desired.values);
    assert_ne!(
        active.fields[&InteractionField::Anchors]
            .as_ref()
            .unwrap()
            .document_path(),
        desired.fields[&InteractionField::Anchors]
            .as_ref()
            .unwrap()
            .document_path()
    );
    assert!(matches!(
        report.pending,
        SettingInspectionPending::Pending {
            value_changed: false,
            source_changed: true,
            boundary: SettingActivationBoundary::ConfigurationReload
        }
    ));
}

#[test]
fn old_profile_captures_remain_inspectable_without_becoming_current_again() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.base.site_rules.push(rule("site", ANCHOR));
    let (mut app, _) = open(layers);
    let captured = app.configuration_inspection_inputs();
    let request = InteractionInspectionRequest::new(URL, true).unwrap();
    let old = captured.inspect_interaction(&request).unwrap();
    activate(
        &mut app,
        ConfigurationLayers::new(Config::default()),
        "next",
    );
    assert!(!app.configuration_inspection_is_current(&captured));
    let retained = captured.inspect_interaction(&request).unwrap();
    assert_eq!(retained.profile, old.profile);
    assert_eq!(retained.active_state(), InteractionCompilationState::Ready);
    let current = app
        .configuration_inspection_inputs()
        .inspect_interaction(&request)
        .unwrap();
    assert_eq!(current.profile.name, "next");
    assert_ne!(current.profile.instance, old.profile.instance);
    assert_eq!(current.active_state(), InteractionCompilationState::NoOp);
}
