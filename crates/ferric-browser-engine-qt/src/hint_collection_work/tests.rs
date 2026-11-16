use super::*;
use ferric_browser_core::MAX_HINT_INSPECTIONS;
use serde_json::{Value, json};

const OWNER: &str = "frame-22222222-2222-4222-8222-222222222222";
const NONCE: &str = "11111111-1111-4111-8111-111111111111";
const ROWS: &str = include_str!("../../../../fuzz/corpus/interaction_proof/ordinary-candidates");

fn ledger() -> Box<CollectionLedger> {
    CollectionLedger::new(1, VerificationSchedule::graph_decode()).unwrap()
}

fn selected(owner: &str, depth: u8) -> AnnotationFocusedReceiver {
    AnnotationFocusedReceiver {
        owner: owner.into(),
        origin: "https://frame.example.test".into(),
        depth,
        composed_depth: depth,
    }
}

fn capture() -> CollectionWork {
    CollectionWork::capture(Some(selected(OWNER, 1)), "", 18, ledger()).unwrap()
}

#[test]
fn producer_grant_protects_late_document_probes_in_the_same_inspection_budget() {
    let ledger = CollectionLedger::new(1, VerificationSchedule::testing_document_probes()).unwrap();
    let mut work = CollectionWork::capture(None, "", 18, ledger).unwrap();
    let allowance = MAX_HINT_INSPECTIONS - 282;
    assert_eq!(plan(&work)["budget"]["inspections"], allowance);
    assert!(work.accept_ordinary("1", "", "[]", &metadata(allowance, 0).to_string()));
    assert_eq!(work.ledger.used.inspections, allowance);
    assert_eq!(plan(&work)["budget"]["inspections"], 0);
    let scope = NativeBinding {
        phase: NativePhase::Validation,
        stage: 0,
        owner: String::new(),
    };
    let grant = work
        .ledger
        .reserve_operation(
            NativeOperation::DocumentValidation {
                document: 0,
                pass: 0,
            },
            scope.clone(),
            NativeBudget {
                inspections: 1,
                ..NativeBudget::default()
            },
        )
        .unwrap();
    assert!(
        work.ledger
            .complete(&grant.token, &scope, NativeOutcome::NoScript)
    );
    assert_eq!(
        plan(&work)["budget"]["inspections"],
        1,
        "NoScript returns the unused unit without reopening its fixed slot"
    );
    assert!(
        work.ledger
            .reserve_operation(
                NativeOperation::DocumentValidation {
                    document: 0,
                    pass: 0
                },
                scope,
                NativeBudget {
                    inspections: 1,
                    ..NativeBudget::default()
                }
            )
            .is_none()
    );
    assert!(work.reject("2", ""));
    assert_eq!(work.ledger.remaining_native(), NativeBudget::default());
}

#[test]
fn original_preflight_ledger_moves_into_collection_without_refilling() {
    let mut budget = CollectionLedger::new(
        42,
        VerificationSchedule::testing(
            100,
            NativeBudget {
                inspections: 10,
                verification_steps: 20,
                annotation_bytes: 30,
            },
        ),
    )
    .unwrap();
    let binding = NativeBinding {
        phase: NativePhase::Preflight,
        stage: 0,
        owner: String::new(),
    };
    let grant = budget
        .reserve(
            0,
            binding.clone(),
            NativeBudget {
                inspections: 10,
                verification_steps: 20,
                annotation_bytes: 30,
            },
        )
        .unwrap();
    let mut budget = CollectionWork::capture(None, "", 18, budget)
        .err()
        .expect("Outstanding preflight cannot be scheduled");
    assert!(budget.complete(
        &grant.token,
        &binding,
        NativeOutcome::Known(NativeBudget {
            inspections: 3,
            verification_steps: 4,
            annotation_bytes: 5
        })
    ));
    let mut budget = CollectionWork::capture(None, OWNER, 18, budget)
        .err()
        .expect("Invalid required owner preserves the ledger");
    assert!(
        budget
            .reserve(
                0,
                binding,
                NativeBudget {
                    inspections: 1,
                    verification_steps: 1,
                    annotation_bytes: 1
                }
            )
            .is_none()
    );
    let work = CollectionWork::capture(None, "", 18, budget).unwrap();
    assert_eq!(plan(&work)["budget"]["inspections"], 49997);
    assert_eq!(
        plan(&work)["budget"]["annotation_bytes"],
        MAX_INTERACTION_ANNOTATION_BYTES - 5
    );
    assert_eq!(plan(&work)["budget"]["predicate_steps"], 100_000);
    assert_eq!(work.ledger.remaining_native().verification_steps, 96);
}

