use super::*;

const ANCESTOR: &str = "frame-22222222-2222-4222-8222-222222222222";
const SELECTED: &str = "frame-33333333-3333-4333-8333-333333333333";

fn foreign_chain() -> (AnnotationRequest, Value, HintCandidate) {
    let mut patch = anchor_patch();
    patch["anchors"][0]["root"] =
        json!({"start":"focused-frame","receiver_origin":"https://foreign.test","steps":[]});
    let mut request = request(patch);
    request.document_roles = AnnotationDocumentRoles::capture(
        [ANCESTOR.into()],
        Some(AnnotationFocusedReceiver {
            owner: SELECTED.into(),
            origin: "https://foreign.test".into(),
            depth: 2,
            composed_depth: 2,
        }),
    )
    .unwrap();
    let mut value = envelope(&request);
    for (id, owner, origin, parent) in [
        (2, ANCESTOR, "https://foreign.test", 1),
        (3, SELECTED, "https://foreign.test", 2),
    ] {
        let mut doc = document();
        doc["id"] = json!(id);
        doc["frame_owner"] = json!(owner);
        doc["origin"] = json!(origin);
        doc["epoch"] = json!(format!("{id:08x}-1111-4111-8111-111111111111"));
        doc["parent_host"] = node(parent, 5);
        value["documents"].as_array_mut().unwrap().push(doc);
    }
    value["roots"][0] = root(1, node(3, 1), "document");
    value["outcomes"][0]["result"]["scope"] = node(3, 1);
    value["outcomes"][0]["result"]["matches"][0]["candidate"]["frame_owner"] = json!(SELECTED);
    let mut row = candidate();
    row.frame_owner = SELECTED.into();
    (request, value, row)
}

