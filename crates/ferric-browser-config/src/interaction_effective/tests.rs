use super::*;
use serde_json::{Value, json};

#[path = "codebook_tests.rs"]
mod codebook;

fn revision() -> InteractionPolicyRevision {
    InteractionPolicyRevision {
        profile: ferric_browser_core::ProfileId::from_display("profileid-1").unwrap(),
        profile_revision: 7,
        configuration_revision: 12,
        policy_revision: 30,
    }
}
fn rule(id: &str, patch: Value) -> SiteRule {
    SiteRule {
        id: id.into(),
        pattern: "https://example.test/*".into(),
        priority: 42,
        set: BTreeMap::new(),
        interaction: Some(serde_json::from_value(patch).unwrap()),
    }
}
fn apply(builder: &mut InteractionPolicyBuilder, id: &str, patch: Value) {
    builder
        .apply_selected_rule(
            &rule(id, patch),
            PolicySourceIdentity::Authored { document_id: 3 },
            9,
            4,
        )
        .unwrap();
}
fn anchor() -> Value {
    json!({"id":"mac", "code":"m", "families":["links"], "target_selector":"#mac"})
}

#[test]
fn traced_composition_retains_noops_clears_and_only_explicit_shared_leaves() {
    let mut builder = InteractionPolicyBuilder::traced();
    builder
        .apply_shared_labels(&AuthoredHints::default(), PolicySourceIdentity::InMemory, 1)
        .unwrap();
    apply(&mut builder, "anchor", json!({"anchors":[anchor()]}));
    apply(&mut builder, "empty", json!({"labels":{}}));
    let rejected = AuthoredHints {
        min_chars: Some(0),
        ..Default::default()
    };
    assert!(
        builder
            .apply_shared_labels(&rejected, PolicySourceIdentity::Generated, 2)
            .is_err()
    );
    let shared = AuthoredHints {
        chars: Some("xyz".into()),
        ..Default::default()
    };
    builder
        .apply_shared_labels(&shared, PolicySourceIdentity::Generated, 2)
        .unwrap();
    apply(&mut builder, "clear", json!({"anchors":[]}));
    let (policy, trace) = builder.finish_with_trace(revision()).unwrap();
    assert_eq!(trace.len(), 4);
    assert!(
        matches!(&trace[0].patch, InteractionContributionPatch::SiteSection(patch) if patch.anchors.as_ref().unwrap().len() == 1)
    );
    assert!(
        matches!(&trace[1].patch, InteractionContributionPatch::SiteSection(patch) if patch.labels == Some(InteractionLabelsPatch::default()))
    );
    assert!(
        matches!(&trace[2].patch, InteractionContributionPatch::SharedLabels(patch) if patch.alphabet.as_deref() == Some("xyz") && patch.min_chars.is_none())
    );
    assert!(
        matches!(&trace[3].patch, InteractionContributionPatch::SiteSection(patch) if patch.anchors == Some(Vec::new()))
    );
    assert_eq!(trace[2].origin.source_revision(), 2);
    assert_eq!(policy.labels(), policy.ordinary_labels());
    assert!(policy.anchors().is_empty());
    assert_eq!(
        policy.origin(InteractionField::Anchors).unwrap(),
        trace[3].origin.as_ref()
    );
    assert_eq!(
        policy
            .ordinary_label_origin(InteractionLabelField::Alphabet)
            .unwrap(),
        trace[2].origin.as_ref()
    );
}

#[test]
fn traced_finish_rejects_partial_winners_until_composed_conflict_is_repaired() {
    assert!(
        InteractionPolicyBuilder::default()
            .finish_with_trace(revision())
            .is_err()
    );
    let mut builder = InteractionPolicyBuilder::traced();
    apply(&mut builder, "anchor", json!({"anchors":[anchor()]}));
    apply(
        &mut builder,
        "collection",
        json!({"collections":[{"id":"list","families":["links"],"codes":["m"],
        "container_self":true,"item_selector":"a.item","primary_self":true,"order":"dom"}]}),
    );
    assert!(
        builder
            .clone()
            .finish_with_trace(revision())
            .unwrap_err()
            .contains("duplicate")
    );
    apply(&mut builder, "clear", json!({"anchors":[]}));
    let (policy, trace) = builder.finish_with_trace(revision()).unwrap();
    assert_eq!(trace.len(), 3);
    assert!(policy.anchors().is_empty());
    assert_eq!(policy.collections().len(), 1);
}