#[test]
fn pending_native_seal_cannot_advance_or_refill_the_next_world() {
    let mut fixture = policy_fixture();
    assert!(fixture.work.prepare_policy(
        "3",
        "",
        &fixture.top.to_string(),
        &fixture.top_metadata.to_string(),
        &fixture.request
    ));
    let binding = NativeBinding {
        phase: NativePhase::Policy,
        stage: 3,
        owner: String::new(),
    };
    let grant = fixture
        .work
        .ledger
        .reserve(
            0,
            binding.clone(),
            NativeBudget {
                inspections: 10,
                verification_steps: 20,
                annotation_bytes: 30,
            },
        )
        .unwrap();
    assert!(!fixture.work.complete_policy("3", "", true));
    assert!(fixture.work.pending_policy("3", "").is_some());
    assert!(fixture.work.ledger.complete(
        &grant.token,
        &binding,
        NativeOutcome::Known(NativeBudget {
            inspections: 1,
            verification_steps: 2,
            annotation_bytes: 3
        })
    ));
    assert!(fixture.work.complete_policy("3", "", true));
    assert_eq!(plan(&fixture.work)["owner"], OWNER);
    assert!(
        !fixture
            .work
            .ledger
            .complete(&grant.token, &binding, NativeOutcome::NoScript)
    );
}

#[test]
fn stage_rejection_cannot_be_refunded_by_a_late_native_reply() {
    let mut fixture = policy_fixture();
    assert!(fixture.work.prepare_policy(
        "3",
        "",
        &fixture.top.to_string(),
        &fixture.top_metadata.to_string(),
        &fixture.request
    ));
    let binding = NativeBinding {
        phase: NativePhase::Policy,
        stage: 3,
        owner: String::new(),
    };
    let grant = fixture
        .work
        .ledger
        .reserve(
            0,
            binding.clone(),
            NativeBudget {
                inspections: 10,
                verification_steps: 20,
                annotation_bytes: 30,
            },
        )
        .unwrap();
    assert!(fixture.work.reject("3", ""));
    assert!(
        !fixture
            .work
            .ledger
            .complete(&grant.token, &binding, NativeOutcome::NoScript)
    );
    assert_eq!(plan(&fixture.work)["budget"]["inspections"], 0);
    assert_eq!(plan(&fixture.work)["budget"]["annotation_bytes"], 0);
    assert_eq!(fixture.work.ledger.remaining_native().verification_steps, 0);
}

#[test]
fn structural_verification_accumulates_across_worlds_separately_from_predicates() {
    let mut fixture = policy_fixture();
    let predicates = fixture.work.ledger.used.predicate_steps;
    assert!(fixture.work.prepare_policy(
        "3",
        "",
        &fixture.top.to_string(),
        &fixture.top_metadata.to_string(),
        &fixture.request
    ));
    let top = fixture
        .work
        .pending_policy("3", "")
        .unwrap()
        .facts
        .verification_steps();
    assert!(top > 0);
    let issued = serde_json::to_value(
        fixture
            .work
            .pending_policy("3", "")
            .unwrap()
            .ordinary_budget,
    )
    .unwrap();
    assert_eq!(
        issued,
        json!({"depth":8,"inspections":MAX_HINT_INSPECTIONS,
        "candidates":MAX_HINT_CANDIDATES,"predicate_steps":ferric_browser_config::MAX_INTERACTION_PREDICATE_STEPS,
        "annotation_bytes":MAX_INTERACTION_ANNOTATION_BYTES})
    );
    assert_eq!(fixture.work.ledger.used.verification_steps, top);
    assert_eq!(
        fixture.work.ledger.used.predicate_steps,
        predicates + fixture.top_metadata["predicate_steps"].as_u64().unwrap() as usize
    );
    assert!(fixture.work.complete_policy("3", "", true));
    assert!(fixture.work.prepare_policy(
        "4",
        OWNER,
        &fixture.child.to_string(),
        &fixture.child_metadata.to_string(),
        &fixture.request
    ));
    let child = fixture
        .work
        .pending_policy("4", OWNER)
        .unwrap()
        .facts
        .verification_steps();
    assert!(child > 0);
    assert_eq!(fixture.work.ledger.used.verification_steps, top + child);
}

#[test]
fn both_worlds_consume_one_structural_allowance_at_the_exact_boundary() {
    for deficit in [0, 1] {
        let mut fixture = policy_fixture();
        let cost = |world: usize, raw: &Value| {
            decode_annotation_facts_with_budget(
                &raw.to_string(),
                &fixture.request,
                Arc::clone(&fixture.work.ordinary[world].as_ref().unwrap().candidates),
                usize::MAX,
            )
            .unwrap()
            .verification_steps()
        };
        let top = cost(0, &fixture.top);
        let child = cost(1, &fixture.child);
        let remaining = fixture.work.ledger.remaining_native().verification_steps;
        assert!(
            fixture
                .work
                .ledger
                .charge_verification(remaining - top - child + deficit)
        );
        assert!(fixture.work.prepare_policy(
            "3",
            "",
            &fixture.top.to_string(),
            &fixture.top_metadata.to_string(),
            &fixture.request
        ));
        assert_eq!(
            fixture.work.ledger.remaining_native().verification_steps,
            child - deficit
        );
        assert!(fixture.work.complete_policy("3", "", true));
        assert_eq!(
            fixture.work.prepare_policy(
                "4",
                OWNER,
                &fixture.child.to_string(),
                &fixture.child_metadata.to_string(),
                &fixture.request
            ),
            deficit == 0
        );
        assert_eq!(fixture.work.ledger.remaining_native().verification_steps, 0);
        assert_eq!(
            fixture.work.pending_policy("4", OWNER).is_some(),
            deficit == 0
        );
    }
}

