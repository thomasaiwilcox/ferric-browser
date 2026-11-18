use ferric_browser_application::{
    BrowserApplication, ConfigurationInspectionCandidateStage, ConfigurationLayers,
    InspectionBoundary, InspectionValidationStage, ProfileOpenRequest, ProfileStorage,
    SettingActivationBoundary, SettingExplanation, SettingInspection, SettingInspectionPending,
    SettingInspectionRequest,
};
use ferric_browser_config::{
    Config, ConfigurationApplyBoundary, ContextsConfig, HintSelectorFamily, HintSelectorMode,
    HintSelectorPolicy, PolicySourceIdentity, SiteRule, load,
};
use ferric_browser_core::PrivacyKind;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

const URL: &str = "https://example.test/page";

fn open(layers: ConfigurationLayers) -> BrowserApplication {
    let mut app = BrowserApplication::new(Config::default());
    activate_profile(&mut app, layers, "work");
    app
}

fn activate_profile(app: &mut BrowserApplication, layers: ConfigurationLayers, name: &str) {
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
    // Establish the runtime layer through the same shared-profile publication
    // authority as production before recording the full activation boundary.
    app.update_configuration(layers.clone()).unwrap();
    app.activate_configuration(layers).unwrap();
}

fn inspect(
    app: &BrowserApplication,
    key: &str,
    url: Option<&str>,
    explain: bool,
) -> SettingInspection {
    app.configuration_inspection_inputs()
        .inspect_setting(&SettingInspectionRequest::new(key, url, explain).unwrap())
        .unwrap()
}

fn rule(id: &str, priority: i32, key: &str, value: &str) -> SiteRule {
    SiteRule {
        id: id.into(),
        pattern: "https://example.test/*".into(),
        priority,
        interaction: None,
        set: BTreeMap::from([(
            key.into(),
            toml::from_str::<toml::Table>(&format!("value={value}"))
                .unwrap()
                .remove("value")
                .unwrap(),
        )]),
    }
}

fn selector_base() -> ConfigurationLayers {
    let mut base = Config::default();
    base.hints.selectors.insert(
        HintSelectorFamily::Buttons,
        HintSelectorPolicy {
            mode: HintSelectorMode::Replace,
            selectors: vec!["#base".into()],
        },
    );
    let mut layers = ConfigurationLayers::new(base);
    layers
        .runtime
        .set_literal(
            "hints.selectors.buttons",
            "{mode='extend',selectors=['#active']}",
        )
        .unwrap();
    layers
}

#[test]
fn selector_inspection_separates_active_composed_value_from_private_desired_preview() {
    let mut layers = selector_base();
    let mut app = open(layers.clone());
    layers
        .runtime
        .set_literal(
            "hints.selectors.buttons",
            "{mode='extend',selectors=['#pending']}",
        )
        .unwrap();
    layers.runtime.site_rules.push(rule(
        "site",
        0,
        "hints.selectors.buttons",
        "{mode='extend',selectors=['#site']}",
    ));
    app.update_configuration(layers).unwrap();
    let before = app.configuration_snapshot();
    let sources = app.configuration_policy_sources();
    let count = app.interaction_compilation_count();
    let report = inspect(&app, "hints.selectors.buttons", Some(URL), true);
    let active = report.active.as_ref().unwrap();
    let desired = report.desired.as_ref().unwrap();
    assert_eq!(
        active.value.as_ref().unwrap()["selectors"]
            .clone()
            .try_into::<Vec<String>>()
            .unwrap(),
        ["#base", "#active"]
    );
    assert_eq!(
        desired.value.as_ref().unwrap()["selectors"]
            .clone()
            .try_into::<Vec<String>>()
            .unwrap(),
        ["#base", "#site", "#pending"]
    );
    assert_eq!(
        active.value.as_ref().unwrap()["mode"].as_str(),
        Some("replace")
    );
    assert_eq!(report.profile.name, "work");
    assert_eq!(
        active.validation.engine_css,
        InspectionValidationStage::Unknown
    );
    assert_eq!(
        active.validation.live_document,
        InspectionValidationStage::Unknown
    );
    assert!(matches!(
        report.pending,
        SettingInspectionPending::Pending {
            boundary: SettingActivationBoundary::ConfigurationReload,
            value_changed: true,
            ..
        }
    ));
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    assert_eq!(app.interaction_compilation_count(), count);
    let terse = inspect(&app, "hints.selectors.buttons", Some(URL), false);
    assert!(terse.active.unwrap().explanation.is_none());
    assert!(terse.desired.unwrap().explanation.is_none());
}

