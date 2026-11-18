use std::collections::BTreeSet;

use super::*;
use crate::hint_collection_work::ledger::{Ledger, NativeBudget, NativeOutcome, Schedule};
use ferric_browser_config::MAX_INTERACTION_PREDICATE_STEPS;
use ferric_browser_core::MAX_HINT_INSPECTIONS;

const OWNER: &str = "frame-22222222-2222-4222-8222-222222222222";

#[test]
fn geometry_slots_share_verification_capacity_and_cannot_be_renewed() {
    assert_eq!(GEOMETRY_BASE, 31_076);
    let mut ledger = Ledger::new(1, Schedule::graph_documents()).unwrap();
    let before = ledger.remaining_native().verification_steps;
    let limit = NativeBudget {
        verification_steps: 100_000,
        ..NativeBudget::default()
    };
    let top = binding(NativePhase::Policy, "");
    assert_eq!(
        Operation::Geometry.id(&top),
        u16::try_from(GEOMETRY_BASE).ok()
    );
    let grant = ledger
        .reserve_operation(Operation::Geometry, top.clone(), limit)
        .unwrap();
    assert_eq!(
        ledger.remaining_native().verification_steps,
        before - 100_000
    );
    assert!(ledger.complete(
        &grant.token,
        &top,
        NativeOutcome::Known(NativeBudget {
            verification_steps: 7,
            ..NativeBudget::default()
        })
    ));
    assert_eq!(ledger.remaining_native().verification_steps, before - 7);
    assert!(
        ledger
            .reserve_operation(Operation::Geometry, top, limit)
            .is_none()
    );
    let selected = binding(NativePhase::Policy, OWNER);
    assert_eq!(
        Operation::Geometry.id(&selected),
        u16::try_from(GEOMETRY_BASE + 1).ok()
    );
    let grant = ledger
        .reserve_operation(Operation::Geometry, selected.clone(), limit)
        .unwrap();
    assert!(!ledger.complete(&grant.token, &selected, NativeOutcome::Unknown));
    assert!(!ledger.has_pending());
    assert!(!ledger.complete(
        &grant.token,
        &selected,
        NativeOutcome::Known(NativeBudget::default())
    ));
    assert_eq!(
        ledger.remaining_native().verification_steps,
        before - 100_007
    );
    assert!(
        Operation::Geometry
            .id(&binding(NativePhase::Preflight, ""))
            .is_none()
    );
    assert!(
        Operation::Geometry
            .id(&binding(NativePhase::Validation, ""))
            .is_none()
    );
}
const ALL_CHECKPOINTS: [Checkpoint; CHECKPOINTS] = [
    Checkpoint::Provider,
    Checkpoint::BindTop,
    Checkpoint::BindSelected,
    Checkpoint::FocusedBeforeFirst,
    Checkpoint::FocusedBeforeSecond,
    Checkpoint::CaptureAncestry,
    Checkpoint::CaptureFinal,
    Checkpoint::FallbackTop,
    Checkpoint::TopCaptureBefore,
    Checkpoint::TopCaptureAfter,
    Checkpoint::TopCaptureFocused,
    Checkpoint::TopCaptureEntered,
    Checkpoint::TopResealBefore,
    Checkpoint::TopResealAfter,
    Checkpoint::TopResealFocused,
    Checkpoint::TopResealEntered,
    Checkpoint::TopDependency,
    Checkpoint::SelectedCaptureBefore,
    Checkpoint::SelectedCaptureAfter,
    Checkpoint::SelectedCaptureFocused,
    Checkpoint::SelectedCaptureEntered,
    Checkpoint::SelectedResealBefore,
    Checkpoint::SelectedResealAfter,
    Checkpoint::SelectedResealFocused,
    Checkpoint::SelectedResealEntered,
    Checkpoint::SelectedDependency,
];

fn binding(phase: NativePhase, owner: &str) -> Binding {
    Binding {
        phase,
        stage: if phase == NativePhase::Policy { 3 } else { 0 },
        owner: owner.into(),
    }
}