#[test]
fn issued_ordinary_budget_is_immutable_and_cannot_overspend_current_remainder() {
    let mut work = capture();
    let original = plan(&work)["budget"].clone();
    let binding = NativeBinding {
        phase: NativePhase::Policy,
        stage: 1,
        owner: String::new(),
    };
    // Exercise the underlying ledger independently of native wrapper phase
    // admission: even another outstanding reservation cannot rewrite a grant.
    let grant = work
        .ledger
        .reserve(
            0,
            binding.clone(),
            NativeBudget {
                inspections: 10,
                verification_steps: 10,
                annotation_bytes: 10,
            },
        )
        .unwrap();
    assert_eq!(plan(&work)["budget"], original);
    assert!(!work.accept_ordinary("1", "", ROWS, &metadata(1, 1).to_string()));
    assert!(work.ledger.complete(
        &grant.token,
        &binding,
        NativeOutcome::Known(NativeBudget {
            inspections: 5,
            verification_steps: 2,
            annotation_bytes: 3,
        })
    ));
    assert_eq!(plan(&work)["budget"], original);
    assert!(!work.accept_ordinary(
        "1",
        "",
        ROWS,
        &metadata(MAX_HINT_INSPECTIONS, 1).to_string()
    ));
    assert_eq!(work.ledger.remaining_native().inspections, 0);
}

#[test]
fn foreign_ordinary_depth_grant_includes_parent_shadow_roots() {
    let mut receiver = selected(OWNER, 1);
    receiver.composed_depth = 2;
    let mut work = CollectionWork::capture(Some(receiver), "", 18, ledger()).unwrap();
    assert_eq!(plan(&work)["budget"]["depth"], 8);
    assert!(work.accept_ordinary("1", "", ROWS, &metadata(12, 1).to_string()));
    assert_eq!(plan(&work)["budget"]["depth"], 6);
}

#[test]
fn foreign_ordinary_grant_requires_valid_composed_depth_authority() {
    for (native, composed) in [(1, 0), (2, 1), (1, 9)] {
        let mut receiver = selected(OWNER, native);
        receiver.composed_depth = composed;
        assert!(CollectionWork::capture(Some(receiver), "", 18, ledger()).is_err());
    }
}

fn metadata(inspections: usize, predicates: usize) -> Value {
    json!({"nonce":NONCE,"revision":1,"selector_revision":"0000000000000012",
        "inspections":inspections,"predicate_steps":predicates,"truncated":false,
        "viewport":{"width":800,"height":600}})
}

fn plan(work: &CollectionWork) -> Value {
    serde_json::from_str(&work.plan_json().unwrap()).unwrap()
}

fn rows(count: usize) -> String {
    let base: Value = serde_json::from_str(ROWS).unwrap();
    let mut result = Vec::with_capacity(count);
    for index in 0..count {
        let mut row = base[0].clone();
        row["element_id"] = json!(index + 1);
        result.push(row);
    }
    serde_json::to_string(&result).unwrap()
}

struct PolicyFixture {
    work: CollectionWork,
    request: AnnotationRequest,
    top: Value,
    child: Value,
    top_metadata: Value,
    child_metadata: Value,
}

