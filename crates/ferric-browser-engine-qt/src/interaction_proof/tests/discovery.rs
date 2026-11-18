use super::*;

fn renderer(patch: Option<Value>) -> (AnnotationRequest, Value, Vec<HintCandidate>) {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/site-policy/witness-observed-anchor.json"
    ))
    .unwrap();
    let mut request = request(patch.unwrap_or_else(|| fixture["patch"].clone()));
    // This fixture invokes the fixed browser test context without a scope.
    // No request identity/revision is reconstructed from renderer output.
    request.stamp.scope_lease.clear();
    let candidates = crate::parse_hint_payload(&fixture["candidates"].to_string()).unwrap();
    (request, fixture["annotation"].clone(), candidates)
}

fn observed_decode(
    value: &Value,
    request: &AnnotationRequest,
    candidates: &[HintCandidate],
) -> Result<CheckedAnnotations, AnnotationError> {
    decode_annotations(&value.to_string(), request, candidates)
}

fn targets(value: &mut Value) -> &mut Vec<Value> {
    value["outcomes"][0]["result"]["anchor-observed"]["targets"]
        .as_array_mut()
        .unwrap()
}

pub(super) fn collection() -> (AnnotationRequest, Value, Vec<HintCandidate>) {
    let patch = json!({"enabled":true,"collections":[{"id":"items","codes":["1"],"families":["links"],"container_self":true,"item_selector":".item","primary_selector":"a","item_attributes":[{"name":"data-include","equals":"yes"}],"identity_attributes":["data-id"],"order":"dom"}]});
    let (request, mut value, candidates) = renderer(Some(patch));
    let primary = targets(&mut value)[0].clone();
    value["roots"]
        .as_array_mut()
        .unwrap()
        .push(root(2, node(2, 100), "element"));
    value["outcomes"] = json!([{"rule":{"kind":"collection","id":"items"},"result":{"collection-observed":{
    "root":1,"container":node(2,2),"container_kind":"document","items":[{
        "item":node(2,100),"parent_item":null,
        "attributes":[{"name":"data-include","fact":{"complete-present":{"value":"yes"}}}],
        "identity_attributes":[{"name":"data-id","fact":{"complete-present":{"value":"one"}}}],
        "primary":{"targets-complete":{"root":2,"targets":[primary]}}
    }]}}}]);
    (request, value, candidates)
}

fn items(value: &mut Value) -> &mut Vec<Value> {
    value["outcomes"][0]["result"]["collection-observed"]["items"]
        .as_array_mut()
        .unwrap()
}

#[test]
fn actual_renderer_fixture_normalizes_without_a_fixture_enum_adapter() {
    let (request, value, candidates) = renderer(None);
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    let wire::ResultFacts::AnchorComplete { matches, .. } = &checked.wire.outcomes[0].result else {
        panic!("raw observations must normalize before becoming checked data");
    };
    assert_eq!(matches.len(), 1);
    assert_eq!(checked.candidate_snapshot(0), Some(&candidates[0]));
    assert_eq!(
        checked.policy_snapshot().revision(),
        request.policy.revision()
    );
}

#[test]
fn canonical_nonmatch_is_valid_evidence_and_does_not_remove_ordinary_candidates() {
    let patch = json!({"enabled":true,"anchors":[{"id":"observed","code":"x","families":["links"],"target_selector":".target","predicates":{"hint_name":{"equals":"Different"}}}]});
    let (request, value, candidates) = renderer(Some(patch));
    let before = candidates.clone();
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    let wire::ResultFacts::AnchorComplete { matches, .. } = &checked.wire.outcomes[0].result else {
        panic!()
    };
    assert!(matches.is_empty());
    assert!(checked.candidate_snapshot(0).is_none());
    assert_eq!(candidates, before);
    assert_eq!(checked.root_count(), 1);
}

#[test]
fn raw_required_name_evidence_is_unprovable_instead_of_a_false_match() {
    let patch = json!({"enabled":true,"anchors":[{"id":"observed","code":"x","families":["links"],"target_selector":".target","predicates":{"hint_name":{"equals":"Item 0"}}}]});
    for (fact, reason) in [
        (json!({"truncated":{}}), wire::UnavailableReason::Truncated),
        (
            json!({"missing":{}}),
            wire::UnavailableReason::MissingEvidence,
        ),
    ] {
        let (request, mut value, candidates) = renderer(Some(patch.clone()));
        targets(&mut value)[0]["name"] = fact;
        let checked = observed_decode(&value, &request, &candidates).unwrap();
        assert!(
            matches!(checked.wire.outcomes[0].result, wire::ResultFacts::Unavailable {reason:actual} if actual==reason)
        );
        assert_eq!(checked.root_count(), 0);
        assert_eq!(checked.document_count(), 0);
    }
}

