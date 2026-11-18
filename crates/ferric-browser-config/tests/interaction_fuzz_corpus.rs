//! Keep positive fuzz seeds on their intended parser/compiler paths.
use ferric_browser_config::{
    CompiledInteractionPredicates, InteractionAttributeEvidence, InteractionNameEvidence,
    InteractionPolicyPatch, InteractionPredicateFacts, InteractionPredicateWork, InteractionRoot,
    InteractionSelector,
};
use ferric_browser_core::LogicalKeyToken;

#[test]
fn positive_interaction_seeds_reach_validated_policy_and_root_parsers() {
    for source in [
        include_str!("../../../fuzz/corpus/interaction_policy/anchor"),
        include_str!("../../../fuzz/corpus/interaction_policy/collection"),
        include_str!("../../../fuzz/corpus/interaction_policy/reciprocal-remaps"),
    ] {
        let patch: InteractionPolicyPatch = toml::from_str(source).unwrap();
        patch.validate_authored().unwrap();
    }
    let root: InteractionRoot =
        toml::from_str(include_str!("../../../fuzz/corpus/interaction_policy/root")).unwrap();
    root.validate(0, |selector| {
        InteractionSelector::parse(selector)
            .map(|_| ())
            .map_err(|error| error.to_string())
    })
    .unwrap();
    InteractionSelector::parse(include_str!(
        "../../../fuzz/corpus/interaction_policy/selector"
    ))
    .unwrap();
    let token = LogicalKeyToken::parse(include_str!(
        "../../../fuzz/corpus/interaction_policy/key-alias"
    ))
    .unwrap();
    assert_eq!(token.as_str(), "Ctrl+Shift+p");
}

#[test]
fn negative_interaction_seeds_reject_unknown_sections_nulls_and_duplicate_fields() {
    for source in [
        include_str!("../../../fuzz/corpus/interaction_policy/unknown-consumer"),
        include_str!("../../../fuzz/corpus/interaction_policy/null-and-duplicates"),
        r#"{"hint_page_keys":null}"#,
        r#"{"enabled":true,"enabled":false}"#,
    ] {
        assert!(serde_json::from_str::<InteractionPolicyPatch>(source).is_err());
    }
}

#[test]
fn independent_fact_seed_reaches_a_positive_conjunctive_match() {
    let probe: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fuzz/corpus/interaction_policy/independent-facts"
    ))
    .unwrap();
    let patch: InteractionPolicyPatch = serde_json::from_value(probe["policy"].clone()).unwrap();
    patch.validate_authored().unwrap();
    let predicate = patch.anchors.as_ref().unwrap()[0]
        .predicates
        .as_ref()
        .unwrap();
    let compiled = CompiledInteractionPredicates::try_from(predicate).unwrap();
    let facts = InteractionPredicateFacts {
        hint_name: InteractionNameEvidence::complete(probe["name"].as_str().unwrap()).unwrap(),
        is_link: Some(true),
        href: InteractionAttributeEvidence::CompletePresent(probe["href"].as_str().unwrap()),
        role: InteractionAttributeEvidence::CompletePresent(probe["role"].as_str().unwrap()),
        ..Default::default()
    };
    assert_eq!(
        compiled.matches(
            &facts,
            |name| {
                assert_eq!(name, "data-id");
                InteractionAttributeEvidence::CompletePresent(
                    probe["attribute_value"].as_str().unwrap(),
                )
            },
            &mut InteractionPredicateWork::default()
        ),
        Ok(true)
    );
}
