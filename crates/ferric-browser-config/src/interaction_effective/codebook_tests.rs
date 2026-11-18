use super::*;
use ferric_browser_core::{HintCodebookError, HintReservationCategory};

#[test]
fn invocation_projection_preserves_literal_family_entry_slots_and_source_provenance() {
    let mut builder = InteractionPolicyBuilder::default();
    apply(
        &mut builder,
        "outer-rule",
        json!({"anchors":[
            {"id":"links-entry","code":"1","families":["links"],"target_selector":"#missing"},
            {"id":"buttons-entry","code":"1","families":["buttons"],"target_selector":"#missing"},
            {"id":"all-entry","code":"1","families":["all"],"target_selector":"#missing"}
        ]}),
    );
    let policy = builder.finish(revision()).unwrap();
    for (family, id) in [
        (InteractionFamily::Links, "links-entry"),
        (InteractionFamily::Buttons, "buttons-entry"),
        (InteractionFamily::All, "all-entry"),
    ] {
        let compiled = policy.compile_hint_codebook(family).unwrap().unwrap();
        assert_eq!(compiled.family(), family);
        assert_eq!(compiled.policy().revision(), revision());
        assert_eq!(
            compiled.policy().origin(InteractionField::Anchors),
            policy.origin(InteractionField::Anchors)
        );
        assert!(compiled.policy().same_meaning(&policy));
        assert_eq!(compiled.codebook().declarations().len(), 1);
        let slot = &compiled.codebook().declarations()[0];
        assert_eq!(slot.category, HintReservationCategory::Anchor);
        assert_eq!(slot.rule_id, id);
        assert_eq!(slot.slot, 0);
        assert_eq!(slot.code, "1");
    }
    assert!(
        policy
            .compile_hint_codebook(InteractionFamily::Scrollables)
            .unwrap()
            .is_none()
    );
}

#[test]
fn complete_effective_validation_cannot_hide_other_family_conflicts_or_split_roots() {
    let mut builder = InteractionPolicyBuilder::default();
    apply(
        &mut builder,
        "first",
        json!({"anchors":[{"id":"a","code":"1","families":["buttons"],"target_selector":"#a"}]}),
    );
    apply(
        &mut builder,
        "second",
        json!({"collections":[{"id":"c","codes":["12"],"families":["buttons"],
        "root":{"start":"top-document","steps":[{"kind":"open-shadow","host_selector":"#other"}]},
        "container_self":true,"item_selector":".item","primary_self":true,"order":"dom"}]}),
    );
    assert!(
        builder
            .finish(revision())
            .unwrap_err()
            .contains("prefix conflict")
    );
}

#[test]
fn disabled_and_no_enhancement_paths_do_not_apply_enhanced_auto_follow_restrictions() {
    for patch in [
        json!({}),
        json!({"enabled":true}),
        json!({"labels":{},"anchors":[],"collections":[],"hint_page_keys":[]}),
        json!({"enabled":false,"anchors":[anchor()],"labels":{"auto_follow":"always"}}),
        json!({"remaps":[{"id":"normal","from":"a","to":"k","contexts":["normal-page-key"]}]}),
    ] {
        let mut builder = InteractionPolicyBuilder::default();
        builder
            .apply_shared_labels(
                &AuthoredHints {
                    auto_follow: Some(crate::HintAutoFollow::Always),
                    ..Default::default()
                },
                PolicySourceIdentity::InMemory,
                1,
            )
            .unwrap();
        apply(&mut builder, "noop", patch);
        let policy = builder.finish(revision()).unwrap();
        assert!(
            policy
                .compile_hint_codebook(InteractionFamily::Links)
                .unwrap()
                .is_none()
        );
    }
    let mut builder = InteractionPolicyBuilder::default();
    apply(
        &mut builder,
        "enhanced",
        json!({"anchors":[anchor()],"labels":{"auto_follow":"always"}}),
    );
    assert!(matches!(
        builder
            .finish(revision())
            .unwrap()
            .compile_hint_codebook(InteractionFamily::Links),
        Err(HintCodebookError::ConflictingAutoFollow)
    ));
}