fn verification(steps: usize) -> NativeBudget {
    NativeBudget {
        verification_steps: steps,
        ..NativeBudget::default()
    }
}

#[test]
fn all_supported_collection_slots_have_distinct_bounded_ids() {
    let mut seen = BTreeSet::new();
    let mut insert = |operation: Operation, binding: &Binding| {
        if let Some(id) = operation.id(binding) {
            assert!(usize::from(id) < SLOT_COUNT);
            assert!(seen.insert(id), "duplicate slot for {operation:?}");
        }
    };
    for frame in 0..64 {
        for pass in [AttachmentPass::Cached, AttachmentPass::Initialize] {
            insert(
                Operation::Attachment { frame, pass },
                &binding(NativePhase::Preflight, ""),
            );
        }
    }
    for (index, checkpoint) in ALL_CHECKPOINTS.into_iter().enumerate() {
        assert_eq!(checkpoint.index(), index);
        let scope = if index < Checkpoint::TopCaptureBefore.index() {
            binding(
                NativePhase::Preflight,
                if matches!(
                    checkpoint,
                    Checkpoint::Provider | Checkpoint::BindTop | Checkpoint::FallbackTop
                ) {
                    ""
                } else {
                    OWNER
                },
            )
        } else {
            binding(
                NativePhase::Policy,
                if index < Checkpoint::SelectedCaptureBefore.index() {
                    ""
                } else {
                    OWNER
                },
            )
        };
        assert!(
            [
                Operation::DocumentFacts {
                    checkpoint,
                    document: 0,
                    pass: 0
                },
                Operation::ChildFocus {
                    checkpoint,
                    parent: 0,
                    pass: 0,
                    child: 0
                },
                Operation::TerminalFocus {
                    checkpoint,
                    pass: 0
                },
            ]
            .iter()
            .any(|operation| operation.id(&scope).is_some()),
            "{checkpoint:?}"
        );
        for document in 0..9 {
            for pass in 0..2 {
                insert(
                    Operation::DocumentFacts {
                        checkpoint,
                        document,
                        pass,
                    },
                    &scope,
                );
                for child in 0..64 {
                    insert(
                        Operation::ChildFocus {
                            checkpoint,
                            parent: document,
                            pass,
                            child,
                        },
                        &scope,
                    );
                }
            }
        }
        for edge in 0..8 {
            for pass in 0..2 {
                insert(
                    Operation::AncestryVerify {
                        checkpoint,
                        edge,
                        pass,
                    },
                    &scope,
                );
            }
        }
        for pass in 0..3 {
            insert(Operation::TerminalFocus { checkpoint, pass }, &scope);
        }
    }
    for edge in 0..8 {
        insert(
            Operation::AncestryCapture { edge },
            &binding(NativePhase::Preflight, OWNER),
        );
    }
    for owner in ["", OWNER] {
        for pass in 0..4 {
            insert(
                Operation::Graph { pass },
                &binding(NativePhase::Policy, owner),
            );
        }
    }
    assert!(seen.len() > 10_000);
    assert_eq!(
        seen.last().copied(),
        u16::try_from(VALIDATION_BASE - 1).ok()
    );
}

fn document_probes() -> Vec<(Operation, Binding)> {
    let mut probes = Vec::new();
    for checkpoint in ALL_CHECKPOINTS {
        let preflight = checkpoint.index() < Checkpoint::TopCaptureBefore.index();
        let owner = if matches!(checkpoint, Checkpoint::BindTop | Checkpoint::FallbackTop)
            || !preflight && checkpoint.index() < Checkpoint::SelectedCaptureBefore.index()
        {
            ""
        } else {
            OWNER
        };
        let scope = binding(
            if preflight {
                NativePhase::Preflight
            } else {
                NativePhase::Policy
            },
            owner,
        );
        for document in 0..9 {
            for pass in 0..2 {
                let operation = Operation::DocumentFacts {
                    checkpoint,
                    document,
                    pass,
                };
                if operation.is_document_probe() && operation.id(&scope).is_some() {
                    probes.push((operation, scope.clone()));
                }
            }
        }
    }
    for document in 0..9 {
        for pass in 0..2 {
            probes.push((
                Operation::DocumentValidation { document, pass },
                binding(NativePhase::Validation, OWNER),
            ));
        }
    }
    probes
}

