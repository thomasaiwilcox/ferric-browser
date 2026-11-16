use ferric_browser_application::{BrowserApplication, ConfigurationLayers};
use ferric_browser_config::{
    Config, ConfigError, EffectiveInteractionPolicy, HintSelectorFamily, HintSelectorMode,
    HintSelectorPolicy, InteractionAutoFollow, InteractionFamily, InteractionField,
    InteractionLabelField, InteractionPolicyPatch, InteractionPolicyRevision, PolicyRuleLocation,
    PolicySourceIdentity, SiteRule,
};
use std::{collections::BTreeMap, fs, path::PathBuf};

const URL: &str = "https://example.test/list";

fn rule(id: &str, priority: i32, patch: &str) -> SiteRule {
    SiteRule {
        id: id.into(),
        pattern: "https://example.test/*".into(),
        priority,
        set: BTreeMap::new(),
        interaction: Some(toml::from_str::<InteractionPolicyPatch>(patch).unwrap()),
    }
}

fn inputs() -> ConfigurationLayers {
    let mut base = Config::default();
    base.hints.chars = "ab".into();
    base.site_rules
        .push(rule("base", i32::MAX, "[labels]\nalphabet='cd'"));
    ConfigurationLayers::new(base)
}

fn revision(value: u64) -> InteractionPolicyRevision {
    InteractionPolicyRevision {
        profile: ferric_browser_core::ProfileId::from_display("profileid-1").unwrap(),
        profile_revision: 9,
        configuration_revision: value,
        policy_revision: value,
    }
}

fn compile(app: &BrowserApplication) -> Result<EffectiveInteractionPolicy, ConfigError> {
    let sources = app.configuration_policy_sources();
    sources.compile_interaction("work", revision(sources.revision()), URL)
}

#[test]
fn inspection_observes_shared_leaves_sections_and_same_id_shadowing_without_effects() {
    use ferric_browser_config::InteractionContributionPatch;
    let mut layers = inputs();
    layers.profile.set_literal("hints.chars", "'ef'").unwrap();
    let mut replacement = rule("base", i32::MIN, "[labels]\nalphabet='gh'");
    replacement.pattern = "https://elsewhere.test/*".into();
    layers.runtime.site_rules.push(replacement);
    layers.runtime.site_rules.push(rule("empty", 0, "[labels]"));
    layers.runtime.set_literal("hints.chars", "'ij'").unwrap();
    layers
        .temporary
        .site_rules
        .push(rule("clear", 0, "anchors=[]"));
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers).unwrap();
    let before = app.configuration_snapshot();
    let sources = app.configuration_policy_sources();
    let count = app.interaction_compilation_count();
    let result = sources
        .inspect_interaction("work", revision(sources.revision()), URL)
        .unwrap();
    let ordinary = compile(&app).unwrap();
    assert_eq!(result.policy.labels(), ordinary.labels());
    assert_eq!(result.policy.ordinary_labels(), ordinary.ordinary_labels());
    assert_eq!(result.policy.labels().alphabet(), "ij");
    assert_eq!(result.trace.len(), 5);
    assert!(
        matches!(&result.trace[0].patch, InteractionContributionPatch::SharedLabels(patch) if patch.alphabet.as_deref() == Some("ab"))
    );
    assert_eq!(
        result.trace[1].origin.source(),
        &PolicySourceIdentity::Profile {
            name: "work".into()
        }
    );
    assert!(
        matches!(&result.trace[2].patch, InteractionContributionPatch::SiteSection(patch) if patch.labels == Some(ferric_browser_config::InteractionLabelsPatch::default()))
    );
    assert_eq!(
        result.trace[3].origin.source(),
        &PolicySourceIdentity::Generated
    );
    assert_eq!(result.trace[4].origin.rule().unwrap().id(), "clear");
    assert!(result.policy.anchors().is_empty());
    assert_eq!(result.selection.shadowed.len(), 1);
    assert!(
        result
            .selection
            .survivors
            .iter()
            .any(|entry| entry.origin.rule().unwrap().id() == "base" && !entry.matched)
    );
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), sources);
    assert_eq!(app.interaction_compilation_count(), count);
}

