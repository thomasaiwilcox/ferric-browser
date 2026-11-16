//! Normalize complete raw searches using browser-owned canonical predicates.
//! Reference validation precedes filtering. These are checked facts, not codes,
//! ordinal ranks, current native receipts or action permits.
use super::{Checker, Classification, NormalizationWork};
use crate::interaction_proof::{
    AnnotationError as E, AnnotationRequest,
    bounded::List,
    wire::{
        Fact, Item, ObservedItem, ObservedPrimary, Outcome, ResultFacts, Root, RootKind,
        UnavailableReason,
    },
};
use ferric_browser_config::{
    CompiledInteractionAttributeTest, CompiledInteractionPredicates, InteractionCollection,
    InteractionPredicates,
};
use ferric_browser_core::HintCandidate;
use std::collections::{BTreeMap, BTreeSet};

fn unavailable(reason: UnavailableReason) -> ResultFacts {
    ResultFacts::Unavailable { reason }
}

pub(super) fn anchor_structure(
    checker: &mut Checker<'_>,
    outcome: &Outcome,
    request: &AnnotationRequest,
    candidates: &[HintCandidate],
) -> Result<(), E> {
    let ResultFacts::AnchorObserved {
        root,
        scope,
        scope_kind,
        targets,
    } = &outcome.result
    else {
        return Err(E::InvalidInventory);
    };
    let authored = request
        .policy
        .anchors()
        .iter()
        .find(|anchor| anchor.id == outcome.rule.id)
        .ok_or(E::InvalidInventory)?;
    let path = checker.path(*root, authored.root.as_ref())?;
    checker.typed_node(*scope, *scope_kind)?;
    if scope.document != path.resolved.document
        || (authored.scope_selector.is_none()
            && (*scope != path.resolved || *scope_kind != path.resolved_kind))
        || (authored.scope_selector.is_some() && *scope_kind != RootKind::Element)
    {
        return Err(E::InvalidAssociation);
    }
    let mut seen = BTreeSet::new();
    for witness in targets.iter() {
        let index = checker.witness_structure(
            witness,
            path,
            authored.predicates.as_ref(),
            candidates,
            true,
        )?;
        if !seen.insert(index) {
            return Err(E::InvalidAssociation);
        }
    }
    Ok(())
}

pub(super) fn anchor(
    checker: &mut NormalizationWork,
    outcome: &Outcome,
    request: &AnnotationRequest,
    candidates: &[HintCandidate],
) -> Result<ResultFacts, E> {
    let ResultFacts::AnchorObserved {
        root,
        scope,
        scope_kind,
        targets,
    } = &outcome.result
    else {
        return Err(E::InvalidInventory);
    };
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
    let mut matches = Vec::new();
    let mut failure = None;
    for witness in targets.iter() {
        match checker.classify_witness(
            witness,
            &candidates[usize::from(witness.candidate.index)],
            &compiled,
        )? {
            Classification::Match => matches.push(witness.clone()),
            Classification::NoMatch => {}
            Classification::Unavailable(reason) => {
                failure.get_or_insert(reason);
            }
        }
    }
    if let Some(reason) = failure {
        return Ok(unavailable(reason));
    }
    Ok(ResultFacts::AnchorComplete {
        root: *root,
        scope: *scope,
        scope_kind: *scope_kind,
        matches: List::from_vec(matches).ok_or(E::WorkLimit)?,
    })
}

fn primary_structure(
    checker: &mut Checker<'_>,
    item: &ObservedItem,
    outer: &Root,
    authored: &InteractionCollection,
    candidates: &[HintCandidate],
) -> Result<(), E> {
    let ObservedPrimary::TargetsComplete { root, targets } = &item.primary else {
        return Ok(());
    };
    let path = checker.root(*root)?;
    if path.start != item.logical_node
        || path.start_kind != RootKind::Element
        || checker.path_base_depth(outer, authored.root.as_ref())?
            + outer.hops.len()
            + path.hops.len()
            > 8
    {
        return Err(E::InvalidRoot);
    }
    checker.steps(
        path,
        authored.primary_root_steps.as_deref().unwrap_or_default(),
    )?;
    if authored.primary_self == Some(true) && path.resolved_kind != RootKind::Element {
        return Err(E::InvalidRoot);
    }
    let mut seen = BTreeSet::new();
    for witness in targets.iter() {
        let index = checker.witness_structure(
            witness,
            path,
            authored.primary_predicates.as_ref(),
            candidates,
            true,
        )?;
        if !seen.insert(index)
            || (authored.primary_self == Some(true)
                && witness.candidate.element_id != path.resolved.node)
        {
            return Err(E::InvalidAssociation);
        }
    }
    Ok(())
}

fn identity(item: &ObservedItem) -> Classification {
    for attribute in item.identity_attributes.iter() {
        match attribute.fact {
            Fact::CompletePresent { .. } => {}
            Fact::Truncated {} => return Classification::Unavailable(UnavailableReason::Truncated),
            Fact::Missing {} | Fact::CompleteAbsent {} => {
                return Classification::Unavailable(UnavailableReason::MissingEvidence);
            }
        }
    }
    Classification::Match
}