fn policy_fixture() -> PolicyFixture {
    use crate::interaction_proof::{AnnotationDocumentRoles, AnnotationGenerations};
    use ferric_browser_config::{
        InteractionPolicyBuilder, InteractionPolicyRevision, PolicySourceIdentity, SiteRule,
    };
    use ferric_browser_core::{DocumentId, HintTarget, ProfileId, TabId, WindowId};
    let fixture: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/site-policy/witness-observed-anchor.json"
    ))
    .unwrap();
    let mut patch = fixture["patch"].clone();
    patch["anchors"][0]["id"] = json!("top");
    let mut foreign = patch["anchors"][0].clone();
    foreign["id"] = json!("foreign");
    foreign["code"] = json!("y");
    foreign["root"] =
        json!({"start":"focused-frame","receiver_origin":"https://frame.example.test","steps":[]});
    patch["anchors"].as_array_mut().unwrap().push(foreign);
    let mut builder = InteractionPolicyBuilder::default();
    builder
        .apply_selected_rule(
            &SiteRule {
                id: "worlds".into(),
                pattern: "https://example.test/*".into(),
                priority: 0,
                set: std::collections::BTreeMap::new(),
                interaction: Some(serde_json::from_value(patch).unwrap()),
            },
            PolicySourceIdentity::Authored { document_id: 1 },
            1,
            0,
        )
        .unwrap();
    let policy = builder
        .finish(InteractionPolicyRevision {
            profile: ProfileId::from_display("profileid-1").unwrap(),
            profile_revision: 1,
            configuration_revision: 1,
            policy_revision: 1,
        })
        .unwrap();
    let request = AnnotationRequest::capture(
        policy,
        WindowId::from_display("windowid-1").unwrap(),
        HintTarget {
            tab: TabId::from_display("tabid-2").unwrap(),
            document: DocumentId::from_display("documentid-3").unwrap(),
            generation: 1,
        },
        ferric_browser_config::InteractionFamily::Links,
        AnnotationGenerations {
            selector_revision: 18,
            request_id: 1,
            ..Default::default()
        },
        None,
        AnnotationDocumentRoles::capture([], Some(selected(OWNER, 1))).unwrap(),
    )
    .unwrap();
    let mut top_metadata = metadata(15, 6);
    top_metadata["viewport"] = json!({"width":1000,"height":800});
    let mut child_metadata = metadata(10, 2);
    child_metadata["nonce"] = json!("33333333-3333-4333-8333-333333333333");
    child_metadata["viewport"] = top_metadata["viewport"].clone();
    let mut work = capture();
    let candidates = fixture["candidates"].to_string();
    assert!(work.accept_ordinary("1", "", &candidates, &top_metadata.to_string()));
    assert!(work.accept_ordinary("2", OWNER, &candidates, &child_metadata.to_string()));
    let mut top = fixture["annotation"].clone();
    top["stamp"] = serde_json::from_str(&request.stamp_json().unwrap()).unwrap();
    let mut observed = top["outcomes"][0].clone();
    observed["rule"]["id"] = json!("top");
    let unavailable = |id: &str| json!({"rule":{"kind":"anchor","id":id},"result":{"unavailable":{"reason":"unsupported-root"}}});
    top["outcomes"] = json!([observed.clone(), unavailable("foreign")]);
    top["discovery_work"] = json!({"inspections":22,"predicate_steps":15});
    let mut child = top.clone();
    child["documents"][0]["frame_owner"] = json!(OWNER);
    child["documents"][0]["origin"] = json!("https://frame.example.test");
    observed["rule"]["id"] = json!("foreign");
    observed["result"]["anchor-observed"]["targets"][0]["candidate"]["frame_owner"] = json!(OWNER);
    child["outcomes"] = json!([unavailable("top"), observed]);
    child["discovery_work"] = json!({"inspections":14,"predicate_steps":7});
    top_metadata["inspections"] = json!(7);
    top_metadata["predicate_steps"] = json!(9);
    child_metadata["inspections"] = json!(4);
    child_metadata["predicate_steps"] = json!(5);
    top_metadata["annotation_bytes"] = json!(top.to_string().len());
    child_metadata["annotation_bytes"] = json!(child.to_string().len());
    PolicyFixture {
        work,
        request,
        top,
        child,
        top_metadata,
        child_metadata,
    }
}

#[test]
fn unknown_selected_ordinary_work_burns_annotation_allowance_and_preserves_top_rows() {
    let mut work = capture();
    assert!(work.accept_ordinary("1", "", ROWS, &metadata(12, 1).to_string()));
    let before = work.plan_json();
    assert!(!work.reject("2", ""));
    assert_eq!(work.plan_json(), before);
    assert!(work.reject("2", OWNER));
    assert_eq!(
        work.ledger.used.annotation_bytes,
        MAX_INTERACTION_ANNOTATION_BYTES
    );
    assert_eq!(plan(&work)["budget"]["annotation_bytes"], 0);
    assert_eq!(work.ordinary[0].as_ref().unwrap().candidates.len(), 1);
    assert!(work.ordinary[1].is_none());
}

#[test]
fn absent_optional_receiver_does_not_issue_or_burn_a_selected_grant() {
    let work = CollectionWork::capture(None, "", 18, ledger()).unwrap();
    assert_eq!(work.steps.len(), 2);
    assert_eq!(plan(&work)["world"], "top");
    assert_eq!(
        plan(&work)["budget"]["annotation_bytes"],
        MAX_INTERACTION_ANNOTATION_BYTES
    );
}