#[test]
fn pending_proxy_rules_are_reported_even_when_a_live_global_keeps_the_value_equal() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers
        .base
        .site_rules
        .push(rule("old", 0, "hints.proxy_targets", "'validated'"));
    let mut app = open(layers.clone());
    layers.base.site_rules.clear();
    layers
        .runtime
        .set_literal("hints.proxy_targets", "'off'")
        .unwrap();
    app.update_configuration(layers.clone()).unwrap();
    let report = inspect(&app, "hints.proxy_targets", Some(URL), true);
    assert_eq!(report.active.unwrap().value.unwrap().as_str(), Some("off"));
    assert_eq!(report.desired.unwrap().value.unwrap().as_str(), Some("off"));
    assert!(matches!(
        report.pending,
        SettingInspectionPending::Pending {
            value_changed: false,
            source_changed: true,
            ..
        }
    ));
    layers.runtime.unset_setting("hints.proxy_targets").unwrap();
    app.update_configuration(layers).unwrap();
    let report = inspect(&app, "hints.proxy_targets", Some(URL), true);
    assert_eq!(
        report.active.unwrap().value.unwrap().as_str(),
        Some("validated")
    );
    assert_eq!(report.desired.unwrap().value.unwrap().as_str(), Some("off"));
}

#[test]
fn ordinary_startup_and_restart_values_stay_active_after_configuration_reload() {
    let mut layers = ConfigurationLayers::new(Config::default());
    let mut app = open(layers.clone());
    let captured = app.configuration_inspection_inputs();
    layers
        .runtime
        .set_literal("session.lazy_restore", "false")
        .unwrap();
    layers
        .runtime
        .set_literal("tabs.presentation", "'windows'")
        .unwrap();
    app.update_configuration(layers.clone()).unwrap();
    let prepared = app.prepare_configuration_reload(layers).unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    for (key, expected, pending, boundary) in [
        (
            "session.lazy_restore",
            toml::Value::Boolean(true),
            toml::Value::Boolean(false),
            SettingActivationBoundary::Startup,
        ),
        (
            "tabs.presentation",
            toml::Value::String("internal".into()),
            toml::Value::String("windows".into()),
            SettingActivationBoundary::Restart,
        ),
    ] {
        let report = inspect(&app, key, None, true);
        assert_eq!(report.active.unwrap().value, Some(expected));
        assert_eq!(report.desired.unwrap().value, Some(pending));
        assert!(
            matches!(report.pending, SettingInspectionPending::Pending { boundary: actual, value_changed: true, .. } if actual == boundary)
        );
    }
    assert!(!app.configuration_inspection_is_current(&captured));
    let old = captured
        .inspect_setting(
            &SettingInspectionRequest::new("session.lazy_restore", None, false).unwrap(),
        )
        .unwrap();
    assert_eq!(old.active.unwrap().value.unwrap().as_bool(), Some(true));
    assert_eq!(old.pending, SettingInspectionPending::Unchanged);
}

#[test]
fn ordinary_site_inspection_preserves_legacy_priority_and_typed_numeric_values() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers
        .base
        .site_rules
        .push(rule("authored", 9999, "content.zoom", "2"));
    layers
        .runtime
        .site_rules
        .push(rule("generated", -9999, "content.zoom", "3"));
    layers.runtime.set_literal("content.zoom", "1.5").unwrap();
    let app = open(layers);
    let report = inspect(&app, "content.zoom", Some(URL), true);
    let active = report.active.unwrap();
    assert_eq!(
        active.value.unwrap().as_float().map(f64::to_bits),
        Some(2.0_f64.to_bits())
    );
    let SettingExplanation::Ordinary {
        fields,
        contributions,
        ..
    } = active.explanation.unwrap()
    else {
        panic!("ordinary explanation expected");
    };
    let origin = fields[&Vec::new()].as_ref().unwrap();
    assert_eq!(origin.origin.rule().unwrap().id(), "authored");
    assert_eq!(contributions.last().unwrap().input.as_integer(), Some(2));
    assert_eq!(report.pending, SettingInspectionPending::Unchanged);
}

