use ferric_browser_core::{
    CheckedHintCodebook, HintAutoFollow, HintCandidate, HintCodeDeclaration, HintCodebookOptions,
    HintEnhancedError, HintError, HintGeometry, HintInteraction, HintInteractionInput,
    HintInteractionKind, HintInteractionOutcome, HintKind, HintRefreshIssue,
    HintReservationCategory, HintReservationReason, HintReservationState, HintRetentionBudget,
    HintSelectionUnavailable, HintSpecialAssignment, HintSpecialBinding,
};
use std::collections::BTreeSet;

#[path = "hint_interaction_enhanced/retirement_oracle.rs"]
mod retirement_oracle;

fn candidate(id: u32) -> HintCandidate {
    HintCandidate {
        element_id: id,
        related_element_id: None,
        kind: HintKind::Link,
        frame_path: "0".into(),
        frame_owner: "top".into(),
        text: format!("target {id}"),
        href: Some(format!("https://example.test/{id}")),
        presentation: None,
        geometry: HintGeometry {
            x: 0.0,
            y: f64::from(id),
            width: 10.0,
            height: 10.0,
        },
    }
}
fn book(alphabet: &str, codes: &[&str], follow: HintAutoFollow) -> CheckedHintCodebook {
    CheckedHintCodebook::compile(
        HintCodebookOptions {
            alphabet: alphabet.into(),
            min_chars: 1,
            fallback_prefix: String::new(),
            excluded_chars: BTreeSet::new(),
            page_source_chars: BTreeSet::new(),
            additional_controls: BTreeSet::new(),
            auto_follow: follow,
        },
        codes
            .iter()
            .enumerate()
            .map(|(slot, code)| HintCodeDeclaration {
                category: HintReservationCategory::Collection,
                rule_id: format!("c{}", slot / 64),
                slot: slot % 64,
                code: (*code).into(),
            })
            .collect(),
    )
    .unwrap()
}
fn live(slot: usize, candidate: &HintCandidate) -> HintSpecialAssignment {
    HintSpecialAssignment {
        slot,
        binding: HintSpecialBinding::Live(candidate.clone()),
    }
}
fn model(
    candidates: Vec<HintCandidate>,
    codes: &[&str],
    assignments: Vec<HintSpecialAssignment>,
) -> HintInteraction {
    HintInteraction::new_enhanced(
        candidates,
        book("abc", codes, HintAutoFollow::Never),
        assignments,
        HintRetentionBudget::Unlimited,
    )
    .unwrap()
}
fn enter(model: &mut HintInteraction) -> HintInteractionOutcome {
    model.handle(HintInteractionInput::Activate)
}
fn type_code(model: &mut HintInteraction, code: &str) -> HintInteractionOutcome {
    let mut outcome = HintInteractionOutcome::Updated;
    for character in code.chars() {
        outcome = model.handle(HintInteractionInput::Character(character));
    }
    outcome
}
fn labels(model: &HintInteraction) -> Vec<(u32, String)> {
    model
        .hints()
        .iter()
        .map(|hint| (hint.candidate.element_id, hint.label.clone()))
        .collect()
}

#[test]
fn mixed_length_exact_case_and_numeric_codes_are_reachable_independently_of_fallback() {
    let candidates = vec![candidate(1), candidate(2), candidate(3)];
    let bindings = candidates
        .iter()
        .enumerate()
        .map(|(slot, candidate)| live(slot, candidate))
        .collect();
    let mut model = model(candidates, &["1", "aZ", "Z9"], bindings);
    assert_eq!(model.snapshot().active_label, None);
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
    for code in ["1", "aZ", "Z9"] {
        model.handle(HintInteractionInput::Clear);
        assert_eq!(type_code(&mut model, code), HintInteractionOutcome::Updated);
        assert_eq!(
            enter(&mut model),
            HintInteractionOutcome::Activate(code.into())
        );
    }
    model.handle(HintInteractionInput::Clear);
    assert_eq!(
        type_code(&mut model, "az"),
        HintInteractionOutcome::NoMatches
    );
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
    model.handle(HintInteractionInput::Backspace);
    assert_eq!(model.snapshot().prefix, "a");
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
    assert_eq!(type_code(&mut model, "Z"), HintInteractionOutcome::Updated);
    assert_eq!(
        enter(&mut model),
        HintInteractionOutcome::Activate("aZ".into())
    );
}