#[test]
fn sparse_composition_clears_lists_and_replaces_receiver_atomically() {
    let mut builder = InteractionPolicyBuilder::default();
    apply(
        &mut builder,
        "lower",
        json!({"enabled":false,
        "labels":{"min_chars":3,"fallback_prefix":"z"}, "anchors":[anchor()],
        "hint_page_keys":["p"], "page_receiver":{"scope":"focused-frame","origins":["https://frame.test"]},
        "remaps":[{"id":"play","from":"p","to":"k","contexts":["passthrough"]}]}),
    );
    let disabled = builder.clone().finish(revision()).unwrap();
    assert!(!disabled.enabled());
    assert_eq!(disabled.anchors().len(), 1);
    assert!(!disabled.enhanced_requested_for(InteractionFamily::Links));
    apply(&mut builder, "empty", json!({"labels":{}}));
    apply(
        &mut builder,
        "higher",
        json!({"enabled":true,"labels":{"min_chars":2},
        "hint_page_keys":[], "page_receiver":{"scope":"top-document"}, "remaps":[]}),
    );
    let enabled = builder.clone().finish(revision()).unwrap();
    assert!(enabled.enabled());
    assert_eq!(enabled.anchors().len(), 1);
    assert_eq!(enabled.labels().min_chars(), 2);
    assert_eq!(enabled.labels().fallback_prefix(), "z");
    assert!(enabled.hint_page_keys().is_empty() && enabled.remaps().is_empty());
    assert_eq!(
        *enabled.page_receiver(),
        InteractionPageReceiver::TopDocument {}
    );
    let origin = enabled.origin(InteractionField::Anchors).unwrap();
    assert_eq!(
        origin.source(),
        &PolicySourceIdentity::Authored { document_id: 3 }
    );
    assert_eq!(origin.source_revision(), 9);
    assert_eq!(origin.rule().unwrap().id(), "lower");
    assert_eq!(origin.rule().unwrap().authored_index(), 4);
    assert_eq!(origin.rule().unwrap().priority(), 42);
    assert_eq!(enabled.revision(), revision());
    apply(
        &mut builder,
        "replace",
        json!({"anchors":[{"id":"watch","code":"w","families":["links"],"target_selector":"#watch"}]}),
    );
    assert_eq!(
        builder.clone().finish(revision()).unwrap().anchors()[0].id,
        "watch"
    );
    apply(
        &mut builder,
        "clear",
        json!({"anchors":[],"collections":[]}),
    );
    let cleared = builder.finish(revision()).unwrap();
    assert!(cleared.anchors().is_empty() && cleared.collections().is_empty());
    assert_eq!(
        cleared
            .origin(InteractionField::Anchors)
            .unwrap()
            .rule()
            .unwrap()
            .id(),
        "clear"
    );
    // Completed snapshots and their cheap clones preserve their original data.
    let retained = disabled.clone();
    assert!(!retained.enabled());
    assert_eq!(retained.anchors()[0].id, "mac");
    assert_eq!(enabled.anchors()[0].id, "mac");
}

#[test]
fn explicit_shared_presence_resets_site_leaves_without_reapplying_defaults() {
    let mut builder = InteractionPolicyBuilder::default();
    let original = builder.clone().finish(revision()).unwrap();
    let shared = AuthoredHints {
        chars: Some(original.labels().alphabet().into()),
        min_chars: Some(original.labels().min_chars()),
        auto_follow: Some(HintAutoFollow::FullMatch),
        ..Default::default()
    };
    apply(
        &mut builder,
        "site",
        json!({"labels":{"alphabet":"jk","min_chars":7,"auto_follow":"never"}}),
    );
    let site = builder.clone().finish(revision()).unwrap();
    assert!(site.enhanced_requested_for(InteractionFamily::Links));
    builder
        .apply_shared_labels(
            &AuthoredHints::default(),
            PolicySourceIdentity::Generated,
            10,
        )
        .unwrap();
    let unchanged = builder.clone().finish(revision()).unwrap();
    assert_eq!(unchanged.labels(), site.labels());
    builder
        .apply_shared_labels(&shared, PolicySourceIdentity::Generated, 11)
        .unwrap();
    let reset = builder.finish(revision()).unwrap();
    assert_eq!(reset.labels(), reset.ordinary_labels());
    assert_eq!(reset.labels(), original.labels());
    assert!(!reset.enhanced_requested_for(InteractionFamily::Links));
    for field in [
        InteractionLabelField::Alphabet,
        InteractionLabelField::MinChars,
        InteractionLabelField::AutoFollow,
    ] {
        let origin = reset.origin(InteractionField::Labels(field)).unwrap();
        assert_eq!(origin.source(), &PolicySourceIdentity::Generated);
        assert_eq!(origin.source_revision(), 11);
        assert!(origin.rule().is_none());
        assert_eq!(reset.ordinary_label_origin(field), Some(origin));
    }
    assert_eq!(site.labels().alphabet(), "jk");
    assert_eq!(
        site.origin(InteractionField::Labels(InteractionLabelField::Alphabet))
            .unwrap()
            .rule()
            .unwrap()
            .id(),
        "site"
    );
}