#[test]
fn declared_policy_byte_debit_includes_private_facts_and_is_charged_once() {
    let mut fixture = policy_fixture();
    let raw = fixture.top.to_string();
    let debit = raw.len() + 512;
    fixture.top_metadata["annotation_bytes"] = json!(debit);
    assert!(fixture.work.prepare_policy(
        "3",
        "",
        &raw,
        &fixture.top_metadata.to_string(),
        &fixture.request
    ));
    assert_eq!(fixture.work.ledger.used.annotation_bytes, debit);
    let pending = fixture.work.pending_policy("3", "").unwrap();
    assert_eq!(pending.annotation_bytes, debit);
    assert_eq!(pending.annotation_limit, MAX_INTERACTION_ANNOTATION_BYTES);
    assert_eq!(pending.viewport, [1000., 800.]);
    let geometry: Value =
        serde_json::from_str(&pending.facts.ordinary_geometry_json().unwrap()).unwrap();
    assert_eq!(
        geometry,
        json!([{"index":0,"kind":"link","geometry":{"x":20.,"y":20.,"width":200.,"height":40.}}])
    );
    assert!(!fixture.work.prepare_policy(
        "3",
        "",
        &raw,
        &fixture.top_metadata.to_string(),
        &fixture.request
    ));
    assert!(!fixture.work.reject("3", OWNER));
    assert_eq!(fixture.work.ledger.used.annotation_bytes, debit);
    assert!(fixture.work.complete_policy("3", "", true));
    assert_eq!(
        plan(&fixture.work)["budget"]["annotation_bytes"],
        MAX_INTERACTION_ANNOTATION_BYTES - debit
    );
    // A native completion is still required to establish that the private
    // producer actually charged this debit. These are ledger-only facts.
    assert!(fixture.work.policy[0].as_ref().unwrap().sealed);
}

#[test]
fn policy_byte_debit_cannot_be_missing_null_fractional_or_below_live_raw_bytes() {
    for invalid in [
        None,
        Some(json!(null)),
        Some(json!(-1)),
        Some(json!(0.5)),
        Some(json!(0)),
        Some(json!(2_097_153)),
    ] {
        let mut fixture = policy_fixture();
        if let Some(value) = invalid {
            fixture.top_metadata["annotation_bytes"] = value;
        } else {
            fixture
                .top_metadata
                .as_object_mut()
                .unwrap()
                .remove("annotation_bytes");
        }
        assert!(!fixture.work.prepare_policy(
            "3",
            "",
            &fixture.top.to_string(),
            &fixture.top_metadata.to_string(),
            &fixture.request
        ));
        assert_eq!(
            fixture.work.ledger.used.annotation_bytes,
            MAX_INTERACTION_ANNOTATION_BYTES
        );
        assert!(fixture.work.ordinary.iter().all(Option::is_some));
        assert!(fixture.work.policy[0].is_none());
    }
}

#[test]
fn ordinary_metadata_must_omit_policy_annotation_debits() {
    for value in [json!(null), json!(0), json!(1)] {
        let mut work = capture();
        let mut data = metadata(1, 0);
        data["annotation_bytes"] = value;
        assert!(!work.accept_ordinary("1", "", ROWS, &data.to_string()));
        assert!(work.ordinary[0].is_none());
    }
}

#[test]
fn raw_policy_slots_share_frozen_rows_and_advance_only_after_native_completion() {
    let mut fixture = policy_fixture();
    let top = fixture.top.to_string();
    let child = fixture.child.to_string();
    assert!(fixture.work.prepare_policy(
        "3",
        "",
        &top,
        &fixture.top_metadata.to_string(),
        &fixture.request
    ));
    assert_eq!(fixture.work.ledger.used.inspections, 32);
    assert_eq!(fixture.work.ledger.used.predicate_steps, 17);
    assert_eq!(fixture.work.ledger.used.candidates, 2);
    assert_eq!(fixture.work.ledger.used.annotation_bytes, top.len());
    assert_eq!(
        Arc::strong_count(&fixture.work.ordinary[0].as_ref().unwrap().candidates),
        2
    );
    assert_eq!(
        fixture
            .work
            .pending_policy("3", "")
            .unwrap()
            .facts
            .original_annotation(),
        top
    );
    let pending = fixture.work.plan_json();
    assert!(
        !fixture
            .work
            .prepare_policy("3", "", "malformed", "malformed", &fixture.request)
    );
    assert!(!fixture.work.complete_policy("4", OWNER, true));
    assert!(!fixture.work.complete_policy("3", OWNER, false));
    assert_eq!(fixture.work.plan_json(), pending);
    assert!(fixture.work.complete_policy("3", "", true));
    assert_eq!(plan(&fixture.work)["owner"], OWNER);
    assert!(fixture.work.prepare_policy(
        "4",
        OWNER,
        &child,
        &fixture.child_metadata.to_string(),
        &fixture.request
    ));
    assert!(fixture.work.complete_policy("4", OWNER, true));
    assert!(fixture.work.plan_json().is_none());
    assert_eq!(fixture.work.ledger.used.inspections, 36);
    assert_eq!(fixture.work.ledger.used.predicate_steps, 22);
    assert_eq!(
        fixture.work.ledger.used.annotation_bytes,
        top.len() + child.len()
    );
    for (index, raw) in [(0, &top), (1, &child)] {
        let policy = fixture.work.policy[index].as_ref().unwrap();
        assert!(policy.sealed);
        assert_eq!(policy.facts.original_annotation(), raw);
        assert!(
            raw.contains("anchor-observed"),
            "no canonical normalization at this stage"
        );
        assert_eq!(
            fixture.work.ordinary[index].as_ref().unwrap().candidates[0].element_id,
            1
        );
    }
    assert!(!fixture.work.complete_policy("4", OWNER, true));
}