#[test]
fn invalid_stale_unassigned_clear_and_backspace_never_launder_prior_selection() {
    let c = candidate(1);
    let mut model = model(vec![c.clone()], &["1"], vec![live(0, &c)]);
    type_code(&mut model, "1");
    assert_eq!(
        type_code(&mut model, "x"),
        HintInteractionOutcome::NoMatches
    );
    assert_eq!(model.snapshot().prefix, "1x");
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
    model.handle(HintInteractionInput::Backspace);
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
    model.handle(HintInteractionInput::Clear);
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
    model.handle(HintInteractionInput::Next);
    assert_eq!(
        enter(&mut model),
        HintInteractionOutcome::Activate("1".into())
    );
    assert_eq!(
        model.handle(HintInteractionInput::SelectLabel("unknown".into())),
        HintInteractionOutcome::NoMatches
    );
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
    model.handle(HintInteractionInput::SelectLabel("1".into()));
    assert!(model.retire_reservation(0, HintReservationReason::Stale));
    assert_eq!(model.snapshot().active_label, None);
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
    assert_eq!(
        model.reservations()[0].state(),
        HintReservationState::Retired(HintReservationReason::Stale)
    );
}

#[test]
fn full_match_is_deliberate_only_and_never_preserves_explicit_confirmation() {
    let c = candidate(1);
    for follow in [HintAutoFollow::FullMatch, HintAutoFollow::Never] {
        let mut model = HintInteraction::new_enhanced(
            vec![c.clone()],
            book("ab", &["1A"], follow),
            vec![live(0, &c)],
            HintRetentionBudget::Unlimited,
        )
        .unwrap();
        assert_eq!(model.initial_activation(), None);
        assert_eq!(type_code(&mut model, "1"), HintInteractionOutcome::Updated);
        assert_eq!(
            enter(&mut model),
            HintInteractionOutcome::Activate("1A".into())
        );
        model.handle(HintInteractionInput::Clear);
        let outcome = type_code(&mut model, "1A");
        assert_eq!(
            outcome,
            if follow == HintAutoFollow::FullMatch {
                HintInteractionOutcome::Activate("1A".into())
            } else {
                HintInteractionOutcome::Updated
            }
        );
        model.refresh(vec![c.clone()]).unwrap();
        assert_eq!(model.initial_activation(), None);
        assert_eq!(
            enter(&mut model),
            HintInteractionOutcome::Activate("1A".into())
        );
    }
}

#[test]
fn unavailable_only_sessions_keep_all_reservations_and_never_fill_missing_slots() {
    let codes: Vec<_> = (0..512).map(|index| format!("{index:08}")).collect();
    let refs: Vec<_> = codes.iter().map(String::as_str).collect();
    let mut model = model(
        Vec::new(),
        &refs,
        vec![HintSpecialAssignment {
            slot: 0,
            binding: HintSpecialBinding::Unavailable(HintReservationReason::Ambiguous),
        }],
    );
    assert_eq!(
        model.interaction_kind(),
        HintInteractionKind::UnavailableOnly
    );
    assert_eq!(model.reservations().len(), 512);
    assert_eq!(
        type_code(&mut model, "00000000"),
        HintInteractionOutcome::NoMatches
    );
    assert_eq!(
        model.snapshot().unavailable_reason,
        Some(HintSelectionUnavailable::Reservation {
            slot: 0,
            state: HintReservationState::Unavailable(HintReservationReason::Ambiguous)
        })
    );
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
    model.refresh(vec![candidate(1)]).unwrap();
    assert_eq!(labels(&model), vec![(1, "a".into())]);
    assert_eq!(
        model.reservations()[0].state(),
        HintReservationState::Unavailable(HintReservationReason::Ambiguous)
    );
    assert_eq!(
        model.reservations()[1].state(),
        HintReservationState::Unavailable(HintReservationReason::Missing)
    );
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
    assert_eq!(
        model.handle(HintInteractionInput::Cancel),
        HintInteractionOutcome::Cancel
    );
}