#[test]
fn enhancement_is_family_local_and_remapping_does_not_grant_admission() {
    for patch in [
        json!({}),
        json!({"enabled":true}),
        json!({"labels":{}}),
        json!({"anchors":[],"collections":[],"hint_page_keys":[],"remaps":[]}),
        json!({"page_receiver":{"scope":"top-document"}}),
        json!({"remaps":[{"id":"play","from":"p","to":"k","contexts":["hint-page-key","passthrough","normal-page-key","passthrough-once"]}]}),
    ] {
        let mut builder = InteractionPolicyBuilder::default();
        apply(&mut builder, "empty", patch);
        let policy = builder.finish(revision()).unwrap();
        assert!(!policy.enhanced_requested_for(InteractionFamily::All));
        assert!(!policy.enhanced_requested_for(InteractionFamily::Links));
        assert!(policy.keys().hint_page_keys().is_empty());
    }
    let mut builder = InteractionPolicyBuilder::default();
    apply(
        &mut builder,
        "missing-target",
        json!({"anchors":[anchor()]}),
    );
    let policy = builder.clone().finish(revision()).unwrap();
    // Applicability is decided without a DOM or a successful target match.
    assert!(policy.enhanced_requested_for(InteractionFamily::Links));
    assert!(!policy.enhanced_requested_for(InteractionFamily::All));
    assert!(!policy.enhanced_requested_for(InteractionFamily::Media));
    apply(&mut builder, "page-key", json!({"hint_page_keys":["p"]}));
    assert!(
        builder
            .finish(revision())
            .unwrap()
            .enhanced_requested_for(InteractionFamily::Media)
    );
}

#[test]
fn rejected_inputs_preserve_values_and_do_not_record_a_partial_origin() {
    let mut builder = InteractionPolicyBuilder::default();
    let bad = rule("retry", json!({"enabled":false,"labels":{"min_chars":0}}));
    assert!(
        builder
            .apply_selected_rule(&bad, PolicySourceIdentity::Generated, 1, 0)
            .is_err()
    );
    let untouched = builder.clone().finish(revision()).unwrap();
    assert!(untouched.enabled());
    assert!(untouched.origin(InteractionField::Enabled).is_none());
    let good = rule("retry", json!({"labels":{"min_chars":3}}));
    for source in [
        PolicySourceIdentity::Profile {
            name: "work".into(),
        },
        PolicySourceIdentity::CommandLine,
    ] {
        assert!(builder.apply_selected_rule(&good, source, 1, 0).is_err());
    }
    assert!(
        builder
            .apply_selected_rule(&good, PolicySourceIdentity::Generated, 1, MAX_SITE_RULES)
            .is_err()
    );
    builder
        .apply_selected_rule(
            &good,
            PolicySourceIdentity::Generated,
            2,
            MAX_SITE_RULES - 1,
        )
        .unwrap();
    assert!(
        builder
            .apply_selected_rule(&good, PolicySourceIdentity::Temporary, 3, 0)
            .is_err()
    );
    let before = builder.clone().finish(revision()).unwrap();
    let invalid_shared = AuthoredHints {
        min_chars: Some(0),
        ..Default::default()
    };
    assert!(
        builder
            .apply_shared_labels(&invalid_shared, PolicySourceIdentity::Temporary, 4)
            .is_err()
    );
    let after = builder.finish(revision()).unwrap();
    assert_eq!(after.labels(), before.labels());
    let origin = after
        .origin(InteractionField::Labels(InteractionLabelField::MinChars))
        .unwrap();
    assert_eq!(origin.source_revision(), 2);
    assert_eq!(origin.rule().unwrap().authored_index(), 255);
}