#[test]
fn equal_global_replacements_and_literal_binding_unsets_have_exact_origins() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers
        .profile
        .set_literal("content.images", "true")
        .unwrap();
    layers
        .runtime
        .set_literal("content.images", "true")
        .unwrap();
    layers.runtime.set_binding("normal", "g.g", "").unwrap();
    let mut app = open(layers.clone());
    let report = inspect(&app, "content.images", None, true);
    let SettingExplanation::Ordinary { fields, .. } = report.active.unwrap().explanation.unwrap()
    else {
        panic!("ordinary explanation expected");
    };
    assert_eq!(
        fields[&Vec::new()].as_ref().unwrap().origin.source(),
        &PolicySourceIdentity::Generated
    );
    let report = inspect(&app, "bindings.normal.g.g", None, true);
    let active = report.active.unwrap();
    assert_eq!(active.value.unwrap().as_str(), Some("unbound"));
    let SettingExplanation::Ordinary { contributions, .. } = active.explanation.unwrap() else {
        panic!("ordinary explanation expected");
    };
    let unset = contributions.last().unwrap();
    assert_eq!(unset.path, ["bindings", "normal", "g.g"]);
    assert_eq!(unset.input.as_str(), Some(""));
    layers.runtime.set_literal("hints.chars", "'abcd'").unwrap();
    app.update_configuration(layers).unwrap();
    assert_eq!(
        inspect(&app, "content.images", None, false).pending,
        SettingInspectionPending::Unchanged
    );
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ferric-setting-inspection-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("child.toml"),
            "[content]\nzoom=2\n[hints.selectors.buttons]\nmode='replace'\nselectors=['#same']\n",
        )
        .unwrap();
        for file in ["root-a.toml", "root-b.toml"] {
            fs::write(
                path.join(file),
                "schema_version=3\ninclude=['child.toml']\n[content]\n",
            )
            .unwrap();
        }
        Self(path)
    }
    fn layers(&self, root: &str) -> ConfigurationLayers {
        let loaded = load(self.0.join(root)).unwrap();
        let mut layers = ConfigurationLayers::new(loaded.config);
        layers.base_authored = loaded.authored;
        layers
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn sparse_authored_values_and_rejected_catalogue_keep_known_active_values() {
    let fixture = Fixture::new();
    let layers = fixture.layers("root-a.toml");
    let mut app = open(layers.clone());
    let report = inspect(&app, "content.zoom", None, true);
    let active = report.active.unwrap();
    assert_eq!(
        active.value.unwrap().as_float().map(f64::to_bits),
        Some(2.0_f64.to_bits())
    );
    let SettingExplanation::Ordinary { fields, .. } = active.explanation.unwrap() else {
        panic!("ordinary explanation expected");
    };
    assert_eq!(
        fields[&Vec::new()].as_ref().unwrap().origin.source(),
        &PolicySourceIdentity::Authored { document_id: 0 }
    );
    let before = app.configuration_snapshot();
    let before_sources = app.configuration_policy_sources();
    let mut invalid = layers;
    invalid.base.ui.font_size_pt = 20.0;
    assert!(app.activate_configuration(invalid.clone()).is_err());
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), before_sources);
    let candidate = app
        .capture_configuration_inspection_candidate(
            &app.capture_configuration_request().unwrap(),
            &invalid,
            ferric_browser_config::ConfigurationApplyBoundary::RuntimeUpdate,
        )
        .unwrap();
    let report = app
        .configuration_inspection_inputs()
        .inspect_setting_with_candidate(
            &SettingInspectionRequest::new("ui.font_size_pt", None, true).unwrap(),
            &candidate,
        )
        .unwrap();
    let active = report.active.unwrap();
    assert_eq!(
        active.value.unwrap().as_float().map(f64::to_bits),
        Some(before.effective.ui.font_size_pt.to_bits())
    );
    assert!(active.provenance_diagnostic.is_none());
    assert!(active.explanation.is_some());
    assert_eq!(
        active.validation.raw_static,
        InspectionValidationStage::Passed
    );
    assert!(report.desired.is_err());
    assert_eq!(report.pending, SettingInspectionPending::Unknown);
    assert_eq!(app.configuration_snapshot(), before);
}