#[test]
fn invalid_initial_slots_snapshots_and_duplicate_target_ownership_are_rejected() {
    let c = candidate(1);
    let make = |rows, assignments| {
        HintInteraction::new_enhanced(
            rows,
            book("ab", &["1", "2"], HintAutoFollow::Never),
            assignments,
            HintRetentionBudget::Unlimited,
        )
    };
    assert_eq!(
        make(vec![c.clone()], vec![live(2, &c)]).unwrap_err(),
        HintEnhancedError::UnknownSlot(2)
    );
    assert_eq!(
        make(vec![c.clone()], vec![live(0, &c), live(0, &c)]).unwrap_err(),
        HintEnhancedError::DuplicateSlot(0)
    );
    assert_eq!(
        make(vec![c.clone()], vec![live(0, &c), live(1, &c)]).unwrap_err(),
        HintEnhancedError::DuplicateSpecialTarget {
            first: 0,
            second: 1
        }
    );
    assert_eq!(
        make(vec![c.clone()], vec![live(0, &candidate(2))]).unwrap_err(),
        HintEnhancedError::UnknownCandidate(0)
    );
    let mut changed = c.clone();
    changed.href = Some("https://different.test/".into());
    assert_eq!(
        make(vec![c.clone()], vec![live(0, &changed)]).unwrap_err(),
        HintEnhancedError::CandidateSnapshotMismatch(0)
    );
    assert_eq!(
        make(vec![c.clone(), c], vec![]).unwrap_err(),
        HintEnhancedError::Candidate(HintError::DuplicateCandidate)
    );
}

#[test]
fn anchor_shadowed_collection_slots_are_holes_without_rank_compression() {
    let rows = vec![candidate(1), candidate(2), candidate(3)];
    let mut checked = book("ab", &["A", "1", "2", "3"], HintAutoFollow::Never)
        .declarations()
        .to_vec();
    checked[0].category = HintReservationCategory::Anchor;
    checked[0].rule_id = "anchor".into();
    for (rank, declaration) in checked[1..].iter_mut().enumerate() {
        declaration.slot = rank;
    }
    let options = HintCodebookOptions {
        alphabet: "ab".into(),
        min_chars: 1,
        fallback_prefix: String::new(),
        excluded_chars: BTreeSet::new(),
        page_source_chars: BTreeSet::new(),
        additional_controls: BTreeSet::new(),
        auto_follow: HintAutoFollow::Never,
    };
    let checked = CheckedHintCodebook::compile(options, checked).unwrap();
    let assignments = vec![
        live(0, &rows[1]),
        live(1, &rows[0]),
        HintSpecialAssignment {
            slot: 2,
            binding: HintSpecialBinding::Unavailable(HintReservationReason::AnchorShadowed),
        },
        live(3, &rows[2]),
    ];
    let mut model = HintInteraction::new_enhanced(
        rows.clone(),
        checked,
        assignments,
        HintRetentionBudget::Unlimited,
    )
    .unwrap();
    assert_eq!(
        labels(&model),
        vec![(1, "1".into()), (2, "A".into()), (3, "3".into())]
    );
    model
        .refresh(vec![rows[2].clone(), rows[0].clone(), rows[1].clone()])
        .unwrap();
    model.consume(&rows[1]);
    model.refresh(rows).unwrap();
    assert_eq!(labels(&model), vec![(1, "1".into()), (3, "3".into())]);
    assert_eq!(
        model.reservations()[0].state(),
        HintReservationState::Consumed
    );
    assert_eq!(
        model.reservations()[2].state(),
        HintReservationState::Unavailable(HintReservationReason::AnchorShadowed)
    );
    assert_eq!(model.reservations()[2].declaration().slot, 1);
    assert_eq!(model.reservations()[3].declaration().slot, 2);
}

