use super::*;
use ferric_browser_config::{SitePolicyExchange, SitePolicyExchangeOperation as Operation};
use ferric_browser_core::{SitePolicyExportSubject, SitePolicyRuleRef, SitePolicySource};

fn upsert(id: &str) -> Operation {
    Operation::UpsertInteraction {
        id: id.into(),
        pattern: PATTERN.into(),
        priority: -7,
        interaction: Box::new(patch("enabled=true")),
    }
}
fn remove(id: &str) -> Operation {
    Operation::UnsetInteraction {
        id: id.into(),
        pattern: PATTERN.into(),
    }
}

#[test]
fn import_is_one_nonactivating_batch_with_local_settings_and_stable_rule_order() {
    let mut layers = ConfigurationLayers::new(Config::default());
    let mut mixed = rule("mixed");
    mixed
        .set
        .insert("content.zoom".into(), toml::Value::Float(1.5));
    let mut lower = rule("lower");
    lower
        .set
        .insert("content.javascript".into(), toml::Value::Boolean(false));
    layers.base.site_rules.push(lower.clone());
    layers.runtime.site_rules = vec![mixed.clone(), rule("only"), rule("neighbor")];
    let app = open(layers);
    let before = app.configuration_snapshot();
    let document = SitePolicyExchange::new(vec![
        upsert("mixed"),
        remove("only"),
        upsert("new-b"),
        upsert("lower"),
        upsert("new-a"),
    ])
    .unwrap();
    let draft = app
        .import_site_policy_draft(&document, false, true)
        .unwrap();
    assert_eq!(draft.rules.len(), 5);
    assert_eq!(draft.rules[0].proposed.as_ref().unwrap().set, mixed.set);
    assert_eq!(draft.rules[3].copied_from.as_ref(), Some(&lower));
    assert_eq!(draft.rules[3].proposed.as_ref().unwrap().set, lower.set);
    assert!(draft.rules[1].proposed.is_none());
    let prepared = app
        .prepare_site_policy_draft(&draft.id, draft.revision)
        .unwrap();
    let sources = prepared.prepared().policy_sources();
    assert_eq!(
        sources
            .desired_layers()
            .runtime
            .site_rules
            .iter()
            .map(|rule| rule.id.as_str())
            .collect::<Vec<_>>(),
        vec!["mixed", "neighbor", "new-b", "lower", "new-a"]
    );
    assert_eq!(app.configuration_snapshot(), before);
    // One batch occupies one profile slot, regardless of its operation count.
    for _ in 0..15 {
        app.import_site_policy_draft(
            &SitePolicyExchange::new(vec![upsert("another")]).unwrap(),
            false,
            false,
        )
        .unwrap();
    }
    assert!(
        app.import_site_policy_draft(&document, false, true)
            .is_err()
    );
    assert!(app.site_policy_draft(&draft.id).unwrap().dirty);
}