#[test]
fn replaced_document_identity_is_pending_even_when_selector_values_are_equal() {
    let fixture = Fixture::new();
    // This mode leaf belongs to the root, whose identity changes at the pending
    // boundary, while the selector list still belongs to the included child.
    for root in ["root-a.toml", "root-b.toml"] {
        fs::write(
            fixture.0.join(root),
            "schema_version=3\ninclude=['child.toml']\n[hints.selectors.buttons]\nmode='replace'\n",
        )
        .unwrap();
    }
    let mut app = open(fixture.layers("root-a.toml"));
    let old = app.configuration_inspection_inputs();
    let layers = fixture.layers("root-b.toml");
    app.update_configuration(layers.clone()).unwrap();
    let report = inspect(&app, "hints.selectors.buttons", None, true);
    assert_eq!(report.active.unwrap().value, report.desired.unwrap().value);
    assert!(matches!(
        report.pending,
        SettingInspectionPending::Pending {
            value_changed: false,
            source_changed: true,
            ..
        }
    ));
    let prepared = app.prepare_configuration_reload(layers).unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    assert_eq!(
        inspect(&app, "hints.selectors.buttons", None, false).pending,
        SettingInspectionPending::Unchanged
    );
    assert!(!app.configuration_inspection_is_current(&old));
    let old_report = old
        .inspect_setting(
            &SettingInspectionRequest::new("hints.selectors.buttons", None, true).unwrap(),
        )
        .unwrap();
    assert_eq!(old_report.pending, SettingInspectionPending::Unchanged);
}

#[test]
fn desired_effective_conflict_cannot_hide_an_independently_valid_active_selector() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.base.hints.selectors.insert(
        HintSelectorFamily::Buttons,
        HintSelectorPolicy {
            mode: HintSelectorMode::Replace,
            selectors: (0..16).map(|index| format!("#a{index}")).collect(),
        },
    );
    let mut app = open(layers.clone());
    for (id, prefix, count) in [("one", "b", 16), ("two", "c", 1)] {
        let value = HintSelectorPolicy {
            mode: HintSelectorMode::Extend,
            selectors: (0..count)
                .map(|index| format!("#{prefix}{index}"))
                .collect(),
        };
        let mut entry = rule(id, 0, "content.images", "true");
        entry.set = BTreeMap::from([(
            "hints.selectors.buttons".into(),
            toml::Value::try_from(value).unwrap(),
        )]);
        layers.runtime.site_rules.push(entry);
    }
    app.update_configuration(layers).unwrap();
    let before = app.configuration_snapshot();
    let report = inspect(&app, "hints.selectors.buttons", Some(URL), true);
    assert_eq!(
        report.active.unwrap().value.unwrap()["selectors"]
            .as_array()
            .unwrap()
            .len(),
        16
    );
    assert!(report.desired.is_err());
    assert_eq!(report.pending, SettingInspectionPending::Unknown);
    assert_eq!(app.configuration_snapshot(), before);
}

#[test]
fn inspection_rejects_invalid_queries_and_requires_a_real_captured_owner() {
    for (key, url) in [
        ("unknown", None),
        ("content.zoom", Some("https://")),
        ("content.zoom", Some("file:///tmp/page")),
        ("ui.font_size_pt", Some(URL)),
    ] {
        assert!(SettingInspectionRequest::new(key, url, true).is_err());
    }
    let app = BrowserApplication::new(Config::default());
    assert!(
        app.configuration_inspection_inputs()
            .inspect_setting(&SettingInspectionRequest::new("content.zoom", None, false).unwrap())
            .is_err()
    );
}

#[test]
fn retained_inspection_uses_its_original_profile_after_owner_switch() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers
        .profile
        .set_literal("content.images", "true")
        .unwrap();
    let mut app = open(layers.clone());
    let captured = app.configuration_inspection_inputs();
    assert!(app.configuration_inspection_is_current(&captured));
    activate_profile(&mut app, layers, "other");
    assert!(!app.configuration_inspection_is_current(&captured));
    let request = SettingInspectionRequest::new("content.images", None, true).unwrap();
    let old = captured.inspect_setting(&request).unwrap();
    let current = app
        .configuration_inspection_inputs()
        .inspect_setting(&request)
        .unwrap();
    assert_eq!(old.profile.name, "work");
    assert_eq!(current.profile.name, "other");
    assert_ne!(old.profile_instance, current.profile_instance);
    for (report, name) in [(old, "work"), (current, "other")] {
        let SettingExplanation::Ordinary { fields, .. } =
            report.active.unwrap().explanation.unwrap()
        else {
            panic!("ordinary explanation expected");
        };
        assert_eq!(
            fields[&Vec::new()].as_ref().unwrap().origin.source(),
            &PolicySourceIdentity::Profile { name: name.into() }
        );
    }
}

