use super::*;
use ferric_browser_core::{
    HintInteraction, HintReservationReason as Reason, HintRetentionBudget,
    HintSpecialBinding as Binding,
};

fn fixture(order: &str) -> (AnnotationRequest, Value, Vec<HintCandidate>) {
    let request = request(
        json!({"enabled":true,"collections":[{"id":"cards","families":["links"],"codes":["1","2","3"],"container_self":true,"item_selector":"a","primary_self":true,"order":order,"identity_attributes":["data-id"]}]}),
    );
    let mut raw = envelope(&request);
    let mut candidates = Vec::new();
    let mut items = Vec::new();
    for (index, (x, y)) in [(90., 0.), (10., 7.), (40., 14.), (70., 40.)]
        .into_iter()
        .enumerate()
    {
        let id = u32::try_from(index).unwrap() + 10;
        let mut candidate = candidate();
        candidate.element_id = id;
        candidate.geometry.x = x;
        candidate.geometry.y = y;
        let mut primary = witness();
        primary["candidate"]["index"] = json!(index);
        primary["candidate"]["element_id"] = json!(id);
        primary["projected_primary_fragment_css"]["x"] = json!(x);
        primary["projected_primary_fragment_css"]["y"] = json!(y);
        let primary_root = u32::try_from(index).unwrap() + 2;
        raw["roots"]
            .as_array_mut()
            .unwrap()
            .push(root(primary_root, node(1, id), "element"));
        items.push(json!({"item":node(1,id),"primary_root":primary_root,"primary":primary,"attributes":[],"identity_attributes":[{"name":"data-id","fact":{"state":"complete-present","value":format!("item-{index}")}}]}));
        candidates.push(candidate);
    }
    raw["outcomes"] = json!([{"rule":{"kind":"collection","id":"cards"},"result":{"state":"collection-complete","root":1,"container":node(1,1),"container_kind":"document","items":items}}]);
    (request, raw, candidates)
}

fn model(checked: &CheckedAnnotations, candidates: Vec<HintCandidate>) -> HintInteraction {
    let codebook = checked
        .policy_snapshot()
        .compile_hint_codebook(InteractionFamily::Links)
        .unwrap()
        .unwrap()
        .into_parts()
        .2;
    let assignments = checked.special_assignments(&candidates, codebook.declarations(), None);
    HintInteraction::new_enhanced(
        candidates,
        codebook,
        assignments,
        HintRetentionBudget::Bytes(MAX_INTERACTION_ANNOTATION_BYTES),
    )
    .unwrap()
}

#[test]
fn visual_orders_use_fixed_rows_and_keep_dom_order_explicit() {
    for (order, expected) in [
        ("dom", [10, 11, 12]),
        ("top-to-bottom", [10, 11, 12]),
        ("row-major-ltr", [11, 10, 12]),
        ("row-major-rtl", [10, 11, 12]),
    ] {
        let (request, raw, candidates) = fixture(order);
        let checked = decode(&raw, &request, &candidates).unwrap();
        let interaction = model(&checked, candidates);
        for (code, id) in ["1", "2", "3"].into_iter().zip(expected) {
            assert_eq!(
                interaction
                    .hints()
                    .iter()
                    .find(|hint| hint.label == code)
                    .unwrap()
                    .candidate
                    .element_id,
                id,
                "{order}/{code}"
            );
        }
    }
}

#[test]
fn row_tolerance_boundaries() {
    for (tolerance, ys, expected) in [
        (0., [0., 0., 14.], [11, 10, 12]),
        (7., [0., 7., 14.], [11, 10, 12]),
        (6.999, [0., 7., 14.], [10, 11, 12]),
        (32., [0., 32., 64.], [11, 10, 12]),
    ] {
        let (mut request, mut raw, mut candidates) = fixture("row-major-ltr");
        let patch = json!({"enabled":true,"collections":[{"id":"cards","families":["links"],"codes":["1","2","3"],"container_self":true,"item_selector":"a","primary_self":true,"order":"row-major-ltr","row_tolerance_px":tolerance,"identity_attributes":["data-id"]}]});
        request.policy = super::request(patch).policy;
        // Only the policy order/tolerance changes; the request revision remains captured.
        for (index, y) in ys.into_iter().enumerate() {
            raw["outcomes"][0]["result"]["items"][index]["primary"]["projected_primary_fragment_css"]
                ["y"] = json!(y);
            candidates[index].geometry.y = y;
        }
        let checked = decode(&raw, &request, &candidates).unwrap();
        let interaction = model(&checked, candidates);
        for (code, id) in ["1", "2", "3"].into_iter().zip(expected) {
            assert_eq!(
                interaction
                    .hints()
                    .iter()
                    .find(|hint| hint.label == code)
                    .unwrap()
                    .candidate
                    .element_id,
                id
            );
        }
    }
}

#[test]
fn anchor_ownership_leaves_an_ordinal_hole_and_equal_urls_stay_distinct() {
    let (mut request, mut raw, candidates) = fixture("dom");
    let mut policy = serde_json::to_value(request.policy.collections()).unwrap();
    // Add a uniquely proved anchor for the second primary, without changing ranks.
    request.policy=super::request(json!({"enabled":true,"collections":policy.take(),"anchors":[{"id":"second","code":"m","families":["links"],"target_selector":"a"}]})).policy;
    let primary = raw["outcomes"][0]["result"]["items"][1]["primary"].clone();
    raw["outcomes"].as_array_mut().unwrap().push(json!({"rule":{"kind":"anchor","id":"second"},"result":{"state":"anchor-complete","root":1,"scope":node(1,1),"scope_kind":"document","matches":[primary]}}));
    let checked = decode(&raw, &request, &candidates).unwrap();
    let interaction = model(&checked, candidates);
    assert_eq!(interaction.hints().len(), 4);
    assert_eq!(
        interaction
            .hints()
            .iter()
            .find(|h| h.label == "m")
            .unwrap()
            .candidate
            .element_id,
        11
    );
    assert_eq!(
        interaction
            .hints()
            .iter()
            .find(|h| h.label == "3")
            .unwrap()
            .candidate
            .element_id,
        12
    );
    let hole = interaction
        .reservations()
        .iter()
        .find(|r| r.declaration().code == "2")
        .unwrap();
    assert_eq!(
        hole.state(),
        ferric_browser_core::HintReservationState::Unavailable(Reason::AnchorShadowed)
    );
}