#[test]
fn all_guards_fail_atomically_and_interaction_only_retargeting_remains_explicit() {
    let mut layers = ConfigurationLayers::new(Config::default());
    let mut mixed = rule("mixed");
    mixed
        .set
        .insert("content.zoom".into(), toml::Value::Float(2.0));
    layers.base.site_rules.push(mixed.clone());
    layers.runtime.site_rules = vec![rule("plain"), mixed];
    let app = open(layers);
    let before = app.configuration_snapshot();
    assert!(
        app.import_site_policy_draft(
            &SitePolicyExchange::new(vec![upsert("plain")]).unwrap(),
            false,
            false
        )
        .is_err()
    );
    for operation in [
        Operation::UpsertInteraction {
            id: "mixed".into(),
            pattern: "https://retarget.test/*".into(),
            priority: -7,
            interaction: Box::new(patch("enabled=true")),
        },
        Operation::UpsertInteraction {
            id: "mixed".into(),
            pattern: PATTERN.into(),
            priority: 3,
            interaction: Box::new(patch("enabled=true")),
        },
        Operation::UnsetInteraction {
            id: "mixed".into(),
            pattern: "https://other.test/*".into(),
        },
        remove("absent"),
    ] {
        let document = SitePolicyExchange::new(vec![upsert("new"), operation]).unwrap();
        assert!(
            app.import_site_policy_draft(&document, false, true)
                .is_err()
        );
        assert_eq!(app.configuration_snapshot(), before);
    }
    let changed = SitePolicyExchange::new(vec![Operation::UpsertInteraction {
        id: "plain".into(),
        pattern: "https://retarget.test/*".into(),
        priority: 3,
        interaction: Box::new(patch("enabled=true")),
    }])
    .unwrap();
    let draft = app.import_site_policy_draft(&changed, false, true).unwrap();
    assert_eq!(draft.rules[0].original.as_ref().unwrap().pattern, PATTERN);
    assert_eq!(draft.rules[0].proposed.as_ref().unwrap().priority, 3);
    assert_eq!(
        draft.rules[0].proposed.as_ref().unwrap().pattern,
        "https://retarget.test/*"
    );
    for _ in 0..15 {
        app.import_site_policy_draft(
            &SitePolicyExchange::new(vec![upsert("new")]).unwrap(),
            false,
            false,
        )
        .unwrap();
    }
    assert!(app.import_site_policy_draft(&changed, false, true).is_err());
}

#[test]
fn removal_round_trip_preserves_set_and_reveals_lower_rules_without_exporting_set() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.base.site_rules.push(rule("only"));
    let mut mixed = rule("mixed");
    mixed
        .set
        .insert("content.images".into(), toml::Value::Boolean(false));
    layers.runtime.site_rules = vec![mixed.clone(), rule("only")];
    let app = open(layers);
    let document =
        SitePolicyExchange::new(vec![remove("mixed"), remove("only"), upsert("new")]).unwrap();
    let draft = app
        .import_site_policy_draft(&document, false, false)
        .unwrap();
    assert_eq!(draft.rules[0].proposed.as_ref().unwrap().set, mixed.set);
    assert!(
        draft.rules[0]
            .proposed
            .as_ref()
            .unwrap()
            .interaction
            .is_none()
    );
    let prepared = app
        .prepare_site_policy_draft(&draft.id, draft.revision)
        .unwrap();
    let revealed = prepared.changes()[1].desired_winner.as_ref().unwrap();
    assert_eq!(revealed.origin.source(), &PolicySourceIdentity::InMemory);
    let exported = app
        .export_site_policy(&SitePolicyExportSubject::Draft(draft.id.clone()))
        .unwrap();
    assert_eq!(exported, document);
    let encoded = exported.encode().unwrap();
    let raw: toml::Value = toml::from_str(&encoded).unwrap();
    assert!(
        raw["rules"]
            .as_array()
            .unwrap()
            .iter()
            .all(|rule| rule.get("set").is_none())
    );
    let again = app
        .import_site_policy_draft(
            &SitePolicyExchange::parse(encoded.as_bytes()).unwrap(),
            false,
            false,
        )
        .unwrap();
    assert_eq!(again.rules[0].proposed.as_ref().unwrap().set, mixed.set);
}

#[test]
fn unsupported_requirements_stay_in_the_draft_and_block_apply_after_edit_rebase_and_export() {
    let app = open(ConfigurationLayers::new(Config::default()));
    let document =
        SitePolicyExchange::with_requirements(vec![upsert("future")], vec!["future-v2".into()])
            .unwrap();
    let draft = app
        .import_site_policy_draft(&document, true, false)
        .unwrap();
    assert!(!draft.validation.as_ref().unwrap().valid);
    assert!(
        draft
            .validation
            .as_ref()
            .unwrap()
            .message
            .contains("future-v2")
    );
    assert!(
        app.prepare_site_policy_draft(&draft.id, draft.revision)
            .is_err()
    );
    let edited = app
        .update_site_policy_draft(
            &draft.id,
            draft.revision,
            vec![draft.rules[0].proposed.clone()],
        )
        .unwrap();
    let rebased = app
        .rebase_site_policy_draft(&draft.id, edited.revision)
        .unwrap();
    assert!(
        app.begin_site_policy_draft_apply(&draft.id, rebased.revision, true)
            .is_err()
    );
    let exported = app
        .export_site_policy(&SitePolicyExportSubject::Draft(draft.id))
        .unwrap();
    assert_eq!(exported.unsupported_capabilities(), vec!["future-v2"]);
}