#[test]
fn rejected_site_catalogue_preserves_the_typed_ordinary_winner_and_its_origin() {
    let fixture = Fixture::new();
    let layers = fixture.layers("root-a.toml");
    let mut app = open(layers.clone());
    let mut invalid = layers;
    invalid.base.ui.font_size_pt = 20.0;
    invalid
        .base
        .site_rules
        .push(rule("active-site", 0, "content.zoom", "3"));
    let before = app.configuration_snapshot();
    let sources = app.configuration_policy_sources();
    assert!(app.activate_configuration(invalid.clone()).is_err());
    let candidate = app
        .capture_configuration_inspection_candidate(
            &app.capture_configuration_request().unwrap(),
            &invalid,
            ferric_browser_config::ConfigurationApplyBoundary::RuntimeUpdate,
        )
        .unwrap();
    let report = app
        .configuration_inspection_inputs()
        .inspect_setting_with_candidate(
            &SettingInspectionRequest::new("content.zoom", Some(URL), true).unwrap(),
            &candidate,
        )
        .unwrap();
    let active = report.active.unwrap();
    assert_eq!(
        active.value.unwrap().as_float().map(f64::to_bits),
        Some(2.0_f64.to_bits())
    );
    assert!(active.provenance_diagnostic.is_none());
    assert!(active.explanation.is_some());
    assert_eq!(
        active.validation.effective_combination,
        InspectionValidationStage::Passed
    );
    assert!(report.desired.is_err());
    assert_eq!(report.pending, SettingInspectionPending::Unknown);
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
}

#[test]
fn ordinary_list_replacement_retires_earlier_value_and_origin() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers
        .profile
        .set_literal(
            "navigation.start_pages",
            "['https://profile.test/','https://old.test/']",
        )
        .unwrap();
    layers
        .runtime
        .set_literal("navigation.start_pages", "['https://runtime.test/']")
        .unwrap();
    let app = open(layers);
    let report = inspect(&app, "navigation.start_pages", None, true);
    let active = report.active.unwrap();
    let value = active.value.unwrap();
    assert_eq!(
        value.clone().try_into::<Vec<String>>().unwrap(),
        ["https://runtime.test/"]
    );
    let SettingExplanation::Ordinary {
        fields,
        contributions,
        ..
    } = active.explanation.unwrap()
    else {
        panic!("ordinary explanation expected");
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(
        fields[&Vec::new()].as_ref().unwrap().origin.source(),
        &PolicySourceIdentity::Generated
    );
    assert!(
        contributions
            .iter()
            .any(|contribution| contribution.origin.origin.source()
                == &PolicySourceIdentity::Profile {
                    name: "work".into()
                })
    );
    assert_eq!(report.pending, SettingInspectionPending::Unchanged);
}

#[test]
fn deserialized_binding_table_replacement_retires_removed_field_origins() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.base.bindings.insert(
        "normal".into(),
        BTreeMap::from([("z".into(), "tab-close".into())]),
    );
    // The command setter accepts scalars/arrays, while the existing runtime
    // document model also admits typed table values through its settings map.
    for (overrides, raw) in [
        (&mut layers.profile, "x='tab-close'\ny='reload'"),
        (&mut layers.runtime, "x='reload'"),
    ] {
        overrides
            .settings
            .insert("bindings.normal".into(), toml::from_str(raw).unwrap());
    }
    layers.resolve().unwrap();
    let app = open(layers);
    let report = inspect(&app, "bindings.normal", None, true);
    let active = report.active.unwrap();
    let value = active.value.unwrap();
    assert_eq!(value.as_table().unwrap().len(), 1);
    assert_eq!(value["x"].as_str(), Some("reload"));
    let SettingExplanation::Ordinary { fields, .. } = active.explanation.unwrap() else {
        panic!("ordinary explanation expected");
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(
        fields[&vec!["x".into()]].as_ref().unwrap().origin.source(),
        &PolicySourceIdentity::Generated
    );
    assert_eq!(report.pending, SettingInspectionPending::Unchanged);
}

