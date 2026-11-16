use super::*;
use crate::{InteractionField, InteractionPolicyBuilder, InteractionPolicyRevision};

const URL: &str = "https://example.test/list";

fn rule(id: &str, priority: i32, enabled: bool) -> SiteRule {
    SiteRule {
        id: id.into(),
        pattern: "https://example.test/*".into(),
        priority,
        set: BTreeMap::new(),
        interaction: Some(crate::InteractionPolicyPatch {
            enabled: Some(enabled),
            ..Default::default()
        }),
    }
}

fn select<'a>(
    base: &'a Config,
    generated: &'a RuntimeOverrides,
    temporary: &'a RuntimeOverrides,
) -> Result<SelectedSitePolicyRules<'a>, ConfigError> {
    select_site_policy_rules(
        base,
        &ConfigurationSources::default(),
        generated,
        temporary,
        SitePolicySourceRevisions {
            authored: 11,
            generated: 22,
            temporary: 33,
        },
        URL,
    )
}

#[test]
fn higher_nonmatching_replacement_shadows_before_matching() {
    let mut base = Config::default();
    base.site_rules.push(rule("same", 100, true));
    let mut generated = RuntimeOverrides::default();
    let mut replacement = rule("same", -100, false);
    replacement.pattern = "https://elsewhere.test/*".into();
    generated.site_rules.push(replacement);
    let temporary = RuntimeOverrides::default();
    let feed = select(&base, &generated, &temporary).unwrap();
    assert!(feed.selected().is_empty());
    assert_eq!(feed.shadowed().len(), 1);
    let shadow = &feed.shadowed()[0];
    assert_eq!(shadow.replaced.source(), &PolicySourceIdentity::InMemory);
    assert_eq!(shadow.replaced.source_revision(), 11);
    assert_eq!(
        shadow.replacement.source(),
        &PolicySourceIdentity::Generated
    );
    assert_eq!(shadow.replacement.source_revision(), 22);
    assert_eq!(shadow.replaced.rule().unwrap().id(), "same");
}

#[test]
fn source_precedence_beats_priority_and_ties_keep_authored_order() {
    let base = Config {
        site_rules: vec![rule("base", 100, false)],
        ..Default::default()
    };
    let generated = RuntimeOverrides {
        site_rules: vec![
            rule("second", 0, false),
            rule("first", -1, false),
            rule("third", 0, false),
        ],
        ..Default::default()
    };
    let temporary = RuntimeOverrides {
        site_rules: vec![rule("last", -100, true)],
        ..Default::default()
    };
    let feed = select(&base, &generated, &temporary).unwrap();
    let ids = feed
        .selected()
        .iter()
        .map(|entry| entry.rule().id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["base", "first", "second", "third", "last"]);
    assert_eq!(
        feed.selected()[1].origin().rule().unwrap().authored_index(),
        1
    );
    let mut composer = InteractionPolicyBuilder::default();
    composer.apply_selected_rules(&feed).unwrap();
    let snapshot = composer
        .finish(InteractionPolicyRevision {
            profile: ferric_browser_core::ProfileId::from_display("profileid-1").unwrap(),
            profile_revision: 1,
            configuration_revision: 2,
            policy_revision: 3,
        })
        .unwrap();
    assert!(snapshot.enabled());
    assert_eq!(
        snapshot.origin(InteractionField::Enabled),
        Some(feed.selected()[4].origin())
    );
}

#[test]
fn reorder_changes_only_equal_priority_order_and_keeps_metadata() {
    let base = Config::default();
    let mut generated = RuntimeOverrides {
        site_rules: vec![rule("one", 0, true), rule("two", 0, false)],
        ..Default::default()
    };
    let temporary = RuntimeOverrides::default();
    assert_eq!(
        select(&base, &generated, &temporary).unwrap().selected()[0]
            .rule()
            .id,
        "one"
    );
    generated.site_rules.swap(0, 1);
    let feed = select(&base, &generated, &temporary).unwrap();
    assert_eq!(feed.selected()[0].rule().id, "two");
    assert_eq!(
        feed.selected()[0].origin().rule().unwrap().authored_index(),
        0
    );
    assert_eq!(
        feed.selected()[1].origin().rule().unwrap().authored_index(),
        1
    );
}