#[test]
fn selected_rule_count_and_profile_origins_keep_existing_bounds() {
    assert_eq!(MAX_SITE_RULES, 256);
    let mut builder = InteractionPolicyBuilder::default();
    for index in 0..MAX_SITE_RULES {
        builder
            .apply_selected_rule(
                &rule(&format!("rule-{index}"), json!({})),
                PolicySourceIdentity::Generated,
                1,
                index,
            )
            .unwrap();
    }
    assert!(
        builder
            .apply_selected_rule(
                &rule("overflow", json!({})),
                PolicySourceIdentity::Temporary,
                2,
                0
            )
            .is_err()
    );
    assert!(
        !builder
            .finish(revision())
            .unwrap()
            .enhanced_requested_for(InteractionFamily::All)
    );
    for length in [31, 32, 33] {
        let mut builder = InteractionPolicyBuilder::default();
        assert_eq!(
            builder
                .apply_shared_labels(
                    &AuthoredHints::default(),
                    PolicySourceIdentity::Profile {
                        name: "a".repeat(length)
                    },
                    1
                )
                .is_ok(),
            length <= 32
        );
    }
}

#[test]
fn composed_conflicts_fail_before_a_snapshot_is_returned() {
    let mut builder = InteractionPolicyBuilder::default();
    apply(&mut builder, "lower", json!({"anchors":[anchor()]}));
    apply(
        &mut builder,
        "higher",
        json!({"collections":[{"id":"list","families":["links"],"codes":["m"],
        "container_self":true,"item_selector":"a.item","primary_self":true,"order":"dom"}]}),
    );
    assert!(
        builder
            .clone()
            .finish(revision())
            .unwrap_err()
            .contains("duplicate")
    );
    apply(&mut builder, "repair", json!({"anchors":[]}));
    assert_eq!(builder.finish(revision()).unwrap().collections().len(), 1);
}

#[test]
fn each_list_replaces_without_merging_ids_or_retaining_prior_entries() {
    let mut builder = InteractionPolicyBuilder::default();
    for (id, anchor_code, collection_code, source, destination) in [
        ("lower", "m", "n", "p", "k"),
        ("higher", "w", "y", "q", "j"),
    ] {
        apply(
            &mut builder,
            id,
            json!({"anchors":[{"id":id,"code":anchor_code,"families":["links"],"target_selector":"a"}],
            "collections":[{"id":id,"families":["links"],"codes":[collection_code],"container_self":true,"item_selector":"a.item","primary_self":true,"order":"dom"}],
            "hint_page_keys":[source], "remaps":[{"id":id,"from":source,"to":destination,"contexts":["passthrough"]}],
            "labels":{"excluded_chars":[if id == "lower" {"x"} else {"z"}]}}),
        );
    }
    let policy = builder.finish(revision()).unwrap();
    assert_eq!(policy.anchors().len(), 1);
    assert_eq!(policy.collections().len(), 1);
    assert_eq!(policy.remaps().len(), 1);
    assert_eq!(policy.anchors()[0].id, "higher");
    assert_eq!(policy.collections()[0].id, "higher");
    assert_eq!(policy.remaps()[0].id, "higher");
    assert_eq!(policy.hint_page_keys(), ["q"]);
    assert_eq!(policy.labels().excluded_chars(), ["z"]);
}

#[test]
fn effective_selectors_use_the_full_32k_budget_without_weakening_authored_limits() {
    assert_eq!(crate::MAX_INTERACTION_AUTHORED_SELECTOR_BYTES, 16 * 1024);
    assert_eq!(crate::MAX_INTERACTION_EFFECTIVE_SELECTOR_BYTES, 32 * 1024);
    let selector = format!("a.{}", "x".repeat(1022));
    let anchors: Vec<_> = (0..16)
        .map(|index| {
            json!({"id":format!("a{index}"), "code":format!("m{index:02}"),
        "families":["links"],"target_selector":selector})
        })
        .collect();
    let collections: Vec<_> = (0..16).map(|index| json!({"id":format!("c{index}"), "codes":[format!("n{index:02}")],
        "families":["links"],"container_self":true,"item_selector":selector,"primary_self":true,"order":"dom"})).collect();
    let mut builder = InteractionPolicyBuilder::default();
    apply(&mut builder, "anchors", json!({"anchors":anchors}));
    apply(
        &mut builder,
        "collections",
        json!({"collections":collections}),
    );
    let complete = builder.values.patch();
    assert!(complete.validate_authored().unwrap_err().contains("16 KiB"));
    builder.clone().finish(revision()).unwrap();
    // Direct malformed composed input exercises the defensive effective cap;
    // validated authored replacement lists cannot themselves exceed16KiB each.
    for (extra, expected) in [(0, true), (1, false)] {
        let mut changed = complete.clone();
        if extra == 1 {
            changed.collections.as_mut().unwrap()[0].primary_self = None;
            changed.collections.as_mut().unwrap()[0].primary_selector = Some("a".into());
        }
        assert_eq!(changed.validate_effective().is_ok(), expected);
    }
    let mut below = complete;
    below.anchors.as_mut().unwrap()[0]
        .target_selector
        .as_mut()
        .unwrap()
        .pop();
    assert!(below.validate_effective().is_ok());
    let mut authored = rule("one-list", json!({"anchors":anchors}));
    authored
        .interaction
        .as_mut()
        .unwrap()
        .anchors
        .as_mut()
        .unwrap()[0]
        .scope_selector = Some("a".into());
    assert!(
        validate_site_rule(&authored)
            .unwrap_err()
            .to_string()
            .contains("16 KiB")
    );
}