#[test]
fn inspection_keeps_reload_rule_origins_separate_from_live_shared_origins() {
    let mut layers = inputs();
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers.clone()).unwrap();
    let old_revision = app.configuration_revision();
    layers.base.site_rules[0] = rule("base", 0, "[labels]\nmin_chars=3");
    layers.runtime.set_literal("hints.min_chars", "2").unwrap();
    app.update_configuration(layers.clone()).unwrap();
    let captured = app.configuration_policy_sources();
    let result = captured
        .inspect_interaction("work", revision(captured.revision()), URL)
        .unwrap();
    let site = result
        .trace
        .iter()
        .find(|entry| entry.origin.rule().is_some())
        .unwrap();
    assert_eq!(site.origin.source_revision(), old_revision);
    assert_eq!(site.origin.rule().unwrap().priority(), i32::MAX);
    assert_eq!(result.policy.labels().alphabet(), "cd");
    let shared = result.trace.last().unwrap();
    assert_eq!(shared.origin.source_revision(), captured.revision());
    assert_eq!(shared.origin.source(), &PolicySourceIdentity::Generated);
    assert_eq!(result.policy.labels().min_chars(), 2);
    let prepared = app.prepare_configuration_reload(layers).unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    let current = app.configuration_policy_sources();
    let result = current
        .inspect_interaction("work", revision(current.revision()), URL)
        .unwrap();
    let site = result
        .trace
        .iter()
        .find(|entry| entry.origin.rule().is_some())
        .unwrap();
    assert_eq!(site.origin.source_revision(), current.reload_revision());
    assert_eq!(site.origin.rule().unwrap().priority(), 0);
    assert_eq!(result.policy.labels().alphabet(), "ab");
    assert_eq!(
        captured
            .inspect_interaction("work", revision(captured.revision()), URL)
            .unwrap()
            .policy
            .labels()
            .alphabet(),
        "cd"
    );
}

#[test]
fn all_source_boundaries_interleave_explicit_globals_after_their_own_site_rules() {
    let mut layers = inputs();
    let mut app = BrowserApplication::new(Config::default());
    let mut retained = Vec::new();
    for (stage, ordinary, enhanced, source, rule_id) in [
        (0, "ab", "cd", PolicySourceIdentity::InMemory, Some("base")),
        (1, "ef", "cd", PolicySourceIdentity::InMemory, Some("base")),
        (
            2,
            "ef",
            "gh",
            PolicySourceIdentity::Generated,
            Some("generated"),
        ),
        (3, "ij", "ij", PolicySourceIdentity::Generated, None),
        (4, "kl", "kl", PolicySourceIdentity::CommandLine, None),
        (
            5,
            "kl",
            "mn",
            PolicySourceIdentity::Temporary,
            Some("temporary"),
        ),
        (6, "op", "op", PolicySourceIdentity::Temporary, None),
    ] {
        match stage {
            0 => {}
            1 => layers.profile.set_literal("hints.chars", "'ef'").unwrap(),
            2 => layers.runtime.site_rules.push(rule(
                "generated",
                i32::MIN,
                "[labels]\nalphabet='gh'",
            )),
            3 => layers.runtime.set_literal("hints.chars", "'ij'").unwrap(),
            4 => layers
                .command_line
                .set_literal("hints.chars", "'kl'")
                .unwrap(),
            5 => layers.temporary.site_rules.push(rule(
                "temporary",
                i32::MIN,
                "[labels]\nalphabet='mn'",
            )),
            6 => layers.temporary.set_literal("hints.chars", "'op'").unwrap(),
            _ => unreachable!(),
        }
        app.activate_configuration(layers.clone()).unwrap();
        let policy = compile(&app).unwrap();
        assert_eq!(policy.ordinary_labels().alphabet(), ordinary);
        assert_eq!(policy.labels().alphabet(), enhanced);
        let origin = policy
            .origin(InteractionField::Labels(InteractionLabelField::Alphabet))
            .unwrap();
        assert_eq!(origin.source(), &source);
        assert_eq!(origin.rule().map(PolicyRuleLocation::id), rule_id);
        assert_eq!(origin.source_revision(), app.configuration_revision());
        retained.push(policy);
    }
    assert_eq!(retained[0].labels().alphabet(), "cd");
    assert_eq!(retained[0].ordinary_labels().alphabet(), "ab");
}