#[test]
fn three_layer_replacement_chain_retains_each_exact_source() {
    let mut base = Config::default();
    base.site_rules.push(rule("same", 0, false));
    let mut generated = RuntimeOverrides::default();
    generated.site_rules.push(rule("same", 1, false));
    let mut temporary = RuntimeOverrides::default();
    temporary.site_rules.push(rule("same", -1, true));
    let feed = select(&base, &generated, &temporary).unwrap();
    assert_eq!(feed.selected().len(), 1);
    assert_eq!(feed.shadowed().len(), 2);
    assert_eq!(feed.shadowed()[1].replaced, feed.shadowed()[0].replacement);
    assert_eq!(
        feed.shadowed()[1].replacement.as_ref(),
        feed.selected()[0].origin()
    );
}

#[test]
fn invalid_shadowed_and_nonmatching_rules_fail_before_selection() {
    let mut base = Config::default();
    let mut invalid = rule("same", 0, false);
    invalid.interaction.as_mut().unwrap().hint_page_keys = Some(vec!["Ctrl+Escape".into()]);
    base.site_rules.push(invalid);
    let mut generated = RuntimeOverrides::default();
    generated.site_rules.push(rule("same", 0, true));
    let temporary = RuntimeOverrides::default();
    assert!(select(&base, &generated, &temporary).is_err());
    base.site_rules[0] = rule("same", 0, true);
    let mut invalid = rule("unmatched", 0, false);
    invalid.pattern = "https://elsewhere.test/*".into();
    invalid.interaction.as_mut().unwrap().labels = Some(crate::InteractionLabelsPatch {
        min_chars: Some(9),
        ..Default::default()
    });
    generated.site_rules.push(invalid);
    assert!(select(&base, &generated, &temporary).is_err());
}

#[test]
fn duplicate_ids_reject_within_a_source_but_identical_patterns_survive() {
    let base = Config::default();
    let mut generated = RuntimeOverrides {
        site_rules: vec![rule("one", 0, true), rule("two", 0, false)],
        ..Default::default()
    };
    let temporary = RuntimeOverrides::default();
    assert_eq!(
        select(&base, &generated, &temporary)
            .unwrap()
            .selected()
            .len(),
        2
    );
    generated.site_rules[1].id = "one".into();
    assert!(select(&base, &generated, &temporary).is_err());
}

#[test]
fn replaced_include_arrays_never_reappear_and_catalogue_mismatch_rejects() {
    let loaded = crate::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/site-policy/includes/root.toml"
    ))
    .unwrap();
    let generated = RuntimeOverrides::default();
    let temporary = RuntimeOverrides::default();
    let feed = select_site_policy_rules(
        &loaded.config,
        &loaded.authored,
        &generated,
        &temporary,
        SitePolicySourceRevisions::default(),
        "https://fixture.test/",
    )
    .unwrap();
    assert!(feed.selected().is_empty());
    let resolved = resolve_site_policy_rules(
        &loaded.config,
        &loaded.authored,
        &generated,
        &temporary,
        SitePolicySourceRevisions {
            authored: 17,
            ..Default::default()
        },
    )
    .unwrap();
    let (selected, trace) = resolved.select_with_trace("https://example.com/list");
    assert!(selected.selected().is_empty());
    assert!(trace.survivors.is_empty());
    assert!(trace.shadowed.is_empty());
    assert_eq!(
        trace.authored_array,
        Some(AuthoredSitePolicyArray {
            document_id: 1,
            source_revision: 17
        })
    );
    assert_eq!(trace.replaced_arrays.len(), 1);
    let retired = &trace.replaced_arrays[0];
    assert_eq!(
        retired.array,
        AuthoredSitePolicyArray {
            document_id: 0,
            source_revision: 17
        }
    );
    assert_eq!(retired.replaced_by, trace.authored_array.unwrap());
    assert_eq!(retired.rules[0].raw.id, "hidden");
    assert_eq!(retired.rules[0].raw.pattern, "https://example.com/*");
    assert_eq!(retired.rules[0].origin.rule().unwrap().id(), "hidden");
    assert_eq!(retired.rules[0].origin.source_revision(), 17);
    let mut mismatched = loaded.config.clone();
    mismatched.site_rules.push(rule("invented", 0, true));
    assert!(
        select_site_policy_rules(
            &mismatched,
            &loaded.authored,
            &generated,
            &temporary,
            SitePolicySourceRevisions::default(),
            URL
        )
        .is_err()
    );
}