#[test]
fn changed_membership_beyond_last_code_or_recycled_item_invalidates_snapshot() {
    let (request, raw, candidates) = fixture("dom");
    let original = decode(&raw, &request, &candidates).unwrap();
    assert!(original.same_collection_binding(&original, "cards"));
    for change in [
        "remove-fourth",
        "reorder",
        "identity",
        "meaning",
        "layout",
        "document",
    ] {
        let mut next = raw.clone();
        let mut current = candidates.clone();
        let items = next["outcomes"][0]["result"]["items"]
            .as_array_mut()
            .unwrap();
        match change {
            "remove-fourth" => {
                items.pop();
                next["roots"].as_array_mut().unwrap().pop();
            }
            "reorder" => items.swap(0, 1),
            "identity" => items[3]["identity_attributes"][0]["fact"]["value"] = json!("recycled"),
            "meaning" => {
                items[3]["primary"]["href"]["value"] = json!("https://example.test/new");
                current[3].href = Some("https://example.test/new".into());
            }
            "layout" => {
                items[3]["primary"]["projected_primary_fragment_css"]["y"] = json!(41.);
                current[3].geometry.y = 41.;
            }
            "document" => {
                next["documents"][0]["epoch"] = json!("22222222-1111-4111-8111-111111111111")
            }
            _ => unreachable!(),
        }
        let checked = decode(&next, &request, &current).unwrap();
        assert!(
            !original.same_collection_binding(&checked, "cards"),
            "{change}"
        );
        let interaction = model(&original, candidates.clone());
        assert!(
            original
                .surviving_special_slots(&checked, interaction.reservations())
                .is_empty(),
            "{change}"
        );
    }
}

#[test]
fn browser_consumption_does_not_compress_remaining_collection_ranks() {
    let (request, raw, candidates) = fixture("dom");
    let checked = decode(&raw, &request, &candidates).unwrap();
    let mut interaction = model(&checked, candidates.clone());
    interaction.consume(&candidates[0]);
    let live = checked.surviving_special_slots(&checked, interaction.reservations());
    let declarations: Vec<_> = interaction
        .reservations()
        .iter()
        .map(|r| r.declaration().clone())
        .collect();
    let assignments = checked.special_assignments(&candidates, &declarations, Some(&live));
    assert!(!matches!(assignments[0].binding, Binding::Live(_)));
    assert_eq!(assignments[1].binding, Binding::Live(candidates[1].clone()));
    assert_eq!(assignments[2].binding, Binding::Live(candidates[2].clone()));
}

#[test]
fn overlapping_collections_reserve_affected_slots_and_keep_ordinary_actions() {
    let (mut request, mut raw, candidates) = fixture("dom");
    let mut definitions = serde_json::to_value(request.policy.collections()).unwrap();
    let mut second = definitions[0].clone();
    second["id"] = json!("other");
    second["codes"] = json!(["x", "y", "v"]);
    definitions.as_array_mut().unwrap().push(second);
    request.policy = super::request(json!({"enabled":true,"collections":definitions})).policy;
    let mut second = raw["outcomes"][0].clone();
    second["rule"]["id"] = json!("other");
    raw["outcomes"].as_array_mut().unwrap().push(second);
    let checked = decode(&raw, &request, &candidates).unwrap();
    let interaction = model(&checked, candidates);
    assert_eq!(interaction.hints().len(), 4);
    assert!(
        interaction
            .reservations()
            .iter()
            .all(|reservation| reservation.state()
                == ferric_browser_core::HintReservationState::Unavailable(Reason::Ambiguous))
    );
}

#[test]
fn shuffled_dom_order_and_equal_positions_have_explicit_oracles() {
    for (order, expected) in [
        ("dom", [13, 12, 11]),
        ("top-to-bottom", [10, 11, 12]),
        ("row-major-ltr", [10, 11, 12]),
        ("row-major-rtl", [10, 11, 12]),
    ] {
        let (request, mut raw, mut candidates) = fixture(order);
        let items = raw["outcomes"][0]["result"]["items"]
            .as_array_mut()
            .unwrap();
        if order.starts_with("row-major") {
            for (item, candidate) in items.iter_mut().zip(&mut candidates) {
                item["primary"]["projected_primary_fragment_css"]["x"] = json!(0.);
                item["primary"]["projected_primary_fragment_css"]["y"] = json!(0.);
                candidate.geometry.x = 0.;
                candidate.geometry.y = 0.;
            }
        }
        items.reverse();
        let checked = decode(&raw, &request, &candidates).unwrap();
        let interaction = model(&checked, candidates);
        for (code, id) in ["1", "2", "3"].into_iter().zip(expected) {
            assert_eq!(
                interaction
                    .hints()
                    .iter()
                    .find(|hint| hint.label == code)
                    .unwrap()
                    .candidate
                    .element_id,
                id,
                "{order}/{code}"
            );
        }
    }
}
