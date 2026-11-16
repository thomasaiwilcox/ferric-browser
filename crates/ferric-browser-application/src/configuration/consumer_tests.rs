//! A test-only typed consumer exercises the production resolver, composition
//! and transaction contracts. It adds no schema field or shipped settings.
use std::{collections::BTreeMap, sync::Arc};

use ferric_browser_config::{
    EffectiveInteractionPolicy, InteractionField, InteractionPolicyBuilder, InteractionPolicyPatch,
    InteractionPolicyRevision, PolicyFieldOrigin, PolicySourceIdentity, SelectedSitePolicyRule,
    SelectedSitePolicyRules, SitePolicySection, SitePolicySourceRevisions, SiteRule,
    compose_site_policy_section, select_site_policy_rules,
};
use serde::Deserialize;

use super::*;
use crate::BrowserApplication;

const URL: &str = "https://example.test/list";

#[test]
fn source_snapshots_retain_startup_origins_across_live_and_reload_publication() {
    let mut app = BrowserApplication::new(Config::default());
    let mut initial = ConfigurationLayers::new(Config::default());
    initial
        .runtime
        .set_literal("session.lazy_restore", "false")
        .unwrap();
    initial.runtime.set_literal("content.zoom", "1.25").unwrap();
    app.activate_configuration(initial).unwrap();
    let original = app.configuration_policy_sources();
    let mut changed = app.configuration_layers().clone();
    changed
        .runtime
        .set_literal("session.lazy_restore", "true")
        .unwrap();
    changed
        .runtime
        .set_literal("tabs.presentation", "'windows'")
        .unwrap();
    changed.runtime.set_literal("content.zoom", "1.5").unwrap();
    changed
        .runtime
        .set_literal(
            "hints.selectors.buttons",
            "{mode='replace',selectors=['#pending']}",
        )
        .unwrap();
    app.update_configuration(changed.clone()).unwrap();
    let live = app.configuration_policy_sources();
    assert_eq!(live.startup_layers(), original.startup_layers());
    assert_eq!(live.startup_revision(), original.startup_revision());
    assert_eq!(live.reload_revision(), original.reload_revision());
    assert!(!app.configuration().session.lazy_restore);
    assert!((app.configuration().content.zoom - 1.5).abs() < f64::EPSILON);
    assert!(app.configuration().hints.selectors.is_empty());
    let prepared = app.prepare_configuration_reload(changed.clone()).unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    let reload = app.configuration_policy_sources();
    assert_eq!(reload.startup_layers(), original.startup_layers());
    assert_eq!(reload.startup_revision(), original.startup_revision());
    assert_eq!(reload.reload_layers(), &changed);
    assert!(reload.reload_revision() > live.reload_revision());
    assert!(!app.configuration().session.lazy_restore);
    assert_eq!(
        app.configuration().tabs.presentation,
        ferric_browser_config::TabsPresentation::Internal
    );
    assert!(!app.configuration().hints.selectors.is_empty());
    // A captured view is immutable, and full activation establishes new origins.
    assert_eq!(
        original.desired_layers().runtime.settings["content.zoom"].as_float(),
        Some(1.25)
    );
    app.activate_configuration(changed).unwrap();
    let activated = app.configuration_policy_sources();
    assert_eq!(activated.startup_revision(), activated.revision());
    assert_eq!(activated.startup_layers(), activated.desired_layers());
    assert!(app.configuration().session.lazy_restore);
    assert_eq!(
        app.configuration().tabs.presentation,
        ferric_browser_config::TabsPresentation::Windows
    );
}

#[test]
fn foreign_preparation_rejects_equal_revisions_before_persistence() {
    let owner = BrowserApplication::new(Config::default());
    let mut other = BrowserApplication::new(Config::default());
    let before = other.configuration_snapshot();
    let before_sources = other.configuration_policy_sources();
    let mut layers = owner.configuration_layers().clone();
    layers
        .temporary
        .set_literal("content.zoom", "1.75")
        .unwrap();
    let prepared = owner.prepare_configuration_update(layers).unwrap();
    assert_eq!(
        owner.configuration_revision(),
        other.configuration_revision()
    );
    let persisted = std::cell::Cell::new(false);
    assert!(
        other
            .publish_configuration_update(prepared, || {
                persisted.set(true);
                Ok(())
            })
            .is_err()
    );
    assert!(!persisted.get());
    assert_eq!(other.configuration_snapshot(), before);
    assert_eq!(other.configuration_policy_sources(), before_sources);
}