#[test]
fn unicode_hint_sources_preserve_admission_without_colliding_with_ascii_labels() {
    let mut builder = InteractionPolicyBuilder::default();
    apply(
        &mut builder,
        "unicode",
        json!({"hint_page_keys":["é","界"]}),
    );
    let policy = builder.finish(revision()).unwrap();
    let compiled = policy
        .compile_hint_codebook(InteractionFamily::Links)
        .unwrap()
        .unwrap();
    assert_eq!(compiled.policy().hint_page_keys(), ["é", "界"]);
    assert_eq!(
        compiled.codebook().effective_alphabet(),
        policy.labels().alphabet()
    );
}

#[test]
fn only_admitted_hint_sources_shrink_labels_not_destinations_or_other_contexts() {
    for admitted in [false, true] {
        let mut builder = InteractionPolicyBuilder::default();
        apply(
            &mut builder,
            "keys",
            json!({"anchors":[anchor()],"hint_page_keys":if admitted {vec!["a"]} else {vec![]},
            "remaps":[{"id":"hint","from":"a","to":"k","contexts":["hint-page-key"]},
                      {"id":"normal","from":"s","to":"f","contexts":["normal-page-key"]},
                      {"id":"pass","from":"d","to":"j","contexts":["passthrough"]}]}),
        );
        let policy = builder.finish(revision()).unwrap();
        let compiled = policy
            .compile_hint_codebook(InteractionFamily::Links)
            .unwrap()
            .unwrap();
        let alphabet = compiled.codebook().effective_alphabet();
        assert_eq!(alphabet.contains('a'), !admitted);
        for character in ['s', 'd', 'f', 'j', 'k'] {
            assert!(alphabet.contains(character));
        }
    }
}

#[test]
fn all_512_collection_slots_are_projected_without_discovery_or_rank_compression() {
    let collections: Vec<_> = (0..8)
        .map(|index| {
            json!({"id":format!("c{index}"),"families":["links"],
        "codes":(0..64).map(|slot|format!("{index:02}{slot:06}")).collect::<Vec<_>>(),
        "container_self":true,"item_selector":".missing","primary_self":true,"order":"dom"})
        })
        .collect();
    let mut builder = InteractionPolicyBuilder::default();
    apply(
        &mut builder,
        "full",
        json!({"labels":{"alphabet":"0123456789abcdefghijklmnopqrstuv","min_chars":8},"collections":collections}),
    );
    let compiled = builder
        .finish(revision())
        .unwrap()
        .compile_hint_codebook(InteractionFamily::Links)
        .unwrap()
        .unwrap();
    assert_eq!(compiled.codebook().declarations().len(), 512);
    for (index, slot) in compiled.codebook().declarations().iter().enumerate() {
        assert_eq!(slot.rule_id, format!("c{}", index / 64));
        assert_eq!(slot.slot, index % 64);
        assert_eq!(slot.category, HintReservationCategory::Collection);
    }
    assert_eq!(compiled.codebook().capacity(8), (1_u64 << 40) - 512);
    assert_eq!(
        compiled
            .codebook()
            .clone()
            .allocator(5000)
            .unwrap()
            .suffix_width(),
        8
    );
}

#[test]
fn the_alphabet_maximum_applies_after_filtering_not_to_authored_characters() {
    for excluded_count in [3, 4] {
        let mut builder = InteractionPolicyBuilder::default();
        let excluded = ["0", "1", "2", "3"]
            .into_iter()
            .take(excluded_count)
            .collect::<Vec<_>>();
        apply(
            &mut builder,
            "filtered",
            json!({"labels":{
                "alphabet":"abcdefghijklmnopqrstuvwxyz0123456789",
                "excluded_chars":excluded
            }}),
        );
        let policy = builder.finish(revision()).unwrap();
        let result = policy.compile_hint_codebook(InteractionFamily::Links);
        if excluded_count == 4 {
            let compiled = result.unwrap().unwrap();
            assert_eq!(compiled.codebook().configured_alphabet().len(), 36);
            assert_eq!(
                compiled.codebook().effective_alphabet(),
                "abcdefghijklmnopqrstuvwxyz456789"
            );
            assert_eq!(
                compiled
                    .codebook()
                    .clone()
                    .allocator(5000)
                    .unwrap()
                    .suffix_width(),
                3
            );
        } else {
            assert!(matches!(
                result,
                Err(HintCodebookError::InvalidEffectiveAlphabet { count: 33 })
            ));
        }
    }
}
