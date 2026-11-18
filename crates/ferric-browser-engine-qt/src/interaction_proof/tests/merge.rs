use super::*;
use crate::interaction_proof::merge::{MergeWorld, WorldProjections, merge_worlds};
use std::sync::Arc;

const OWNER: &str = "frame-22222222-2222-4222-8222-222222222222";

#[test]
fn collection_item_and_primary_roots_survive_document_namespace_remapping() {
    let (request, wire, rows) = super::discovery::collection();
    let candidates: Arc<[HintCandidate]> = rows.into();
    let facts =
        decode_annotation_facts(&wire.to_string(), &request, Arc::clone(&candidates)).unwrap();
    let projections: WorldProjections = serde_json::from_value(json!([
        {"owner":"","geometry":{"viewport":{"width":1000.,"height":800.},"mappings":[]}}
    ]))
    .unwrap();
    let original = candidates[0].clone();
    let (_, checked) = merge_worlds(
        &request,
        vec![MergeWorld {
            owner: String::new(),
            facts: Box::new(facts),
            candidates,
        }],
        &projections,
        21,
        38,
        100_000,
    )
    .unwrap();
    let graph = serde_json::to_value(&checked).unwrap();
    assert_eq!(graph["documents"][0]["id"], 1);
    assert_eq!(graph["documents"][0]["document_node"], 2);
    let result = &graph["outcomes"][0]["result"]["collection-complete"];
    assert_eq!(result["container"], node(1, 2));
    assert_eq!(result["items"][0]["item"], node(1, 100));
    assert_eq!(result["items"][0]["primary_root"], 2);
    assert_eq!(checked.candidate_snapshot(0), Some(&original));
}

fn fixture(observed: bool) -> (AnnotationRequest, Vec<MergeWorld>, Value) {
    let mut patch = anchor_patch();
    patch["anchors"].as_array_mut().unwrap().push(json!({
        "id":"remote","code":"r","families":["links"],"target_selector":"a",
        "root":{"start":"focused-frame","receiver_origin":"https://example.test"},
        "predicates":{"hint_name":{"equals":"Mac"}}
    }));
    let mut request = request(patch);
    request.document_roles = AnnotationDocumentRoles::capture(
        [],
        Some(AnnotationFocusedReceiver {
            owner: OWNER.into(),
            origin: "https://example.test".into(),
            depth: 1,
            composed_depth: 1,
        }),
    )
    .unwrap();
    let mut worlds = Vec::new();
    for owner in ["", OWNER] {
        let mut wire = encoded(&envelope(&request));
        let mut positive = wire["outcomes"][0].clone();
        if observed {
            let result = positive["result"]["anchor-complete"].take();
            positive["result"] = json!({"anchor-observed":{
                "root":result["root"],"scope":result["scope"],"scope_kind":result["scope_kind"],"targets":result["matches"],
            }});
            positive["result"]["anchor-observed"]["targets"][0]["fragment_index"] = json!(0);
        }
        let unavailable = |id| {
            json!({"rule":{"kind":"anchor","id":id},
            "result":{"unavailable":{"reason":"unsupported-root"}}})
        };
        let mut row = candidate();
        row.frame_owner = owner.into();
        if owner.is_empty() {
            wire["outcomes"] = json!([positive, unavailable("remote")]);
        } else {
            positive["rule"]["id"] = json!("remote");
            let target = if observed {
                &mut positive["result"]["anchor-observed"]["targets"][0]
            } else {
                &mut positive["result"]["anchor-complete"]["matches"][0]
            };
            target["candidate"]["frame_owner"] = json!(owner);
            wire["documents"][0]["frame_owner"] = json!(owner);
            wire["outcomes"] = json!([unavailable("mac"), positive]);
        }
        wire["discovery_work"] = json!({"inspections":2,"predicate_steps":0});
        let candidates: Arc<[HintCandidate]> = vec![row].into();
        let facts =
            decode_annotation_facts(&wire.to_string(), &request, Arc::clone(&candidates)).unwrap();
        worlds.push(MergeWorld {
            owner: owner.into(),
            facts: Box::new(facts),
            candidates,
        });
    }
    let projections = json!([
        {"owner":"","geometry":{"viewport":{"width":1000.,"height":800.},"mappings":[]}},
        {"owner":OWNER,"geometry":{"viewport":{"width":1000.,"height":800.},"mappings":[{
            "left":100.,"top":50.,"scaleX":1.,"scaleY":1.,
            "content":{"left":100.,"top":50.,"right":500.,"bottom":500.,"width":400.,"height":450.}
        }],"targets":[{"index":0,"fragment_index":0,"rect":{"x":110.,"y":60.,"width":80.,"height":20.}}]}}
    ]);
    (request, worlds, projections)
}