#[test]
fn foreign_compiled_consumer_cannot_escape_the_publication_owner_check() {
    let owner = BrowserApplication::new(Config::default());
    let mut other = BrowserApplication::new(Config::default());
    let before = other.configuration_snapshot();
    let prepared = owner
        .prepare_configuration_reload(ConfigurationLayers::new(Config::default()))
        .unwrap()
        .prepare_policy_consumer(|_, sources| Ok(sources.clone()))
        .unwrap();
    let persisted = std::cell::Cell::new(false);
    assert!(
        other
            .publish_configuration_consumer(prepared, || {
                persisted.set(true);
                Ok(())
            })
            .is_err()
    );
    assert!(!persisted.get());
    assert_eq!(other.configuration_snapshot(), before);
}

#[test]
fn exhausted_revision_rejects_before_sources_or_effective_state_change() {
    let mut state = ConfigurationState::new(Config::default());
    state.revision = u64::MAX;
    let before = state.clone();
    let mut candidate = Config::default();
    candidate.hints.chars = "ab".into();
    for boundary in [
        ConfigurationApplyBoundary::RuntimeUpdate,
        ConfigurationApplyBoundary::Reload,
    ] {
        assert!(
            state
                .update_layers_at(ConfigurationLayers::new(candidate.clone()), boundary)
                .is_err()
        );
        assert_eq!(state, before);
    }
    assert!(
        state
            .activate_layers(ConfigurationLayers::new(candidate.clone()))
            .is_err()
    );
    assert_eq!(state, before);
    assert!(state.replace_effective(candidate).is_err());
    assert_eq!(state, before);
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DummyPatch {
    value: Option<u8>,
    entries: Option<Vec<String>>,
}

impl DummyPatch {
    fn validate(&self) -> Result<(), String> {
        if self.value.is_some_and(|value| value > 10)
            || self.entries.as_ref().is_some_and(|entries| {
                entries.len() > 4
                    || entries
                        .iter()
                        .any(|entry| entry.len() > 8 || !entry.is_ascii())
            })
        {
            return Err("dummy patch outside test schema bounds".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
struct RawRule {
    source: PolicySourceIdentity,
    revision: u64,
    index: usize,
    rule: SiteRule,
}

impl RawRule {
    fn matches(&self, selected: &SelectedSitePolicyRule<'_>) -> bool {
        let origin = selected.origin();
        origin.source() == &self.source
            && origin.source_revision() == self.revision
            && origin.rule().is_some_and(|location| {
                location.id() == self.rule.id
                    && usize::from(location.authored_index()) == self.index
                    && location.priority() == self.rule.priority
            })
            && selected.rule() == &self.rule
    }
}

#[derive(Clone)]
struct DummyCatalogue {
    raw: Vec<RawRule>,
    patches: Vec<(RawRule, DummyPatch)>,
}

struct DummySection {
    catalogue: Arc<DummyCatalogue>,
    /// Independently enumerated from the exact prepared source layers. Never
    /// reconstructed from the supplied patch catalogue or selected matches.
    expected_raw: Vec<RawRule>,
}

#[derive(Default)]
struct DummyValues {
    value: u8,
    entries: Vec<String>,
    origins: Vec<PolicyFieldOrigin>,
}

struct DummySnapshot {
    values: DummyValues,
    catalogue: Arc<DummyCatalogue>,
}

impl SitePolicySection for DummySection {
    type Patch = DummyPatch;
    type Accumulator = DummyValues;
    type Snapshot = Arc<DummySnapshot>;

    fn validate_inputs(&self, selected: &SelectedSitePolicyRules<'_>) -> Result<(), String> {
        let catalogue = &self.catalogue;
        if catalogue.raw != self.expected_raw || catalogue.raw.len() != catalogue.patches.len() {
            return Err("dummy catalogue shape mismatch".into());
        }
        for raw in &catalogue.raw {
            if catalogue
                .patches
                .iter()
                .filter(|(key, _)| key == raw)
                .count()
                != 1
                || catalogue.raw.iter().filter(|other| *other == raw).count() != 1
            {
                return Err("dummy catalogue duplicate or missing raw identity".into());
            }
        }
        for (key, patch) in &catalogue.patches {
            if !catalogue.raw.contains(key) {
                return Err("dummy catalogue has an orphan or mismatched entry".into());
            }
            patch.validate()?;
        }
        if selected
            .selected()
            .iter()
            .any(|rule| !catalogue.raw.iter().any(|raw| raw.matches(rule)))
        {
            return Err("dummy catalogue belongs to another selected context".into());
        }
        Ok(())
    }

    fn initial(&self) -> Self::Accumulator {
        DummyValues::default()
    }

    fn patch<'a>(&'a self, selected: &'a SelectedSitePolicyRule<'_>) -> Option<&'a Self::Patch> {
        self.catalogue
            .patches
            .iter()
            .find(|(key, _)| key.matches(selected))
            .map(|(_, patch)| patch)
    }

    fn apply(
        &self,
        accumulator: &mut Self::Accumulator,
        patch: &Self::Patch,
        origin: &Arc<PolicyFieldOrigin>,
    ) -> Result<(), String> {
        if let Some(value) = patch.value {
            accumulator.value = value;
        }
        if let Some(entries) = &patch.entries {
            accumulator.entries.clone_from(entries);
        }
        accumulator.origins.push(origin.as_ref().clone());
        Ok(())
    }

    fn finish(&self, values: Self::Accumulator) -> Result<Self::Snapshot, String> {
        Ok(Arc::new(DummySnapshot {
            values,
            catalogue: Arc::clone(&self.catalogue),
        }))
    }
}

struct Pair {
    interaction: EffectiveInteractionPolicy,
    dummy: Arc<DummySnapshot>,
    revision: u64,
}

fn rule(id: &str, priority: i32, enabled: bool) -> SiteRule {
    SiteRule {
        id: id.into(),
        pattern: "https://example.test/*".into(),
        priority,
        set: BTreeMap::new(),
        interaction: Some(InteractionPolicyPatch {
            enabled: Some(enabled),
            ..Default::default()
        }),
    }
}

fn fixture() -> (ConfigurationLayers, Arc<DummyCatalogue>) {
    let mut layers = ConfigurationLayers::new(Config {
        site_rules: vec![rule("low", 100, false), rule("replaced", 0, false)],
        ..Config::default()
    });
    let mut replacement = rule("replaced", -10, false);
    replacement.pattern = "https://elsewhere.test/*".into();
    layers.runtime.site_rules = vec![rule("middle", -100, false), replacement];
    layers.temporary.site_rules = vec![rule("high", -200, true)];
    let mut raw = Vec::new();
    for (source, revision, rules) in [
        (PolicySourceIdentity::InMemory, 11, &layers.base.site_rules),
        (
            PolicySourceIdentity::Generated,
            22,
            &layers.runtime.site_rules,
        ),
        (
            PolicySourceIdentity::Temporary,
            33,
            &layers.temporary.site_rules,
        ),
    ] {
        for (index, rule) in rules.iter().enumerate() {
            raw.push(RawRule {
                source: source.clone(),
                revision,
                index,
                rule: rule.clone(),
            });
        }
    }
    let patches = raw
        .iter()
        .enumerate()
        .map(|(index, raw)| {
            (
                raw.clone(),
                DummyPatch {
                    value: Some(u8::try_from(index).unwrap()),
                    entries: Some(if index == 4 {
                        Vec::new()
                    } else {
                        vec![raw.rule.id.clone()]
                    }),
                },
            )
        })
        .collect();
    (layers, Arc::new(DummyCatalogue { raw, patches }))
}

fn expected_raw(layers: &ConfigurationLayers) -> Vec<RawRule> {
    let mut inventory = Vec::new();
    let mut has_authored_array = false;
    for (index, document) in layers.base_authored.documents().iter().enumerate() {
        if let Some(rules) = document.site_rules() {
            has_authored_array = true;
            for (rule_index, rule) in rules.iter().enumerate() {
                inventory.push(RawRule {
                    source: PolicySourceIdentity::Authored {
                        document_id: u32::try_from(index).unwrap(),
                    },
                    revision: 11,
                    index: rule_index,
                    rule: rule.clone(),
                });
            }
        }
    }
    if !has_authored_array {
        inventory.extend(
            layers
                .base
                .site_rules
                .iter()
                .enumerate()
                .map(|(index, rule)| RawRule {
                    source: PolicySourceIdentity::InMemory,
                    revision: 11,
                    index,
                    rule: rule.clone(),
                }),
        );
    }
    for (source, revision, rules) in [
        (
            PolicySourceIdentity::Generated,
            22,
            &layers.runtime.site_rules,
        ),
        (
            PolicySourceIdentity::Temporary,
            33,
            &layers.temporary.site_rules,
        ),
    ] {
        inventory.extend(rules.iter().enumerate().map(|(index, rule)| RawRule {
            source: source.clone(),
            revision,
            index,
            rule: rule.clone(),
        }));
    }
    inventory
}

fn compile(
    effective: &Config,
    layers: &ConfigurationLayers,
    revision: u64,
    catalogue: Arc<DummyCatalogue>,
) -> Result<Pair, ConfigError> {
    // This focused fixture uses a reload boundary and no pending rule change.
    // The full compiler must respect pending/reload-required settings as well.
    if effective.site_rules != layers.resolve()?.site_rules {
        return Err(ConfigError::Validation(
            "policy rules are not active at this boundary".into(),
        ));
    }
    let feed = select_site_policy_rules(
        &layers.base,
        &layers.base_authored,
        &layers.runtime,
        &layers.temporary,
        SitePolicySourceRevisions {
            authored: 11,
            generated: 22,
            temporary: 33,
        },
        URL,
    )?;
    let policy_revision = InteractionPolicyRevision {
        profile: ferric_browser_core::ProfileId::from_display("profileid-1").unwrap(),
        profile_revision: 0,
        configuration_revision: revision,
        policy_revision: revision,
    };
    let interaction = InteractionPolicyBuilder::default()
        .finish_selected_rules(&feed, policy_revision)
        .map_err(ConfigError::Validation)?;
    let dummy = compose_site_policy_section(
        &feed,
        &DummySection {
            catalogue,
            expected_raw: expected_raw(layers),
        },
    )
    .map_err(ConfigError::Validation)?;
    Ok(Pair {
        interaction,
        dummy,
        revision,
    })
}

fn prepare(
    app: &BrowserApplication,
    layers: ConfigurationLayers,
    catalogue: Arc<DummyCatalogue>,
) -> Result<PreparedConfigurationConsumer<Pair>, ConfigError> {
    app.prepare_configuration_reload(layers)?
        .prepare_consumer(|effective, layers, revision| {
            compile(effective, layers, revision, catalogue)
        })
}

#[test]
fn typed_consumers_share_selection_provenance_and_one_configuration_publication() {
    let mut app = BrowserApplication::new(Config::default());
    let (mut layers, catalogue) = fixture();
    let prepared = prepare(&app, layers.clone(), Arc::clone(&catalogue)).unwrap();
    assert_eq!(app.configuration_revision(), 0);
    layers.temporary.site_rules.clear();
    let (configuration, pair) = app
        .publish_configuration_consumer(prepared, || Ok(()))
        .unwrap();
    assert_eq!(configuration.revision, 1);
    assert_eq!(pair.revision, configuration.revision);
    assert_eq!(
        pair.interaction.revision().configuration_revision,
        configuration.revision
    );
    assert!(pair.interaction.enabled());
    assert_eq!(pair.dummy.values.value, 4);
    assert!(pair.dummy.values.entries.is_empty());
    let origins = &pair.dummy.values.origins;
    assert_eq!(
        origins
            .iter()
            .map(|origin| origin.rule().unwrap().id())
            .collect::<Vec<_>>(),
        ["low", "middle", "high"]
    );
    assert_eq!(
        pair.interaction.origin(InteractionField::Enabled),
        origins.last()
    );
    assert!(Arc::ptr_eq(&pair.dummy.catalogue, &catalogue));
    assert_eq!(app.configuration().site_rules.len(), 4);
}

#[test]
fn shadowed_and_nonmatching_dummy_patches_validate_before_any_publication() {
    let app = BrowserApplication::new(Config::default());
    let before = app.configuration_snapshot();
    for index in [1, 3] {
        let (layers, mut catalogue) = fixture();
        Arc::make_mut(&mut catalogue).patches[index].1.value = Some(11);
        assert!(prepare(&app, layers, catalogue).is_err());
        assert_eq!(app.configuration_snapshot(), before);
    }
}

#[test]
fn dummy_catalogue_rejects_duplicate_orphan_and_wrong_source_entries() {
    let app = BrowserApplication::new(Config::default());
    for mutation in 0..4 {
        let (layers, mut catalogue) = fixture();
        let mutable = Arc::make_mut(&mut catalogue);
        match mutation {
            0 => mutable.patches[1] = mutable.patches[0].clone(),
            1 => mutable.patches[0].0.source = PolicySourceIdentity::Temporary,
            2 => mutable.patches[0].0.revision += 1,
            _ => mutable.patches[0].0.rule.priority += 1,
        }
        assert!(prepare(&app, layers, catalogue).is_err());
        assert_eq!(app.configuration_revision(), 0);
    }
    assert!(serde_json::from_str::<DummyPatch>(r#"{"value":1,"executable":"no"}"#).is_err());
}

#[test]
fn regression_dummy_cannot_drop_shadowed_or_nonmatching_raw_input() {
    let app = BrowserApplication::new(Config::default());
    for index in [1, 3] {
        let (layers, mut catalogue) = fixture();
        let mutable = Arc::make_mut(&mut catalogue);
        mutable.raw.remove(index);
        mutable.patches.remove(index);
        assert!(
            prepare(&app, layers, catalogue).is_err(),
            "omitted raw entry {index}"
        );
    }
}

#[test]
fn regression_dummy_cannot_forge_unselected_raw_identity_and_patch_together() {
    let app = BrowserApplication::new(Config::default());
    for index in [1, 3] {
        let (layers, mut catalogue) = fixture();
        let mutable = Arc::make_mut(&mut catalogue);
        mutable.raw[index].revision += 1;
        mutable.patches[index].0 = mutable.raw[index].clone();
        assert!(
            prepare(&app, layers, catalogue).is_err(),
            "forged raw entry {index}"
        );
    }
}

#[test]
fn replaced_include_dummy_entries_still_require_validation_and_membership() {
    let loaded = ferric_browser_config::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ferric-browser-config/tests/fixtures/site-policy/includes/root.toml"
    ))
    .unwrap();
    let mut layers = ConfigurationLayers::new(loaded.config);
    layers.base_authored = loaded.authored;
    let raw = expected_raw(&layers);
    assert_eq!(raw.len(), 1);
    assert!(layers.base.site_rules.is_empty());
    let catalogue = Arc::new(DummyCatalogue {
        patches: raw
            .iter()
            .map(|key| {
                (
                    key.clone(),
                    DummyPatch {
                        value: Some(1),
                        entries: None,
                    },
                )
            })
            .collect(),
        raw,
    });
    let app = BrowserApplication::new(Config::default());
    assert!(prepare(&app, layers.clone(), Arc::clone(&catalogue)).is_ok());
    let mut invalid = (*catalogue).clone();
    invalid.patches[0].1.value = Some(11);
    assert!(prepare(&app, layers.clone(), Arc::new(invalid)).is_err());
    let omitted = DummyCatalogue {
        raw: Vec::new(),
        patches: Vec::new(),
    };
    assert!(prepare(&app, layers, Arc::new(omitted)).is_err());
}

#[test]
fn runtime_boundary_cannot_publish_desired_reload_only_rules_as_active_consumer() {
    let app = BrowserApplication::new(Config::default());
    let (layers, catalogue) = fixture();
    let prepared = app.prepare_configuration_update(layers).unwrap();
    assert!(prepared.snapshot().effective.site_rules.is_empty());
    assert!(!prepared.snapshot().pending_changes.is_empty());
    assert!(
        prepared
            .prepare_consumer(|effective, layers, revision| compile(
                effective, layers, revision, catalogue
            ))
            .is_err()
    );
    assert_eq!(app.configuration_revision(), 0);
}

#[test]
fn typed_publication_reuses_stale_reservation_and_persistence_rejection() {
    let mut app = BrowserApplication::new(Config::default());
    let (layers, catalogue) = fixture();
    let original = app.configuration_snapshot();
    let prepared = prepare(&app, layers.clone(), Arc::clone(&catalogue)).unwrap();
    assert!(
        app.publish_configuration_consumer(prepared, || Err(ConfigError::Validation(
            "disk rejected".into()
        )))
        .is_err()
    );
    assert_eq!(app.configuration_snapshot(), original);

    let prepared = prepare(&app, layers.clone(), Arc::clone(&catalogue)).unwrap();
    let reservation = app.reserve_configuration_reload().unwrap();
    assert!(
        app.publish_configuration_consumer(prepared, || panic!("reserved update must not persist"))
            .is_err()
    );
    assert_eq!(app.configuration_snapshot(), original);
    app.release_configuration_reload(reservation).unwrap();

    let prepared = prepare(&app, layers, catalogue).unwrap();
    app.update_configuration(ConfigurationLayers::new(Config::default()))
        .unwrap();
    let intervening = app.configuration_snapshot();
    assert!(
        app.publish_configuration_consumer(prepared, || panic!("stale update must not persist"))
            .is_err()
    );
    assert_eq!(app.configuration_snapshot(), intervening);
}