#[test]
fn structural_ancestors_cannot_supply_semantic_roots_or_witnesses() {
    let (request, value, row) = foreign_chain();
    decode(&value, &request, std::slice::from_ref(&row)).unwrap();
    let mut forged = value.clone();
    forged["roots"][0] = root(1, node(2, 1), "document");
    forged["outcomes"][0]["result"]["scope"] = node(2, 1);
    forged["outcomes"][0]["result"]["matches"][0]["candidate"]["frame_owner"] = json!(ANCESTOR);
    let mut ancestor_row = row.clone();
    ancestor_row.frame_owner = ANCESTOR.into();
    assert_eq!(
        decode(&forged, &request, &[ancestor_row]).unwrap_err(),
        AnnotationError::InvalidAssociation
    );

    let mut no_receiver = request.clone();
    no_receiver.document_roles =
        AnnotationDocumentRoles::capture([ANCESTOR.into(), SELECTED.into()], None).unwrap();
    assert_eq!(
        decode(&value, &no_receiver, &[row]).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
}

#[test]
fn native_receiver_role_requires_exact_owner_origin_and_full_depth() {
    let (request, value, row) = foreign_chain();
    for (owner, origin, depth) in [
        (SELECTED, "https://foreign.test", 1),
        (SELECTED, "https://other.test", 2),
        ("", "https://foreign.test", 2),
    ] {
        let mut changed = request.clone();
        changed.document_roles = AnnotationDocumentRoles::capture(
            [ANCESTOR.into()],
            Some(AnnotationFocusedReceiver {
                owner: owner.into(),
                origin: origin.into(),
                depth,
                composed_depth: depth,
            }),
        )
        .unwrap();
        assert!(decode(&value, &changed, std::slice::from_ref(&row)).is_err());
    }
    let mut local = value;
    // A world-local native check precedes merging: its root has no aggregate
    // parent link yet, but the native receiver's full depth still applies.
    local["documents"] = json!([local["documents"][2].clone()]);
    local["documents"][0]["parent_host"] = Value::Null;
    decode(&local, &request, std::slice::from_ref(&row)).unwrap();
    local["documents"][0]["frame_path"] = json!("0.0");
    assert_eq!(
        decode(&local, &request, &[row]).unwrap_err(),
        AnnotationError::InvalidDocument
    );
}

#[test]
fn same_origin_hops_cannot_cross_collector_namespaces() {
    let owner = SELECTED;
    let descriptor = json!({"start":"top-document","steps":[{"kind":"same-origin-frame","host_selector":"iframe"}]});
    let mut patch = anchor_patch();
    patch["anchors"][0]["root"] = descriptor;
    let mut request = request(patch);
    request.document_roles = AnnotationDocumentRoles::capture(
        [],
        Some(AnnotationFocusedReceiver {
            owner: owner.into(),
            origin: "https://example.test".into(),
            depth: 1,
            composed_depth: 1,
        }),
    )
    .unwrap();
    let mut value = envelope(&request);
    let mut child = document();
    child["id"] = json!(2);
    child["frame_owner"] = json!(owner);
    child["epoch"] = json!("22222222-2222-4222-8222-222222222222");
    child["parent_host"] = node(1, 5);
    value["documents"].as_array_mut().unwrap().push(child);
    value["roots"][0]["hops"] =
        json!([{"kind":"same-origin-frame","host":node(1,5),"document":node(2,1)}]);
    value["roots"][0]["resolved"] = node(2, 1);
    value["outcomes"][0]["result"]["scope"] = node(2, 1);
    value["outcomes"][0]["result"]["matches"][0]["candidate"]["frame_owner"] = json!(owner);
    let mut row = candidate();
    row.frame_owner = owner.into();
    let mut same_world = value.clone();
    same_world["documents"][1]["frame_owner"] = json!("");
    same_world["documents"][1]["frame_path"] = json!("0.0");
    same_world["outcomes"][0]["result"]["matches"][0]["candidate"]["frame_owner"] = json!("");
    same_world["outcomes"][0]["result"]["matches"][0]["candidate"]["frame_path"] = json!("0.0");
    let mut local_row = row.clone();
    local_row.frame_owner.clear();
    local_row.frame_path = "0.0".into();
    decode(&same_world, &request, &[local_row]).unwrap();
    assert_eq!(
        decode(&value, &request, &[row]).unwrap_err(),
        AnnotationError::InvalidRoot
    );
}

#[test]
fn focused_top_documents_need_a_selected_role_without_a_second_namespace() {
    let (request, value, row) = focused_chain(1, json!([]));
    decode(&value, &request, std::slice::from_ref(&row)).unwrap();
    assert!(request.document_roles.is_collector(""));
    assert!(!request.document_roles.contains(SELECTED));
    let mut top_only = request;
    top_only.document_roles = AnnotationDocumentRoles::default();
    assert_eq!(
        decode(&value, &top_only, &[row]).unwrap_err(),
        AnnotationError::InvalidRoot
    );
}

#[test]
fn native_document_role_inventory_is_bounded_and_cannot_overlap() {
    let receiver = || AnnotationFocusedReceiver {
        owner: SELECTED.into(),
        origin: "https://foreign.test".into(),
        depth: 8,
        composed_depth: 8,
    };
    for structural in [
        vec![String::new()],
        vec!["bad-owner".into()],
        vec![ANCESTOR.into(), ANCESTOR.into()],
        vec![SELECTED.into()],
    ] {
        assert_eq!(
            AnnotationDocumentRoles::capture(structural, Some(receiver())).unwrap_err(),
            AnnotationError::InvalidAssociation
        );
    }
    let owners = (1..=8)
        .map(|id| format!("frame-{id:08x}-1111-4111-8111-111111111111"))
        .collect::<Vec<_>>();
    AnnotationDocumentRoles::capture(owners[..7].to_vec(), Some(receiver())).unwrap();
    assert_eq!(
        AnnotationDocumentRoles::capture(owners, Some(receiver())).unwrap_err(),
        AnnotationError::InvalidAssociation
    );
    for (origin, depth) in [
        ("null", 1),
        ("https://foreign.test/", 1),
        ("https://foreign.test", 0),
        ("https://foreign.test", 9),
    ] {
        let mut focused = receiver();
        focused.origin = origin.into();
        focused.depth = depth;
        assert_eq!(
            AnnotationDocumentRoles::capture([], Some(focused)).unwrap_err(),
            AnnotationError::InvalidAssociation
        );
    }
}

#[test]
fn native_and_composed_depth_are_distinct_checked_authority() {
    for (native, composed, valid) in [
        (1, 1, true),
        (1, 2, true),
        (1, 8, true),
        (1, 0, false),
        (2, 1, false),
        (1, 9, false),
    ] {
        let result = AnnotationDocumentRoles::capture(
            [],
            Some(AnnotationFocusedReceiver {
                owner: SELECTED.into(),
                origin: "https://foreign.test".into(),
                depth: native,
                composed_depth: composed,
            }),
        );
        assert_eq!(
            result.is_ok(),
            valid,
            "native={native}, composed={composed}"
        );
    }
}