#[test]
fn absent_global_leaves_do_not_reset_sites_but_an_explicit_default_does() {
    let mut layers = inputs();
    layers.base.hints = Config::default().hints;
    let default = layers.base.hints.chars.clone();
    layers.runtime.set_literal("content.zoom", "1.25").unwrap();
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers.clone()).unwrap();
    assert_eq!(compile(&app).unwrap().labels().alphabet(), "cd");
    layers
        .runtime
        .set_setting("hints.chars", toml::Value::String(default.clone()))
        .unwrap();
    app.activate_configuration(layers).unwrap();
    let policy = compile(&app).unwrap();
    assert_eq!(policy.labels().alphabet(), default);
    assert_eq!(policy.labels(), policy.ordinary_labels());
    assert!(!policy.enhanced_requested_for(InteractionFamily::Links));
}

#[test]
fn pending_rules_are_validated_but_only_reload_rules_compose_with_new_live_globals() {
    let mut layers = inputs();
    layers.base.site_rules[0].interaction = Some(
        toml::from_str(
            "[[anchors]]\nid='old-anchor'\ncode='m'\nfamilies=['links']\ntarget_selector='#old'",
        )
        .unwrap(),
    );
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers.clone()).unwrap();
    let old_revision = app.configuration_revision();
    layers.base.site_rules[0].interaction = Some(
        toml::from_str(
            "[[anchors]]\nid='new-anchor'\ncode='w'\nfamilies=['links']\ntarget_selector='#new'",
        )
        .unwrap(),
    );
    layers.runtime.set_literal("hints.chars", "'gh'").unwrap();
    let prepared = app
        .prepare_configuration_update(layers.clone())
        .unwrap()
        .prepare_policy_consumer(|_, sources| {
            sources.compile_interaction("work", revision(sources.revision()), URL)
        })
        .unwrap();
    let (_, policy) = app
        .publish_configuration_consumer(prepared, || Ok(()))
        .unwrap();
    assert_eq!(policy.anchors()[0].id, "old-anchor");
    assert_eq!(policy.labels().alphabet(), "gh");
    assert_eq!(
        policy
            .origin(InteractionField::Anchors)
            .unwrap()
            .source_revision(),
        old_revision
    );
    assert_eq!(
        policy
            .origin(InteractionField::Labels(InteractionLabelField::Alphabet))
            .unwrap()
            .source_revision(),
        app.configuration_revision()
    );
    let prepared = app.prepare_configuration_reload(layers).unwrap();
    app.publish_configuration_update(prepared, || Ok(()))
        .unwrap();
    let reloaded = compile(&app).unwrap();
    assert_eq!(reloaded.anchors()[0].id, "new-anchor");
    assert_eq!(
        reloaded
            .origin(InteractionField::Anchors)
            .unwrap()
            .source_revision(),
        app.configuration_revision()
    );
}

#[test]
fn whole_id_replacement_precedes_matching_and_equal_priority_keeps_authored_order() {
    let mut layers = inputs();
    let mut replacement = rule("base", i32::MIN, "[labels]\nalphabet='ef'");
    replacement.pattern = "https://elsewhere.test/*".into();
    layers.runtime.site_rules.push(replacement);
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers.clone()).unwrap();
    let policy = compile(&app).unwrap();
    assert_eq!(policy.labels(), policy.ordinary_labels());
    layers.runtime.site_rules.clear();
    layers.base.site_rules = vec![
        rule("one", 0, "[labels]\nalphabet='ef'"),
        rule("two", 0, "[labels]\nalphabet='gh'"),
    ];
    for expected in ["two", "one"] {
        app.activate_configuration(layers.clone()).unwrap();
        let policy = compile(&app).unwrap();
        let location = policy
            .origin(InteractionField::Labels(InteractionLabelField::Alphabet))
            .unwrap()
            .rule()
            .unwrap();
        assert_eq!(location.id(), expected);
        assert_eq!(location.authored_index(), 1);
        layers.base.site_rules.swap(0, 1);
    }
}

