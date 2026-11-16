#![no_main]

use ferric_browser_config::{
    CompiledInteractionAttributeTest, CompiledInteractionPredicates, InteractionAttributeEvidence,
    InteractionNameEvidence, InteractionPolicyPatch, InteractionPredicateFacts,
    InteractionPredicateWork, InteractionRoot, InteractionSelector, SitePolicyExchange,
};
use ferric_browser_core::LogicalKeyToken;
use libfuzzer_sys::fuzz_target;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Probe {
    policy: InteractionPolicyPatch,
    name: String,
    href: String,
    role: String,
    attribute_value: String,
}

fn check_policy(
    policy: InteractionPolicyPatch,
    name: &str,
    href: &str,
    role: &str,
    attribute: &str,
) {
    if policy.validate_authored().is_err() {
        return;
    }
    let encoded = toml::to_string(&policy).expect("validated policy serializes");
    let round_trip: InteractionPolicyPatch = toml::from_str(&encoded).expect("TOML round trip");
    assert_eq!(round_trip, policy);
    assert!(round_trip.validate_authored().is_ok());
    let json = serde_json::to_string(&policy).expect("validated policy JSON");
    assert_eq!(
        serde_json::from_str::<InteractionPolicyPatch>(&json).unwrap(),
        policy
    );
    assert!(policy.compile_authored_keys().is_ok());

    // Facts are synthetic and grant no authority. Share one work counter over
    // all matches, including failed/unavailable observations and long roles.
    let mut work = InteractionPredicateWork::default();
    let facts = InteractionPredicateFacts {
        hint_name: InteractionNameEvidence::complete(name)
            .unwrap_or_else(|_| InteractionNameEvidence::truncated()),
        is_link: Some(true),
        href: InteractionAttributeEvidence::CompletePresent(href),
        role: InteractionAttributeEvidence::CompletePresent(role),
        width_px: Some(0.),
        height_px: Some(0.),
        viewport_x: Some(0.),
        viewport_y: Some(0.),
    };
    let predicates = policy
        .anchors
        .iter()
        .flatten()
        .filter_map(|anchor| anchor.predicates.as_ref())
        .chain(
            policy
                .collections
                .iter()
                .flatten()
                .filter_map(|collection| collection.primary_predicates.as_ref()),
        );
    for predicate in predicates {
        let compiled = CompiledInteractionPredicates::try_from(predicate)
            .expect("validated predicate compiles");
        let _ = compiled.matches(
            &facts,
            |_| InteractionAttributeEvidence::CompletePresent(attribute),
            &mut work,
        );
        let unavailable = compiled.matches(
            &InteractionPredicateFacts::default(),
            |_| InteractionAttributeEvidence::Missing,
            &mut work,
        );
        if !predicate.is_empty() {
            assert!(!matches!(unavailable, Ok(true)));
        }
        for test in predicate.attributes.iter().flatten() {
            check_attribute(test, attribute);
        }
    }
    for test in policy
        .collections
        .iter()
        .flatten()
        .flat_map(|collection| collection.item_attributes.iter().flatten())
    {
        let compiled =
            CompiledInteractionAttributeTest::try_from(test).expect("validated item attribute");
        let _ = compiled.matches(
            InteractionAttributeEvidence::CompletePresent(attribute),
            &mut work,
        );
        check_attribute(test, attribute);
    }
}

fn check_attribute(test: &ferric_browser_config::InteractionAttributeTest, value: &str) {
    // Independent literal oracle; this is neither name normalization nor DOM
    // authority. Bounded operands fit a fresh standalone predicate budget.
    let expected = test.present == Some(true)
        || test.equals.as_deref().is_some_and(|text| text == value)
        || test
            .prefix
            .as_deref()
            .is_some_and(|text| value.starts_with(text));
    let compiled = CompiledInteractionAttributeTest::try_from(test).unwrap();
    assert_eq!(
        compiled.matches(
            InteractionAttributeEvidence::CompletePresent(value),
            &mut InteractionPredicateWork::default()
        ),
        Ok(expected)
    );
}

fuzz_target!(|data: &[u8]| {
    // Existing aggregate configuration input bound; this harness is pure and
    // has no application publication or native input entry point.
    if data.len() > 2 * 1024 * 1024 {
        return;
    }
    if let Ok(document) = SitePolicyExchange::parse(data) {
        match document.encode() {
            Ok(encoded) => assert_eq!(
                SitePolicyExchange::parse(encoded.as_bytes()).unwrap(),
                document
            ),
            Err(error) => assert_eq!(error, "site policy exchange exceeds 2 MiB"),
        }
    }
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    // The probe is fuzz-harness data, never a product configuration envelope.
    // It lets policy and valid candidate facts mutate independently.
    if let Ok(probe) = serde_json::from_str::<Probe>(input) {
        check_policy(
            probe.policy,
            &probe.name,
            &probe.href,
            &probe.role,
            &probe.attribute_value,
        );
    }
    if let Ok(token) = LogicalKeyToken::parse(input) {
        assert_eq!(LogicalKeyToken::parse(token.as_str()).unwrap(), token);
    }
    if let Ok(selector) = InteractionSelector::parse(input) {
        assert!(input.len() <= 1024);
        drop(selector);
    }
    if let Ok(root) = toml::from_str::<InteractionRoot>(input)
        && root
            .validate(0, |selector| {
                InteractionSelector::parse(selector)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            })
            .is_ok()
    {
        let encoded = toml::to_string(&root).unwrap();
        assert_eq!(toml::from_str::<InteractionRoot>(&encoded).unwrap(), root);
    }
    if let Ok(policy) = toml::from_str::<InteractionPolicyPatch>(input) {
        check_policy(policy, input, input, input, input);
    }
    if let Ok(policy) = serde_json::from_str::<InteractionPolicyPatch>(input) {
        check_policy(policy, input, input, input, input);
    }
});