#[test]
fn document_profile_has_exact_positions_and_graph_only_schedule_stays_closed() {
    let probes = document_probes();
    let unit = NativeBudget {
        inspections: 1,
        ..NativeBudget::default()
    };
    let mut graph_only = Ledger::new(1, Schedule::graph_decode()).unwrap();
    let mut prototype = Ledger::new(1, Schedule::testing_document_probes()).unwrap();
    let mut counts = [0; 4];
    for (operation, scope) in &probes {
        counts[match scope.phase {
            NativePhase::Preflight => 0,
            NativePhase::Policy => {
                if scope.owner.is_empty() {
                    1
                } else {
                    2
                }
            }
            NativePhase::Validation => 3,
            NativePhase::Projection => unreachable!(),
        }] += 1;
        // The graph-only profile cannot fund a different native counter.
        assert!(
            graph_only
                .reserve_operation(*operation, scope.clone(), unit)
                .is_none()
        );
    }
    assert_eq!(counts, [76, 62, 126, 18]);
    assert_eq!(probes.len(), 282);
    for (operation, scope) in probes {
        let grant = prototype
            .reserve_operation(operation, scope.clone(), unit)
            .unwrap();
        assert!(prototype.complete(&grant.token, &scope, NativeOutcome::Known(unit)));
    }
}

#[test]
fn document_probe_prototype_refuses_other_valid_native_kinds() {
    let unit = NativeBudget {
        inspections: 1,
        ..NativeBudget::default()
    };
    let preflight = binding(NativePhase::Preflight, "");
    let selected = binding(NativePhase::Preflight, OWNER);
    for (operation, scope) in [
        (
            Operation::Attachment {
                frame: 0,
                pass: AttachmentPass::Cached,
            },
            preflight.clone(),
        ),
        (
            Operation::ChildFocus {
                checkpoint: Checkpoint::Provider,
                parent: 0,
                pass: 0,
                child: 0,
            },
            preflight.clone(),
        ),
        (
            Operation::TerminalFocus {
                checkpoint: Checkpoint::FocusedBeforeFirst,
                pass: 0,
            },
            selected.clone(),
        ),
        (Operation::AncestryCapture { edge: 0 }, selected.clone()),
        (
            Operation::AncestryVerify {
                checkpoint: Checkpoint::CaptureFinal,
                edge: 0,
                pass: 0,
            },
            selected,
        ),
        (
            Operation::DocumentFacts {
                checkpoint: Checkpoint::BindTop,
                document: 1,
                pass: 0,
            },
            preflight,
        ),
    ] {
        assert!(
            operation.id(&scope).is_some(),
            "A valid identity must remain unfunded: {operation:?}"
        );
        let mut closed = Ledger::new(1, Schedule::testing_document_probes()).unwrap();
        assert!(closed.reserve_operation(operation, scope, unit).is_none());
    }
}