#[test]
fn disabled_and_empty_sections_retain_data_and_ordinary_settings_until_explicit_list_clear() {
    let mut layers = inputs();
    layers.base.site_rules[0] = rule(
        "base",
        0,
        "enabled=false\n[labels]\nalphabet='cd'\n[[anchors]]\nid='mac'\ncode='m'\nfamilies=['links']\ntarget_selector='#mac'",
    );
    layers.base.site_rules[0]
        .set
        .insert("content.zoom".into(), toml::Value::Float(1.25));
    layers.runtime.site_rules.push(rule("empty", 0, "[labels]"));
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers.clone()).unwrap();
    let disabled = compile(&app).unwrap();
    assert!(!disabled.enabled());
    assert_eq!(disabled.anchors().len(), 1);
    assert_eq!(disabled.labels().alphabet(), "cd");
    assert_eq!(
        app.configuration().site_rules[0].set["content.zoom"].as_float(),
        Some(1.25)
    );
    layers
        .temporary
        .site_rules
        .push(rule("clear", 0, "enabled=true\nanchors=[]"));
    app.activate_configuration(layers).unwrap();
    let policy = compile(&app).unwrap();
    assert!(policy.enabled());
    assert!(policy.anchors().is_empty());
    assert_eq!(policy.labels().alphabet(), "cd");
    assert_eq!(
        policy.origin(InteractionField::Anchors).unwrap().source(),
        &PolicySourceIdentity::Temporary
    );
}

#[test]
fn unsupported_site_surfaces_and_wrong_revision_reject_without_partial_output() {
    for profile in [false, true] {
        let mut layers = inputs();
        let extra = rule("unsupported", 0, "[labels]\nalphabet='ef'");
        if profile {
            layers.profile.site_rules.push(extra);
        } else {
            layers.command_line.site_rules.push(extra);
        }
        let mut app = BrowserApplication::new(Config::default());
        let before = app.configuration_snapshot();
        assert!(app.activate_configuration(layers.clone()).is_err());
        let prepared = app.prepare_configuration_update(layers).unwrap();
        let sources = prepared.policy_sources();
        assert!(
            sources
                .compile_interaction("work", revision(sources.revision()), URL)
                .is_err()
        );
        assert!(
            app.publish_configuration_update(prepared, || panic!(
                "unsupported source must not persist"
            ))
            .is_err()
        );
        assert_eq!(app.configuration_snapshot(), before);
    }
    let app = BrowserApplication::new(Config::default());
    let sources = app.configuration_policy_sources();
    assert!(
        sources
            .compile_interaction("work", revision(sources.revision() + 1), URL)
            .is_err()
    );
}

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        let files = Self(
            std::env::temp_dir().join(format!("ferric-policy-compile-{}", uuid::Uuid::new_v4())),
        );
        fs::create_dir_all(&files.0).unwrap();
        files
    }
    fn load(&self, source: &str) -> ConfigurationLayers {
        let path = self.0.join("config.toml");
        fs::write(&path, source).unwrap();
        let loaded = ferric_browser_config::load(path).unwrap();
        let mut layers = ConfigurationLayers::new(loaded.config);
        layers.base_authored = loaded.authored;
        layers
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn include_shared_leaf_provenance_preserves_selector_leaf_merge_and_catalogue_consistency() {
    let files = Files::new();
    fs::write(
        files.0.join("first.toml"),
        "[hints]\nchars='ab'\nmin_chars=2\n[hints.selectors.links]\nmode='replace'",
    )
    .unwrap();
    fs::write(
        files.0.join("second.toml"),
        "[hints]\nchars='cd'\n[hints.selectors.links]\nselectors=['#link']",
    )
    .unwrap();
    let layers = files.load("schema_version=3\ninclude=['first.toml','second.toml']\n[hints]\nauto_follow='never'\n[[site_rules]]\nid='site'\npattern='https://example.test/*'\n[site_rules.interaction.labels]\nmin_chars=3");
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers.clone()).unwrap();
    let policy = compile(&app).unwrap();
    assert_eq!(policy.labels().alphabet(), "cd");
    assert_eq!(policy.ordinary_labels().min_chars(), 2);
    assert_eq!(policy.labels().min_chars(), 3);
    assert_eq!(
        policy
            .ordinary_label_origin(InteractionLabelField::Alphabet)
            .unwrap()
            .source(),
        &PolicySourceIdentity::Authored { document_id: 1 }
    );
    assert_eq!(
        policy
            .ordinary_label_origin(InteractionLabelField::MinChars)
            .unwrap()
            .source(),
        &PolicySourceIdentity::Authored { document_id: 0 }
    );
    assert_eq!(
        policy
            .ordinary_label_origin(InteractionLabelField::AutoFollow)
            .unwrap()
            .source(),
        &PolicySourceIdentity::Authored { document_id: 2 }
    );
    let mut mismatched = layers;
    mismatched.base.hints.chars = "ef".into();
    let before = app.configuration_snapshot();
    assert!(app.activate_configuration(mismatched.clone()).is_err());
    let prepared = app.prepare_configuration_update(mismatched).unwrap();
    let sources = prepared.policy_sources();
    assert!(
        sources
            .compile_interaction("work", revision(sources.revision()), URL)
            .is_err()
    );
    assert!(
        app.publish_configuration_update(prepared, || panic!(
            "inconsistent catalogue must not persist"
        ))
        .is_err()
    );
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(compile(&app).unwrap().labels().alphabet(), "cd");
}