#[test]
fn array_history_tracks_immediate_replacements_including_empty_arrays() {
    let mut authored = ConfigurationSources::default();
    for (path, raw) in [
        (
            "child.toml",
            "[[site_rules]]\nid='same'\npattern='https://example.test/*'\npriority=7\n[site_rules.interaction]\nenabled=true\n",
        ),
        ("absent.toml", "[content]\nzoom=1.25\n"),
        ("clear.toml", "site_rules=[]\n"),
        (
            "parent.toml",
            "[[site_rules]]\nid='same'\npattern='https://example.test/*'\n[site_rules.interaction]\nenabled=false\n",
        ),
    ] {
        authored.push(
            crate::ConfigurationSourceDocument::parse(
                std::path::Path::new(path),
                &toml::from_str(raw).unwrap(),
            )
            .unwrap(),
        );
    }
    let base = Config {
        site_rules: authored.documents()[3].site_rules().unwrap().to_vec(),
        ..Default::default()
    };
    let mut generated = RuntimeOverrides::default();
    let mut replacement = rule("same", 0, true);
    replacement.pattern = "https://elsewhere.test/*".into();
    generated.site_rules.push(replacement);
    let temporary = RuntimeOverrides::default();
    let resolved = resolve_site_policy_rules(
        &base,
        &authored,
        &generated,
        &temporary,
        SitePolicySourceRevisions {
            authored: 31,
            generated: 32,
            temporary: 33,
        },
    )
    .unwrap();
    let (selected, trace) = resolved.select_with_trace(URL);
    assert!(selected.selected().is_empty());
    assert_eq!(trace.authored_array.unwrap().document_id, 3);
    assert_eq!(
        trace
            .replaced_arrays
            .iter()
            .map(|array| (array.array.document_id, array.replaced_by.document_id))
            .collect::<Vec<_>>(),
        [(0, 2), (2, 3)]
    );
    assert_eq!(
        trace.replaced_arrays[0].rules[0]
            .raw
            .interaction
            .as_ref()
            .unwrap()
            .enabled,
        Some(true)
    );
    assert_eq!(
        trace.replaced_arrays[0].rules[0]
            .origin
            .rule()
            .unwrap()
            .priority(),
        7
    );
    assert!(trace.replaced_arrays[1].rules.is_empty());
    assert_eq!(trace.shadowed.len(), 1);
    assert_eq!(
        trace.shadowed[0].replaced.source(),
        &PolicySourceIdentity::Authored { document_id: 3 }
    );
    assert_eq!(
        trace.shadowed[0].replacement.source(),
        &PolicySourceIdentity::Generated
    );
    assert_eq!(trace.survivors.len(), 1);
    assert!(!trace.survivors[0].matched);
    assert!(crate::ConfigurationSourceDocument::parse(std::path::Path::new("bad-hidden.toml"),
        &toml::from_str("[[site_rules]]\nid='same'\npattern='https://example.test/*'\n[[site_rules]]\nid='same'\npattern='https://elsewhere.test/*'\n").unwrap()).is_err());
}

#[test]
fn selected_rules_do_not_borrow_the_diagnostic_catalogue() {
    let base = Config {
        site_rules: vec![rule("standalone", 0, true)],
        ..Default::default()
    };
    let generated = RuntimeOverrides::default();
    let temporary = RuntimeOverrides::default();
    let selected = {
        let catalogue = ConfigurationSources::default();
        let resolved = resolve_site_policy_rules(
            &base,
            &catalogue,
            &generated,
            &temporary,
            SitePolicySourceRevisions::default(),
        )
        .unwrap();
        let (_, trace) = resolved.select_with_trace(URL);
        assert!(trace.authored_array.is_none());
        assert!(trace.replaced_arrays.is_empty());
        resolved.select(URL)
    };
    assert_eq!(selected.selected()[0].rule().id, "standalone");
}