#[test]
fn budget_failure_after_a_planned_binding_commits_neither_updates_nor_cursor() {
    let a = candidate(1);
    let b = candidate(2);
    let two_entries = model(vec![a.clone(), b.clone()], &[], vec![])
        .retained_payload_bytes()
        .unwrap();
    let mut model = HintInteraction::new_enhanced(
        vec![a.clone()],
        book("abc", &[], HintAutoFollow::Never),
        vec![],
        HintRetentionBudget::Bytes(two_entries),
    )
    .unwrap();
    let initial_bytes = model.retained_payload_bytes();
    assert!(matches!(
        model.refresh(vec![a.clone(), b.clone(), candidate(3)]),
        Err(HintError::RetainedStateExhausted { .. })
    ));
    assert_eq!(labels(&model), vec![(1, "a".into())]);
    assert_eq!(model.retained_payload_bytes(), initial_bytes);
    assert_eq!(model.ordinary_codes_remaining(), Some(2));
    model.refresh(vec![a.clone()]).unwrap();
    model.handle(HintInteractionInput::SelectLabel("b".into()));
    assert_eq!(
        model.snapshot().unavailable_reason,
        Some(HintSelectionUnavailable::Unassigned)
    );
    model.refresh(vec![a, b]).unwrap();
    assert_eq!(labels(&model), vec![(1, "a".into()), (2, "b".into())]);
    assert_eq!(model.ordinary_codes_remaining(), Some(1));
}

#[test]
fn every_selection_route_stays_blocked_after_rejected_positive_admission() {
    let a = candidate(1);
    let bytes = model(vec![a.clone()], &[], vec![])
        .retained_payload_bytes()
        .unwrap();
    let mut blocked = HintInteraction::new_enhanced(
        vec![a.clone()],
        book("abc", &[], HintAutoFollow::FullMatch),
        vec![],
        HintRetentionBudget::Bytes(bytes),
    )
    .unwrap();
    assert!(blocked.refresh(vec![a.clone(), candidate(2)]).is_err());
    let routes = vec![
        vec![HintInteractionInput::Character('a')],
        vec![HintInteractionInput::SelectLabel("a".into())],
        vec![HintInteractionInput::Next],
        vec![HintInteractionInput::Previous],
        vec![HintInteractionInput::SetText("target".into())],
        vec![
            HintInteractionInput::EnterTextMode,
            HintInteractionInput::Character('t'),
            HintInteractionInput::Backspace,
            HintInteractionInput::Backspace,
        ],
        vec![HintInteractionInput::RotateCollision],
        vec![HintInteractionInput::Clear],
        vec![HintInteractionInput::Backspace],
        vec![HintInteractionInput::Activate],
    ];
    for route in routes {
        let mut model = blocked.clone();
        for input in route {
            assert!(!matches!(
                model.handle(input),
                HintInteractionOutcome::Activate(_)
            ));
            assert_eq!(model.snapshot().active_label, None);
            assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
            assert_eq!(
                model.snapshot().unavailable_reason,
                Some(HintSelectionUnavailable::RefreshBlocked)
            );
        }
    }
    blocked.consume(&a);
    assert_eq!(
        blocked.snapshot().unavailable_reason,
        Some(HintSelectionUnavailable::RefreshBlocked)
    );
}

#[test]
fn overflowing_label_input_never_confirms_the_previous_eight_character_selection() {
    let a = candidate(1);
    let mut model = model(vec![a.clone()], &["12345678"], vec![live(0, &a)]);
    type_code(&mut model, "12345678");
    assert_eq!(
        enter(&mut model),
        HintInteractionOutcome::Activate("12345678".into())
    );
    assert_eq!(
        type_code(&mut model, "x"),
        HintInteractionOutcome::NoMatches
    );
    assert_eq!(model.snapshot().prefix, "12345678");
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
    model.handle(HintInteractionInput::Backspace);
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
}

