use ferric_browser_core::{
    HintAutoFollow, HintCandidate, HintGeometry, HintInteraction, HintInteractionInput as Input,
    HintInteractionOutcome as Outcome, HintKind, HintValidationError as Error,
    HintValidationStatus as Status, HintValidationUpdate as Update, assign_labels_with_options,
};
use std::collections::BTreeSet;

fn candidate(id: u32) -> HintCandidate {
    HintCandidate {
        element_id: id,
        related_element_id: None,
        frame_owner: String::new(),
        frame_path: "0".into(),
        kind: HintKind::Link,
        text: format!("Target {id}"),
        href: Some(format!("https://example.test/{id}")),
        presentation: None,
        geometry: HintGeometry {
            x: f64::from(id),
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
    }
}
fn model(follow: HintAutoFollow) -> HintInteraction {
    let labels = assign_labels_with_options(vec![candidate(1), candidate(2)], "ab", 1).unwrap();
    let mut model = HintInteraction::new(labels, "ab".into(), follow);
    model.begin_hint_validation().unwrap();
    model
}
fn valid(label: &str, id: u32) -> Update {
    Update {
        label: label.into(),
        candidate: Some(candidate(id)),
    }
}

#[test]
fn explicit_early_selection_waits_for_its_exact_binding_and_is_returned_once() {
    let mut m = model(HintAutoFollow::FullMatch);
    assert!(!m.hint_selection_is_validated("a"));
    assert_eq!(m.handle(Input::Character('a')), Outcome::Updated);
    assert_eq!(m.pending_hint_selection(), Some("a"));
    assert_eq!(m.handle(Input::Activate), Outcome::Updated);
    assert_eq!(
        m.apply_hint_validation(1, &[valid("b", 2)], false),
        Ok(None)
    );
    assert_eq!(m.pending_hint_selection(), Some("a"));
    assert_eq!(
        m.apply_hint_validation(2, &[valid("a", 1)], true),
        Ok(Some("a".into()))
    );
    assert!(m.hint_selection_is_validated("a"));
    assert_eq!(m.pending_hint_selection(), None);
    assert_eq!(m.apply_hint_validation(2, &[], true), Err(Error::Closed));
}

#[test]
fn invalid_batch_is_atomic_and_does_not_advance_the_sequence() {
    let mut m = model(HintAutoFollow::Never);
    let before = m.clone();
    let mut wrong = candidate(2);
    wrong.href = Some("https://example.test/replacement".into());
    assert_eq!(
        m.apply_hint_validation(
            1,
            &[
                valid("a", 1),
                Update {
                    label: "b".into(),
                    candidate: Some(wrong),
                }
            ],
            true
        ),
        Err(Error::InvalidUpdate)
    );
    assert_eq!(m, before);
    assert_eq!(
        m.apply_hint_validation(2, &[valid("a", 1)], false),
        Err(Error::InvalidSequence)
    );
    assert_eq!(m, before);
    assert_eq!(
        m.apply_hint_validation(1, &[valid("a", 1)], true),
        Err(Error::Incomplete)
    );
    assert_eq!(m, before);
    assert_eq!(
        m.apply_hint_validation(1, &[valid("a", 1), valid("a", 1)], false),
        Err(Error::InvalidUpdate)
    );
    assert_eq!(m, before);
    assert_eq!(
        m.apply_hint_validation(1, &[valid("a", 1)], false),
        Ok(None)
    );
}

#[test]
fn withdrawn_proposal_never_receives_a_substitute_action() {
    let mut m = model(HintAutoFollow::FullMatch);
    m.handle(Input::Character('a'));
    assert_eq!(
        m.apply_hint_validation(
            1,
            &[Update {
                label: "a".into(),
                candidate: None
            }],
            false
        ),
        Ok(None)
    );
    assert_eq!(m.hint_validation_status("a"), Some(Status::Withdrawn));
    assert_eq!(m.pending_hint_selection(), None);
    assert!(!m.hint_selection_is_validated("a"));
    assert_eq!(m.snapshot().active_label, None);
    assert_eq!(m.handle(Input::Activate), Outcome::NoMatches);
    assert_eq!(m.hints()[0].label, "b");
    m.apply_hint_validation(2, &[valid("b", 2)], true).unwrap();
    m.refresh(vec![candidate(2), candidate(3)]).unwrap();
    assert!(!m.hints().iter().any(|h| h.label == "a"));
}

#[test]
fn editing_and_request_retirement_cancel_pending_selection() {
    for input in [
        Input::Backspace,
        Input::Clear,
        Input::Next,
        Input::Previous,
        Input::EnterTextMode,
        Input::SetText("Target".into()),
    ] {
        let mut m = model(HintAutoFollow::FullMatch);
        m.handle(Input::Character('a'));
        m.handle(input);
        assert_eq!(m.pending_hint_selection(), None);
        assert_eq!(
            m.apply_hint_validation(1, &[valid("a", 1), valid("b", 2)], true),
            Ok(None)
        );
    }
    let mut m = model(HintAutoFollow::FullMatch);
    m.handle(Input::Character('a'));
    assert_eq!(m.handle(Input::Cancel), Outcome::Cancel);
    assert!(!m.hint_selection_is_validated("a"));
    assert_eq!(
        m.apply_hint_validation(1, &[valid("a", 1)], false),
        Err(Error::Closed)
    );
    assert_eq!(m.handle(Input::Activate), Outcome::NoMatches);
}

#[test]
fn background_completion_does_not_auto_follow_or_replay_input() {
    for follow in [
        HintAutoFollow::Always,
        HintAutoFollow::UniqueMatch,
        HintAutoFollow::FullMatch,
    ] {
        let mut m = model(follow);
        assert_eq!(m.initial_activation(), None);
        m.apply_hint_validation(
            1,
            &[Update {
                label: "b".into(),
                candidate: None,
            }],
            false,
        )
        .unwrap();
        assert_eq!(m.apply_hint_validation(2, &[valid("a", 1)], true), Ok(None));
        assert_eq!(m.initial_activation(), None);
    }
}

#[test]
fn identity_geometry_and_non_link_meaning_are_checked() {
    for changed in [
        {
            let mut c = candidate(1);
            c.element_id = 99;
            c
        },
        {
            let mut c = candidate(1);
            c.related_element_id = Some(3);
            c
        },
        {
            let mut c = candidate(1);
            c.frame_owner = "other".into();
            c
        },
        {
            let mut c = candidate(1);
            c.geometry.width = f64::NAN;
            c
        },
    ] {
        let mut m = model(HintAutoFollow::Never);
        assert_eq!(
            m.apply_hint_validation(
                1,
                &[Update {
                    label: "a".into(),
                    candidate: Some(changed)
                }],
                false
            ),
            Err(Error::InvalidUpdate)
        );
        assert_eq!(m.hint_validation_status("a"), Some(Status::Provisional));
    }
    let mut c = candidate(1);
    c.kind = HintKind::Button;
    c.href = None;
    let mut m = HintInteraction::new(
        assign_labels_with_options(vec![c.clone()], "ab", 1).unwrap(),
        "ab".into(),
        HintAutoFollow::Never,
    );
    m.begin_hint_validation().unwrap();
    c.text = "Different action".into();
    assert_eq!(
        m.apply_hint_validation(
            1,
            &[Update {
                label: "a".into(),
                candidate: Some(c)
            }],
            true
        ),
        Err(Error::InvalidUpdate)
    );
}

fn enhanced() -> HintInteraction {
    use ferric_browser_core::{
        CheckedHintCodebook, HintCodeDeclaration, HintCodebookOptions, HintReservationCategory,
        HintRetentionBudget,
    };
    let book = CheckedHintCodebook::compile(
        HintCodebookOptions {
            alphabet: "ab".into(),
            min_chars: 1,
            fallback_prefix: String::new(),
            excluded_chars: BTreeSet::default(),
            page_source_chars: BTreeSet::default(),
            additional_controls: BTreeSet::default(),
            auto_follow: HintAutoFollow::FullMatch,
        },
        vec![HintCodeDeclaration {
            category: HintReservationCategory::Collection,
            rule_id: "videos".into(),
            slot: 0,
            code: "1".into(),
        }],
    )
    .unwrap();
    HintInteraction::new_provisional_enhanced(
        vec![candidate(1), candidate(2)],
        book,
        vec![],
        2,
        HintRetentionBudget::Bytes(4096),
    )
    .unwrap()
}

#[test]
fn initial_pending_slot_settles_once_without_relabeling_ordinary_targets() {
    use ferric_browser_core::{HintReservationState, HintSpecialAssignment, HintSpecialBinding};
    let mut m = enhanced();
    assert_eq!(
        m.reservations()[0].state(),
        HintReservationState::PendingInitial
    );
    let before = m.clone();
    // An already represented ordinary target cannot acquire a second code.
    assert!(
        m.settle_initial_hint_reservations(
            1,
            vec![HintSpecialAssignment {
                slot: 0,
                binding: HintSpecialBinding::Live(candidate(1)),
            }]
        )
        .is_err()
    );
    assert_eq!(m, before);
    m.settle_initial_hint_reservations(
        1,
        vec![HintSpecialAssignment {
            slot: 0,
            binding: HintSpecialBinding::Live(candidate(3)),
        }],
    )
    .unwrap();
    assert_eq!(
        m.hints()
            .iter()
            .map(|h| h.label.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "1"]
    );
    assert_eq!(m.hint_validation_status("1"), Some(Status::Provisional));
    m.handle(Input::Character('1'));
    assert_eq!(m.pending_hint_selection(), Some("1"));
    assert_eq!(
        m.apply_hint_validation(2, &[valid("a", 1), valid("b", 2), valid("1", 3)], true),
        Ok(Some("1".into()))
    );
    assert!(
        m.settle_initial_hint_reservations(
            3,
            vec![HintSpecialAssignment {
                slot: 0,
                binding: HintSpecialBinding::Live(candidate(4)),
            }]
        )
        .is_err()
    );
}

#[test]
fn initially_missing_slot_stays_unavailable_and_completion_requires_settlement() {
    use ferric_browser_core::{
        HintReservationReason, HintReservationState, HintSpecialAssignment, HintSpecialBinding,
    };
    let mut m = enhanced();
    assert_eq!(
        m.apply_hint_validation(1, &[valid("a", 1), valid("b", 2)], true),
        Err(Error::Incomplete)
    );
    m.settle_initial_hint_reservations(
        1,
        vec![HintSpecialAssignment {
            slot: 0,
            binding: HintSpecialBinding::Unavailable(HintReservationReason::Missing),
        }],
    )
    .unwrap();
    let before = m.clone();
    assert!(
        m.settle_initial_hint_reservations(
            2,
            vec![HintSpecialAssignment {
                slot: 0,
                binding: HintSpecialBinding::Live(candidate(3)),
            }]
        )
        .is_err()
    );
    assert_eq!(m, before);
    m.apply_hint_validation(2, &[valid("a", 1), valid("b", 2)], true)
        .unwrap();
    m.refresh(vec![candidate(1), candidate(2), candidate(3)])
        .unwrap();
    assert_eq!(
        m.reservations()[0].state(),
        HintReservationState::Unavailable(HintReservationReason::Missing)
    );
    assert!(!m.hints().iter().any(|h| h.label == "1"));
}

#[test]
fn full_size_batch_settles_without_losing_targets_or_reallocating_codes() {
    let hints =
        assign_labels_with_options((1..=5000).map(candidate).collect(), "asdfghjkl", 1).unwrap();
    let updates: Vec<_> = hints
        .iter()
        .map(|h| Update {
            label: h.label.clone(),
            candidate: Some(h.candidate.clone()),
        })
        .collect();
    let mut m = HintInteraction::new(hints.clone(), "asdfghjkl".into(), HintAutoFollow::Never);
    m.begin_hint_validation().unwrap();
    assert_eq!(m.apply_hint_validation(1, &updates, true), Ok(None));
    assert_eq!(m.hints(), hints);
    assert!(!m.validation_in_progress());
}

#[test]
fn initialization_cannot_reopen_consumed_or_already_read_bindings() {
    let labels = assign_labels_with_options(vec![candidate(1), candidate(2)], "ab", 1).unwrap();
    let mut m = HintInteraction::new(labels.clone(), "ab".into(), HintAutoFollow::Never);
    m.consume(&candidate(1));
    assert_eq!(m.begin_hint_validation(), Err(Error::InvalidInitialState));
    let mut m = HintInteraction::new(labels, "ab".into(), HintAutoFollow::Never);
    m.handle(Input::Character('a'));
    assert_eq!(m.begin_hint_validation(), Err(Error::InvalidInitialState));
}

#[test]
fn initial_settlement_cannot_exceed_retained_budget_or_partially_commit() {
    use ferric_browser_core::{
        HintEnhancedError, HintError, HintSpecialAssignment, HintSpecialBinding,
    };
    let mut m = enhanced();
    let before = m.clone();
    let mut oversized = candidate(3);
    oversized.href = Some(format!("https://example.test/{}", "x".repeat(5000)));
    assert!(matches!(
        m.settle_initial_hint_reservations(
            1,
            vec![HintSpecialAssignment {
                slot: 0,
                binding: HintSpecialBinding::Live(oversized),
            }]
        ),
        Err(HintEnhancedError::Candidate(
            HintError::RetainedStateExhausted { .. }
        ))
    ));
    assert_eq!(m, before);
    assert!(
        m.settle_initial_hint_reservations(
            2,
            vec![HintSpecialAssignment {
                slot: 0,
                binding: HintSpecialBinding::Live(candidate(3)),
            }]
        )
        .is_err()
    );
    assert_eq!(m, before);
}

#[test]
fn closing_a_completed_request_blocks_even_previously_validated_selections() {
    let mut m = model(HintAutoFollow::Never);
    m.apply_hint_validation(1, &[valid("a", 1), valid("b", 2)], true)
        .unwrap();
    assert!(m.hint_selection_is_validated("a"));
    m.close_hint_validation();
    assert!(!m.hint_selection_is_validated("a"));
    assert_eq!(m.handle(Input::Activate), Outcome::NoMatches);
}