#[test]
fn exact_source_export_ignores_shadowing_and_temporary_copy_keeps_generated_settings() {
    let mut layers = ConfigurationLayers::new(Config::default());
    let mut generated = rule("same");
    generated
        .set
        .insert("content.zoom".into(), toml::Value::Float(1.75));
    layers.runtime.site_rules.push(generated.clone());
    let mut temporary = generated.clone();
    temporary.pattern = "https://temporary.test/*".into();
    layers.temporary.site_rules.push(temporary);
    let app = open(layers);
    let subject = SitePolicyExportSubject::Rule(SitePolicyRuleRef {
        source: SitePolicySource::Generated,
        id: "same".into(),
    });
    let exported = app.export_site_policy(&subject).unwrap();
    match &exported.rules()[0] {
        Operation::UpsertInteraction { pattern, .. } => assert_eq!(pattern, PATTERN),
        Operation::UnsetInteraction { .. } => panic!("expected upsert"),
    }
    assert!(
        app.export_site_policy(&SitePolicyExportSubject::Rule(SitePolicyRuleRef {
            source: SitePolicySource::Authored(999),
            id: "same".into()
        }))
        .is_err()
    );
    let shadowed = app
        .import_site_policy_draft(
            &SitePolicyExchange::new(vec![upsert("same")]).unwrap(),
            false,
            true,
        )
        .unwrap();
    let resolution = shadowed.rules[0].resolution.as_ref().unwrap();
    assert_eq!(resolution.source.as_deref(), Some("temporary"));
    assert_eq!(
        resolution.winner.as_ref().unwrap().pattern,
        "https://temporary.test/*"
    );
    assert!(
        app.import_site_policy_draft(
            &SitePolicyExchange::new(vec![upsert("same")]).unwrap(),
            true,
            true
        )
        .is_err()
    );
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.runtime.site_rules.push(generated.clone());
    let copy_app = open(layers);
    let copied = copy_app
        .import_site_policy_draft(
            &SitePolicyExchange::new(vec![upsert("same")]).unwrap(),
            true,
            true,
        )
        .unwrap();
    assert!(copied.rules[0].original.is_none());
    assert_eq!(copied.rules[0].copied_from.as_ref(), Some(&generated));
    assert_eq!(
        copied.rules[0].proposed.as_ref().unwrap().set,
        generated.set
    );
    assert_eq!(copied.source, "temporary");
}

#[test]
fn maximum_batch_is_one_draft_and_a_missing_unset_cannot_delete_a_lower_rule() {
    let mut layers = ConfigurationLayers::new(Config::default());
    layers.base.site_rules.push(rule("lower"));
    let app = open(layers);
    assert!(
        app.import_site_policy_draft(
            &SitePolicyExchange::new(vec![remove("lower")]).unwrap(),
            false,
            true
        )
        .is_err()
    );
    let mut operations = vec![upsert("lower")];
    operations.extend((0..255).map(|index| upsert(&format!("rule-{index}"))));
    let document = SitePolicyExchange::new(operations).unwrap();
    let draft = app
        .import_site_policy_draft(&document, false, true)
        .unwrap();
    assert_eq!(draft.rules.len(), 256);
    assert!(
        app.prepare_site_policy_draft(&draft.id, draft.revision)
            .is_ok()
    );
    assert_eq!(app.configuration().site_rules.len(), 1);
}