#[test]
fn malformed_policy_metadata_burns_the_matching_grant_without_losing_ordinary_rows() {
    for (pointer, value) in [
        ("/nonce", json!("44444444-4444-4444-8444-444444444444")),
        ("/revision", json!(2)),
        ("/selector_revision", json!("0000000000000013")),
        ("/viewport/width", json!(1001)),
        ("/inspections", json!(50_001)),
        ("/predicate_steps", json!(100_001)),
    ] {
        let mut fixture = policy_fixture();
        *fixture.top_metadata.pointer_mut(pointer).unwrap() = value;
        assert!(!fixture.work.prepare_policy(
            "3",
            "",
            &fixture.top.to_string(),
            &fixture.top_metadata.to_string(),
            &fixture.request
        ));
        assert!(fixture.work.policy[0].is_none());
        assert_eq!(fixture.work.ledger.used.inspections, MAX_HINT_INSPECTIONS);
        assert_eq!(
            fixture.work.ledger.used.annotation_bytes,
            MAX_INTERACTION_ANNOTATION_BYTES
        );
        for snapshot in fixture.work.ordinary.iter().flatten() {
            assert_eq!(snapshot.candidates.len(), 1);
        }
        assert_eq!(plan(&fixture.work)["owner"], OWNER);
    }
}

#[test]
fn policy_raw_totals_references_and_viewport_must_match_the_original_inventory() {
    for (pointer, value) in [
        ("/discovery_work/inspections", json!(21)),
        ("/discovery_work/predicate_steps", json!(14)),
        (
            "/outcomes/0/result/anchor-observed/targets/0/candidate/index",
            json!(1),
        ),
        (
            "/outcomes/0/result/anchor-observed/targets/0/top_viewport_css/width",
            json!(999),
        ),
        ("/documents/0/frame_owner", json!(OWNER)),
    ] {
        let mut fixture = policy_fixture();
        *fixture.top.pointer_mut(pointer).unwrap() = value;
        fixture.top_metadata["annotation_bytes"] = json!(fixture.top.to_string().len());
        assert!(!fixture.work.prepare_policy(
            "3",
            "",
            &fixture.top.to_string(),
            &fixture.top_metadata.to_string(),
            &fixture.request
        ));
        assert_eq!(
            fixture.work.ordinary[0].as_ref().unwrap().candidates.len(),
            1
        );
        assert!(fixture.work.policy[0].is_none());
    }
}

#[test]
fn failed_native_policy_seal_is_one_shot_and_cannot_refund_or_replace_the_top_slot() {
    let mut fixture = policy_fixture();
    assert!(fixture.work.prepare_policy(
        "3",
        "",
        &fixture.top.to_string(),
        &fixture.top_metadata.to_string(),
        &fixture.request
    ));
    assert!(!fixture.work.complete_policy("3", "", false));
    let after = fixture.work.plan_json();
    assert!(!fixture.work.complete_policy("3", "", true));
    assert!(!fixture.work.prepare_policy(
        "3",
        "",
        &fixture.top.to_string(),
        &fixture.top_metadata.to_string(),
        &fixture.request
    ));
    assert_eq!(fixture.work.plan_json(), after);
    assert_eq!(fixture.work.ledger.used.candidates, 2);
    assert_eq!(
        fixture.work.ledger.used.predicate_steps,
        ferric_browser_config::MAX_INTERACTION_PREDICATE_STEPS
    );
    assert_eq!(
        fixture.work.ordinary[0].as_ref().unwrap().candidates[0].frame_owner,
        ""
    );
    assert_eq!(
        fixture.work.ordinary[1].as_ref().unwrap().candidates[0].frame_owner,
        OWNER
    );
}

#[test]
fn selected_native_failure_preserves_the_already_sealed_top_graph() {
    let mut fixture = policy_fixture();
    let raw = fixture.top.to_string();
    assert!(fixture.work.prepare_policy(
        "3",
        "",
        &raw,
        &fixture.top_metadata.to_string(),
        &fixture.request
    ));
    assert!(fixture.work.complete_policy("3", "", true));
    assert!(fixture.work.prepare_policy(
        "4",
        OWNER,
        &fixture.child.to_string(),
        &fixture.child_metadata.to_string(),
        &fixture.request
    ));
    assert!(!fixture.work.complete_policy("4", OWNER, false));
    assert!(fixture.work.plan_json().is_none());
    let top = fixture.work.policy[0].as_ref().unwrap();
    assert!(top.sealed);
    assert_eq!(top.facts.original_annotation(), raw);
    assert!(fixture.work.policy[1].is_none());
    assert_eq!(
        fixture.work.ordinary[0].as_ref().unwrap().candidates.len(),
        1
    );
    assert_eq!(
        fixture.work.ordinary[1].as_ref().unwrap().candidates.len(),
        1
    );
}