#[test]
fn malformed_refreshes_preserve_the_prior_model_without_claiming_new_fact_admission() {
    let a = candidate(1);
    let mut model = model(vec![a.clone()], &[], vec![]);
    let before = model.clone();
    assert_eq!(
        model.refresh(vec![a.clone(), a.clone()]),
        Err(HintError::DuplicateCandidate)
    );
    assert_eq!(model, before);
    let mut malformed = a;
    malformed.geometry.width = 0.0;
    assert_eq!(
        model.refresh(vec![malformed]),
        Err(HintError::InvalidGeometry { index: 0 })
    );
    assert_eq!(model, before);
}

#[test]
fn disappearing_special_targets_retire_permanently_but_may_receive_unused_ordinary_codes() {
    let a = candidate(1);
    let b = candidate(2);
    let mut model = HintInteraction::new_enhanced(
        vec![a.clone(), b.clone()],
        book("ab", &["12345678"], HintAutoFollow::Never),
        vec![live(0, &a)],
        HintRetentionBudget::Unlimited,
    )
    .unwrap();
    assert_eq!(model.ordinary_suffix_width(), Some(1));
    assert_eq!(
        labels(&model),
        vec![(1, "12345678".into()), (2, "a".into())]
    );
    type_code(&mut model, "12345678");
    model.refresh(vec![b.clone()]).unwrap();
    assert_eq!(model.snapshot().active_label, None);
    model.refresh(vec![a, b]).unwrap();
    assert_eq!(labels(&model), vec![(1, "b".into()), (2, "a".into())]);
    assert_eq!(
        model.reservations()[0].state(),
        HintReservationState::Retired(HintReservationReason::Missing)
    );
    assert!(!model.retire_reservation(0, HintReservationReason::Stale));
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
}

#[test]
fn late_ordinary_completion_consumes_known_identity_after_disappearance() {
    let original = candidate(1);
    let mut model = model(vec![original.clone()], &[], vec![]);
    model.refresh(vec![]).unwrap();
    model.consume(&original);
    model.refresh(vec![original]).unwrap();
    assert!(model.hints().is_empty());
    assert_eq!(model.ordinary_codes_remaining(), Some(2));
}

#[test]
fn late_special_completion_preserves_retirement_and_prevents_ordinary_fallback() {
    let original = candidate(1);
    let mut model = model(vec![original.clone()], &["1"], vec![live(0, &original)]);
    model.refresh(vec![]).unwrap();
    model.consume(&original);
    model.refresh(vec![original]).unwrap();
    assert!(model.hints().is_empty());
    assert_eq!(
        model.reservations()[0].state(),
        HintReservationState::Retired(HintReservationReason::Missing)
    );
    assert_eq!(model.ordinary_codes_remaining(), Some(3));
}

#[test]
fn late_completion_consumes_physical_identity_even_after_meaning_changes() {
    let original = candidate(1);
    let mut changed = original.clone();
    changed.href = Some("https://example.test/changed".into());
    let mut model = model(vec![original.clone()], &[], vec![]);
    model.refresh(vec![changed.clone()]).unwrap();
    model.consume(&original);
    assert!(model.hints().is_empty());
    model.refresh(vec![changed]).unwrap();
    assert!(model.hints().is_empty());
    assert_eq!(model.ordinary_codes_remaining(), Some(1));
}