#[test]
fn replaced_include_arrays_never_reenter_composition() {
    let files = Files::new();
    fs::write(files.0.join("child.toml"), "[hints]\nchars='ab'\n[[site_rules]]\nid='removed'\npattern='https://example.test/*'\n[site_rules.interaction.labels]\nalphabet='cd'").unwrap();
    let layers = files.load("schema_version=3\ninclude=['child.toml']\nsite_rules=[]");
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers).unwrap();
    let policy = compile(&app).unwrap();
    assert_eq!(policy.labels().alphabet(), "ab");
    assert_eq!(policy.labels(), policy.ordinary_labels());
}

#[test]
fn final_validation_rejects_unrepaired_conflicts_but_allows_a_later_list_clear() {
    let mut layers = inputs();
    layers.base.site_rules[0].interaction = Some(serde_json::from_value(serde_json::json!({
        "anchors": [{"id":"anchor", "code":"a", "families":["links"], "target_selector":"#anchor"}]
    })).unwrap());
    let collections = (0..8)
        .map(|index| {
            serde_json::json!({
                "id":format!("c{index}"), "families":["links"],
                "codes":(0..64).map(|code| format!("b{index:02}{code:02}")).collect::<Vec<_>>(),
                "container_self":true, "item_selector":".item", "primary_self":true, "order":"dom"
            })
        })
        .collect::<Vec<_>>();
    let mut generated = rule("collection", 0, "");
    generated.interaction =
        Some(serde_json::from_value(serde_json::json!({"collections":collections})).unwrap());
    layers.runtime.site_rules.push(generated);
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers.clone()).unwrap();
    // Each raw section is valid, but the composed reservation count is 513.
    assert!(compile(&app).is_err());
    layers
        .temporary
        .site_rules
        .push(rule("clear", 0, "anchors=[]"));
    app.activate_configuration(layers).unwrap();
    let policy = compile(&app).unwrap();
    assert!(policy.anchors().is_empty());
    assert_eq!(
        policy
            .collections()
            .iter()
            .map(|collection| collection.codes.len())
            .sum::<usize>(),
        512
    );
}