pub(super) fn collection_structure(
    checker: &mut Checker<'_>,
    outcome: &Outcome,
    request: &AnnotationRequest,
    candidates: &[HintCandidate],
) -> Result<(), E> {
    let ResultFacts::CollectionObserved {
        root,
        container,
        container_kind,
        items,
    } = &outcome.result
    else {
        return Err(E::InvalidInventory);
    };
    let authored = request
        .policy
        .collections()
        .iter()
        .find(|collection| collection.id == outcome.rule.id)
        .ok_or(E::InvalidInventory)?;
    let outer = checker.path(*root, authored.root.as_ref())?;
    checker.typed_node(*container, *container_kind)?;
    if container.document != outer.resolved.document
        || (authored.container_self == Some(true)
            && (*container != outer.resolved || *container_kind != outer.resolved_kind))
        || (authored.container_self != Some(true) && *container_kind != RootKind::Element)
    {
        return Err(E::InvalidAssociation);
    }
    let attribute_tests = authored.item_attributes.as_deref().unwrap_or_default();
    let attribute_names = attribute_tests
        .iter()
        .map(|test| test.name.as_str())
        .collect();
    let identity_names = authored
        .identity_attributes
        .iter()
        .flatten()
        .map(String::as_str)
        .collect();
    let mut seen_items = BTreeSet::new();
    // Logical items are emitted in DOM traversal order, so an ancestor must
    // already exist. This also rejects cycles without an unbounded graph walk.
    // Validate all targets even when the item will be excluded by attributes.
    for item in items.iter() {
        checker.record(1)?;
        checker.typed_node(item.logical_node, RootKind::Element)?;
        if item.logical_node.document != container.document
            || seen_items.contains(&item.logical_node)
        {
            return Err(E::InvalidAssociation);
        }
        if let Some(parent) = item.parent_item {
            checker.typed_node(parent, RootKind::Element)?;
            if parent.document != item.logical_node.document || !seen_items.contains(&parent) {
                return Err(E::InvalidAssociation);
            }
        }
        seen_items.insert(item.logical_node);
        checker.attributes(&item.attributes, &attribute_names)?;
        checker.attributes(&item.identity_attributes, &identity_names)?;
        primary_structure(checker, item, outer, authored, candidates)?;
    }
    Ok(())
}

pub(super) fn collection(
    checker: &mut NormalizationWork,
    outcome: &Outcome,
    request: &AnnotationRequest,
    candidates: &[HintCandidate],
) -> Result<ResultFacts, E> {
    let ResultFacts::CollectionObserved {
        root,
        container,
        container_kind,
        items,
    } = &outcome.result
    else {
        return Err(E::InvalidInventory);
    };
    let authored = request
        .policy
        .collections()
        .iter()
        .find(|collection| collection.id == outcome.rule.id)
        .ok_or(E::InvalidInventory)?;
    let attribute_tests = authored.item_attributes.as_deref().unwrap_or_default();
    let compiled_attributes = attribute_tests
        .iter()
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
    let mut retained = Vec::new();
    let mut failure = None;
    let mut primaries = BTreeSet::new();
    for item in items.iter() {
        match checker.classify_attributes(&item.attributes, &compiled_attributes)? {
            Classification::NoMatch => continue,
            Classification::Unavailable(reason) => {
                failure.get_or_insert(reason);
                continue;
            }
            Classification::Match => {}
        }
        let (primary_root, targets) = match &item.primary {
            ObservedPrimary::TargetsComplete { root, targets } => (root, targets),
            ObservedPrimary::Unavailable { reason } => {
                failure.get_or_insert(*reason);
                continue;
            }
        };
        let mut selected = None;
        for witness in targets.iter() {
            match checker.classify_witness(
                witness,
                &candidates[usize::from(witness.candidate.index)],
                &compiled,
            )? {
                Classification::NoMatch => {}
                Classification::Unavailable(reason) => {
                    failure.get_or_insert(reason);
                }
                Classification::Match => {
                    if selected.is_some() {
                        failure.get_or_insert(UnavailableReason::AmbiguousPrimary);
                    } else {
                        selected = Some(witness);
                    }
                }
            }
        }
        let Some(primary) = selected else {
            continue;
        };
        if let Classification::Unavailable(reason) = identity(item) {
            failure.get_or_insert(reason);
            continue;
        }
        if !primaries.insert(primary.candidate.index) {
            failure.get_or_insert(UnavailableReason::AmbiguousPrimary);
        }
        retained.push(Item {
            logical_node: item.logical_node,
            primary_root: *primary_root,
            primary: primary.clone(),
            attributes: item.attributes.clone(),
            identity_attributes: item.identity_attributes.clone(),
        });
    }
    let kept: BTreeSet<_> = retained.iter().map(|item| item.logical_node).collect();
    let mut has_kept_ancestor = BTreeMap::new();
    for item in items.iter() {
        checker.charge(1)?;
        let ancestor = item.parent_item.is_some_and(|parent| {
            kept.contains(&parent) || has_kept_ancestor.get(&parent) == Some(&true)
        });
        if ancestor && kept.contains(&item.logical_node) {
            failure.get_or_insert(UnavailableReason::NestedItems);
        }
        has_kept_ancestor.insert(item.logical_node, ancestor);
    }
    if let Some(reason) = failure {
        return Ok(unavailable(reason));
    }
    Ok(ResultFacts::CollectionComplete {
        root: *root,
        container: *container,
        container_kind: *container_kind,
        items: List::from_vec(retained).ok_or(E::WorkLimit)?,
    })
}