#[test]
fn consumed_labels_and_refresh_exhaustion_never_change_width_or_reuse_ownership() {
    let rows = vec![candidate(1), candidate(2)];
    let mut model = HintInteraction::new_enhanced(
        rows.clone(),
        book("ab", &[], HintAutoFollow::Never),
        vec![],
        HintRetentionBudget::Unlimited,
    )
    .unwrap();
    model.consume(&rows[0]);
    model
        .refresh(vec![rows[0].clone(), rows[1].clone(), candidate(3)])
        .unwrap();
    assert_eq!(labels(&model), vec![(2, "b".into())]);
    assert_eq!(model.ordinary_suffix_width(), Some(1));
    assert_eq!(model.ordinary_codes_remaining(), Some(0));
    assert_eq!(
        model.snapshot().refresh_issue,
        Some(HintRefreshIssue::NamespaceExhausted)
    );
    assert_eq!(
        type_code(&mut model, "a"),
        HintInteractionOutcome::NoMatches
    );
    assert_eq!(
        model.snapshot().unavailable_reason,
        Some(HintSelectionUnavailable::ConsumedLabel)
    );
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
}

#[test]
fn changed_ordinary_meanings_retire_old_codes_even_after_the_ledger_replaces_the_entry() {
    let original = candidate(1);
    let mut changed = original.clone();
    changed.href = Some("https://example.test/new".into());
    let mut model = model(vec![original.clone()], &[], vec![]);
    model.refresh(vec![changed]).unwrap();
    assert_eq!(labels(&model), vec![(1, "b".into())]);
    assert_eq!(
        model.handle(HintInteractionInput::SelectLabel("a".into())),
        HintInteractionOutcome::NoMatches
    );
    assert_eq!(
        model.snapshot().unavailable_reason,
        Some(HintSelectionUnavailable::RetiredLabel)
    );
    model.refresh(vec![original]).unwrap();
    assert_eq!(labels(&model), vec![(1, "c".into())]);
    assert_eq!(
        model.handle(HintInteractionInput::SelectLabel("b".into())),
        HintInteractionOutcome::NoMatches
    );
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
}

#[test]
fn query_navigation_and_collision_rotation_remain_explicit_and_query_sources_are_verbatim() {
    let mut row = candidate(1);
    row.text = "query/x Ω".into();
    let options = HintCodebookOptions {
        alphabet: "abx".into(),
        min_chars: 1,
        fallback_prefix: String::new(),
        excluded_chars: BTreeSet::new(),
        page_source_chars: BTreeSet::from(['x']),
        additional_controls: BTreeSet::new(),
        auto_follow: HintAutoFollow::Never,
    };
    let checked = CheckedHintCodebook::compile(options, vec![]).unwrap();
    let mut model =
        HintInteraction::new_enhanced(vec![row], checked, vec![], HintRetentionBudget::Unlimited)
            .unwrap();
    assert_eq!(
        model.handle(HintInteractionInput::RotateCollision),
        HintInteractionOutcome::NoMatches
    );
    model.handle(HintInteractionInput::Next);
    assert_eq!(
        model.handle(HintInteractionInput::RotateCollision),
        HintInteractionOutcome::RotateCollision
    );
    assert_eq!(
        type_code(&mut model, "x"),
        HintInteractionOutcome::InvalidCharacter('x')
    );
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
    model.handle(HintInteractionInput::SetText("query/x Ω".into()));
    assert_eq!(model.snapshot().query, "query/x Ω");
    assert_eq!(
        enter(&mut model),
        HintInteractionOutcome::Activate("a".into())
    );
    model.handle(HintInteractionInput::Clear);
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
    model.handle(HintInteractionInput::Previous);
    assert_eq!(
        enter(&mut model),
        HintInteractionOutcome::Activate("a".into())
    );
}

#[test]
fn ambiguous_anchor_claims_can_remain_holes_with_independent_ordinary_fallback() {
    let assignments = (0..3)
        .map(|slot| HintSpecialAssignment {
            slot,
            binding: HintSpecialBinding::Unavailable(if slot == 2 {
                HintReservationReason::AnchorShadowed
            } else {
                HintReservationReason::Ambiguous
            }),
        })
        .collect();
    let mut model = model(vec![candidate(1)], &["1", "2", "3"], assignments);
    assert_eq!(labels(&model), vec![(1, "a".into())]);
    model.refresh(vec![candidate(1)]).unwrap();
    for code in ["1", "2", "3"] {
        model.handle(HintInteractionInput::Clear);
        assert_eq!(
            type_code(&mut model, code),
            HintInteractionOutcome::NoMatches
        );
        assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
    }
    model.handle(HintInteractionInput::Clear);
    assert_eq!(type_code(&mut model, "a"), HintInteractionOutcome::Updated);
    assert_eq!(
        enter(&mut model),
        HintInteractionOutcome::Activate("a".into())
    );
}

