//! Independent small finite-state model: scalar identities/meanings and four
//! literal labels, without production trie, binding or retirement helpers.
use super::{candidate, enter, labels, live};
use ferric_browser_core::{
    CheckedHintCodebook, HintAutoFollow, HintCandidate, HintCodebookOptions, HintInteraction,
    HintInteractionInput, HintInteractionKind, HintInteractionOutcome, HintRefreshIssue,
    HintReservationReason, HintReservationState, HintRetentionBudget,
};
use std::collections::{BTreeMap, BTreeSet};

const WORDS: [&str; 4] = ["aa", "ab", "ba", "bb"];

#[derive(Clone, Copy, Debug)]
struct Fact {
    alternate: bool,
    rank: usize,
    consumed: bool,
    retired: bool,
}
#[derive(Clone, Copy, Debug)]
struct Row {
    id: u32,
    alternate: bool,
    rank: usize,
}
struct Reference {
    facts: BTreeMap<u32, Fact>,
    rows: Vec<Row>,
    owners: Vec<(u32, bool)>,
    exhausted: bool,
}

fn input(id: u32, alternate: bool) -> HintCandidate {
    let mut candidate = candidate(id);
    candidate.href = Some(format!("https://example.test/{id}/{}", u8::from(alternate)));
    candidate
}
fn namespace() -> CheckedHintCodebook {
    CheckedHintCodebook::compile(
        HintCodebookOptions {
            alphabet: "ab".into(),
            min_chars: 2,
            fallback_prefix: String::new(),
            excluded_chars: BTreeSet::new(),
            page_source_chars: BTreeSet::new(),
            additional_controls: BTreeSet::new(),
            auto_follow: HintAutoFollow::Never,
        },
        vec![],
    )
    .unwrap()
}

impl Reference {
    fn initial() -> Self {
        Self {
            facts: BTreeMap::from([(
                1,
                Fact {
                    alternate: false,
                    rank: 0,
                    consumed: false,
                    retired: false,
                },
            )]),
            rows: vec![Row {
                id: 1,
                alternate: false,
                rank: 0,
            }],
            owners: vec![(1, false)],
            exhausted: false,
        }
    }
    fn refresh(&mut self, incoming: &[(u32, bool)]) {
        let mut rows = Vec::new();
        self.exhausted = false;
        for &(id, alternate) in incoming {
            if let Some(previous) = self.facts.get(&id) {
                if previous.consumed {
                    continue;
                }
                if !previous.retired && previous.alternate == alternate {
                    rows.push(Row {
                        id,
                        alternate,
                        rank: previous.rank,
                    });
                    continue;
                }
                self.facts.get_mut(&id).unwrap().retired = true;
            }
            if self.owners.len() == WORDS.len() {
                self.exhausted = true;
                continue;
            }
            let rank = self.owners.len();
            self.owners.push((id, alternate));
            self.facts.insert(
                id,
                Fact {
                    alternate,
                    rank,
                    consumed: false,
                    retired: false,
                },
            );
            rows.push(Row {
                id,
                alternate,
                rank,
            });
        }
        self.rows = rows;
    }
    fn consume_captured(&mut self, id: u32) {
        if let Some(fact) = self.facts.get_mut(&id) {
            fact.consumed = true;
        }
        self.rows.retain(|row| row.id != id);
    }
}

fn verify(actual: &HintInteraction, reference: &Reference, sequence: usize, step: usize) {
    let expected: Vec<_> = reference
        .rows
        .iter()
        .map(|row| (row.id, WORDS[row.rank].to_owned()))
        .collect();
    assert_eq!(labels(actual), expected, "sequence={sequence}, step={step}");
    assert_eq!(actual.ordinary_suffix_width(), Some(2));
    assert_eq!(
        actual.ordinary_codes_remaining(),
        Some(u64::try_from(WORDS.len() - reference.owners.len()).unwrap())
    );
    assert_eq!(
        actual.snapshot().refresh_issue,
        reference
            .exhausted
            .then_some(HintRefreshIssue::NamespaceExhausted)
    );
    assert_eq!(actual.snapshot().active_label, None);
    assert_eq!(actual.initial_activation(), None);
    for hint in actual.hints() {
        let rank = WORDS.iter().position(|word| *word == hint.label).unwrap();
        let (owner, meaning) = reference.owners[rank];
        let current = reference.rows.iter().find(|row| row.rank == rank).unwrap();
        assert_eq!((current.id, current.alternate), (owner, meaning));
        assert_eq!(
            hint.candidate,
            input(owner, meaning),
            "retargeted code in sequence={sequence}, step={step}"
        );
    }
    for word in WORDS {
        let mut probe = actual.clone();
        probe.handle(HintInteractionInput::Clear);
        probe.handle(HintInteractionInput::SelectLabel(word.into()));
        let outcome = enter(&mut probe);
        assert_eq!(
            outcome,
            if expected.iter().any(|(_, label)| label == word) {
                HintInteractionOutcome::Activate(word.into())
            } else {
                HintInteractionOutcome::NoMatches
            },
            "sequence={sequence}, step={step}, code={word}"
        );
    }
}