#[test]
fn annotation_bytes_are_shared_across_policy_worlds_at_the_exact_boundary() {
    for excess in [false, true] {
        let mut fixture = policy_fixture();
        let mut child = fixture.child.to_string();
        let top = fixture.top.to_string();
        let padded = format!(
            "{top}{}",
            " ".repeat(MAX_INTERACTION_ANNOTATION_BYTES - child.len() - top.len())
        );
        fixture.top_metadata["annotation_bytes"] = json!(padded.len());
        assert!(fixture.work.prepare_policy(
            "3",
            "",
            &padded,
            &fixture.top_metadata.to_string(),
            &fixture.request
        ));
        assert!(fixture.work.complete_policy("3", "", true));
        if excess {
            child.push(' ');
        }
        fixture.child_metadata["annotation_bytes"] = json!(child.len());
        assert_eq!(
            fixture.work.prepare_policy(
                "4",
                OWNER,
                &child,
                &fixture.child_metadata.to_string(),
                &fixture.request
            ),
            !excess
        );
        assert_eq!(
            fixture.work.ledger.used.annotation_bytes,
            MAX_INTERACTION_ANNOTATION_BYTES
        );
        assert!(fixture.work.policy[0].as_ref().unwrap().sealed);
        assert_eq!(
            fixture.work.ordinary[0].as_ref().unwrap().candidates.len(),
            1
        );
    }
}

#[test]
fn selected_ordinary_precedes_top_policy_and_retains_separate_local_identities() {
    let mut work = capture();
    let first = plan(&work);
    assert_eq!(first["world"], "top");
    assert_eq!(first["phase"], "ordinary");
    assert_eq!(first["budget"]["candidates"], 5000);
    assert!(work.accept_ordinary("1", "", ROWS, &metadata(15, 6).to_string()));
    let second = plan(&work);
    assert_eq!(second["world"], "selected");
    assert_eq!(second["phase"], "ordinary");
    assert_eq!(second["owner"], OWNER);
    assert_eq!(second["budget"]["inspections"], 49985);
    assert_eq!(second["budget"]["predicate_steps"], 99994);
    assert_eq!(second["budget"]["depth"], 7);
    assert!(work.accept_ordinary("2", OWNER, ROWS, &metadata(10, 2).to_string()));
    let policy = plan(&work);
    assert_eq!(policy["world"], "top");
    assert_eq!(policy["phase"], "policy");
    assert!(policy["budget"].get("candidates").is_none());
    assert!(policy["budget"].get("depth").is_none());
    assert_eq!(policy["budget"]["inspections"], 49975);
    let top = work.ordinary[0].as_ref().unwrap();
    let child = work.ordinary[1].as_ref().unwrap();
    assert_eq!(top.candidates[0].element_id, child.candidates[0].element_id);
    assert!(top.candidates[0].frame_owner.is_empty());
    assert_eq!(child.candidates[0].frame_owner, OWNER);
    assert_eq!(top.candidates[0].geometry, child.candidates[0].geometry);
    assert_eq!(top.metadata.nonce, NONCE);
}

#[test]
fn repeated_or_wrong_world_callbacks_do_not_change_the_active_grant() {
    let mut work = capture();
    let before = work.plan_json();
    for (stage, owner) in [("2", ""), ("01", ""), ("1", OWNER), ("1", "forged")] {
        assert!(!work.accept_ordinary(stage, owner, "malformed", "malformed"));
        assert!(!work.reject(stage, owner));
        assert_eq!(work.plan_json(), before);
    }
    assert!(work.accept_ordinary("1", "", ROWS, &metadata(12, 1).to_string()));
    let selected = work.plan_json();
    assert!(!work.accept_ordinary("1", "", ROWS, &metadata(12, 1).to_string()));
    assert!(!work.reject("1", ""));
    assert_eq!(work.plan_json(), selected);
}

#[test]
fn mandatory_selected_scope_never_creates_a_top_grant() {
    let work = CollectionWork::capture(Some(selected(OWNER, 2)), OWNER, 18, ledger()).unwrap();
    let first = plan(&work);
    assert_eq!(first["world"], "selected");
    assert_eq!(first["budget"]["depth"], 6);
    assert_eq!(work.steps.len(), 2);
    assert!(CollectionWork::capture(Some(selected(OWNER, 1)), "other", 18, ledger()).is_err());
    assert!(CollectionWork::capture(Some(selected("", 1)), OWNER, 18, ledger()).is_err());
    assert!(CollectionWork::capture(None, OWNER, 18, ledger()).is_err());
    for depth in [0, 9] {
        assert!(CollectionWork::capture(Some(selected(OWNER, depth)), "", 18, ledger()).is_err());
    }
    let top = CollectionWork::capture(Some(selected("", 1)), "", 18, ledger()).unwrap();
    assert_eq!(
        top.steps.len(),
        2,
        "same-origin focus stays in Top's namespace"
    );
}