#[test]
fn shared_label_projection_is_sparse_for_minimum_width_and_auto_follow_too() {
    let mut layers = inputs();
    layers.base.site_rules[0] = rule(
        "base",
        0,
        "[labels]\nalphabet='cd'\nmin_chars=3\nauto_follow='never'",
    );
    layers.profile.set_literal("hints.min_chars", "4").unwrap();
    layers.runtime.set_literal("hints.chars", "'ab'").unwrap();
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(layers.clone()).unwrap();
    let policy = compile(&app).unwrap();
    assert_eq!(policy.labels().alphabet(), "ab");
    assert_eq!(policy.ordinary_labels().min_chars(), 4);
    assert_eq!(policy.labels().min_chars(), 3);
    assert_eq!(policy.labels().auto_follow(), InteractionAutoFollow::Never);
    layers
        .command_line
        .set_literal("hints.min_chars", "5")
        .unwrap();
    app.activate_configuration(layers.clone()).unwrap();
    let policy = compile(&app).unwrap();
    assert_eq!(policy.ordinary_labels().min_chars(), 5);
    assert_eq!(policy.labels().min_chars(), 5);
    assert_eq!(policy.labels().auto_follow(), InteractionAutoFollow::Never);
    layers
        .temporary
        .set_literal("hints.auto_follow", "'full-match'")
        .unwrap();
    app.activate_configuration(layers).unwrap();
    let policy = compile(&app).unwrap();
    assert_eq!(policy.labels(), policy.ordinary_labels());
    assert!(!policy.enhanced_requested_for(InteractionFamily::Links));
    assert_eq!(
        policy
            .origin(InteractionField::Labels(InteractionLabelField::AutoFollow))
            .unwrap()
            .source(),
        &PolicySourceIdentity::Temporary
    );
}

#[test]
fn invalid_desired_catalogue_rejects_even_with_a_valid_retained_reload_capture() {
    let files = Files::new();
    let good = files.load("schema_version=3\n[hints]\nchars='ab'");
    let mut bad = good.clone();
    bad.base.hints.chars = "cd".into();
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(good.clone()).unwrap();
    let before = app.configuration_snapshot();
    let prepared = app.prepare_configuration_update(bad.clone()).unwrap();
    let sources = prepared.policy_sources();
    assert_eq!(sources.reload_layers(), &good);
    assert_eq!(sources.desired_layers(), &bad);
    assert!(
        sources
            .compile_interaction("work", revision(sources.revision()), URL)
            .unwrap_err()
            .to_string()
            .contains("catalogue")
    );
    assert!(
        app.publish_configuration_update(prepared, || panic!(
            "invalid metadata must reject before persistence"
        ))
        .is_err()
    );
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(compile(&app).unwrap().labels().alphabet(), "ab");
}

#[test]
fn complete_activation_rejects_invalid_catalogue_without_replacing_reload_sources() {
    let files = Files::new();
    let good = files.load("schema_version=3\n[hints]\nchars='ab'");
    let mut bad = good.clone();
    bad.base.hints.chars = "cd".into();
    let mut app = BrowserApplication::new(Config::default());
    app.activate_configuration(good.clone()).unwrap();
    let before = app.configuration_snapshot();
    let before_sources = app.configuration_policy_sources();
    assert!(app.activate_configuration(bad).is_err());
    assert_eq!(app.configuration_snapshot(), before);
    assert_eq!(app.configuration_policy_sources(), before_sources);
    let sources = app.configuration_policy_sources();
    assert_eq!(sources.reload_layers(), &good);
    assert_eq!(sources.desired_layers(), &good);
    assert_eq!(compile(&app).unwrap().labels().alphabet(), "ab");
    let prepared = app
        .prepare_configuration_reload(good)
        .unwrap()
        .prepare_policy_consumer(|_, sources| {
            sources.compile_interaction("work", revision(sources.revision()), URL)
        })
        .unwrap();
    app.publish_configuration_consumer(prepared, || Ok(()))
        .unwrap();
    assert_eq!(compile(&app).unwrap().labels().alphabet(), "ab");
}

#[test]
fn standalone_resolved_selectors_do_not_acquire_authored_input_limits_during_label_projection() {
    for count in [16, 17, 31, 32, 33] {
        let mut layers = inputs();
        layers.base.hints.selectors.insert(
            HintSelectorFamily::Links,
            HintSelectorPolicy {
                mode: HintSelectorMode::Extend,
                selectors: (0..count).map(|index| format!(".item{index}")).collect(),
            },
        );
        assert_eq!(
            ferric_browser_config::validate(&layers.base).is_ok(),
            count <= 32
        );
        let mut app = BrowserApplication::new(Config::default());
        if count <= 32 {
            app.activate_configuration(layers).unwrap();
            assert!(
                compile(&app).is_ok(),
                "resolved selector count {count} incorrectly rejected"
            );
        } else {
            assert!(app.activate_configuration(layers).is_err());
        }
    }
}