#[test]
fn exhaustive_refresh_and_consume_sequences_preserve_exact_ordinary_ownership() {
    // Nine snapshots (absent/original/alternate for each of two identities)
    // and two consume operations. Every length-four sequence is exercised.
    for sequence in 0..11_usize.pow(4) {
        let mut actual = HintInteraction::new_enhanced(
            vec![input(1, false)],
            namespace(),
            vec![],
            HintRetentionBudget::Unlimited,
        )
        .unwrap();
        let mut reference = Reference::initial();
        let mut remaining = sequence;
        for step in 0..4 {
            let operation = remaining % 11;
            remaining /= 11;
            if operation < 9 {
                let incoming: Vec<_> = [(1, operation % 3), (2, operation / 3)]
                    .into_iter()
                    .filter_map(|(id, state)| (state != 0).then_some((id, state == 2)))
                    .collect();
                actual
                    .refresh(
                        incoming
                            .iter()
                            .map(|(id, meaning)| input(*id, *meaning))
                            .collect(),
                    )
                    .unwrap();
                reference.refresh(&incoming);
            } else {
                let id = if operation == 9 { 1 } else { 2 };
                // Captured original facts may complete after disappearing or
                // being replaced by a new meaning for this physical identity.
                reference.consume_captured(id);
                actual.consume(&input(id, false));
            }
            verify(&actual, &reference, sequence, step);
        }
    }
}

#[test]
fn exhaustive_special_fates_are_monotonic_and_missing_slots_never_become_live() {
    let a = candidate(1);
    let b = candidate(2);
    for sequence in 0..8_usize.pow(4) {
        let mut model = HintInteraction::new_enhanced(
            vec![a.clone(), b.clone()],
            super::book("ab", &["1", "2"], HintAutoFollow::Never),
            vec![live(0, &a)],
            HintRetentionBudget::Unlimited,
        )
        .unwrap();
        let mut previous = HintReservationState::Live;
        let mut remaining = sequence;
        for _ in 0..4 {
            match remaining % 8 {
                0 => model.refresh(vec![a.clone(), b.clone()]).unwrap(),
                1 => model.refresh(vec![b.clone()]).unwrap(),
                2 => {
                    let mut changed = a.clone();
                    changed.href = Some("https://changed.test/".into());
                    model.refresh(vec![changed, b.clone()]).unwrap();
                }
                3 => model.consume(&a),
                4 => {
                    model.retire_reservation(0, HintReservationReason::Stale);
                }
                5 => model.refresh(vec![]).unwrap(),
                6 => model.consume(&b),
                _ => assert!(!model.retire_reservation(1, HintReservationReason::Stale)),
            }
            remaining /= 8;
            let current = model.reservations()[0].state();
            if previous != HintReservationState::Live {
                assert_eq!(current, previous);
            }
            assert_eq!(
                model.reservations()[1].state(),
                HintReservationState::Unavailable(HintReservationReason::Missing)
            );
            assert_eq!(
                model.hints().iter().any(|hint| hint.label == "1"),
                current == HintReservationState::Live
            );
            assert_eq!(
                model.interaction_kind(),
                if model.hints().is_empty() {
                    HintInteractionKind::UnavailableOnly
                } else {
                    HintInteractionKind::Enhanced
                }
            );
            let mut probe = model.clone();
            probe.handle(HintInteractionInput::Clear);
            probe.handle(HintInteractionInput::SelectLabel("2".into()));
            assert_eq!(enter(&mut probe), HintInteractionOutcome::NoMatches);
            assert_eq!(model.snapshot().active_label, None);
            previous = current;
        }
    }
}
