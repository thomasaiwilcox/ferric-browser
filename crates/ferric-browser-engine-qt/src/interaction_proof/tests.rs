use super::*;
use ferric_browser_config::{
    InteractionPolicyBuilder, InteractionPolicyRevision, PolicySourceIdentity, SiteRule,
};
use ferric_browser_core::{DocumentId, HintGeometry, HintKind, ProfileId, TabId};
use serde_json::{Value, json};
use std::collections::BTreeMap;

mod collections;
mod discovery;
mod merge;
mod receivers;

#[test]
fn preview_diagnostics_preserve_failure_reason_without_dom_facts() {
    let request = request(anchor_patch());
    let mut raw = envelope(&request);
    raw["outcomes"][0]["result"] = json!({"unavailable":{"reason":"inspection-limit"}});
    raw["documents"] = json!([]);
    raw["roots"] = json!([]);
    raw["discovery_work"] = json!({"inspections":123,"predicate_steps":456});
    let checked = decode(&raw, &request, &[candidate()]).unwrap();
    assert_eq!(
        checked.preview_diagnostics(),
        json!({
            "unavailable_rules":[{"rule":{"kind":"anchor","id":"mac"},"reason":"inspection-limit"}],
            "discovery_work":{"inspections":123,"predicate_steps":456}
        })
    );

    let checked = decode(&envelope(&request), &request, &[candidate()]).unwrap();
    assert_eq!(
        checked.preview_diagnostics(),
        json!({"unavailable_rules":[],"discovery_work":null})
    );
}

#[test]
fn anchor_assignment_uses_only_admitted_canonical_candidates() {
    use ferric_browser_core::{HintReservationReason, HintSpecialBinding};
    let request = request(anchor_patch());
    let candidate = candidate();
    let checked = decode(
        &envelope(&request),
        &request,
        std::slice::from_ref(&candidate),
    )
    .unwrap();
    let compiled = checked
        .policy_snapshot()
        .compile_hint_codebook(InteractionFamily::Links)
        .unwrap()
        .unwrap();
    let declarations = compiled.codebook().declarations();
    assert_eq!(
        checked.special_assignments(std::slice::from_ref(&candidate), declarations, None)[0]
            .binding,
        HintSpecialBinding::Live(candidate.clone())
    );
    assert_eq!(
        checked.special_assignments(&[], declarations, None)[0].binding,
        HintSpecialBinding::Unavailable(HintReservationReason::Incomplete)
    );
    let mut substituted = candidate;
    substituted.href = Some("https://example.test/replacement".into());
    assert_eq!(
        checked.special_assignments(&[substituted], declarations, None)[0].binding,
        HintSpecialBinding::Unavailable(HintReservationReason::Incomplete)
    );
}

#[test]
fn anchor_refresh_keeps_layout_changes_but_rejects_document_or_meaning_changes() {
    let request = request(anchor_patch());
    let original = envelope(&request);
    let checked = decode(&original, &request, &[candidate()]).unwrap();
    let mut moved = original.clone();
    moved["outcomes"][0]["result"]["matches"][0]["projected_primary_fragment_css"]["x"] =
        json!(20.);
    let mut moved_candidate = candidate();
    moved_candidate.geometry.x = 20.;
    let current = decode(&moved, &request, &[moved_candidate]).unwrap();
    assert!(checked.same_anchor_binding(&current, "mac"));
    let mut replaced = original.clone();
    replaced["documents"][0]["epoch"] = json!("22222222-1111-4111-8111-111111111111");
    assert!(
        !checked.same_anchor_binding(&decode(&replaced, &request, &[candidate()]).unwrap(), "mac")
    );
    let mut changed = original;
    changed["outcomes"][0]["result"]["matches"][0]["href"]["value"] =
        json!("https://example.test/new");
    let mut changed_candidate = candidate();
    changed_candidate.href = Some("https://example.test/new".into());
    assert!(!checked.same_anchor_binding(
        &decode(&changed, &request, &[changed_candidate]).unwrap(),
        "mac"
    ));
}

#[test]
fn duplicate_anchor_claims_reserve_both_codes_and_keep_ordinary_fallback() {
    use ferric_browser_core::{
        HintInteraction, HintReservationReason, HintRetentionBudget, HintSpecialBinding,
    };
    let mut patch = anchor_patch();
    let mut second = patch["anchors"][0].clone();
    second["id"] = json!("second");
    second["code"] = json!("n");
    patch["anchors"].as_array_mut().unwrap().push(second);
    let request = request(patch);
    let mut raw = envelope(&request);
    let mut second = raw["outcomes"][0].clone();
    second["rule"]["id"] = json!("second");
    raw["outcomes"].as_array_mut().unwrap().push(second);
    let candidate = candidate();
    let checked = decode(&raw, &request, std::slice::from_ref(&candidate)).unwrap();
    let codebook = checked
        .policy_snapshot()
        .compile_hint_codebook(InteractionFamily::Links)
        .unwrap()
        .unwrap()
        .into_parts()
        .2;
    let assignments = checked.special_assignments(
        std::slice::from_ref(&candidate),
        codebook.declarations(),
        None,
    );
    assert!(assignments.iter().all(|assignment| assignment.binding
        == HintSpecialBinding::Unavailable(HintReservationReason::Ambiguous)));
    let interaction = HintInteraction::new_enhanced(
        vec![candidate],
        codebook,
        assignments,
        HintRetentionBudget::Bytes(MAX_INTERACTION_ANNOTATION_BYTES),
    )
    .unwrap();
    assert_eq!(interaction.hints().len(), 1);
    assert!(!["m", "n"].contains(&interaction.hints()[0].label.as_str()));
}

fn request(patch: Value) -> AnnotationRequest {
    request_family(patch, InteractionFamily::Links)
}
fn request_family(patch: Value, family: InteractionFamily) -> AnnotationRequest {
    request_patches(vec![patch], family)
}
fn request_patches(patches: Vec<Value>, family: InteractionFamily) -> AnnotationRequest {
    let mut builder = InteractionPolicyBuilder::default();
    for (index, patch) in patches.into_iter().enumerate() {
        builder
            .apply_selected_rule(
                &SiteRule {
                    id: format!("fixture-{index}"),
                    pattern: "https://example.test/*".into(),
                    priority: 0,
                    set: BTreeMap::new(),
                    interaction: Some(serde_json::from_value(patch).unwrap()),
                },
                PolicySourceIdentity::Authored {
                    document_id: u32::try_from(index).unwrap() + 1,
                },
                1,
                0,
            )
            .unwrap();
    }
    let policy = builder
        .finish(InteractionPolicyRevision {
            profile: ProfileId::from_display("profileid-1").unwrap(),
            profile_revision: u64::MAX,
            configuration_revision: 9_007_199_254_740_993,
            policy_revision: 3,
        })
        .unwrap();
    AnnotationRequest::capture(
        policy,
        WindowId::from_display("windowid-1").unwrap(),
        HintTarget {
            tab: TabId::from_display("tabid-2").unwrap(),
            document: DocumentId::from_display("documentid-3").unwrap(),
            generation: u64::MAX,
        },
        family,
        AnnotationGenerations {
            request_id: u64::MAX,
            ..Default::default()
        },
        Some("scope-fixture"),
        AnnotationDocumentRoles::default(),
    )
    .unwrap()
}

fn focused_chain(depth: u32, steps: Value) -> (AnnotationRequest, Value, HintCandidate) {
    let mut patch = anchor_patch();
    patch["anchors"][0]["root"] =
        json!({"start":"focused-frame","receiver_origin":"https://example.test","steps":steps});
    let mut request = request(patch);
    request.document_roles = AnnotationDocumentRoles::capture(
        [],
        Some(AnnotationFocusedReceiver {
            owner: String::new(),
            origin: "https://example.test".into(),
            depth: u8::try_from(depth.max(1)).unwrap(),
            composed_depth: u8::try_from(depth.max(1)).unwrap(),
        }),
    )
    .unwrap();
    let mut wire = envelope(&request);
    let documents: Vec<_> = (0..=depth)
        .map(|level| {
            let mut document = document();
            document["id"] = json!(level + 1);
            document["frame_path"] = json!(format!("0{}", ".0".repeat(level as usize)));
            document["epoch"] = json!(format!("{:08x}-1111-4111-8111-111111111111", level + 1));
            if level > 0 {
                document["parent_host"] = node(level, 100 + level)
            }
            document
        })
        .collect();
    wire["documents"] = json!(documents);
    wire["roots"] = json!([root(1, node(depth + 1, 1), "document")]);
    wire["outcomes"][0]["result"]["scope"] = node(depth + 1, 1);
    let frame_path = format!("0{}", ".0".repeat(depth as usize));
    wire["outcomes"][0]["result"]["matches"][0]["candidate"]["frame_path"] = json!(frame_path);
    let mut candidate = candidate();
    candidate.frame_path = frame_path;
    (request, wire, candidate)
}