#[test]
fn document_probe_prototype_refuses_unfunded_checkpoints_and_wrong_units() {
    let unit = NativeBudget {
        inspections: 1,
        ..NativeBudget::default()
    };
    for checkpoint in [
        Checkpoint::Provider,
        Checkpoint::CaptureAncestry,
        Checkpoint::TopCaptureEntered,
        Checkpoint::SelectedResealEntered,
    ] {
        let mut closed = Ledger::new(1, Schedule::testing_document_probes()).unwrap();
        let scope = match checkpoint {
            Checkpoint::Provider | Checkpoint::CaptureAncestry => {
                binding(NativePhase::Preflight, OWNER)
            }
            Checkpoint::TopCaptureEntered => binding(NativePhase::Policy, ""),
            Checkpoint::SelectedResealEntered => binding(NativePhase::Policy, OWNER),
            _ => unreachable!(),
        };
        assert!(
            closed
                .reserve_operation(
                    Operation::DocumentFacts {
                        checkpoint,
                        document: 0,
                        pass: 0
                    },
                    scope,
                    unit
                )
                .is_none()
        );
    }
    for limit in [
        NativeBudget::default(),
        NativeBudget {
            inspections: 2,
            ..unit
        },
        NativeBudget {
            verification_steps: 1,
            ..unit
        },
        NativeBudget {
            annotation_bytes: 1,
            ..unit
        },
    ] {
        let mut closed = Ledger::new(1, Schedule::testing_document_probes()).unwrap();
        assert!(
            closed
                .reserve_operation(
                    Operation::DocumentFacts {
                        checkpoint: Checkpoint::BindTop,
                        document: 0,
                        pass: 0
                    },
                    binding(NativePhase::Preflight, ""),
                    limit
                )
                .is_none()
        );
    }
}

#[test]
fn document_validation_positions_and_phase_cannot_expand() {
    let mut seen = BTreeSet::new();
    for document in 0..9 {
        for pass in 0..2 {
            let id = Operation::DocumentValidation { document, pass }
                .id(&binding(NativePhase::Validation, ""))
                .unwrap();
            assert!(usize::from(id) >= VALIDATION_BASE && usize::from(id) < GEOMETRY_BASE);
            assert!(seen.insert(id));
        }
    }
    assert_eq!(seen.len(), 18);
    assert_eq!(seen.last().copied(), u16::try_from(GEOMETRY_BASE - 1).ok());
    assert!(
        Operation::DocumentValidation {
            document: 9,
            pass: 0
        }
        .id(&binding(NativePhase::Validation, ""))
        .is_none()
    );
    assert!(
        Operation::DocumentValidation {
            document: 0,
            pass: 2
        }
        .id(&binding(NativePhase::Validation, ""))
        .is_none()
    );
    assert!(
        Operation::DocumentValidation {
            document: 0,
            pass: 0
        }
        .id(&binding(NativePhase::Preflight, ""))
        .is_none()
    );
    assert!(
        !Binding {
            stage: 1,
            ..binding(NativePhase::Validation, "")
        }
        .valid()
    );
}

#[test]
fn full_producer_work_keeps_all_unissued_document_probes_inside_the_shared_cap() {
    let mut ledger = Ledger::new(1, Schedule::testing_document_probes()).unwrap();
    // Simulate a producer using its entire issued discovery allowance. Native
    // seals must still fit without a second inspection pool or a larger cap.
    ledger.used.inspections = MAX_HINT_INSPECTIONS - 282;
    let unit = NativeBudget {
        inspections: 1,
        ..NativeBudget::default()
    };
    for (operation, scope) in document_probes() {
        let grant = ledger
            .reserve_operation(operation, scope.clone(), unit)
            .unwrap();
        assert!(ledger.complete(&grant.token, &scope, NativeOutcome::Known(unit)));
    }
    assert_eq!(ledger.used.inspections, MAX_HINT_INSPECTIONS);
    assert_eq!(ledger.remaining_native().inspections, 0);
}