fn merge(
    request: &AnnotationRequest,
    worlds: Vec<MergeWorld>,
    projections: Value,
    spent: usize,
) -> Result<(Vec<HintCandidate>, CheckedAnnotations), AnnotationError> {
    let projections: WorldProjections = serde_json::from_value(projections).unwrap();
    merge_worlds(request, worlds, &projections, 4, spent, 100_000)
}

#[test]
fn original_worlds_merge_with_injective_indices_and_unchanged_action_identity() {
    let (request, worlds, projections) = fixture(false);
    let (rows, checked) = merge(&request, worlds, projections, 0).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].element_id, 10);
    assert_eq!(rows[1].element_id, 10);
    assert_eq!(rows[1].frame_owner, OWNER);
    assert_eq!(rows[1].frame_path, "0");
    assert_eq!(rows[1].geometry.x, 110.);
    let graph = serde_json::to_value(&checked).unwrap();
    assert_eq!(graph["documents"][0]["id"], 1);
    assert_eq!(graph["documents"][1]["id"], 2);
    assert_eq!(
        graph["documents"][1]["document_node"], 1,
        "DOM object IDs stay local and unchanged"
    );
    assert_eq!(graph["roots"][1]["start"], node(2, 1));
    assert_eq!(
        graph["outcomes"][0]["result"]["anchor-complete"]["matches"][0]["candidate"]["index"],
        0
    );
    assert_eq!(
        graph["outcomes"][1]["result"]["anchor-complete"]["matches"][0]["candidate"]["index"],
        1
    );
}

#[test]
fn a_dropped_foreign_reference_becomes_unavailable_without_proving_absence() {
    let (request, worlds, mut projections) = fixture(true);
    projections[1]["geometry"]["mappings"][0]["content"] = json!({
        "left":300.,"top":50.,"right":500.,"bottom":500.,"width":200.,"height":450.,
    });
    projections[1]["geometry"]["targets"] = json!([]);
    let (rows, checked) = merge(&request, worlds, projections, 0).unwrap();
    assert_eq!(rows.len(), 1);
    let graph = serde_json::to_value(&checked).unwrap();
    assert!(graph["outcomes"][0]["result"]["anchor-complete"].is_object());
    assert_eq!(
        graph["outcomes"][1]["result"],
        json!({"unavailable":{"reason":"missing-evidence"}})
    );
    assert_eq!(graph["documents"].as_array().unwrap().len(), 1);
}

#[test]
fn canonical_matching_uses_the_single_global_producer_remainder() {
    let (request, worlds, projections) = fixture(true);
    let (_, checked) = merge(&request, worlds, projections, 100_000).unwrap();
    let graph = serde_json::to_value(&checked).unwrap();
    for outcome in graph["outcomes"].as_array().unwrap() {
        assert_eq!(
            outcome["result"],
            json!({"unavailable":{"reason":"predicate-limit"}})
        );
    }
}

#[test]
fn an_unrelated_projection_cannot_select_a_collector_or_replace_its_inventory() {
    let (request, worlds, mut projections) = fixture(false);
    projections[1]["geometry"]["targets"][0]["index"] = json!(1);
    assert_eq!(
        merge(&request, worlds, projections, 0).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}

#[test]
fn a_native_projection_limit_never_turns_a_missing_target_into_absence() {
    let (request, worlds, mut projections) = fixture(true);
    projections[1]["geometry"]["targets"] = json!([]);
    projections[1]["geometry"]["limited"] = json!(true);
    let (_, checked) = merge(&request, worlds, projections, 0).unwrap();
    let graph = serde_json::to_value(&checked).unwrap();
    assert_eq!(
        graph["outcomes"][1]["result"],
        json!({"unavailable":{"reason":"truncated"}})
    );
}

#[test]
fn excluding_top_for_a_foreign_modal_downgrades_top_without_relabeling_dom_ids() {
    let (request, worlds, mut projections) = fixture(false);
    projections[0]["excluded"] = json!(true);
    let (rows, checked) = merge(&request, worlds, projections, 0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].frame_owner, OWNER);
    assert_eq!(rows[0].element_id, 10);
    let graph = serde_json::to_value(&checked).unwrap();
    assert_eq!(
        graph["outcomes"][0]["result"],
        json!({"unavailable":{"reason":"unsupported-root"}})
    );
    assert_eq!(
        graph["outcomes"][1]["result"]["anchor-complete"]["matches"][0]["candidate"]["index"],
        0
    );
}