#[test]
fn prefixed_ordinary_retirement_uses_checked_rank_history() {
    let options = HintCodebookOptions {
        alphabet: "ba".into(),
        min_chars: 3,
        fallback_prefix: "12".into(),
        excluded_chars: BTreeSet::new(),
        page_source_chars: BTreeSet::new(),
        additional_controls: BTreeSet::new(),
        auto_follow: HintAutoFollow::Never,
    };
    let row = candidate(1);
    let mut model = HintInteraction::new_enhanced(
        vec![row.clone()],
        CheckedHintCodebook::compile(options, vec![]).unwrap(),
        vec![],
        HintRetentionBudget::Unlimited,
    )
    .unwrap();
    assert_eq!(labels(&model), vec![(1, "12b".into())]);
    let mut changed = row;
    changed.href = Some("https://example.test/new".into());
    model.refresh(vec![changed]).unwrap();
    assert_eq!(labels(&model), vec![(1, "12a".into())]);
    assert_eq!(
        type_code(&mut model, "12b"),
        HintInteractionOutcome::NoMatches
    );
    assert_eq!(
        model.snapshot().unavailable_reason,
        Some(HintSelectionUnavailable::RetiredLabel)
    );
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
}

#[test]
fn exact_logical_retention_budget_is_explicit_and_failed_admission_does_not_allocate_codes() {
    let row = candidate(1);
    let baseline = model(vec![row.clone()], &[], vec![])
        .retained_payload_bytes()
        .unwrap();
    let make = |budget| {
        HintInteraction::new_enhanced(
            vec![row.clone()],
            book("abc", &[], HintAutoFollow::FullMatch),
            vec![],
            budget,
        )
    };
    assert!(matches!(
        make(HintRetentionBudget::Bytes(baseline - 1)),
        Err(HintEnhancedError::Candidate(
            HintError::RetainedStateExhausted { .. }
        ))
    ));
    let mut model = make(HintRetentionBudget::Bytes(baseline)).unwrap();
    assert_eq!(model.retained_payload_bytes(), Some(baseline));
    assert!(matches!(
        model.refresh(vec![row.clone(), candidate(2)]),
        Err(HintError::RetainedStateExhausted { .. })
    ));
    assert_eq!(model.ordinary_codes_remaining(), Some(2));
    assert_eq!(model.retained_payload_bytes(), Some(baseline));
    assert_eq!(labels(&model), vec![(1, "a".into())]);
    assert_eq!(type_code(&mut model, "a"), HintInteractionOutcome::Updated);
    assert_eq!(enter(&mut model), HintInteractionOutcome::NoMatches);
    assert_eq!(
        model.snapshot().refresh_issue,
        Some(HintRefreshIssue::RetainedStateExhausted)
    );
    model.refresh(vec![row]).unwrap();
    model.handle(HintInteractionInput::Clear);
    model.handle(HintInteractionInput::Next);
    assert_eq!(
        enter(&mut model),
        HintInteractionOutcome::Activate("a".into())
    );
    assert_eq!(
        model.retention_budget(),
        Some(HintRetentionBudget::Bytes(baseline))
    );
}