#[test]
fn malformed_raw_references_are_rejected_even_after_an_early_predicate_nonmatch() {
    let patch = json!({"enabled":true,"anchors":[{"id":"observed","code":"x","families":["links"],"target_selector":".target","predicates":{"hint_name":{"equals":"Different"}}}]});
    let (request, mut value, candidates) = renderer(Some(patch));
    targets(&mut value)[0]["candidate"]["index"] = json!(99);
    assert_eq!(
        observed_decode(&value, &request, &candidates).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}

#[test]
fn structural_decode_obeys_exact_request_allowance_and_reports_actual_work() {
    let (request, value, candidates) = renderer(None);
    let raw = value.to_string();
    let baseline = decode_annotation_facts(&raw, &request, candidates.clone()).unwrap();
    let spent = baseline.verification_steps();
    assert!(spent > 1);
    for allowance in [spent, spent + 1, usize::MAX] {
        let facts =
            decode_annotation_facts_with_budget(&raw, &request, candidates.clone(), allowance)
                .unwrap();
        assert_eq!(facts.verification_steps(), spent);
        assert_eq!(facts.original_annotation(), raw);
    }
    assert_eq!(
        decode_annotation_facts_with_budget(&raw, &request, candidates, spent - 1).unwrap_err(),
        AnnotationError::WorkLimit
    );
}

#[test]
fn exhausted_verification_rejects_before_parsing_a_new_payload() {
    let (request, _, candidates) = renderer(None);
    assert_eq!(
        decode_annotation_facts_with_budget("invalid JSON", &request, candidates.clone(), 0)
            .unwrap_err(),
        AnnotationError::WorkLimit
    );
    assert_eq!(
        decode_annotation_facts_with_budget("invalid JSON", &request, candidates, 1).unwrap_err(),
        AnnotationError::InvalidSchema
    );
}

#[test]
fn raw_stage_preserves_original_facts_until_explicit_normalization() {
    let patch = json!({"enabled":true,"anchors":[{"id":"observed","code":"x","families":["links"],"target_selector":".target","predicates":{"hint_name":{"equals":"Different"}}}]});
    let (request, value, candidates) = renderer(Some(patch));
    let encoded = format!(" \n{value}\n");
    let before = candidates.clone();
    let facts = decode_annotation_facts(&encoded, &request, candidates).unwrap();
    assert_eq!(facts.original_annotation(), encoded);
    assert!(matches!(
        facts.wire.outcomes[0].result,
        wire::ResultFacts::AnchorObserved { .. }
    ));
    assert_eq!(facts.candidates.as_ref(), before.as_slice());
    let raw_documents = facts.association_documents_json().unwrap();
    let checked = facts.normalize().unwrap();
    let wire::ResultFacts::AnchorComplete { matches, .. } = &checked.wire.outcomes[0].result else {
        panic!("normalized anchor")
    };
    assert!(matches.is_empty());
    assert!(checked.candidate_snapshot(0).is_none());
    assert_eq!(checked.original_annotation(), encoded);
    assert_eq!(checked.association_documents_json().unwrap(), raw_documents);
}

#[test]
fn raw_stage_rejects_bad_excluded_references_before_native_association() {
    let (request, mut value, candidates) = collection();
    items(&mut value)[0]["attributes"][0]["fact"] = json!({"complete-present":{"value":"no"}});
    items(&mut value)[0]["primary"]["targets-complete"]["targets"][0]["candidate"]["index"] =
        json!(99);
    assert_eq!(
        decode_annotation_facts(&value.to_string(), &request, candidates).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}

#[test]
fn two_stage_normalization_matches_immediate_decoder_without_resetting_work() {
    for spent in [0, ferric_browser_config::MAX_INTERACTION_PREDICATE_STEPS] {
        let patch = json!({"enabled":true,"anchors":[{"id":"observed","code":"x","families":["links"],"target_selector":".target","predicates":{"hint_name":{"equals":"Item 0"}}}]});
        let (request, mut value, candidates) = renderer(Some(patch));
        value["discovery_work"]["predicate_steps"] = json!(spent);
        let encoded = value.to_string();
        let immediate = decode_annotations(&encoded, &request, &candidates).unwrap();
        let staged = decode_annotation_facts(&encoded, &request, candidates)
            .unwrap()
            .normalize()
            .unwrap();
        assert_eq!(
            serde_json::to_value(&staged).unwrap(),
            serde_json::to_value(&immediate).unwrap()
        );
        if spent == ferric_browser_config::MAX_INTERACTION_PREDICATE_STEPS {
            assert!(matches!(
                staged.wire.outcomes[0].result,
                wire::ResultFacts::Unavailable {
                    reason: wire::UnavailableReason::PredicateLimit
                }
            ));
        } else {
            let wire::ResultFacts::AnchorComplete { matches, .. } = &staged.wire.outcomes[0].result
            else {
                panic!("complete anchor")
            };
            assert_eq!(matches.len(), 1);
        }
    }
}

#[test]
fn observed_fragment_identity_is_required_and_bounded() {
    let (request, value, candidates) = renderer(None);
    for field in [Value::Null, json!(8), json!(-1), json!(0.5)] {
        let mut changed = value.clone();
        targets(&mut changed)[0]["fragment_index"] = field;
        assert!(observed_decode(&changed, &request, &candidates).is_err());
    }
    let mut missing = value;
    targets(&mut missing)[0]
        .as_object_mut()
        .unwrap()
        .remove("fragment_index");
    assert!(observed_decode(&missing, &request, &candidates).is_err());
}

#[test]
fn discovery_work_never_defaults_to_zero_and_is_bounded_when_present() {
    let (request, value, candidates) = renderer(None);
    for work in [
        Value::Null,
        json!({"predicate_steps":100_001,"inspections":1}),
        json!({"predicate_steps":0,"inspections":50_001}),
        json!({"predicate_steps":-1,"inspections":1}),
    ] {
        let mut changed = value.clone();
        changed["discovery_work"] = work;
        assert!(observed_decode(&changed, &request, &candidates).is_err());
    }
    let mut missing = value;
    missing.as_object_mut().unwrap().remove("discovery_work");
    assert!(observed_decode(&missing, &request, &candidates).is_err());
}

#[test]
fn canonical_work_exhaustion_makes_raw_evidence_unavailable() {
    let patch = json!({"enabled":true,"anchors":[{"id":"observed","code":"x","families":["links"],"target_selector":".target","predicates":{"hint_name":{"equals":"Item 0"}}}]});
    let (request, mut value, candidates) = renderer(Some(patch));
    value["discovery_work"]["predicate_steps"] = json!(100_000);
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    assert!(matches!(
        checked.wire.outcomes[0].result,
        wire::ResultFacts::Unavailable {
            reason: wire::UnavailableReason::PredicateLimit
        }
    ));
}

#[test]
fn excluded_item_may_have_an_unavailable_primary_and_missing_identity() {
    let (request, mut value, candidates) = collection();
    items(&mut value)[0]["attributes"][0]["fact"] = json!({"complete-present":{"value":"no"}});
    items(&mut value)[0]["primary"] = json!({"unavailable":{"reason":"unsupported-root"}});
    items(&mut value)[0]["identity_attributes"][0]["fact"] = json!({"missing":{}});
    value["roots"].as_array_mut().unwrap().truncate(1);
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    let wire::ResultFacts::CollectionComplete { items, .. } = &checked.wire.outcomes[0].result
    else {
        panic!()
    };
    assert!(items.is_empty());
    assert_eq!(checked.root_count(), 1);
}

#[test]
fn excluded_item_cannot_sanitize_a_malformed_primary_reference() {
    let (request, mut value, candidates) = collection();
    items(&mut value)[0]["attributes"][0]["fact"] = json!({"complete-present":{"value":"no"}});
    items(&mut value)[0]["primary"]["targets-complete"]["targets"][0]["candidate"]["index"] =
        json!(99);
    assert_eq!(
        observed_decode(&value, &request, &candidates).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}

#[test]
fn item_classification_preserves_exact_primary_and_identity() {
    let (request, value, candidates) = collection();
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    let wire::ResultFacts::CollectionComplete { items, .. } = &checked.wire.outcomes[0].result
    else {
        panic!()
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].logical_node.node.get(), 100);
    assert_eq!(items[0].primary_root.get(), 2);
    assert_eq!(checked.candidate_snapshot(0), Some(&candidates[0]));
}

#[test]
fn malformed_original_orphans_are_rejected_before_normalization_prunes() {
    let (request, mut value, candidates) = collection();
    items(&mut value)[0]["attributes"][0]["fact"] = json!({"complete-present":{"value":"no"}});
    value["roots"]
        .as_array_mut()
        .unwrap()
        .push(root(3, node(2, 200), "element"));
    assert_eq!(
        observed_decode(&value, &request, &candidates).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}

#[test]
fn ordered_core_short_circuit_semantics_are_preserved() {
    let patch = json!({"enabled":true,"anchors":[{"id":"observed","code":"x","families":["links"],"target_selector":".target","predicates":{"hint_name":{"equals":"Different"},"role":"button"}}]});
    let (request, mut value, candidates) = renderer(Some(patch));
    targets(&mut value)[0]["role"] = json!({"missing":{}});
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    assert!(
        matches!(&checked.wire.outcomes[0].result,wire::ResultFacts::AnchorComplete{matches,..} if matches.is_empty())
    );

    let patch = json!({"enabled":true,"anchors":[{"id":"observed","code":"x","families":["links"],"target_selector":".target","predicates":{"hint_name":{"equals":"Item 0"},"attributes":[{"name":"data-include","equals":"yes"}]}}]});
    let (request, mut value, candidates) = renderer(Some(patch));
    targets(&mut value)[0]["name"] = json!({"missing":{}});
    targets(&mut value)[0]["attributes"] =
        json!([{"name":"data-include","fact":{"complete-present":{"value":"no"}}}]);
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    assert!(matches!(
        checked.wire.outcomes[0].result,
        wire::ResultFacts::Unavailable {
            reason: wire::UnavailableReason::MissingEvidence
        }
    ));
}

#[test]
fn unavailable_item_attributes_cannot_exclude_an_unknown_primary() {
    for (fact, reason) in [
        (
            json!({"missing":{}}),
            wire::UnavailableReason::MissingEvidence,
        ),
        (json!({"truncated":{}}), wire::UnavailableReason::Truncated),
    ] {
        let (request, mut value, candidates) = collection();
        items(&mut value)[0]["attributes"][0]["fact"] = fact;
        items(&mut value)[0]["primary"] = json!({"unavailable":{"reason":"unsupported-root"}});
        value["roots"].as_array_mut().unwrap().truncate(1);
        let checked = observed_decode(&value, &request, &candidates).unwrap();
        assert!(
            matches!(checked.wire.outcomes[0].result,wire::ResultFacts::Unavailable{reason:actual} if actual==reason)
        );
    }
}

#[test]
fn eligible_item_requires_complete_identity_evidence() {
    for (fact, reason) in [
        (
            json!({"missing":{}}),
            wire::UnavailableReason::MissingEvidence,
        ),
        (json!({"truncated":{}}), wire::UnavailableReason::Truncated),
        (
            json!({"complete-absent":{}}),
            wire::UnavailableReason::MissingEvidence,
        ),
    ] {
        let (request, mut value, candidates) = collection();
        items(&mut value)[0]["identity_attributes"][0]["fact"] = fact;
        let checked = observed_decode(&value, &request, &candidates).unwrap();
        assert!(
            matches!(checked.wire.outcomes[0].result,wire::ResultFacts::Unavailable{reason:actual} if actual==reason)
        );
    }
}

fn observed_as_strict(value: &mut Value, index: usize) {
    let mut result = value["outcomes"][index]["result"]["anchor-observed"].clone();
    let targets = result.as_object_mut().unwrap().remove("targets").unwrap();
    result["matches"] = targets;
    value["outcomes"][index]["result"] = json!({"anchor-complete":result});
}

#[test]
fn strict_complete_with_discovery_work_keeps_its_existing_contract() {
    let patch = json!({"enabled":true,"anchors":[{"id":"observed","code":"x","families":["links"],"target_selector":".target","predicates":{"hint_name":{"equals":"Different"}}}]});
    let (request, mut value, candidates) = renderer(Some(patch));
    observed_as_strict(&mut value, 0);
    assert_eq!(
        observed_decode(&value, &request, &candidates).unwrap_err(),
        AnnotationError::IncompleteEvidence
    );
    value["outcomes"][0]["result"]["anchor-complete"]["matches"][0]["name"] = json!({"missing":{}});
    assert_eq!(
        observed_decode(&value, &request, &candidates).unwrap_err(),
        AnnotationError::IncompleteEvidence
    );
}

#[test]
fn mixed_and_raw_rules_share_one_remaining_canonical_allowance() {
    use ferric_browser_config::{
        CompiledInteractionPredicates, InteractionAttributeEvidence, InteractionNameEvidence,
        InteractionPredicateFacts, InteractionPredicateWork,
    };
    let patch = json!({"enabled":true,"anchors":[
        {"id":"observed","code":"x","families":["links"],"target_selector":".target","predicates":{"hint_name":{"equals":"Item 0"}}},
        {"id":"second","code":"y","families":["links"],"target_selector":".target","predicates":{"hint_name":{"equals":"Item 0"}}}]} );
    let (request, mut value, candidates) = renderer(Some(patch));
    let mut second = value["outcomes"][0].clone();
    second["rule"]["id"] = json!("second");
    value["outcomes"].as_array_mut().unwrap().push(second);
    let compiled = CompiledInteractionPredicates::try_from(
        request.policy.anchors()[0].predicates.as_ref().unwrap(),
    )
    .unwrap();
    let facts = InteractionPredicateFacts {
        hint_name: InteractionNameEvidence::complete("Item 0").unwrap(),
        ..Default::default()
    };
    let mut work = InteractionPredicateWork::default();
    assert!(
        compiled
            .matches(&facts, |_| InteractionAttributeEvidence::Missing, &mut work)
            .unwrap()
    );
    value["discovery_work"]["predicate_steps"] = json!(work.remaining());
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    assert!(
        matches!(&checked.wire.outcomes[0].result,wire::ResultFacts::AnchorComplete{matches,..} if matches.len()==1)
    );
    assert!(matches!(
        checked.wire.outcomes[1].result,
        wire::ResultFacts::Unavailable {
            reason: wire::UnavailableReason::PredicateLimit
        }
    ));

    let mut strict_first = value.clone();
    observed_as_strict(&mut strict_first, 0);
    let checked = observed_decode(&strict_first, &request, &candidates).unwrap();
    assert!(matches!(
        checked.wire.outcomes[1].result,
        wire::ResultFacts::Unavailable {
            reason: wire::UnavailableReason::PredicateLimit
        }
    ));
    let mut strict_second = value.clone();
    observed_as_strict(&mut strict_second, 1);
    assert_eq!(
        observed_decode(&strict_second, &request, &candidates).unwrap_err(),
        AnnotationError::WorkLimit
    );

    value["outcomes"][1]["result"]["anchor-observed"]["targets"][0]["candidate"]["index"] =
        json!(99);
    assert_eq!(
        observed_decode(&value, &request, &candidates).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}

fn nested_collection(outer_included: bool) -> (AnnotationRequest, Value, Vec<HintCandidate>) {
    let (_, mut value, mut candidates) = collection();
    let patch = json!({"enabled":true,"collections":[{"id":"items","codes":["1"],"families":["links"],"container_self":true,"item_selector":".item","primary_root_steps":[{"kind":"open-shadow","self":true}],"primary_selector":"a","item_attributes":[{"name":"data-include","equals":"yes"}],"identity_attributes":["data-id"],"order":"dom"}]});
    let (request, _, _) = renderer(Some(patch));
    value["roots"][1]["hops"] = json!([{"open-shadow":{"host":node(2,100),"root":node(2,101)}}]);
    value["roots"][1]["resolved"] = node(2, 101);
    value["roots"][1]["resolved_kind"] = json!("open-shadow");
    if !outer_included {
        items(&mut value)[0]["attributes"][0]["fact"] = json!({"complete-present":{"value":"no"}});
    }
    let mut intermediate = items(&mut value)[0].clone();
    intermediate["item"] = node(2, 200);
    intermediate["parent_item"] = node(2, 100);
    intermediate["attributes"][0]["fact"] = json!({"complete-present":{"value":"no"}});
    intermediate["primary"] = json!({"unavailable":{"reason":"unsupported-root"}});
    let mut child = items(&mut value)[0].clone();
    child["item"] = node(2, 300);
    child["parent_item"] = node(2, 200);
    child["attributes"][0]["fact"] = json!({"complete-present":{"value":"yes"}});
    child["primary"]["targets-complete"]["root"] = json!(3);
    child["primary"]["targets-complete"]["targets"][0]["candidate"]["index"] = json!(1);
    child["primary"]["targets-complete"]["targets"][0]["candidate"]["element_id"] = json!(20);
    items(&mut value).extend([intermediate, child]);
    let mut child_root = root(3, node(2, 300), "element");
    child_root["hops"] = json!([{"open-shadow":{"host":node(2,300),"root":node(2,301)}}]);
    child_root["resolved"] = node(2, 301);
    child_root["resolved_kind"] = json!("open-shadow");
    value["roots"].as_array_mut().unwrap().push(child_root);
    let mut candidate = candidates[0].clone();
    candidate.element_id = 20;
    candidates.push(candidate);
    (request, value, candidates)
}

#[test]
fn eligible_nesting_is_not_hidden_by_an_excluded_intermediate_item() {
    let (request, value, candidates) = nested_collection(true);
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    assert!(matches!(
        checked.wire.outcomes[0].result,
        wire::ResultFacts::Unavailable {
            reason: wire::UnavailableReason::NestedItems
        }
    ));
    let (request, value, candidates) = nested_collection(false);
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    let wire::ResultFacts::CollectionComplete { items, .. } = &checked.wire.outcomes[0].result
    else {
        panic!()
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].logical_node.node.get(), 300);
    assert_eq!(checked.root_count(), 2);
    assert!(checked.candidate_snapshot(0).is_none());
    assert_eq!(checked.candidate_snapshot(1), Some(&candidates[1]));
}

#[test]
fn dangling_cyclic_and_cross_document_parent_metadata_is_rejected() {
    let (request, value, candidates) = nested_collection(false);
    for parent in [node(2, 999), node(2, 200), node(2, 300), node(1, 100)] {
        let mut changed = value.clone();
        items(&mut changed)[1]["parent_item"] = parent;
        assert!(observed_decode(&changed, &request, &candidates).is_err());
    }
}

#[test]
fn primary_self_known_ineligible_element_does_not_disable_valid_items() {
    let (_, mut value, candidates) = collection();
    let patch = json!({"enabled":true,"collections":[{"id":"items","codes":["1"],"families":["links"],"container_self":true,"item_selector":".item","primary_self":true,"item_attributes":[{"name":"data-include","equals":"yes"}],"identity_attributes":["data-id"],"order":"dom"}]});
    let (request, _, _) = renderer(Some(patch));
    items(&mut value)[0]["item"] = node(2, 1);
    value["roots"][1] = root(2, node(2, 1), "element");
    let mut hidden = items(&mut value)[0].clone();
    hidden["item"] = node(2, 200);
    hidden["primary"] = json!({"targets-complete":{"root":3,"targets":[]}});
    items(&mut value).push(hidden);
    value["roots"]
        .as_array_mut()
        .unwrap()
        .push(root(3, node(2, 200), "element"));
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    let wire::ResultFacts::CollectionComplete { items, .. } = &checked.wire.outcomes[0].result
    else {
        panic!()
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].logical_node.node.get(), 1);
}

#[test]
fn predicate_false_cannot_hide_inconsistent_finite_geometry_or_references() {
    let patch = json!({"enabled":true,"anchors":[{"id":"observed","code":"x","families":["links"],"target_selector":".target","predicates":{"hint_name":{"equals":"Different"}}}]});
    let (request, value, candidates) = renderer(Some(patch));
    for (field, replacement) in [
        (
            "frame_owner",
            json!("frame-22222222-2222-4222-8222-222222222222"),
        ),
        ("frame_path", json!("0.1")),
        ("element_id", json!(999)),
        ("related_element_id", json!(10)),
    ] {
        let mut changed = value.clone();
        targets(&mut changed)[0]["candidate"][field] = replacement;
        assert_eq!(
            observed_decode(&changed, &request, &candidates).unwrap_err(),
            AnnotationError::InvalidAssociation
        );
    }
    let mut changed = value.clone();
    targets(&mut changed)[0]["projected_primary_fragment_css"]["x"] = json!(300.);
    assert_eq!(
        observed_decode(&changed, &request, &candidates).unwrap_err(),
        AnnotationError::InvalidGeometry
    );
    let mut changed = value;
    let duplicate = targets(&mut changed)[0].clone();
    targets(&mut changed).push(duplicate);
    assert_eq!(
        observed_decode(&changed, &request, &candidates).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}

#[test]
fn raw_names_use_canonical_unicode_and_ascii_case_matching() {
    for (expected, case, observed) in [
        ("item 0", json!("ascii-insensitive"), "Item 0"),
        ("é", json!("sensitive"), " e\u{301} "),
    ] {
        let patch = json!({"enabled":true,"anchors":[{"id":"observed","code":"x","families":["links"],"target_selector":".target","predicates":{"hint_name":{"equals":expected,"case":case}}}]});
        let (request, mut value, candidates) = renderer(Some(patch));
        targets(&mut value)[0]["name"] = json!({"complete-present":{"value":observed}});
        let checked = observed_decode(&value, &request, &candidates).unwrap();
        assert!(
            matches!(&checked.wire.outcomes[0].result,wire::ResultFacts::AnchorComplete{matches,..} if matches.len()==1)
        );
        assert_eq!(checked.candidate_snapshot(0).unwrap().text, "Item 0");
    }
}

#[test]
fn raw_size_and_viewport_predicates_use_the_unclipped_border_and_primary_centre() {
    let patch = json!({"enabled":true,"anchors":[{"id":"observed","code":"x","families":["links"],"target_selector":".target","predicates":{"width_px":{"min":200,"max":200},"height_px":{"min":40,"max":40},"viewport_x":{"min":0.12,"max":0.12},"viewport_y":{"min":0.05,"max":0.05}}}]});
    let (request, value, candidates) = renderer(Some(patch));
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    assert!(
        matches!(&checked.wire.outcomes[0].result,wire::ResultFacts::AnchorComplete{matches,..} if matches.len()==1)
    );
}

fn add_second_primary(value: &mut Value, candidates: &mut Vec<HintCandidate>) {
    let mut candidate = candidates[0].clone();
    candidate.element_id = 20;
    candidate.text = "Second".into();
    candidates.push(candidate);
    let primary = &mut items(value)[0]["primary"]["targets-complete"]["targets"];
    let mut second = primary[0].clone();
    second["candidate"]["index"] = json!(1);
    second["candidate"]["element_id"] = json!(20);
    second["name"] = json!({"complete-present":{"value":"Second"}});
    primary.as_array_mut().unwrap().push(second);
}

#[test]
fn complete_raw_primary_counts_distinguish_ineligibility_unique_and_ambiguous() {
    for count in 0..=2 {
        let (request, mut value, mut candidates) = collection();
        add_second_primary(&mut value, &mut candidates);
        items(&mut value)[0]["primary"]["targets-complete"]["targets"]
            .as_array_mut()
            .unwrap()
            .truncate(count);
        let before = candidates.clone();
        let checked = observed_decode(&value, &request, &candidates).unwrap();
        if count == 2 {
            assert!(matches!(
                checked.wire.outcomes[0].result,
                wire::ResultFacts::Unavailable {
                    reason: wire::UnavailableReason::AmbiguousPrimary
                }
            ));
            assert_eq!(checked.root_count(), 0);
        } else {
            let wire::ResultFacts::CollectionComplete { items, .. } =
                &checked.wire.outcomes[0].result
            else {
                panic!()
            };
            assert_eq!(items.len(), count);
        }
        assert_eq!(candidates, before);
    }
}

#[test]
fn canonical_primary_filtering_preserves_original_candidate_index_one() {
    let patch = json!({"enabled":true,"collections":[{"id":"items","codes":["1"],"families":["links"],"container_self":true,"item_selector":".item","primary_selector":"a","item_attributes":[{"name":"data-include","equals":"yes"}],"identity_attributes":["data-id"],"primary_predicates":{"hint_name":{"equals":"Second"}},"order":"dom"}]});
    let (request, _, _) = renderer(Some(patch));
    let (_, mut value, mut candidates) = collection();
    add_second_primary(&mut value, &mut candidates);
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    let wire::ResultFacts::CollectionComplete { items: kept, .. } =
        &checked.wire.outcomes[0].result
    else {
        panic!()
    };
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].primary.candidate.index, 1);
    assert_eq!(checked.candidate_snapshot(0), None);
    assert_eq!(checked.candidate_snapshot(1), Some(&candidates[1]));
    // A later positive cannot hide uncertainty about the earlier primary.
    items(&mut value)[0]["primary"]["targets-complete"]["targets"][0]["name"] =
        json!({"missing":{}});
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    assert!(matches!(
        checked.wire.outcomes[0].result,
        wire::ResultFacts::Unavailable {
            reason: wire::UnavailableReason::MissingEvidence
        }
    ));
}

#[test]
fn duplicate_primary_ambiguity_counts_only_attribute_included_items() {
    for include_second in [false, true] {
        let (request, mut value, candidates) = collection();
        let mut second = items(&mut value)[0].clone();
        second["item"] = node(2, 200);
        second["primary"]["targets-complete"]["root"] = json!(3);
        second["attributes"][0]["fact"] =
            json!({"complete-present":{"value":if include_second {"yes"} else {"no"}}});
        items(&mut value).push(second);
        value["roots"]
            .as_array_mut()
            .unwrap()
            .push(root(3, node(2, 200), "element"));
        let checked = observed_decode(&value, &request, &candidates).unwrap();
        if include_second {
            assert!(matches!(
                checked.wire.outcomes[0].result,
                wire::ResultFacts::Unavailable {
                    reason: wire::UnavailableReason::AmbiguousPrimary
                }
            ));
            assert_eq!(checked.root_count(), 0);
        } else {
            let wire::ResultFacts::CollectionComplete { items, .. } =
                &checked.wire.outcomes[0].result
            else {
                panic!()
            };
            assert_eq!(items.len(), 1);
            assert_eq!(checked.root_count(), 2);
            assert_eq!(checked.candidate_snapshot(0), Some(&candidates[0]));
        }
    }
}

#[test]
fn native_association_inventory_preserves_raw_documents_removed_by_normalization() {
    let patch = json!({"enabled":true,"anchors":[{"id":"observed","code":"x","families":["links"],"target_selector":".target","predicates":{"hint_name":{"equals":"Item 0"}}}]});
    let (request, mut value, candidates) = renderer(Some(patch));
    let original = value["documents"].clone();
    targets(&mut value)[0]["name"] = json!({"missing":{}});
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    assert_eq!(checked.document_count(), 0);
    assert_eq!(checked.root_count(), 0);
    let raw: Value = serde_json::from_str(&checked.association_documents_json().unwrap()).unwrap();
    assert_eq!(raw, original);
    let normalized = serde_json::to_value(&checked).unwrap();
    assert_eq!(normalized["documents"], json!([]));
    assert!(normalized.get("association_documents").is_none());
}

#[test]
fn excluded_item_child_document_remains_in_the_raw_native_association_table() {
    let patch = json!({"enabled":true,"collections":[{"id":"items","codes":["1"],"families":["links"],"container_self":true,"item_selector":".item","primary_root_steps":[{"kind":"same-origin-frame","self":true}],"primary_selector":"a","item_attributes":[{"name":"data-include","equals":"yes"}],"identity_attributes":["data-id"],"order":"dom"}]});
    let (request, _, _) = renderer(Some(patch));
    let (_, mut value, mut candidates) = collection();
    items(&mut value)[0]["attributes"][0]["fact"] = json!({"complete-absent":{}});
    let origin = value["documents"][0]["origin"].clone();
    value["documents"].as_array_mut().unwrap().push(json!({"id":3,"frame_owner":"","frame_path":"0.0","epoch":"11111111-1111-4111-8111-111111111111","origin":origin,"document_node":3,"parent_host":node(2,100)}));
    value["roots"][1] = json!({"id":2,"start":node(2,100),"start_kind":"element","hops":[{"same-origin-frame":{"host":node(2,100),"document":node(3,3)}}],"resolved":node(3,3),"resolved_kind":"document"});
    candidates[0].frame_path = "0.0".into();
    items(&mut value)[0]["primary"]["targets-complete"]["targets"][0]["candidate"]["frame_path"] =
        json!("0.0");
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    let wire::ResultFacts::CollectionComplete { items, .. } = &checked.wire.outcomes[0].result
    else {
        panic!()
    };
    assert!(items.is_empty());
    assert_eq!(checked.document_count(), 1);
    assert_eq!(checked.root_count(), 1);
    let raw: Value = serde_json::from_str(&checked.association_documents_json().unwrap()).unwrap();
    assert_eq!(raw.as_array().unwrap().len(), 2);
    assert_eq!(raw[1]["id"], 3);
}

#[test]
fn focused_receiver_requirements_survive_raw_match_filtering() {
    let (request, value, candidate) = focused_chain(1, json!([]));
    let mut value = encoded(&value);
    let result = &mut value["outcomes"][0]["result"];
    let mut facts = result
        .as_object_mut()
        .unwrap()
        .remove("anchor-complete")
        .unwrap();
    let targets = facts.as_object_mut().unwrap().remove("matches").unwrap();
    facts["targets"] = targets;
    facts["targets"][0]["fragment_index"] = json!(0);
    facts["targets"][0]["name"] = json!({"missing":{}});
    *result = json!({"anchor-observed":facts});
    value["discovery_work"] = json!({"predicate_steps":0,"inspections":0});
    let immediate = observed_decode(&value, &request, std::slice::from_ref(&candidate)).unwrap();
    let pending = decode_annotation_facts(&value.to_string(), &request, vec![candidate]).unwrap();
    assert_eq!(pending.association_focused_documents_json().unwrap(), "[2]");
    let checked = pending.normalize().unwrap();
    assert_eq!(
        serde_json::to_value(&checked).unwrap(),
        serde_json::to_value(&immediate).unwrap()
    );
    assert_eq!(checked.original_annotation(), value.to_string());
    assert!(matches!(
        checked.wire.outcomes[0].result,
        wire::ResultFacts::Unavailable { .. }
    ));
    assert_eq!(checked.document_count(), 0);
    let focused: Value =
        serde_json::from_str(&checked.association_focused_documents_json().unwrap()).unwrap();
    assert_eq!(focused, json!([2]));
    assert!(
        serde_json::to_value(&checked)
            .unwrap()
            .get("association_focused_documents")
            .is_none()
    );
    let (request, value, candidates) = renderer(None);
    let checked = observed_decode(&value, &request, &candidates).unwrap();
    assert_eq!(checked.association_focused_documents_json().unwrap(), "[]");
    // Top cannot stand in for a selected descendant even before the native
    // token check: its graph depth contradicts the captured receiver role.
    let (request, value, candidate) = focused_chain(0, json!([]));
    assert_eq!(
        decode(&value, &request, &[candidate]).unwrap_err(),
        AnnotationError::InvalidRoot
    );
}