#[test]
fn atomic_path_normalization_keeps_authored_and_runtime_field_origins() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("root-a.toml"), "schema_version=3\ninclude=['child.toml']\n[downloads]\ndirectory='/tmp/ferric-authored-downloads'\n").unwrap();
    let mut layers = fixture.layers("root-a.toml");
    let mut app = open(layers.clone());
    let report = inspect(&app, "downloads.directory", None, true);
    let active = report.active.unwrap();
    assert_eq!(
        active.value.unwrap()["path"].as_str(),
        Some("/tmp/ferric-authored-downloads")
    );
    let SettingExplanation::Ordinary { fields, .. } = active.explanation.unwrap() else {
        panic!("ordinary explanation expected");
    };
    assert_eq!(
        fields[&vec!["path".into()]]
            .as_ref()
            .unwrap()
            .origin
            .source(),
        &PolicySourceIdentity::Authored { document_id: 1 }
    );
    // A deserialized plain path is accepted and normalized by the canonical
    // compiler even though runtime command setters store the tagged form.
    layers.runtime.settings.insert(
        "downloads.directory".into(),
        toml::Value::String("/tmp/ferric-runtime-downloads".into()),
    );
    app.update_configuration(layers).unwrap();
    let report = inspect(&app, "downloads.directory", None, true);
    let active = report.active.unwrap();
    assert_eq!(
        active.value.unwrap()["path"].as_str(),
        Some("/tmp/ferric-runtime-downloads")
    );
    let SettingExplanation::Ordinary { fields, .. } = active.explanation.unwrap() else {
        panic!("ordinary explanation expected");
    };
    assert_eq!(
        fields[&vec!["path".into()]]
            .as_ref()
            .unwrap()
            .origin
            .source(),
        &PolicySourceIdentity::Generated
    );
    assert_eq!(report.pending, SettingInspectionPending::Unchanged);
}

fn staged_mixed_layers() -> ConfigurationLayers {
    let mut layers = selector_base();
    layers
        .runtime
        .set_literal("content.images", "false")
        .unwrap();
    layers
        .runtime
        .set_literal("hints.proxy_targets", "'validated'")
        .unwrap();
    layers
        .runtime
        .set_literal(
            "hints.selectors.buttons",
            "{mode='extend',selectors=['#pending']}",
        )
        .unwrap();
    layers
        .runtime
        .set_literal("session.lazy_restore", "false")
        .unwrap();
    layers
        .runtime
        .set_literal("tabs.presentation", "'windows'")
        .unwrap();
    layers
}

#[test]
fn raw_candidate_keeps_active_live_values_and_reports_each_activation_boundary() {
    let app = open(selector_base());
    let inputs = app.configuration_inspection_inputs();
    let owner = app.capture_configuration_request().unwrap();
    let before = app.configuration_snapshot();
    let sources = app.configuration_policy_sources();
    let count = app.interaction_compilation_count();
    let layers = staged_mixed_layers();
    let candidate = app
        .capture_configuration_inspection_candidate(
            &owner,
            &layers,
            ConfigurationApplyBoundary::RuntimeUpdate,
        )
        .unwrap();
    assert_eq!(
        candidate.stage(),
        ConfigurationInspectionCandidateStage::Unprepared
    );
    for (key, active, desired, boundary) in [
        (
            "content.images",
            toml::Value::Boolean(true),
            toml::Value::Boolean(false),
            SettingActivationBoundary::Publication,
        ),
        (
            "hints.proxy_targets",
            toml::Value::String("off".into()),
            toml::Value::String("validated".into()),
            SettingActivationBoundary::Publication,
        ),
        (
            "session.lazy_restore",
            toml::Value::Boolean(true),
            toml::Value::Boolean(false),
            SettingActivationBoundary::Startup,
        ),
        (
            "tabs.presentation",
            toml::Value::String("internal".into()),
            toml::Value::String("windows".into()),
            SettingActivationBoundary::Restart,
        ),
    ] {
        let report = inputs
            .inspect_setting_with_candidate(
                &SettingInspectionRequest::new(key, None, true).unwrap(),
                &candidate,
            )
            .unwrap();
        assert_eq!(report.active.unwrap().value, Some(active));
        assert_eq!(report.desired.unwrap().value, Some(desired));
        assert_eq!(report.candidate_revision, Some(candidate.revision()));
        assert!(
            matches!(report.pending, SettingInspectionPending::Pending { boundary: actual, .. } if actual == boundary)
        );
    }
    let report = inputs
        .inspect_setting_with_candidate(
            &SettingInspectionRequest::new("hints.selectors.buttons", None, true).unwrap(),
            &candidate,
        )
        .unwrap();
    assert_eq!(
        report.active.unwrap().value.unwrap()["selectors"]
            .clone()
            .try_into::<Vec<String>>()
            .unwrap(),
        ["#base", "#active"]
    );
    assert_eq!(
        report.desired.unwrap().value.unwrap()["selectors"]
            .clone()
            .try_into::<Vec<String>>()
            .unwrap(),
        ["#base", "#pending"]
    );
    assert!(matches!(
        report.pending,
        SettingInspectionPending::Pending {
            boundary: SettingActivationBoundary::ConfigurationReload,
            ..
        }
    ));
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    assert_eq!(app.interaction_compilation_count(), count);
    assert!(app.configuration_request_is_current(&owner));
    assert!(app.configuration_inspection_is_current(&inputs));
}