#[test]
fn last_authored_array_owns_provenance_without_matching_earlier_arrays() {
    let mut authored = ConfigurationSources::default();
    let older: toml::Value =
        toml::from_str("[[site_rules]]\nid='old'\npattern='https://example.test/*'\n").unwrap();
    let later: toml::Value =
        toml::from_str("[[site_rules]]\nid='new'\npattern='https://example.test/*'\n").unwrap();
    authored.push(
        crate::ConfigurationSourceDocument::parse(std::path::Path::new("old.toml"), &older)
            .unwrap(),
    );
    authored.push(
        crate::ConfigurationSourceDocument::parse(std::path::Path::new("new.toml"), &later)
            .unwrap(),
    );
    let base = Config {
        site_rules: authored.documents()[1].site_rules().unwrap().to_vec(),
        ..Default::default()
    };
    let generated = RuntimeOverrides::default();
    let temporary = RuntimeOverrides::default();
    let feed = select_site_policy_rules(
        &base,
        &authored,
        &generated,
        &temporary,
        SitePolicySourceRevisions::default(),
        URL,
    )
    .unwrap();
    assert_eq!(feed.selected().len(), 1);
    assert_eq!(feed.selected()[0].rule().id, "new");
    assert_eq!(
        feed.selected()[0].origin().source(),
        &PolicySourceIdentity::Authored { document_id: 1 }
    );
}

#[test]
fn surviving_limit_applies_before_matching_at_each_layer_boundary() {
    let base = Config {
        site_rules: (0..MAX_SITE_RULES)
            .map(|i| rule(&format!("rule-{i}"), 0, true))
            .collect(),
        ..Default::default()
    };
    let mut generated = RuntimeOverrides::default();
    let temporary = RuntimeOverrides::default();
    assert_eq!(
        select(&base, &generated, &temporary)
            .unwrap()
            .selected()
            .len(),
        MAX_SITE_RULES
    );
    generated.site_rules.push(rule("rule-0", 0, false));
    assert_eq!(
        select(&base, &generated, &temporary)
            .unwrap()
            .selected()
            .len(),
        MAX_SITE_RULES
    );
    generated.site_rules.push(rule("extra", 0, false));
    generated.site_rules[1].pattern = "https://elsewhere.test/*".into();
    assert!(select(&base, &generated, &temporary).is_err());
}

#[test]
fn applying_a_feed_is_atomic_when_builder_contains_a_duplicate() {
    let base = Config {
        site_rules: vec![rule("first", 0, false), rule("duplicate", 1, false)],
        ..Default::default()
    };
    let generated = RuntimeOverrides::default();
    let temporary = RuntimeOverrides::default();
    let feed = select(&base, &generated, &temporary).unwrap();
    let mut builder = InteractionPolicyBuilder::default();
    builder
        .apply_selected_rule(&base.site_rules[1], PolicySourceIdentity::InMemory, 1, 1)
        .unwrap();
    assert!(builder.apply_selected_rules(&feed).is_err());
    // If the failed call had applied its first rule, this would now duplicate.
    builder
        .apply_selected_rule(&base.site_rules[0], PolicySourceIdentity::InMemory, 1, 0)
        .unwrap();
}

#[test]
fn regression_generic_composer_checks_seed_ids_even_for_rules_without_interaction() {
    let mut base = Config::default();
    let prior = rule("same", 0, false);
    let mut replacement = prior.clone();
    replacement.interaction = None;
    base.site_rules.push(replacement);
    let generated = RuntimeOverrides::default();
    let temporary = RuntimeOverrides::default();
    let feed = select(&base, &generated, &temporary).unwrap();
    let revision = InteractionPolicyRevision {
        profile: ferric_browser_core::ProfileId::from_display("profileid-1").unwrap(),
        profile_revision: 1,
        configuration_revision: 2,
        policy_revision: 3,
    };
    assert!(
        InteractionPolicyBuilder::default()
            .finish_selected_rules(&feed, revision)
            .unwrap()
            .enabled()
    );
    let mut seed = InteractionPolicyBuilder::default();
    seed.apply_selected_rule(&prior, PolicySourceIdentity::InMemory, 0, 0)
        .unwrap();
    assert!(seed.finish_selected_rules(&feed, revision).is_err());
}