#[test]
fn regression_parent_document_node_is_not_an_element_host() {
    let (request, mut wire, candidate) = focused_chain(1, json!([]));
    assert!(decode(&wire, &request, std::slice::from_ref(&candidate)).is_ok());
    wire["documents"][1]["parent_host"] = node(1, 1);
    assert_eq!(
        decode(&wire, &request, &[candidate]).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}

fn two_shadow_roots() -> (AnnotationRequest, Value) {
    let descriptor =
        json!({"start":"top-document","steps":[{"kind":"open-shadow","host_selector":"#host"}]});
    let request = request(json!({"enabled":true,"anchors":[
        {"id":"one","code":"x","families":["links"],"target_selector":"a","root":descriptor},
        {"id":"two","code":"y","families":["links"],"target_selector":"a","root":descriptor}]}));
    let mut wire = envelope(&request);
    let mut roots = Vec::new();
    let mut outcomes = Vec::new();
    for (id, name) in [(1, "one"), (2, "two")] {
        let mut value = root(id, node(1, 1), "document");
        value["hops"] = json!([{"kind":"open-shadow","host":node(1,2),"root":node(1,3)}]);
        value["resolved"] = node(1, 3);
        value["resolved_kind"] = json!("open-shadow");
        roots.push(value);
        outcomes.push(json!({"rule":{"kind":"anchor","id":name},"result":{"state":"anchor-complete","root":id,"scope":node(1,3),"scope_kind":"open-shadow","matches":[]}}));
    }
    wire["roots"] = json!(roots);
    wire["outcomes"] = json!(outcomes);
    (request, wire)
}

#[test]
fn regression_one_shadow_root_cannot_name_two_hosts() {
    let (request, mut wire) = two_shadow_roots();
    assert!(decode(&wire, &request, &[]).is_ok());
    wire["roots"][1]["hops"][0]["host"] = node(1, 4);
    assert_eq!(
        decode(&wire, &request, &[]).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}
#[test]
fn regression_one_host_cannot_name_two_shadow_roots() {
    let (request, mut wire) = two_shadow_roots();
    assert!(decode(&wire, &request, &[]).is_ok());
    wire["roots"][1]["hops"][0]["root"] = node(1, 4);
    wire["roots"][1]["resolved"] = node(1, 4);
    wire["outcomes"][1]["result"]["scope"] = node(1, 4);
    assert_eq!(
        decode(&wire, &request, &[]).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}

#[test]
fn regression_one_direct_assignment_cannot_name_two_slots() {
    let (_, mut wire) = two_shadow_roots();
    // Capture a real different typed policy rather than changing its fields.
    let descriptor = json!({"start":"top-document","steps":[{"kind":"open-shadow","host_selector":"shadow-shell"},{"kind":"assigned-slot","host_selector":"slot","assigned_selector":".one"}]});
    let mut second_descriptor = descriptor.clone();
    second_descriptor["steps"][1]["assigned_selector"] = json!(".two");
    let request = request_family(
        json!({"enabled":true,"anchors":[
        {"id":"one","code":"x","families":["links"],"target_selector":"a","root":descriptor},
        {"id":"two","code":"y","families":["links"],"target_selector":"a","root":second_descriptor}]}),
        InteractionFamily::Links,
    );
    wire["stamp"] = serde_json::from_str(&request.stamp_json().unwrap()).unwrap();
    for index in 0..2 {
        wire["roots"][index]["hops"] = json!([{"kind":"open-shadow","host":node(1,2),"root":node(1,3)},{"kind":"assigned-slot","slot":node(1,4),"assigned":node(1,10),"owner_document":1}]);
        wire["roots"][index]["resolved"] = node(1, 10);
        wire["roots"][index]["resolved_kind"] = json!("element");
        wire["outcomes"][index]["result"]["scope"] = node(1, 10);
        wire["outcomes"][index]["result"]["scope_kind"] = json!("element");
    }
    assert!(decode(&wire, &request, &[]).is_ok());
    // The mapping is one-way: two direct elements may share the same slot.
    let mut multiple = wire.clone();
    multiple["roots"][1]["hops"][1]["assigned"] = node(1, 11);
    multiple["roots"][1]["resolved"] = node(1, 11);
    multiple["outcomes"][1]["result"]["scope"] = node(1, 11);
    assert!(decode(&multiple, &request, &[]).is_ok());
    wire["roots"][1]["hops"][1]["slot"] = node(1, 5);
    assert_eq!(
        decode(&wire, &request, &[]).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}

#[test]
fn regression_focused_ancestry_does_not_reset_anchor_depth() {
    for (depth, valid) in [(7, true), (8, false)] {
        let (request, mut wire, candidate) = focused_chain(
            depth,
            json!([{"kind":"open-shadow","host_selector":"shadow-shell"}]),
        );
        wire["roots"][0]["hops"] =
            json!([{"kind":"open-shadow","host":node(depth+1,2),"root":node(depth+1,3)}]);
        wire["roots"][0]["resolved"] = node(depth + 1, 3);
        wire["roots"][0]["resolved_kind"] = json!("open-shadow");
        wire["outcomes"][0]["result"]["scope"] = node(depth + 1, 3);
        wire["outcomes"][0]["result"]["scope_kind"] = json!("open-shadow");
        let result = decode(&wire, &request, &[candidate]);
        if valid {
            result.unwrap();
        } else {
            assert_eq!(result.unwrap_err(), AnnotationError::InvalidRoot);
        }
    }
}

#[test]
fn parent_shadow_depth_is_shared_with_focused_top_and_foreign_local_roots() {
    let owner = "frame-22222222-2222-4222-8222-222222222222";
    let mut observed = Vec::new();
    let mut expected = Vec::new();
    for foreign in [false, true] {
        for (composed, local, valid) in [(2, 6, true), (2, 7, false), (1, 7, true)] {
            let steps = json!(
                (0..local)
                    .map(|_| json!({"kind":"open-shadow","host_selector":"shadow-shell"}))
                    .collect::<Vec<_>>()
            );
            let (mut request, mut wire, mut row) = focused_chain(1, steps);
            request.document_roles = AnnotationDocumentRoles::capture(
                [],
                Some(AnnotationFocusedReceiver {
                    owner: if foreign { owner.into() } else { String::new() },
                    origin: "https://example.test".into(),
                    depth: 1,
                    composed_depth: composed,
                }),
            )
            .unwrap();
            if foreign {
                let mut selected = wire["documents"][1].clone();
                selected["frame_owner"] = json!(owner);
                selected["frame_path"] = json!("0");
                selected["parent_host"] = Value::Null;
                wire["documents"] = json!([selected]);
                row.frame_owner = owner.into();
                row.frame_path = "0".into();
                wire["outcomes"][0]["result"]["matches"][0]["candidate"]["frame_owner"] =
                    json!(owner);
                wire["outcomes"][0]["result"]["matches"][0]["candidate"]["frame_path"] = json!("0");
            }
            wire["roots"][0]["hops"] = json!((0..local).map(|level| json!({"kind":"open-shadow","host":node(2,100+level*2),"root":node(2,101+level*2)})).collect::<Vec<_>>());
            let resolved = node(2, 99 + local * 2);
            wire["roots"][0]["resolved"] = resolved.clone();
            wire["roots"][0]["resolved_kind"] = json!("open-shadow");
            wire["outcomes"][0]["result"]["scope"] = resolved;
            wire["outcomes"][0]["result"]["scope_kind"] = json!("open-shadow");
            let result = decode(&wire, &request, &[row]);
            observed.push(result.err());
            expected.push((!valid).then_some(AnnotationError::InvalidRoot));
        }
    }
    assert_eq!(observed, expected);
}

#[test]
fn regression_raw_policy_name_is_not_the_legacy_display_label() {
    let mut patch = anchor_patch();
    patch["anchors"][0]["predicates"]["hint_name"]["equals"] = json!("Ma\u{feff}c");
    let request = request(patch);
    let mut wire = envelope(&request);
    let mut candidate = candidate();
    candidate.text = "Ma c".into();
    wire["outcomes"][0]["result"]["matches"][0]["name"]["value"] = json!("Ma\u{feff}c");
    let checked = decode(&wire, &request, &[candidate]).unwrap();
    assert_eq!(checked.candidate_snapshot(0).unwrap().text, "Ma c");
}
#[test]
fn regression_empty_raw_name_can_have_a_fallback_display_label() {
    let request = request_family(
        json!({"enabled":true,"anchors":[{"id":"mac","code":"m","families":["all"],"target_selector":"button","predicates":{"hint_name":{"equals":""}}}]}),
        InteractionFamily::All,
    );
    let mut wire = envelope(&request);
    let mut candidate = candidate();
    candidate.kind = HintKind::Button;
    candidate.text = "Button".into();
    candidate.href = None;
    wire["outcomes"][0]["result"]["matches"][0]["name"]["value"] = json!("");
    wire["outcomes"][0]["result"]["matches"][0]["href"] = json!({"state":"complete-absent"});
    let checked = decode(&wire, &request, &[candidate]).unwrap();
    assert_eq!(checked.candidate_snapshot(0).unwrap().text, "Button");
}

#[test]
fn regression_focused_ancestry_is_shared_with_collection_primary_steps() {
    for (depth, valid) in [(6, true), (7, false)] {
        let (focused, mut wire, candidate) = focused_chain(depth, json!([]));
        let root_descriptor = json!({"start":"focused-frame","receiver_origin":"https://example.test","steps":[{"kind":"open-shadow","host_selector":"shadow-shell"}]});
        let mut request = request(
            json!({"enabled":true,"collections":[{"id":"list","families":["links"],"codes":["1"],"root":root_descriptor,"container_selector":"#list","item_selector":"article","primary_root_steps":[{"kind":"assigned-slot","host_selector":"slot","assigned_selector":"a"}],"primary_self":true,"order":"dom"}]}),
        );
        request.document_roles = focused.document_roles;
        wire["stamp"] = serde_json::from_str(&request.stamp_json().unwrap()).unwrap();
        wire["roots"][0]["hops"] =
            json!([{"kind":"open-shadow","host":node(depth+1,2),"root":node(depth+1,3)}]);
        wire["roots"][0]["resolved"] = node(depth + 1, 3);
        wire["roots"][0]["resolved_kind"] = json!("open-shadow");
        let mut primary = root(2, node(depth + 1, 30), "element");
        primary["hops"] = json!([{"kind":"assigned-slot","slot":node(depth+1,31),"assigned":node(depth+1,10),"owner_document":depth+1}]);
        primary["resolved"] = node(depth + 1, 10);
        wire["roots"].as_array_mut().unwrap().push(primary);
        let mut witness = witness();
        witness["candidate"]["frame_path"] = json!(candidate.frame_path);
        wire["outcomes"] = json!([{"rule":{"kind":"collection","id":"list"},"result":{"state":"collection-complete","root":1,"container":node(depth+1,20),"container_kind":"element","items":[{"item":node(depth+1,30),"primary_root":2,"primary":witness,"attributes":[],"identity_attributes":[]}]}}]);
        let result = decode(&wire, &request, &[candidate]);
        if valid {
            result.unwrap();
        } else {
            assert_eq!(result.unwrap_err(), AnnotationError::InvalidRoot);
        }
    }
}

#[test]
fn regression_one_frame_host_cannot_name_two_current_documents() {
    let (request0, mut wire, candidate) = focused_chain(1, json!([]));
    let mut patch = anchor_patch();
    patch["anchors"][0]["root"] =
        json!({"start":"focused-frame","receiver_origin":"https://example.test","steps":[]});
    patch["anchors"].as_array_mut().unwrap().push(json!({"id":"second","code":"z","families":["links"],"target_selector":"a","root":{"start":"top-document","steps":[{"kind":"same-origin-frame","host_selector":"iframe"}]}}));
    let mut request = request(patch);
    request.document_roles = request0.document_roles.clone();
    assert_eq!(request0.stamp.family, request.stamp.family);
    wire["stamp"] = serde_json::from_str(&request.stamp_json().unwrap()).unwrap();
    let mut second = wire["documents"][1].clone();
    second["id"] = json!(3);
    second["frame_path"] = json!("0.1");
    second["epoch"] = json!("33333333-3333-4333-8333-333333333333");
    second["parent_host"] = node(1, 102);
    wire["documents"].as_array_mut().unwrap().push(second);
    let mut second_root = root(2, node(1, 1), "document");
    second_root["hops"] =
        json!([{"kind":"same-origin-frame","host":node(1,102),"document":node(3,1)}]);
    second_root["resolved"] = node(3, 1);
    wire["roots"].as_array_mut().unwrap().push(second_root);
    wire["outcomes"].as_array_mut().unwrap().push(json!({"rule":{"kind":"anchor","id":"second"},"result":{"state":"anchor-complete","root":2,"scope":node(3,1),"scope_kind":"document","matches":[]}}));
    assert!(decode(&wire, &request, std::slice::from_ref(&candidate)).is_ok());
    wire["documents"][2]["parent_host"] = node(1, 101);
    wire["roots"][1]["hops"][0]["host"] = node(1, 101);
    assert_eq!(
        decode(&wire, &request, &[candidate]).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}

#[test]
fn regression_parent_host_cannot_be_a_shadow_root_object() {
    let (request0, mut wire, candidate) = focused_chain(1, json!([]));
    let mut patch = anchor_patch();
    patch["anchors"][0]["root"] =
        json!({"start":"focused-frame","receiver_origin":"https://example.test","steps":[]});
    patch["anchors"].as_array_mut().unwrap().push(json!({"id":"shadow","code":"z","families":["links"],"target_selector":"a","root":{"start":"top-document","steps":[{"kind":"open-shadow","host_selector":"shadow-shell"}]}}));
    let mut request = request(patch);
    request.document_roles = request0.document_roles;
    wire["stamp"] = serde_json::from_str(&request.stamp_json().unwrap()).unwrap();
    let mut shadow = root(2, node(1, 1), "document");
    shadow["hops"] = json!([{"kind":"open-shadow","host":node(1,50),"root":node(1,99)}]);
    shadow["resolved"] = node(1, 99);
    shadow["resolved_kind"] = json!("open-shadow");
    wire["roots"].as_array_mut().unwrap().push(shadow);
    wire["outcomes"].as_array_mut().unwrap().push(json!({"rule":{"kind":"anchor","id":"shadow"},"result":{"state":"anchor-complete","root":2,"scope":node(1,99),"scope_kind":"open-shadow","matches":[]}}));
    assert!(decode(&wire, &request, std::slice::from_ref(&candidate)).is_ok());
    wire["documents"][1]["parent_host"] = node(1, 99);
    assert_eq!(
        decode(&wire, &request, &[candidate]).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}
fn anchor_patch() -> Value {
    json!({"enabled":true,"anchors":[{"id":"mac","code":"m","families":["links"],"target_selector":"a", "predicates":{"hint_name":{"equals":"Mac"}}}]})
}

#[test]
fn collector_plan_fixtures_match_checked_selectors_and_exact_captured_stamp() {
    let fixtures: Vec<Value> = serde_json::from_str(include_str!(
        "../../tests/fixtures/site-policy/root-query-plans.json"
    ))
    .unwrap();
    for fixture in fixtures {
        let captured = request(fixture["patch"].clone());
        let plan: Value = serde_json::from_str(&captured.collector_plan_json().unwrap()).unwrap();
        assert_eq!(plan["version"], INTERACTION_ANNOTATION_VERSION);
        assert_eq!(plan["observations"], true);
        assert_eq!(plan["rules"], fixture["rules"], "{}", fixture["tag"]);
        assert_eq!(
            plan["stamp"],
            serde_json::from_str::<Value>(&captured.stamp_json().unwrap()).unwrap()
        );
        for rule in plan["rules"].as_array().unwrap() {
            for forbidden in ["code", "codes", "remaps", "labels", "families"] {
                assert!(!rule.as_object().unwrap().contains_key(forbidden));
            }
        }
    }
}

#[test]
fn collector_plan_uses_literal_family_and_retains_the_original_policy_capture() {
    let mut patch = anchor_patch();
    let captured = request(patch.clone());
    let original = captured.collector_plan_json().unwrap();
    patch["anchors"][0]["target_selector"] = json!("button.changed");
    assert_eq!(captured.collector_plan_json().unwrap(), original);
    let all = request_family(patch.clone(), InteractionFamily::All);
    let plan: Value = serde_json::from_str(&all.collector_plan_json().unwrap()).unwrap();
    assert_eq!(plan["rules"], json!([]));
    patch["enabled"] = json!(false);
    let disabled = request(patch);
    let plan: Value = serde_json::from_str(&disabled.collector_plan_json().unwrap()).unwrap();
    assert_eq!(plan["rules"], json!([]));
}

#[test]
fn collector_plan_near_selector_limit_matches_the_native_broker_fixture() {
    let selector = vec!["a"; 512].join(" ");
    assert_eq!(selector.len(), 1_023);
    assert_eq!(32 * selector.len(), 32_736);
    let compact_steps: Vec<_> = (0..512)
        .map(|index| json!([if index == 0 { None } else { Some("descendant") }, "a", 1]))
        .collect();
    for with_predicates in [false, true] {
        let value = "\\\"".repeat(128);
        let predicates = json!({"hint_name":{"equals":value},"attributes":
            (0..4).map(|index| json!({"name":format!("data-{index}"),"equals":value})).collect::<Vec<_>>()});
        let mut anchors = Vec::new();
        let mut collections = Vec::new();
        let mut expected = Vec::new();
        for index in 0..32 {
            let id = format!("max{index}");
            let is_anchor = index < 16;
            let mut source = json!({"id":id,"families":["links"]});
            let mut projected = json!({"rule":{"kind":if is_anchor { "anchor" } else { "collection" },"id":id},
                "root":{"start":"top-document","steps":[]}});
            if is_anchor {
                source["code"] = json!(format!("{index:08}"));
                source["target_selector"] = json!(selector);
                projected["target_selector"] = json!({"steps":compact_steps});
            } else {
                source["codes"] = json!([format!("{index:08}")]);
                source["container_self"] = json!(true);
                source["item_selector"] = json!(selector);
                source["primary_self"] = json!(true);
                source["order"] = json!("dom");
                projected["slot_count"] = json!(1);
                projected["container_self"] = json!(true);
                projected["item_selector"] = json!({"steps":compact_steps});
                projected["primary_root_steps"] = json!([]);
                projected["primary_self"] = json!(true);
                projected["order"] = json!("dom");
            }
            if with_predicates {
                let field = if is_anchor {
                    "predicates"
                } else {
                    "primary_predicates"
                };
                source[field] = predicates.clone();
                projected[field] = predicates.clone();
            }
            if is_anchor {
                anchors.push(source);
            } else {
                collections.push(source);
            }
            expected.push(projected);
        }
        // Lists replace whole lists: use distinct anchor/collection fields,
        // not two anchor contributions that would erase the first sixteen.
        let captured = request_patches(
            vec![
                json!({"enabled":true,"anchors":anchors}),
                json!({"collections":collections}),
            ],
            InteractionFamily::Links,
        );
        let raw = captured.collector_plan_json().unwrap();
        let plan: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(plan["rules"], json!(expected));
        assert!(raw.encode_utf16().count() < 1024 * 1024);
        // Qt additionally measures and executes the entire generated collector
        // script, including its runtime, through the shipping native broker.
    }
}

fn search_outcome(wire: &mut Value, kind: &str, id: &str, stage: Value, cardinality: &str) {
    wire["outcomes"] = json!([{"rule":{"kind":kind,"id":id},"result":{
        "state":"search-complete","root":1,"stage":stage,"cardinality":cardinality
    }}]);
}

#[test]
fn search_complete_absence_and_ambiguity_need_no_invented_scope_or_container() {
    for (patch, kind, id, stage) in [
        (
            json!({"anchors":[{"id":"mac","code":"m","families":["links"],"scope_selector":"nav","target_selector":"a"}]}),
            "anchor",
            "mac",
            json!({"anchor-scope":{}}),
        ),
        (
            json!({"collections":[{"id":"list","codes":["1"],"families":["links"],"container_selector":"main","item_selector":"article","primary_selector":"a","order":"dom"}]}),
            "collection",
            "list",
            json!({"collection-container":{}}),
        ),
    ] {
        let request = request(patch);
        for cardinality in ["absent", "ambiguous"] {
            let mut wire = envelope(&request);
            search_outcome(&mut wire, kind, id, stage.clone(), cardinality);
            // A negative result must not retain even a valid ordinary target
            // supplied alongside it as potential fallback.
            let checked = decode(&wire, &request, &[candidate()]).unwrap();
            assert_eq!(checked.rule_count(), 1);
            assert_eq!(checked.root_count(), 1);
            assert_eq!(checked.candidate_snapshot(0), None);
            assert_eq!(serde_json::to_value(checked).unwrap(), encoded(&wire));
        }
    }
}

#[test]
fn search_complete_stage_must_name_an_authored_selector_of_the_exact_rule_kind() {
    for stage in [
        json!({"anchor-scope":{}}),
        json!({"collection-container":{}}),
    ] {
        let request = request(anchor_patch());
        let mut wire = envelope(&request);
        search_outcome(&mut wire, "anchor", "mac", stage, "absent");
        assert_eq!(
            decode(&wire, &request, &[]).unwrap_err(),
            AnnotationError::InvalidInventory
        );
    }
    let request = request(
        json!({"collections":[{"id":"list","codes":["1"],"families":["links"],"container_self":true,"item_selector":"article","primary_self":true,"order":"dom"}]}),
    );
    let mut wire = envelope(&request);
    search_outcome(
        &mut wire,
        "collection",
        "list",
        json!({"collection-container":{}}),
        "ambiguous",
    );
    assert_eq!(
        decode(&wire, &request, &[]).unwrap_err(),
        AnnotationError::InvalidInventory
    );
}

fn search_root_prefix() -> (AnnotationRequest, Value) {
    let mut patch = anchor_patch();
    patch["anchors"][0]["root"] = json!({"start":"top-document","steps":[
        {"kind":"open-shadow","host_selector":"shell-card"},
        {"kind":"assigned-slot","host_selector":"slot","assigned_selector":".card"}
    ]});
    let request = request(patch);
    let mut wire = envelope(&request);
    wire["roots"][0]["hops"] = json!([{"kind":"open-shadow","host":node(1,2),"root":node(1,3)}]);
    wire["roots"][0]["resolved"] = node(1, 3);
    wire["roots"][0]["resolved_kind"] = json!("open-shadow");
    (request, wire)
}

#[test]
fn search_complete_root_host_refers_only_to_the_successful_authored_prefix() {
    for cardinality in ["absent", "ambiguous"] {
        let (request, mut wire) = search_root_prefix();
        search_outcome(
            &mut wire,
            "anchor",
            "mac",
            json!({"root-host":{"step":1}}),
            cardinality,
        );
        assert!(decode(&wire, &request, &[]).is_ok());
        wire["roots"][0] = root(1, node(1, 1), "document");
        wire["outcomes"][0]["result"]["stage"] = json!({"root-host":{"step":0}});
        assert!(decode(&wire, &request, &[]).is_ok());
    }
    for step in [0, 2, 8, 255] {
        let (request, mut wire) = search_root_prefix();
        search_outcome(
            &mut wire,
            "anchor",
            "mac",
            json!({"root-host":{"step":step}}),
            "absent",
        );
        assert_eq!(
            decode(&wire, &request, &[]).unwrap_err(),
            AnnotationError::InvalidRoot
        );
    }
}

#[test]
fn search_complete_self_host_cannot_be_missing_or_ambiguous() {
    let mut patch = anchor_patch();
    patch["anchors"][0]["root"] = json!({"start":"top-document","steps":[
        {"kind":"assigned-slot","host_selector":"slot","assigned_selector":".host"},
        {"kind":"open-shadow","self":true}
    ]});
    let request = request(patch);
    let mut wire = envelope(&request);
    wire["roots"][0]["hops"] =
        json!([{"kind":"assigned-slot","slot":node(1,2),"assigned":node(1,10),"owner_document":1}]);
    wire["roots"][0]["resolved"] = node(1, 10);
    wire["roots"][0]["resolved_kind"] = json!("element");
    search_outcome(
        &mut wire,
        "anchor",
        "mac",
        json!({"root-host":{"step":1}}),
        "absent",
    );
    assert_eq!(
        decode(&wire, &request, &[]).unwrap_err(),
        AnnotationError::InvalidInventory
    );
}

#[test]
fn search_complete_attempted_hop_shares_focused_ancestry_depth() {
    for (depth, valid) in [(7, true), (8, false)] {
        let (request, mut wire, _) = focused_chain(
            depth,
            json!([{"kind":"open-shadow","host_selector":"shell-card"}]),
        );
        search_outcome(
            &mut wire,
            "anchor",
            "mac",
            json!({"root-host":{"step":0}}),
            "absent",
        );
        let result = decode(&wire, &request, &[]);
        if valid {
            result.unwrap();
        } else {
            assert_eq!(result.unwrap_err(), AnnotationError::InvalidRoot);
        }
    }
}

#[test]
fn search_complete_assignment_self_requires_the_exact_resolved_element() {
    let mut patch = anchor_patch();
    patch["anchors"][0]["root"] = json!({"start":"top-document","steps":[
        {"kind":"assigned-slot","host_selector":"slot","assigned_selector":"slot.nested"},
        {"kind":"assigned-slot","self":true,"assigned_selector":"a"}
    ]});
    let request = request(patch);
    let mut wire = envelope(&request);
    wire["roots"][0]["hops"] =
        json!([{"kind":"assigned-slot","slot":node(1,2),"assigned":node(1,10),"owner_document":1}]);
    wire["roots"][0]["resolved"] = node(1, 10);
    wire["roots"][0]["resolved_kind"] = json!("element");
    search_outcome(
        &mut wire,
        "anchor",
        "mac",
        json!({"assigned-element":{"step":1,"slot":node(1,10)}}),
        "ambiguous",
    );
    assert!(decode(&wire, &request, &[]).is_ok());
    wire["outcomes"][0]["result"]["stage"] =
        json!({"assigned-element":{"step":1,"slot":node(1,11)}});
    assert_eq!(
        decode(&wire, &request, &[]).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}

#[test]
fn search_complete_retains_start_path_and_exact_used_reference_checks() {
    let (request, mut wire) = search_root_prefix();
    search_outcome(
        &mut wire,
        "anchor",
        "mac",
        json!({"root-host":{"step":1}}),
        "absent",
    );
    assert!(decode(&wire, &request, &[]).is_ok());
    let mut bad_start = wire.clone();
    bad_start["roots"][0]["start"] = node(1, 20);
    bad_start["roots"][0]["start_kind"] = json!("element");
    assert_eq!(
        decode(&bad_start, &request, &[]).unwrap_err(),
        AnnotationError::InvalidRoot
    );
    let mut wrong_prefix = wire.clone();
    wrong_prefix["roots"][0]["hops"] =
        json!([{"kind":"assigned-slot","slot":node(1,2),"assigned":node(1,20),"owner_document":1}]);
    wrong_prefix["roots"][0]["resolved"] = node(1, 20);
    wrong_prefix["roots"][0]["resolved_kind"] = json!("element");
    assert_eq!(
        decode(&wrong_prefix, &request, &[]).unwrap_err(),
        AnnotationError::InvalidRoot
    );
    let mut orphan = wire.clone();
    orphan["roots"]
        .as_array_mut()
        .unwrap()
        .push(root(2, node(1, 1), "document"));
    assert_eq!(
        decode(&orphan, &request, &[]).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
    let result = &mut wire["outcomes"][0]["result"];
    result["matches"] = json!([]);
    assert_eq!(
        decode(&wire, &request, &[]).unwrap_err(),
        AnnotationError::InvalidSchema
    );
}

#[test]
fn search_complete_direct_assignment_retains_the_actual_slot_not_an_assigned_target() {
    for cardinality in ["absent", "ambiguous"] {
        let (request, mut wire) = search_root_prefix();
        search_outcome(
            &mut wire,
            "anchor",
            "mac",
            json!({"assigned-element":{"step":1,"slot":node(1,4)}}),
            cardinality,
        );
        assert!(decode(&wire, &request, &[]).is_ok());
        assert_eq!(
            decode(&wire, &request, &[]).unwrap().candidate_snapshot(0),
            None
        );
        for invalid in [node(1, 1), node(1, 3), node(99, 4)] {
            wire["outcomes"][0]["result"]["stage"] =
                json!({"assigned-element":{"step":1,"slot":invalid}});
            assert!(decode(&wire, &request, &[]).is_err());
        }
        wire["roots"][0] = root(1, node(1, 1), "document");
        wire["outcomes"][0]["result"]["stage"] =
            json!({"assigned-element":{"step":0,"slot":node(1,4)}});
        assert_eq!(
            decode(&wire, &request, &[]).unwrap_err(),
            AnnotationError::InvalidInventory
        );
    }
}

#[test]
fn search_complete_schema_never_defaults_missing_or_live_cardinality() {
    let mut patch = anchor_patch();
    patch["anchors"][0]["scope_selector"] = json!("nav");
    let request = request(patch);
    let mut wire = envelope(&request);
    search_outcome(
        &mut wire,
        "anchor",
        "mac",
        json!({"anchor-scope":{}}),
        "absent",
    );
    assert!(decode(&wire, &request, &[]).is_ok());
    for pointer in [
        "/outcomes/0/result/stage",
        "/outcomes/0/result/cardinality",
        "/outcomes/0/result/root",
    ] {
        let mut malformed = wire.clone();
        *malformed.pointer_mut(pointer).unwrap() = Value::Null;
        assert_eq!(
            decode(&malformed, &request, &[]).unwrap_err(),
            AnnotationError::InvalidSchema
        );
    }
    for field in ["stage", "cardinality", "root"] {
        let mut omitted = wire.clone();
        omitted["outcomes"][0]["result"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            decode(&omitted, &request, &[]).unwrap_err(),
            AnnotationError::InvalidSchema
        );
    }
    for cardinality in [json!("live"), json!("one"), json!(0), Value::Null] {
        wire["outcomes"][0]["result"]["cardinality"] = cardinality;
        assert_eq!(
            decode(&wire, &request, &[]).unwrap_err(),
            AnnotationError::InvalidSchema
        );
    }
}
fn node(document: u32, node: u32) -> Value {
    json!({"document":document,"node":node})
}
fn candidate() -> HintCandidate {
    HintCandidate {
        element_id: 10,
        related_element_id: None,
        kind: HintKind::Link,
        frame_path: "0".into(),
        frame_owner: String::new(),
        text: "Mac".into(),
        href: Some("https://example.test/mac".into()),
        geometry: HintGeometry {
            x: 10.,
            y: 10.,
            width: 80.,
            height: 20.,
        },
        presentation: None,
    }
}
fn root(id: u32, start: Value, kind: &str) -> Value {
    json!({"id":id,"start":start,"start_kind":kind,"hops":[],"resolved":start,"resolved_kind":kind})
}
fn document() -> Value {
    json!({"id":1,"frame_owner":"","frame_path":"0","epoch":"11111111-1111-4111-8111-111111111111","origin":"https://example.test","document_node":1,"parent_host":null})
}
fn witness() -> Value {
    json!({"candidate":{"index":0,"element_id":10,"related_element_id":null,"frame_owner":"","frame_path":"0"},
        "name":{"state":"complete-present","value":" Mac "}, "href":{"state":"complete-present","value":"https://example.test/mac"},
        "role":{"state":"missing"},"attributes":[],"target_border_box_css":{"width":80.,"height":20.},
        "projected_primary_fragment_css":{"x":10.,"y":10.,"width":80.,"height":20.},"top_viewport_css":{"width":1000.,"height":800.}})
}
fn envelope(request: &AnnotationRequest) -> Value {
    json!({"version":1,"stamp":serde_json::from_str::<Value>(&request.stamp_json().unwrap()).unwrap(),
        "documents":[document()],"roots":[root(1,node(1,1),"document")],
        "outcomes":[{"rule":{"kind":"anchor","id":"mac"},"result":{"state":"anchor-complete","root":1,"scope":node(1,1),"scope_kind":"document","matches":[witness()]}}]})
}
fn decode(
    value: &Value,
    request: &AnnotationRequest,
    candidates: &[HintCandidate],
) -> Result<CheckedAnnotations, AnnotationError> {
    decode_annotations(&encoded(value).to_string(), request, candidates)
}

// Readable fixtures become externally tagged wire enums. Preserve extra fields
// so negative cases reach the strict decoder rather than a lossy test adapter.
fn encoded(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(encoded).collect()),
        Value::Object(values) => {
            let mut values: serde_json::Map<_, _> = values
                .iter()
                .map(|(key, value)| (key.clone(), encoded(value)))
                .collect();
            let tag = if values.contains_key("state") {
                Some("state")
            } else if values
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(|kind| {
                    matches!(kind, "open-shadow" | "same-origin-frame" | "assigned-slot")
                })
            {
                Some("kind")
            } else {
                None
            };
            if let Some(tag) = tag {
                let tag = values.remove(tag).unwrap().as_str().unwrap().to_owned();
                json!({tag: Value::Object(values)})
            } else {
                Value::Object(values)
            }
        }
        _ => value.clone(),
    }
}

#[test]
fn checked_annotations_round_trip_and_keep_u64_echoes_exact() {
    let request = request(anchor_patch());
    let wire = envelope(&request);
    assert_eq!(
        wire["stamp"]["profile_revision"],
        json!(u64::MAX.to_string())
    );
    assert_eq!(
        wire["stamp"]["configuration_revision"],
        json!("9007199254740993")
    );
    let checked = decode(&wire, &request, &[candidate()]).unwrap();
    assert_eq!(
        (
            checked.rule_count(),
            checked.root_count(),
            checked.document_count()
        ),
        (1, 1, 1)
    );
    let encoded = serde_json::to_string(&checked).unwrap();
    assert!(decode_annotations(&encoded, &request, &[candidate()]).is_ok());
    for field in [
        "profile_revision",
        "configuration_revision",
        "policy_revision",
        "target_generation",
        "selector_revision",
        "view_generation",
        "viewport_generation",
        "collection_epoch",
        "provider_epoch",
        "request_id",
    ] {
        for bad in [
            json!(0),
            json!("+1"),
            json!("-1"),
            json!("01"),
            json!(" 1"),
            json!("1 "),
            json!("18446744073709551616"),
        ] {
            let mut changed = wire.clone();
            changed["stamp"][field] = bad;
            assert_eq!(
                decode(&changed, &request, &[candidate()]).unwrap_err(),
                AnnotationError::InvalidSchema,
                "{field}"
            );
        }
        let mut stale = wire.clone();
        stale["stamp"][field] = json!("7");
        assert_eq!(
            decode(&stale, &request, &[candidate()]).unwrap_err(),
            AnnotationError::StaleRequest
        );
    }
}

#[test]
fn unknown_fields_duplicates_missing_quality_and_bad_versions_reject_atomically() {
    let request = request(anchor_patch());
    let wire = envelope(&request);
    for pointer in [
        "",
        "/stamp",
        "/documents/0",
        "/roots/0",
        "/roots/0/start",
        "/outcomes/0",
        "/outcomes/0/rule",
        "/outcomes/0/result",
        "/outcomes/0/result/matches/0",
        "/outcomes/0/result/matches/0/candidate",
        "/outcomes/0/result/matches/0/name",
        "/outcomes/0/result/matches/0/projected_primary_fragment_css",
    ] {
        for field in ["command", "transport", "label", "selector_override"] {
            let mut changed = wire.clone();
            changed.pointer_mut(pointer).unwrap()[field] = json!("forged");
            assert_eq!(
                decode(&changed, &request, &[candidate()]).unwrap_err(),
                AnnotationError::InvalidSchema,
                "{pointer}/{field}"
            );
        }
    }
    let raw =
        encoded(&wire)
            .to_string()
            .replacen("\"version\":1", "\"version\":1,\"version\":1", 1);
    assert_eq!(
        decode_annotations(&raw, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidSchema
    );
    for pointer in [
        "/documents/0/parent_host",
        "/outcomes/0/result/matches/0/candidate/related_element_id",
        "/outcomes/0/result/matches/0/name",
    ] {
        let mut changed = wire.clone();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        changed
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert_eq!(
            decode(&changed, &request, &[candidate()]).unwrap_err(),
            AnnotationError::InvalidSchema
        );
    }
    let mut changed = wire.clone();
    changed["version"] = json!(2);
    assert_eq!(
        decode(&changed, &request, &[candidate()]).unwrap_err(),
        AnnotationError::WrongVersion
    );
    for state in ["missing", "truncated", "complete-absent"] {
        let mut changed = wire.clone();
        changed["outcomes"][0]["result"]["matches"][0]["name"] = json!({"state":state});
        assert_eq!(
            decode(&changed, &request, &[candidate()]).unwrap_err(),
            AnnotationError::IncompleteEvidence
        );
    }
    let mut changed = wire.clone();
    changed["outcomes"][0]["result"]["matches"][0]["name"]["value"] = json!("Watch");
    assert_eq!(
        decode(&changed, &request, &[candidate()]).unwrap_err(),
        AnnotationError::IncompleteEvidence
    );
}

#[test]
fn rule_inventory_is_complete_applicable_and_never_renderer_authored() {
    let request = request(anchor_patch());
    let wire = envelope(&request);
    for id in ["unknown", "", "mac2"] {
        let mut changed = wire.clone();
        changed["outcomes"][0]["rule"]["id"] = json!(id);
        assert_eq!(
            decode(&changed, &request, &[candidate()]).unwrap_err(),
            AnnotationError::InvalidInventory
        );
    }
    let mut duplicate = wire.clone();
    duplicate["outcomes"]
        .as_array_mut()
        .unwrap()
        .push(wire["outcomes"][0].clone());
    assert_eq!(
        decode(&duplicate, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidInventory
    );
    let mut omitted = wire.clone();
    omitted["outcomes"] = json!([]);
    assert_eq!(
        decode(&omitted, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidInventory
    );
    let mut inactive = request.clone();
    inactive.stamp.family = InteractionFamily::Buttons;
    let mut inactive_wire = wire.clone();
    inactive_wire["stamp"] = serde_json::from_str(&inactive.stamp_json().unwrap()).unwrap();
    assert_eq!(
        decode(&inactive_wire, &inactive, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidInventory
    );
    let mut absent = wire.clone();
    absent["outcomes"][0]["result"]["matches"] = json!([]);
    assert!(decode(&absent, &request, &[candidate()]).is_ok());
    let mut ambiguous = wire.clone();
    let mut second = candidate();
    second.element_id = 11;
    let mut second_witness = witness();
    second_witness["candidate"]["index"] = json!(1);
    second_witness["candidate"]["element_id"] = json!(11);
    ambiguous["outcomes"][0]["result"]["matches"]
        .as_array_mut()
        .unwrap()
        .push(second_witness);
    assert!(decode(&ambiguous, &request, &[candidate(), second]).is_ok());
    let mut unavailable = wire.clone();
    unavailable["documents"] = json!([]);
    unavailable["roots"] = json!([]);
    unavailable["outcomes"][0]["result"] =
        json!({"state":"unavailable","reason":"annotation-limit"});
    assert!(decode(&unavailable, &request, &[candidate()]).is_ok());
    unavailable["outcomes"][0]["result"]["matches"] = json!([]);
    assert_eq!(
        decode(&unavailable, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidSchema
    );
}

#[test]
fn graph_and_exact_candidate_identity_reject_dangling_cycles_and_replacements() {
    let request = request(anchor_patch());
    let wire = envelope(&request);
    for pointer in [
        "/outcomes/0/result/matches/0/candidate/index",
        "/outcomes/0/result/matches/0/candidate/element_id",
        "/outcomes/0/result/matches/0/candidate/related_element_id",
    ] {
        let mut changed = wire.clone();
        *changed.pointer_mut(pointer).unwrap() = json!(9);
        assert_eq!(
            decode(&changed, &request, &[candidate()]).unwrap_err(),
            AnnotationError::InvalidAssociation
        );
    }
    for pointer in [
        "/outcomes/0/result/matches/0/candidate/frame_owner",
        "/outcomes/0/result/matches/0/candidate/frame_path",
    ] {
        let mut changed = wire.clone();
        *changed.pointer_mut(pointer).unwrap() = json!("different");
        assert_eq!(
            decode(&changed, &request, &[candidate()]).unwrap_err(),
            AnnotationError::InvalidAssociation
        );
    }
    let mut changed = wire.clone();
    changed["documents"][0]["parent_host"] = node(1, 5);
    assert_eq!(
        decode(&changed, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidDocument
    );
    let mut changed = wire.clone();
    changed["roots"][0]["resolved"] = node(1, 99);
    assert_eq!(
        decode(&changed, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidRoot
    );
    let mut changed = wire.clone();
    changed["documents"]
        .as_array_mut()
        .unwrap()
        .push(document());
    assert_eq!(
        decode(&changed, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
    let mut changed = wire.clone();
    changed["roots"]
        .as_array_mut()
        .unwrap()
        .push(root(2, node(1, 1), "document"));
    assert_eq!(
        decode(&changed, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
    let mut changed = wire.clone();
    changed["documents"][0]["frame_owner"] = json!("frame-22222222-2222-4222-8222-222222222222");
    assert_eq!(
        decode(&changed, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidDocument
    );
    assert_eq!(
        decode(&wire, &request, &[candidate(), candidate()]).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}

#[test]
fn structural_items_have_exact_primary_roots_and_complete_identity() {
    let request = request(
        json!({"enabled":true,"collections":[{"id":"list","families":["links"],"codes":["1"],"container_selector":"#list","item_selector":".item","primary_selector":"a","identity_attributes":["data-id"],"order":"dom"}]}),
    );
    let mut wire = envelope(&request);
    wire["roots"]
        .as_array_mut()
        .unwrap()
        .push(root(2, node(1, 30), "element"));
    wire["outcomes"] = json!([{"rule":{"kind":"collection","id":"list"},"result":{"state":"collection-complete","root":1,"container":node(1,20),"container_kind":"element","items":[{"item":node(1,30),"primary_root":2,"primary":witness(),"attributes":[],"identity_attributes":[{"name":"data-id","fact":{"state":"complete-present","value":"item-1"}}]}]}}]);
    assert!(decode(&wire, &request, &[candidate()]).is_ok());
    for state in ["missing", "truncated", "complete-absent"] {
        let mut changed = wire.clone();
        changed["outcomes"][0]["result"]["items"][0]["identity_attributes"][0]["fact"] =
            json!({"state":state});
        assert_eq!(
            decode(&changed, &request, &[candidate()]).unwrap_err(),
            AnnotationError::IncompleteEvidence
        );
    }
    let mut changed = wire.clone();
    changed["roots"][1]["start"] = node(1, 31);
    changed["roots"][1]["resolved"] = node(1, 31);
    assert_eq!(
        decode(&changed, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidRoot
    );
    let mut duplicate = wire.clone();
    let mut item = wire["outcomes"][0]["result"]["items"][0].clone();
    item["item"] = node(1, 31);
    item["primary_root"] = json!(3);
    duplicate["roots"]
        .as_array_mut()
        .unwrap()
        .push(root(3, node(1, 31), "element"));
    duplicate["outcomes"][0]["result"]["items"]
        .as_array_mut()
        .unwrap()
        .push(item);
    assert_eq!(
        decode(&duplicate, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}

#[test]
fn explicit_shadow_slot_paths_cannot_change_kind_or_owner_document() {
    let mut patch = anchor_patch();
    patch["anchors"][0]["root"] = json!({"start":"top-document","steps":[{"kind":"open-shadow","host_selector":"nav-shell"},{"kind":"assigned-slot","host_selector":"slot","assigned_selector":"a"}]});
    let request = request(patch);
    let mut wire = envelope(&request);
    wire["roots"][0]["hops"] = json!([{"kind":"open-shadow","host":node(1,2),"root":node(1,3)},{"kind":"assigned-slot","slot":node(1,4),"assigned":node(1,10),"owner_document":1}]);
    wire["roots"][0]["resolved"] = node(1, 10);
    wire["roots"][0]["resolved_kind"] = json!("element");
    wire["outcomes"][0]["result"]["scope"] = node(1, 10);
    wire["outcomes"][0]["result"]["scope_kind"] = json!("element");
    assert!(decode(&wire, &request, &[candidate()]).is_ok());
    let mut changed = wire.clone();
    changed["roots"][0]["hops"][1]["owner_document"] = json!(2);
    assert_eq!(
        decode(&changed, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidRoot
    );
    let mut changed = wire.clone();
    changed["roots"][0]["hops"][1] =
        json!({"kind":"open-shadow","host":node(1,4),"root":node(1,10)});
    changed["roots"][0]["resolved_kind"] = json!("open-shadow");
    assert_eq!(
        decode(&changed, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidRoot
    );
}

#[test]
fn finite_components_still_reject_overflowing_edges_and_invalid_projection() {
    let request = request(anchor_patch());
    let wire = envelope(&request);
    for (field, value) in [
        ("width", 0.),
        ("height", -1.),
        ("x", f64::MAX),
        ("y", f64::MAX),
        ("width", f64::MAX),
        ("x", -1000.),
    ] {
        let mut changed = wire.clone();
        changed["outcomes"][0]["result"]["matches"][0]["projected_primary_fragment_css"][field] =
            json!(value);
        assert_eq!(
            decode(&changed, &request, &[candidate()]).unwrap_err(),
            AnnotationError::InvalidGeometry
        );
    }
    let mut changed = wire.clone();
    changed["outcomes"][0]["result"]["matches"][0]["top_viewport_css"]["width"] = json!(0);
    assert_eq!(
        decode(&changed, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidGeometry
    );
}

#[test]
fn annotation_byte_budget_is_independent_of_five_thousand_large_candidates() {
    let request = request(anchor_patch());
    let wire = envelope(&request);
    let raw = encoded(&wire).to_string();
    for bytes in [
        MAX_INTERACTION_ANNOTATION_BYTES - 1,
        MAX_INTERACTION_ANNOTATION_BYTES,
    ] {
        let padded = format!("{raw}{}", " ".repeat(bytes - raw.len()));
        assert!(decode_annotations(&padded, &request, &[candidate()]).is_ok());
    }
    assert_eq!(
        decode_annotations(
            &" ".repeat(MAX_INTERACTION_ANNOTATION_BYTES + 1),
            &request,
            &[candidate()]
        )
        .unwrap_err(),
        AnnotationError::TooLarge
    );
    let href = format!(
        "https://example.test/{}",
        "x".repeat(8192 - "https://example.test/".len())
    );
    let candidates: Vec<_> = (0..5000)
        .map(|index| {
            let mut value = candidate();
            value.element_id += index;
            value.href = Some(href.clone());
            value
        })
        .collect();
    let payload:Vec<_>=candidates.iter().map(|value| json!({"element_id":value.element_id,"kind":"link","frame_path":"0","frame_owner":"","text":"Mac","href":value.href,"geometry":{"x":10.,"y":10.,"width":80.,"height":20.}})).collect();
    let ordinary = serde_json::to_string(&payload).unwrap();
    assert!(ordinary.len() > MAX_INTERACTION_ANNOTATION_BYTES);
    let validated = crate::hint_payload::parse_hint_payload(&ordinary).unwrap();
    assert_eq!(validated.len(), 5000);
    let mut annotation = wire.clone();
    annotation["outcomes"][0]["result"]["matches"][0]["href"]["value"] = json!(href);
    assert!(decode(&annotation, &request, &validated).is_ok());
    assert_eq!(validated[4999].href.as_ref().unwrap().len(), 8192);
    assert_eq!(validated[4999].element_id, 5009);
}

#[test]
fn referenced_candidate_meaning_and_policy_are_retained_after_inputs_change() {
    let request = request(json!({"enabled":true,"anchors":[
        {"id":"one","code":"x","families":["links"],"target_selector":"a"},
        {"id":"two","code":"y","families":["links"],"target_selector":"a"}]}));
    let mut wire = envelope(&request);
    let base = wire["outcomes"][0].clone();
    wire["outcomes"] = json!([base.clone(), base]);
    wire["outcomes"][0]["rule"]["id"] = json!("one");
    wire["outcomes"][1]["rule"]["id"] = json!("two");
    let mut candidates = vec![candidate()];
    let checked = decode(&wire, &request, &candidates).unwrap();
    assert_eq!(checked.candidates.len(), 1);
    candidates[0].text = "changed".into();
    candidates[0].href = None;
    drop(candidates);
    drop(request);
    assert_eq!(checked.candidate_snapshot(0).unwrap().text, "Mac");
    assert_eq!(checked.candidate_snapshot(0).unwrap().kind, HintKind::Link);
    assert_eq!(
        checked.candidate_snapshot(0).unwrap().href.as_deref(),
        Some("https://example.test/mac")
    );
    assert_eq!(checked.policy_snapshot().anchors()[1].id, "two");
    let encoded = serde_json::to_value(&checked).unwrap();
    assert_eq!(encoded.as_object().unwrap().len(), 5);
    assert!(!encoded.as_object().unwrap().contains_key("candidates"));
}

#[test]
fn role_work_exhaustion_is_shared_across_rules() {
    let request = request(json!({"enabled":true,"anchors":[
        {"id":"one","code":"x","families":["links"],"predicates":{"role":"button"}},
        {"id":"two","code":"y","families":["links"],"predicates":{"role":"button"}}]}));
    let mut wire = envelope(&request);
    let base = wire["outcomes"][0].clone();
    wire["outcomes"] = json!([base.clone(), base]);
    wire["outcomes"][0]["rule"]["id"] = json!("one");
    wire["outcomes"][1]["rule"]["id"] = json!("two");
    for (length, valid) in [(40_000, true), (60_000, false)] {
        let value = format!("{} button", "x".repeat(length));
        for index in 0..2 {
            wire["outcomes"][index]["result"]["matches"][0]["role"] =
                json!({"state":"complete-present","value":value});
        }
        let result = decode(&wire, &request, &[candidate()]);
        if valid {
            result.unwrap();
        } else {
            assert_eq!(result.unwrap_err(), AnnotationError::WorkLimit);
        }
    }
}

#[test]
fn sequence_caps_cover_outcomes_matches_attributes_and_hops() {
    let request = request(anchor_patch());
    let wire = envelope(&request);
    let mut too_many_outcomes = wire.clone();
    too_many_outcomes["outcomes"] = json!(vec![wire["outcomes"][0].clone(); 81]);
    assert_eq!(
        decode(&too_many_outcomes, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidSchema
    );
    let mut too_many_matches = wire.clone();
    too_many_matches["outcomes"][0]["result"]["matches"] = json!(vec![witness(); 5001]);
    // Raw bytes may reject first; use streaming wire decode directly here to
    // isolate the sequence bound from the independently tested byte bound.
    assert!(
        serde_json::from_str::<wire::Envelope>(&encoded(&too_many_matches).to_string()).is_err()
    );
    let mut too_many_attributes = wire.clone();
    too_many_attributes["outcomes"][0]["result"]["matches"][0]["attributes"] = json!(vec![
        json!({"name":"data-id","fact":{"state":"missing"}});
        5
    ]);
    assert_eq!(
        decode(&too_many_attributes, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidSchema
    );
    let mut too_many_hops = wire.clone();
    too_many_hops["roots"][0]["hops"] = json!(vec![
        json!({"kind":"open-shadow","host":node(1,2),"root":node(1,3)});
        9
    ]);
    assert_eq!(
        decode(&too_many_hops, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidSchema
    );
}

#[test]
fn maximum_inventory_and_eight_hop_root_heavy_encoding_fit_annotation_budget() {
    let mut anchors = Vec::new();
    let mut collections = Vec::new();
    let mut roots = Vec::new();
    let mut outcomes = Vec::new();
    for index in 0..80_u32 {
        let name = format!("rule-{index}");
        let kind = if index < 64 { "anchor" } else { "collection" };
        let code = format!("{}{:02}", if index < 64 { 'a' } else { 'c' }, index);
        let steps:Vec<_>=(0..8).map(|depth| json!({"kind":"open-shadow","host_selector":if depth==0 {format!("#host-{index}")} else {"shadow-shell".into()}})).collect();
        let descriptor = json!({"start":"top-document","steps":steps});
        if index < 64 {
            anchors.push(json!({"id":name,"code":code,"families":["links"],"target_selector":"a","root":descriptor}));
        } else {
            collections.push(json!({"id":name,"codes":[code],"families":["links"],"container_selector":"#items","item_selector":"article","primary_selector":"a","order":"dom","root":descriptor}));
        }
        let base = (index + 1) * 1000;
        let mut value = root(index + 1, node(1, 1), "document");
        value["hops"]=json!((0..8).map(|depth| json!({"kind":"open-shadow","host":node(1,base+20+depth),"root":node(1,base+1+depth)})).collect::<Vec<_>>());
        value["resolved"] = node(1, base + 8);
        value["resolved_kind"] = json!("open-shadow");
        roots.push(value);
        let result = if index < 64 {
            json!({"state":"anchor-complete","root":index+1,"scope":node(1,base+8),"scope_kind":"open-shadow","matches":[]})
        } else {
            json!({"state":"collection-complete","root":index+1,"container":node(1,base+99),"container_kind":"element","items":[]})
        };
        outcomes.push(json!({"rule":{"kind":kind,"id":name},"result":result}));
    }
    let request = request_patches(
        vec![
            json!({"enabled":true,"anchors":anchors}),
            json!({"collections":collections}),
        ],
        InteractionFamily::Links,
    );
    let mut wire = envelope(&request);
    wire["roots"] = json!(roots);
    wire["outcomes"] = json!(outcomes);
    let raw = encoded(&wire).to_string();
    assert!(raw.len() < MAX_INTERACTION_ANNOTATION_BYTES);
    let checked = decode_annotations(&raw, &request, &[]).unwrap();
    assert_eq!((checked.rule_count(), checked.root_count()), (80, 80));
    let mut missing = wire.clone();
    missing["outcomes"].as_array_mut().unwrap().pop();
    assert_eq!(
        decode(&missing, &request, &[]).unwrap_err(),
        AnnotationError::InvalidInventory
    );
    let disabled = request_patches(
        vec![
            json!({"enabled":false,"anchors":request.policy.anchors()}),
            json!({"collections":request.policy.collections()}),
        ],
        InteractionFamily::Links,
    );
    assert_eq!(
        disabled.policy.anchors().len() + disabled.policy.collections().len(),
        80
    );
    assert_eq!(
        decode(&wire, &disabled, &[]).unwrap_err(),
        AnnotationError::InvalidInventory
    );
}

#[test]
fn finite_fragment_centre_does_not_hide_overflowing_edges() {
    let request = request(anchor_patch());
    let mut wire = envelope(&request);
    wire["outcomes"][0]["result"]["matches"][0]["projected_primary_fragment_css"]["x"] =
        json!(f64::MAX * 0.75);
    wire["outcomes"][0]["result"]["matches"][0]["projected_primary_fragment_css"]["width"] =
        json!(f64::MAX * 0.5);
    wire["outcomes"][0]["result"]["matches"][0]["top_viewport_css"]["width"] = json!(f64::MAX);
    assert_eq!(
        decode(&wire, &request, &[candidate()]).unwrap_err(),
        AnnotationError::InvalidGeometry
    );
}

#[test]
fn bounded_name_quality_uses_decoded_utf8_and_characters_not_display_truncation() {
    let mut patch = anchor_patch();
    patch["anchors"][0]["predicates"]["hint_name"] = json!({"contains":""});
    let request = request(patch);
    let mut wire = envelope(&request);
    for count in [511, 512, 513] {
        let name = "😀".repeat(count);
        let mut candidate = candidate();
        candidate.text = "😀".repeat(256);
        wire["outcomes"][0]["result"]["matches"][0]["name"]["value"] = json!(name);
        let result = decode(&wire, &request, &[candidate]);
        if count <= 512 {
            result.unwrap();
        } else {
            assert_eq!(result.unwrap_err(), AnnotationError::IncompleteEvidence);
        }
    }
}

#[test]
fn focused_foreign_document_facts_cannot_widen_the_browser_owner_catalogue() {
    let mut patch = anchor_patch();
    patch["anchors"][0]["root"] =
        json!({"start":"focused-frame","receiver_origin":"https://foreign.test","steps":[]});
    let base = request(patch);
    let owner = "frame-22222222-2222-4222-8222-222222222222".to_owned();
    let request = AnnotationRequest::capture(
        base.policy.clone(),
        WindowId::from_display("windowid-1").unwrap(),
        HintTarget {
            tab: TabId::from_display("tabid-2").unwrap(),
            document: DocumentId::from_display("documentid-3").unwrap(),
            generation: u64::MAX,
        },
        InteractionFamily::Links,
        AnnotationGenerations {
            request_id: u64::MAX,
            ..Default::default()
        },
        Some("scope-fixture"),
        AnnotationDocumentRoles::capture(
            [],
            Some(AnnotationFocusedReceiver {
                owner: owner.clone(),
                origin: "https://foreign.test".into(),
                depth: 1,
                composed_depth: 1,
            }),
        )
        .unwrap(),
    )
    .unwrap();
    let mut wire = envelope(&request);
    let mut foreign = document();
    foreign["id"] = json!(2);
    foreign["frame_owner"] = json!(owner);
    foreign["origin"] = json!("https://foreign.test");
    foreign["epoch"] = json!("22222222-2222-4222-8222-222222222222");
    foreign["parent_host"] = node(1, 5);
    wire["documents"].as_array_mut().unwrap().push(foreign);
    wire["roots"] = json!([root(1, node(2, 1), "document")]);
    wire["outcomes"][0]["result"]["scope"] = node(2, 1);
    wire["outcomes"][0]["result"]["matches"][0]["candidate"]["frame_owner"] = json!(owner);
    let mut candidate = candidate();
    candidate.frame_owner = owner;
    assert!(decode(&wire, &request, std::slice::from_ref(&candidate)).is_ok());
    assert_eq!(
        decode(&wire, &base, std::slice::from_ref(&candidate)).unwrap_err(),
        AnnotationError::InvalidDocument
    );
    wire["documents"][1]["origin"] = json!("https://other.test");
    assert_eq!(
        decode(&wire, &request, &[candidate]).unwrap_err(),
        AnnotationError::InvalidRoot
    );
}

#[test]
fn oversized_later_ancestor_paths_reject_before_graph_traversal() {
    let mut patch = anchor_patch();
    patch["anchors"][0]["root"] =
        json!({"start":"focused-frame","receiver_origin":"https://foreign.test","steps":[]});
    let mut request = request(patch);
    let mut wire = envelope(&request);
    let common_owner = "frame-ffffffff-ffff-4fff-8fff-ffffffffffff".to_owned();
    let mut structural = vec![common_owner.clone()];
    let mut documents = Vec::new();
    let mut first_owner = String::new();
    for id in 1..=3 {
        let owner = format!("frame-{id:08x}-1111-4111-8111-111111111111");
        if id == 1 {
            first_owner = owner.clone();
        } else {
            structural.push(owner.clone());
        }
        let mut child = document();
        child["id"] = json!(id);
        child["frame_owner"] = json!(owner);
        child["origin"] = json!("https://foreign.test");
        child["parent_host"] = node(11, 100 + id);
        child["epoch"] = json!(format!("{id:08x}-2222-4222-8222-222222222222"));
        documents.push(child);
    }
    request.document_roles = AnnotationDocumentRoles::capture(
        structural,
        Some(AnnotationFocusedReceiver {
            owner: first_owner.clone(),
            origin: "https://foreign.test".into(),
            depth: 3,
            composed_depth: 3,
        }),
    )
    .unwrap();
    let long_index = "0".repeat(100_000);
    for (id, path, parent) in [
        (11, format!("0.{long_index}.0"), Some(node(12, 200))),
        (12, format!("0.{long_index}"), Some(node(13, 200))),
        (13, "0".into(), None),
    ] {
        let mut ancestor = document();
        ancestor["id"] = json!(id);
        ancestor["frame_owner"] = json!(common_owner);
        ancestor["frame_path"] = json!(path);
        ancestor["parent_host"] = parent.unwrap_or(Value::Null);
        ancestor["epoch"] = json!(format!("{id:08x}-3333-4333-8333-333333333333"));
        documents.push(ancestor);
    }
    wire["documents"] = json!(documents);
    wire["outcomes"][0]["result"]["matches"][0]["candidate"]["frame_owner"] = json!(first_owner);
    let mut candidate = candidate();
    candidate.frame_owner = first_owner;
    assert!(encoded(&wire).to_string().len() < MAX_INTERACTION_ANNOTATION_BYTES);
    assert_eq!(
        decode(&wire, &request, &[candidate]).unwrap_err(),
        AnnotationError::InvalidDocument
    );
}