#[test]
fn invalid_positions_worlds_phases_and_json_cannot_create_slots() {
    let top = binding(NativePhase::Preflight, "");
    assert!(
        Operation::Attachment {
            frame: 64,
            pass: AttachmentPass::Cached
        }
        .id(&top)
        .is_none()
    );
    assert!(
        Operation::DocumentFacts {
            checkpoint: Checkpoint::BindTop,
            document: 9,
            pass: 0
        }
        .id(&top)
        .is_none()
    );
    assert!(
        Operation::DocumentFacts {
            checkpoint: Checkpoint::BindTop,
            document: 0,
            pass: 2
        }
        .id(&top)
        .is_none()
    );
    assert!(
        Operation::DocumentFacts {
            checkpoint: Checkpoint::BindSelected,
            document: 0,
            pass: 0
        }
        .id(&top)
        .is_none()
    );
    assert!(
        Operation::ChildFocus {
            checkpoint: Checkpoint::Provider,
            parent: 0,
            pass: 0,
            child: 64
        }
        .id(&top)
        .is_none()
    );
    assert!(
        Operation::AncestryCapture { edge: 8 }
            .id(&binding(NativePhase::Preflight, OWNER))
            .is_none()
    );
    assert!(
        Operation::Graph { pass: 4 }
            .id(&binding(NativePhase::Policy, ""))
            .is_none()
    );
    assert!(
        Operation::Graph { pass: 0 }
            .id(&binding(NativePhase::Projection, ""))
            .is_none()
    );
    assert!(
        Operation::Graph { pass: 0 }
            .id(&binding(NativePhase::Policy, "invalid-owner"))
            .is_none()
    );
    for raw in [
        "65535",
        r#"{"kind":"invented","pass":0}"#,
        r#"{"kind":"graph","pass":0,"extra":1}"#,
        r#"{"kind":"child-focus","checkpoint":"provider","parent":0,"pass":0,"child":256}"#,
    ] {
        assert!(serde_json::from_str::<Operation>(raw).is_err(), "{raw}");
    }
}

#[test]
fn production_graph_profile_cannot_fund_unimplemented_metadata_or_projection() {
    let mut ledger = Ledger::new(1, Schedule::graph_decode()).unwrap();
    let scope = binding(NativePhase::Policy, "");
    assert!(
        ledger
            .reserve_operation(
                Operation::DocumentFacts {
                    checkpoint: Checkpoint::TopCaptureBefore,
                    document: 0,
                    pass: 0
                },
                scope.clone(),
                verification(1)
            )
            .is_none()
    );
    for limit in [
        NativeBudget {
            inspections: 1,
            ..verification(1)
        },
        NativeBudget {
            annotation_bytes: 1,
            ..verification(1)
        },
        verification(MAX_INTERACTION_PREDICATE_STEPS + 1),
    ] {
        assert!(
            ledger
                .reserve_operation(Operation::Graph { pass: 0 }, scope.clone(), limit)
                .is_none()
        );
    }
    assert!(
        ledger
            .reserve_operation(
                Operation::Graph { pass: 0 },
                binding(NativePhase::Projection, ""),
                verification(1)
            )
            .is_none()
    );
    let grant = ledger
        .reserve_operation(
            Operation::Graph { pass: 0 },
            scope.clone(),
            verification(10),
        )
        .unwrap();
    assert!(ledger.complete(&grant.token, &scope, NativeOutcome::NoScript));
    assert!(
        ledger
            .reserve_operation(Operation::Graph { pass: 0 }, scope, verification(10))
            .is_none(),
        "A new amount cannot reopen the same native slot"
    );
}

#[test]
fn eight_graph_slots_and_two_rust_decodes_share_the_reviewed_subtotal() {
    let mut ledger = Ledger::new(1, Schedule::graph_decode()).unwrap();
    for owner in ["", OWNER] {
        let scope = binding(NativePhase::Policy, owner);
        for pass in 0..4 {
            let limit = verification(MAX_INTERACTION_PREDICATE_STEPS);
            let grant = ledger
                .reserve_operation(Operation::Graph { pass }, scope.clone(), limit)
                .unwrap();
            assert!(ledger.complete(&grant.token, &scope, NativeOutcome::Known(limit)));
        }
    }
    assert!(ledger.charge_verification(2 * MAX_INTERACTION_PREDICATE_STEPS));
    assert_eq!(ledger.remaining_native().verification_steps, 0);
    assert_eq!(ledger.used.inspections, 0);
    assert_eq!(ledger.used.predicate_steps, 0);
    assert_eq!(ledger.used.annotation_bytes, 0);
}