#[test]
fn regression_nonmatching_replacement_cannot_retain_seeded_lower_rule() {
    let mut base = Config::default();
    let prior = rule("same", 0, false);
    base.site_rules.push(prior.clone());
    let mut generated = RuntimeOverrides::default();
    let mut replacement = rule("same", 0, true);
    replacement.pattern = "https://elsewhere.test/*".into();
    generated.site_rules.push(replacement);
    let temporary = RuntimeOverrides::default();
    let feed = select(&base, &generated, &temporary).unwrap();
    assert!(feed.selected().is_empty());
    let mut seed = InteractionPolicyBuilder::default();
    seed.apply_selected_rule(&prior, PolicySourceIdentity::InMemory, 0, 0)
        .unwrap();
    assert!(
        seed.finish_selected_rules(
            &feed,
            InteractionPolicyRevision {
                profile: ferric_browser_core::ProfileId::from_display("profileid-1").unwrap(),
                profile_revision: 1,
                configuration_revision: 2,
                policy_revision: 3,
            }
        )
        .is_err()
    );
}

#[test]
fn generic_composer_accepts_shared_leaf_seed_and_preserves_its_origin() {
    let base = Config::default();
    let generated = RuntimeOverrides::default();
    let temporary = RuntimeOverrides::default();
    let feed = select(&base, &generated, &temporary).unwrap();
    let mut seed = InteractionPolicyBuilder::default();
    seed.apply_shared_labels(
        &crate::AuthoredHints {
            min_chars: Some(2),
            ..Default::default()
        },
        PolicySourceIdentity::Profile {
            name: "work".into(),
        },
        42,
    )
    .unwrap();
    let snapshot = seed
        .finish_selected_rules(
            &feed,
            InteractionPolicyRevision {
                profile: ferric_browser_core::ProfileId::from_display("profileid-1").unwrap(),
                profile_revision: 1,
                configuration_revision: 2,
                policy_revision: 3,
            },
        )
        .unwrap();
    assert_eq!(snapshot.labels().min_chars(), 2);
    assert_eq!(snapshot.ordinary_labels().min_chars(), 2);
    let origin = snapshot
        .origin(InteractionField::Labels(
            crate::InteractionLabelField::MinChars,
        ))
        .unwrap();
    assert_eq!(origin.source_revision(), 42);
    assert_eq!(
        origin.source(),
        &PolicySourceIdentity::Profile {
            name: "work".into()
        }
    );
    assert_eq!(origin.rule(), None);
}

#[test]
fn exact_survivor_lookup_precedes_matching_and_preserves_the_winning_full_rule() {
    let base = Config {
        site_rules: vec![rule("same", 100, true)],
        ..Config::default()
    };
    let mut replacement = rule("same", -100, false);
    replacement.pattern = "https://elsewhere.test/*".into();
    replacement
        .set
        .insert("content.zoom".into(), toml::Value::Float(1.75));
    let generated = RuntimeOverrides {
        site_rules: vec![replacement.clone()],
        ..RuntimeOverrides::default()
    };
    let temporary = RuntimeOverrides::default();
    let authored = ConfigurationSources::default();
    let resolved = resolve_site_policy_rules(
        &base,
        &authored,
        &generated,
        &temporary,
        SitePolicySourceRevisions {
            authored: 11,
            generated: 22,
            temporary: 33,
        },
    )
    .unwrap();
    assert!(resolved.select(URL).selected().is_empty());
    let survivor = resolved.surviving_rule("same").unwrap();
    assert_eq!(survivor.rule(), &replacement);
    assert_eq!(survivor.origin().source(), &PolicySourceIdentity::Generated);
    assert_eq!(survivor.origin().source_revision(), 22);
    assert_eq!(survivor.origin().rule().unwrap().authored_index(), 0);
    assert!(resolved.surviving_rule("Same").is_none());
    assert!(resolved.surviving_rule("missing").is_none());
    // Looking at a nonmatching full snapshot grants no selected consumer rule.
    assert!(resolved.select(URL).selected().is_empty());
}

#[test]
fn exact_survivor_lookup_never_revives_a_retired_include_array() {
    let loaded = crate::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/site-policy/includes/root.toml"
    ))
    .unwrap();
    assert_eq!(
        loaded.authored.documents()[0].site_rules().unwrap()[0].id,
        "hidden"
    );
    assert!(loaded.config.site_rules.is_empty());
    let generated = RuntimeOverrides::default();
    let temporary = RuntimeOverrides::default();
    let resolved = resolve_site_policy_rules(
        &loaded.config,
        &loaded.authored,
        &generated,
        &temporary,
        SitePolicySourceRevisions::default(),
    )
    .unwrap();
    assert!(resolved.surviving_rule("hidden").is_none());
    assert!(resolved.select(URL).selected().is_empty());
}
