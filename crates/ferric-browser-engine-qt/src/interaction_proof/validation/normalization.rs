//! Canonical policy matching after the original graph has been checked.
//! This stage cannot change the request, candidates or native obligations.
use super::{E, NormalizationWork, discovery, require_match};
use crate::interaction_proof::{
    AnnotationRequest,
    wire::{Envelope, Outcome, ResultFacts, RuleKind},
};
use ferric_browser_config::{
    CompiledInteractionAttributeTest, CompiledInteractionPredicates, InteractionPredicates,
};
use ferric_browser_core::HintCandidate;

pub(in crate::interaction_proof) fn normalize(
    wire: &Envelope,
    request: &AnnotationRequest,
    candidates: &[HintCandidate],
    mut work: NormalizationWork,
) -> Result<Vec<Outcome>, E> {
    let mut outcomes = Vec::with_capacity(wire.outcomes.len());
    for outcome in wire.outcomes.iter() {
        let result = match (&outcome.rule.kind, &outcome.result) {
            (_, ResultFacts::Unavailable { .. } | ResultFacts::SearchComplete { .. }) => {
                outcome.result.clone()
            }
            (RuleKind::Anchor, ResultFacts::AnchorObserved { .. }) => {
                discovery::anchor(&mut work, outcome, request, candidates)?
            }
            (RuleKind::Collection, ResultFacts::CollectionObserved { .. }) => {
                discovery::collection(&mut work, outcome, request, candidates)?
            }
            (RuleKind::Anchor, ResultFacts::AnchorComplete { matches, .. }) => {
                let authored = request
                    .policy
                    .anchors()
                    .iter()
                    .find(|anchor| anchor.id == outcome.rule.id)
                    .ok_or(E::InvalidInventory)?;
                let compiled = CompiledInteractionPredicates::try_from(
                    authored
                        .predicates
                        .as_ref()
                        .unwrap_or(&InteractionPredicates::default()),
                )
                .map_err(|_| E::InvalidInventory)?;
                for witness in matches.iter() {
                    require_match(
                        work.classify_witness(
                            witness,
                            candidates
                                .get(usize::from(witness.candidate.index))
                                .ok_or(E::InvalidAssociation)?,
                            &compiled,
                        )?,
                    )?;
                }
                outcome.result.clone()
            }
            (RuleKind::Collection, ResultFacts::CollectionComplete { items, .. }) => {
                let authored = request
                    .policy
                    .collections()
                    .iter()
                    .find(|collection| collection.id == outcome.rule.id)
                    .ok_or(E::InvalidInventory)?;
                let attributes = authored
                    .item_attributes
                    .iter()
                    .flatten()
                    .map(CompiledInteractionAttributeTest::try_from)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| E::InvalidInventory)?;
                let compiled = CompiledInteractionPredicates::try_from(
                    authored
                        .primary_predicates
                        .as_ref()
                        .unwrap_or(&InteractionPredicates::default()),
                )
                .map_err(|_| E::InvalidInventory)?;
                for item in items.iter() {
                    require_match(work.classify_attributes(&item.attributes, &attributes)?)?;
                    require_match(
                        work.classify_witness(
                            &item.primary,
                            candidates
                                .get(usize::from(item.primary.candidate.index))
                                .ok_or(E::InvalidAssociation)?,
                            &compiled,
                        )?,
                    )?;
                }
                outcome.result.clone()
            }
            _ => return Err(E::InvalidInventory),
        };
        outcomes.push(Outcome {
            rule: outcome.rule.clone(),
            result,
        });
    }
    Ok(outcomes)
}