fn href_path(rule: &mut SiteRule) -> &mut String {
    let patch = rule.interaction.as_mut().unwrap();
    let predicates = if let Some(anchors) = &mut patch.anchors {
        anchors[0].predicates.as_mut().unwrap()
    } else {
        patch.collections.as_mut().unwrap()[0]
            .primary_predicates
            .as_mut()
            .unwrap()
    };
    predicates.href.as_mut().unwrap().path.as_mut().unwrap()
}

#[test]
fn effective_serialized_128k_and_authored_64k_boundaries_are_distinct() {
    assert_eq!(crate::MAX_INTERACTION_AUTHORED_BYTES, 64 * 1024);
    assert_eq!(crate::MAX_INTERACTION_EFFECTIVE_BYTES, 128 * 1024);
    let mut first = rule(
        "anchors",
        json!({"anchors":[{"id":"a","code":"m","families":["links"],"predicates":{"href":{"path":"/"}}}]}),
    );
    let mut second = rule(
        "collections",
        json!({"collections":[{"id":"c","families":["links"],"codes":["n"],
        "container_self":true,"item_selector":"a.item","primary_self":true,"order":"dom","primary_predicates":{"href":{"path":"/"}}}]}),
    );
    let mut initial = InteractionPolicyBuilder::default();
    initial
        .apply_selected_rule(&first, PolicySourceIdentity::Generated, 1, 0)
        .unwrap();
    initial
        .apply_selected_rule(&second, PolicySourceIdentity::Temporary, 2, 0)
        .unwrap();
    let first_size = toml::to_string(first.interaction.as_ref().unwrap())
        .unwrap()
        .len();
    let composed_size = toml::to_string(&initial.values.patch()).unwrap().len();
    let first_padding = crate::MAX_INTERACTION_AUTHORED_BYTES - first_size;
    let total_padding = crate::MAX_INTERACTION_EFFECTIVE_BYTES - composed_size;
    href_path(&mut first).push_str(&"x".repeat(first_padding));
    href_path(&mut second).push_str(&"y".repeat(total_padding - first_padding));
    assert_eq!(
        toml::to_string(first.interaction.as_ref().unwrap())
            .unwrap()
            .len(),
        crate::MAX_INTERACTION_AUTHORED_BYTES
    );
    validate_site_rule(&first).unwrap();
    let mut authored_below = first.clone();
    href_path(&mut authored_below).pop();
    assert_eq!(
        toml::to_string(authored_below.interaction.as_ref().unwrap())
            .unwrap()
            .len(),
        crate::MAX_INTERACTION_AUTHORED_BYTES - 1
    );
    validate_site_rule(&authored_below).unwrap();
    for offset in [-1, 0, 1] {
        let mut last = second.clone();
        if offset < 0 {
            href_path(&mut last).pop();
        }
        if offset > 0 {
            href_path(&mut last).push('y');
        }
        validate_site_rule(&last).unwrap();
        let mut builder = InteractionPolicyBuilder::default();
        builder
            .apply_selected_rule(&first, PolicySourceIdentity::Generated, 1, 0)
            .unwrap();
        builder
            .apply_selected_rule(&last, PolicySourceIdentity::Temporary, 2, 0)
            .unwrap();
        let expected_size = usize::try_from(
            isize::try_from(crate::MAX_INTERACTION_EFFECTIVE_BYTES).unwrap() + offset,
        )
        .unwrap();
        assert_eq!(
            toml::to_string(&builder.values.patch()).unwrap().len(),
            expected_size
        );
        assert_eq!(builder.finish(revision()).is_ok(), offset <= 0);
    }
    href_path(&mut first).push('x');
    assert!(
        validate_site_rule(&first)
            .unwrap_err()
            .to_string()
            .contains("64 KiB")
    );
}