#[test]
fn prepared_reload_candidate_is_private_until_its_exact_publication() {
    let mut layers = selector_base();
    let mut app = open(layers.clone());
    layers
        .runtime
        .set_literal(
            "hints.selectors.buttons",
            "{mode='extend',selectors=['#reload']}",
        )
        .unwrap();
    let inputs = app.configuration_inspection_inputs();
    let prepared = app.prepare_configuration_reload(layers).unwrap();
    let candidate = prepared.inspection_candidate();
    assert_eq!(
        candidate.stage(),
        ConfigurationInspectionCandidateStage::Prepared
    );
    let request = SettingInspectionRequest::new("hints.selectors.buttons", None, true).unwrap();
    let report = inputs
        .inspect_setting_with_candidate(&request, &candidate)
        .unwrap();
    let expected = report.desired.unwrap().value;
    assert_ne!(report.active.unwrap().value, expected);
    assert!(matches!(
        report.pending,
        SettingInspectionPending::Pending {
            boundary: SettingActivationBoundary::Publication,
            ..
        }
    ));
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    assert!(!app.configuration_inspection_candidate_is_current(&candidate));
    assert!(
        app.configuration_inspection_inputs()
            .inspect_setting_with_candidate(&request, &candidate)
            .is_err()
    );
    assert_eq!(
        inspect(&app, "hints.selectors.buttons", None, false)
            .active
            .unwrap()
            .value,
        expected
    );
    assert_eq!(
        inputs
            .inspect_setting_with_candidate(&request, &candidate)
            .unwrap()
            .desired
            .unwrap()
            .value,
        expected
    );
}

#[test]
fn candidate_origins_resolve_in_their_own_changed_include_catalogue() {
    let fixture = Fixture::new();
    for root in ["root-a.toml", "root-b.toml"] {
        fs::write(fixture.0.join(root), "schema_version=3\ninclude=['child.toml']\n[ui]\nfont_size_pt=14\n[hints.selectors.buttons]\nmode='replace'\n").unwrap();
    }
    let app = open(fixture.layers("root-a.toml"));
    let inputs = app.configuration_inspection_inputs();
    let candidate = app
        .capture_configuration_inspection_candidate(
            &app.capture_configuration_request().unwrap(),
            &fixture.layers("root-b.toml"),
            ConfigurationApplyBoundary::RuntimeUpdate,
        )
        .unwrap();
    let report = inputs
        .inspect_setting_with_candidate(
            &SettingInspectionRequest::new("ui.font_size_pt", None, true).unwrap(),
            &candidate,
        )
        .unwrap();
    let active = report.active.unwrap();
    let desired = report.desired.unwrap();
    assert_eq!(active.value, desired.value);
    for (value, root, boundary) in [
        (active, "root-a.toml", InspectionBoundary::Desired),
        (desired, "root-b.toml", InspectionBoundary::Candidate),
    ] {
        let SettingExplanation::Ordinary { fields, .. } = value.explanation.unwrap() else {
            panic!("ordinary explanation expected");
        };
        let origin = fields[&Vec::new()].as_ref().unwrap();
        assert_eq!(origin.boundary, boundary);
        assert_eq!(origin.document_path(), Some(fixture.0.join(root).as_path()));
    }
    assert!(matches!(
        report.pending,
        SettingInspectionPending::Pending {
            boundary: SettingActivationBoundary::Publication,
            value_changed: false,
            source_changed: true
        }
    ));
    let report = inputs
        .inspect_setting_with_candidate(
            &SettingInspectionRequest::new("hints.selectors.buttons", None, true).unwrap(),
            &candidate,
        )
        .unwrap();
    assert!(matches!(
        report.desired.unwrap().explanation.unwrap(),
        SettingExplanation::Selectors {
            boundary: InspectionBoundary::Candidate,
            ..
        }
    ));
    assert!(matches!(
        report.pending,
        SettingInspectionPending::Pending {
            boundary: SettingActivationBoundary::ConfigurationReload,
            value_changed: false,
            source_changed: true
        }
    ));
    assert_eq!(
        inputs
            .inspect_setting(&SettingInspectionRequest::new("ui.font_size_pt", None, true).unwrap())
            .unwrap()
            .pending,
        SettingInspectionPending::Unchanged
    );
}