#[test]
fn resource_rejection_keeps_negative_facts_and_does_not_resurrect_special_ownership() {
    let a = candidate(1);
    let b = candidate(2);
    let build = |budget| {
        HintInteraction::new_enhanced(
            vec![a.clone(), b.clone()],
            book("abc", &["1"], HintAutoFollow::Never),
            vec![live(0, &a)],
            budget,
        )
        .unwrap()
    };
    let bytes = build(HintRetentionBudget::Unlimited)
        .retained_payload_bytes()
        .unwrap();
    let mut model = build(HintRetentionBudget::Bytes(bytes));
    assert!(model.refresh(vec![b.clone(), candidate(3)]).is_err());
    assert_eq!(
        model.reservations()[0].state(),
        HintReservationState::Retired(HintReservationReason::Missing)
    );
    assert_eq!(model.snapshot().total, 1);
    assert_eq!(model.ordinary_codes_remaining(), Some(2));
    model.refresh(vec![a, b]).unwrap();
    assert_eq!(labels(&model), vec![(1, "b".into()), (2, "a".into())]);
    assert_eq!(
        model.reservations()[0].state(),
        HintReservationState::Retired(HintReservationReason::Missing)
    );
}

#[test]
fn failed_positive_refresh_still_remembers_later_observed_ordinary_meaning_changes() {
    let a = candidate(1);
    let b = candidate(2);
    let initial = model(vec![a.clone(), b.clone()], &[], vec![]);
    let budget = HintRetentionBudget::Bytes(initial.retained_payload_bytes().unwrap());
    let mut model = HintInteraction::new_enhanced(
        vec![a.clone(), b.clone()],
        book("abc", &[], HintAutoFollow::Never),
        vec![],
        budget,
    )
    .unwrap();
    let mut changed = b.clone();
    changed.href = Some("https://example.test/changed".into());
    // New candidate0 sorts before the observed changed meaning of candidate2.
    assert!(
        model
            .refresh(vec![candidate(0), a.clone(), changed])
            .is_err()
    );
    assert_eq!(model.snapshot().active_label, None);
    assert_eq!(model.ordinary_codes_remaining(), Some(1));
    model.refresh(vec![a, b]).unwrap();
    assert_eq!(labels(&model), vec![(1, "a".into()), (2, "c".into())]);
    assert_eq!(
        model.handle(HintInteractionInput::SelectLabel("b".into())),
        HintInteractionOutcome::NoMatches
    );
    assert_eq!(
        model.snapshot().unavailable_reason,
        Some(HintSelectionUnavailable::RetiredLabel)
    );
}

#[test]
fn physical_owner_and_related_identity_keep_consumption_separate() {
    let a = candidate(1);
    let mut b = a.clone();
    b.frame_owner = "other".into();
    let mut c = a.clone();
    c.related_element_id = Some(2);
    let mut model = model(vec![a.clone(), b.clone(), c.clone()], &[], vec![]);
    let original = model
        .hints()
        .iter()
        .map(|hint| (hint.candidate.clone(), hint.label.clone()))
        .collect::<Vec<_>>();
    model.consume(&a);
    model.refresh(vec![a, b, c]).unwrap();
    assert_eq!(model.hints().len(), 2);
    for hint in model.hints() {
        assert!(
            original
                .iter()
                .any(|(candidate, label)| candidate == &hint.candidate && label == &hint.label)
        );
    }
}

#[test]
fn maximum_initial_ordinary_count_and_url_payload_do_not_inherit_a_hidden_lifetime_cap() {
    let href = format!(
        "https://example.test/{}",
        "x".repeat(8192 - "https://example.test/".len())
    );
    let rows = (0..5000)
        .map(|id| {
            let mut row = candidate(id);
            row.href = Some(href.clone());
            row
        })
        .collect();
    let model = HintInteraction::new_enhanced(
        rows,
        book(
            "abcdefghijklmnopqrstuvwxyz012345",
            &[],
            HintAutoFollow::Never,
        ),
        vec![],
        HintRetentionBudget::Unlimited,
    )
    .unwrap();
    assert_eq!(model.hints().len(), 5000);
    assert_eq!(model.ordinary_suffix_width(), Some(3));
    assert!(model.retained_payload_bytes().unwrap() >= 5000 * 8192);
    assert_eq!(
        model.retention_budget(),
        Some(HintRetentionBudget::Unlimited)
    );
}