#[test]
fn aggregate_boundaries_are_not_restarted_for_the_selected_world() {
    for (inspections, predicates, count, expected) in [
        (24999, 49999, 2499, true),
        (25000, 50000, 2500, true),
        (25001, 50000, 2500, false),
        (25000, 50001, 2500, false),
        (25000, 50000, 2501, false),
    ] {
        let mut work = capture();
        assert!(work.accept_ordinary("1", "", &rows(2500), &metadata(25000, 50000).to_string()));
        assert_eq!(
            work.accept_ordinary(
                "2",
                OWNER,
                &rows(count),
                &metadata(inspections, predicates).to_string()
            ),
            expected
        );
        if expected {
            assert_eq!(work.ledger.used.candidates, 2500 + count);
            assert_eq!(work.ledger.used.inspections, 25000 + inspections);
            assert_eq!(work.ledger.used.predicate_steps, 50000 + predicates);
        } else {
            assert!(work.ordinary[0].is_some());
            assert!(work.ordinary[1].is_none());
            assert_eq!(plan(&work)["world"], "top");
            assert_eq!(plan(&work)["budget"]["inspections"], 0);
            assert_eq!(plan(&work)["budget"]["predicate_steps"], 0);
        }
    }
}

#[test]
fn malformed_accounting_consumes_its_grant_without_refunding_top_work() {
    for (pointer, value) in [
        ("/inspections", json!(-1)),
        ("/inspections", json!(0.5)),
        ("/predicate_steps", json!(100_001)),
        ("/revision", json!(9_007_199_254_740_992_u64)),
        ("/nonce", json!("not-a-nonce")),
        ("/selector_revision", json!("0000000000000013")),
        ("/viewport/width", json!(0)),
        ("/viewport/height", json!("600")),
    ] {
        let mut work = capture();
        assert!(work.accept_ordinary("1", "", ROWS, &metadata(12, 4).to_string()));
        let mut invalid = metadata(10, 1);
        *invalid.pointer_mut(pointer).unwrap() = value;
        assert!(!work.accept_ordinary("2", OWNER, ROWS, &invalid.to_string()));
        assert_eq!(work.ordinary[0].as_ref().unwrap().metadata.inspections, 12);
        assert!(work.ordinary[1].is_none());
        assert_eq!(plan(&work)["budget"]["inspections"], 0);
        assert_eq!(plan(&work)["budget"]["predicate_steps"], 0);
        assert!(!work.accept_ordinary("2", OWNER, ROWS, &metadata(0, 0).to_string()));
    }
}

#[test]
fn native_owner_stamping_does_not_repair_forged_owner_or_depth() {
    let mut forged: Value = serde_json::from_str(ROWS).unwrap();
    forged[0]["frame_owner"] = json!(OWNER);
    let mut work = capture();
    assert!(!work.accept_ordinary("1", "", &forged.to_string(), &metadata(1, 0).to_string()));
    let mut work = CollectionWork::capture(Some(selected(OWNER, 8)), OWNER, 18, ledger()).unwrap();
    let mut deep: Value = serde_json::from_str(ROWS).unwrap();
    deep[0]["frame_path"] = json!("0.0");
    assert!(!work.accept_ordinary("1", OWNER, &deep.to_string(), &metadata(1, 0).to_string()));
}

#[test]
fn ordinary_bytes_are_independent_of_the_annotation_only_limit() {
    let mut ordinary: Value = serde_json::from_str(&rows(400)).unwrap();
    for row in ordinary.as_array_mut().unwrap() {
        row["href"] = json!(format!("https://example.test/{}", "x".repeat(6000)));
    }
    let encoded = ordinary.to_string();
    assert!(encoded.len() > MAX_INTERACTION_ANNOTATION_BYTES);
    let mut work = capture();
    assert!(work.accept_ordinary("1", "", &encoded, &metadata(4000, 0).to_string()));
    assert_eq!(
        plan(&work)["budget"]["annotation_bytes"],
        MAX_INTERACTION_ANNOTATION_BYTES
    );
}

#[test]
fn rejecting_a_policy_grant_cannot_publish_or_reuse_ordinary_state() {
    let mut work = capture();
    assert!(work.accept_ordinary("1", "", ROWS, &metadata(12, 1).to_string()));
    assert!(work.accept_ordinary("2", OWNER, ROWS, &metadata(12, 1).to_string()));
    assert!(!work.accept_ordinary("3", "", ROWS, &metadata(0, 0).to_string()));
    assert!(work.reject("3", ""));
    let child = plan(&work);
    assert_eq!(child["phase"], "policy");
    assert_eq!(child["owner"], OWNER);
    assert_eq!(child["budget"]["annotation_bytes"], 0);
    assert_eq!(child["ordinary"]["candidates"], 1);
    assert!(work.reject("4", OWNER));
    assert!(work.plan_json().is_none());
    assert!(work.ordinary.iter().all(Option::is_some));
}