#[test]
fn invalid_or_foreign_candidates_never_replace_an_active_value() {
    let mut layers = ConfigurationLayers::new(Config::default());
    let mut app = open(layers.clone());
    let inputs = app.configuration_inspection_inputs();
    let owner = app.capture_configuration_request().unwrap();
    layers.runtime.set_literal("content.zoom", "99").unwrap();
    let candidate = app
        .capture_configuration_inspection_candidate(
            &owner,
            &layers,
            ConfigurationApplyBoundary::RuntimeUpdate,
        )
        .unwrap();
    let request = SettingInspectionRequest::new("content.zoom", None, true).unwrap();
    let report = inputs
        .inspect_setting_with_candidate(&request, &candidate)
        .unwrap();
    assert_eq!(
        report
            .active
            .unwrap()
            .value
            .unwrap()
            .as_float()
            .map(f64::to_bits),
        Some(1.0_f64.to_bits())
    );
    assert!(report.desired.is_err());
    assert_eq!(report.pending, SettingInspectionPending::Unknown);
    let foreign = open(ConfigurationLayers::new(Config::default()));
    assert!(
        foreign
            .configuration_inspection_inputs()
            .inspect_setting_with_candidate(&request, &candidate)
            .is_err()
    );
    assert!(!foreign.configuration_inspection_candidate_is_current(&candidate));
    app.update_configuration(ConfigurationLayers::new(Config::default()))
        .unwrap();
    assert!(!app.configuration_inspection_candidate_is_current(&candidate));
    assert!(
        app.capture_configuration_inspection_candidate(
            &owner,
            &layers,
            ConfigurationApplyBoundary::RuntimeUpdate
        )
        .is_err()
    );
    assert!(
        app.configuration_inspection_inputs()
            .inspect_setting_with_candidate(&request, &candidate)
            .is_err()
    );
    assert!(
        inputs
            .inspect_setting_with_candidate(&request, &candidate)
            .unwrap()
            .active
            .is_ok()
    );
}

#[test]
fn pending_live_site_rule_still_needs_reload_after_runtime_publication() {
    let mut layers = ConfigurationLayers::new(Config::default());
    let app = open(layers.clone());
    let inputs = app.configuration_inspection_inputs();
    let owner = app.capture_configuration_request().unwrap();
    layers
        .runtime
        .site_rules
        .push(rule("pending", 0, "hints.proxy_targets", "'validated'"));
    let request = SettingInspectionRequest::new("hints.proxy_targets", Some(URL), true).unwrap();
    for (application, expected) in [
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
            .capture_configuration_inspection_candidate(&owner, &layers, application)
            .unwrap();
        let report = inputs
            .inspect_setting_with_candidate(&request, &candidate)
            .unwrap();
        assert_eq!(report.active.unwrap().value.unwrap().as_str(), Some("off"));
        assert_eq!(
            report.desired.unwrap().value.unwrap().as_str(),
            Some("validated")
        );
        assert!(
            matches!(report.pending, SettingInspectionPending::Pending { boundary, .. } if boundary == expected)
        );
    }
    assert_eq!(
        inspect(&app, "hints.proxy_targets", Some(URL), true).pending,
        SettingInspectionPending::Unchanged
    );
}
