//! Ordered collection views over the existing native-sealed graph. The retained
//! checked proof is the session snapshot; no second inventory or authority store.
use super::{CheckedAnnotations, wire};
use ferric_browser_config::InteractionCollectionOrder;
use ferric_browser_core::{
    HintCandidate, HintCodeDeclaration, HintReservationCategory, HintReservationReason as Reason,
    HintSpecialAssignment, HintSpecialBinding as Binding,
};
use std::collections::{BTreeMap, BTreeSet};

type PhysicalTarget = (String, String, u32, Option<u32>);

fn physical_target(candidate: &HintCandidate) -> PhysicalTarget {
    (
        candidate.frame_owner.clone(),
        candidate.frame_path.clone(),
        candidate.element_id,
        candidate.related_element_id,
    )
}

struct CollectionSnapshot<'a> {
    root: &'a wire::Root,
    container: wire::Node,
    kind: wire::RootKind,
    items: Vec<&'a wire::Item>,
}

fn order_items(items: &mut [&wire::Item], order: InteractionCollectionOrder, tolerance: f64) {
    if order == InteractionCollectionOrder::Dom {
        return;
    }
    items.sort_by(|a, b| {
        a.primary
            .projected_primary_fragment_css
            .y
            .total_cmp(&b.primary.projected_primary_fragment_css.y)
            .then_with(|| a.logical_node.cmp(&b.logical_node))
    });
    if order == InteractionCollectionOrder::TopToBottom {
        return;
    }
    let mut start = 0;
    while start < items.len() {
        // Every row is measured against its first Y, never its previous item.
        let anchor = items[start].primary.projected_primary_fragment_css.y;
        let mut end = start + 1;
        while end < items.len()
            && items[end].primary.projected_primary_fragment_css.y - anchor <= tolerance
        {
            end += 1;
        }
        items[start..end].sort_by(|a, b| {
            let x = a
                .primary
                .projected_primary_fragment_css
                .x
                .total_cmp(&b.primary.projected_primary_fragment_css.x);
            (if order == InteractionCollectionOrder::RowMajorRtl {
                x.reverse()
            } else {
                x
            })
            .then_with(|| a.logical_node.cmp(&b.logical_node))
        });
        start = end;
    }
}

impl CheckedAnnotations {
    fn collection_snapshot(&self, id: &str) -> Option<CollectionSnapshot<'_>> {
        let authored = self
            .policy
            .collections()
            .iter()
            .find(|collection| collection.id == id)?;
        let result = &self
            .wire
            .outcomes
            .iter()
            .find(|outcome| {
                outcome.rule.kind == wire::RuleKind::Collection && outcome.rule.id == id
            })?
            .result;
        let wire::ResultFacts::CollectionComplete {
            root,
            container,
            container_kind,
            items,
        } = result
        else {
            return None;
        };
        let mut items: Vec<_> = items.iter().collect();
        order_items(
            &mut items,
            authored.order,
            authored.row_tolerance_px.unwrap_or(8.),
        );
        Some(CollectionSnapshot {
            root: self.wire.roots.iter().find(|entry| entry.id == *root)?,
            container: *container,
            kind: *container_kind,
            items,
        })
    }

    pub(super) fn same_collection_binding(&self, current: &Self, id: &str) -> bool {
        let (Some(old), Some(new)) = (
            self.collection_snapshot(id),
            current.collection_snapshot(id),
        ) else {
            return false;
        };
        old.kind == new.kind
            && self.same_node(current, old.container, new.container)
            && self.same_root(current, old.root, new.root)
            && old.items.len() == new.items.len()
            && old.items.iter().zip(&new.items).all(|(old, new)| {
                let old_root = self
                    .wire
                    .roots
                    .iter()
                    .find(|root| root.id == old.primary_root);
                let new_root = current
                    .wire
                    .roots
                    .iter()
                    .find(|root| root.id == new.primary_root);
                self.same_node(current, old.logical_node, new.logical_node)
                    && old_root
                        .zip(new_root)
                        .is_some_and(|(old, new)| self.same_root(current, old, new))
                    && old.attributes == new.attributes
                    && old.identity_attributes == new.identity_attributes
                    && self.same_witness(current, &old.primary, &new.primary)
                    && old.primary.target_border_box_css == new.primary.target_border_box_css
                    && old.primary.projected_primary_fragment_css
                        == new.primary.projected_primary_fragment_css
                    && old.primary.top_viewport_css == new.primary.top_viewport_css
            })
    }

    fn collection_unavailable_reason(&self, id: &str) -> Reason {
        match self
            .wire
            .outcomes
            .iter()
            .find(|outcome| {
                outcome.rule.kind == wire::RuleKind::Collection && outcome.rule.id == id
            })
            .map(|outcome| &outcome.result)
        {
            Some(wire::ResultFacts::SearchComplete {
                cardinality: wire::SearchCardinality::Absent,
                ..
            }) => Reason::Missing,
            Some(wire::ResultFacts::SearchComplete {
                cardinality: wire::SearchCardinality::Ambiguous,
                ..
            }) => Reason::Ambiguous,
            _ => Reason::Incomplete,
        }
    }

    pub(super) fn assign_collection_slots(
        &self,
        candidates: &[HintCandidate],
        declarations: &[HintCodeDeclaration],
        live_slots: Option<&BTreeSet<usize>>,
        anchor_targets: &BTreeSet<PhysicalTarget>,
        assignments: &mut [HintSpecialAssignment],
    ) {
        // Rank the whole proved membership before resolving any ownership holes.
        let snapshots: BTreeMap<_, _> = self
            .policy
            .collections()
            .iter()
            .map(|collection| {
                (
                    collection.id.as_str(),
                    self.collection_snapshot(&collection.id),
                )
            })
            .collect();
        for (slot, declaration) in declarations
            .iter()
            .enumerate()
            .filter(|(_, declaration)| declaration.category == HintReservationCategory::Collection)
        {
            if live_slots.is_some_and(|live| !live.contains(&slot)) {
                continue;
            }
            let binding = match snapshots
                .get(declaration.rule_id.as_str())
                .and_then(Option::as_ref)
            {
                Some(snapshot) => match snapshot.items.get(declaration.slot) {
                    None => Binding::Unavailable(Reason::Missing),
                    Some(item) => self
                        .candidate_snapshot(item.primary.candidate.index)
                        .filter(|candidate| candidates.contains(candidate))
                        .map_or(Binding::Unavailable(Reason::Incomplete), |candidate| {
                            if anchor_targets.contains(&physical_target(candidate)) {
                                Binding::Unavailable(Reason::AnchorShadowed)
                            } else {
                                Binding::Live(candidate.clone())
                            }
                        }),
                },
                None => {
                    Binding::Unavailable(self.collection_unavailable_reason(&declaration.rule_id))
                }
            };
            assignments[slot].binding = binding;
        }
        let mut claims = BTreeMap::<_, Vec<usize>>::new();
        for assignment in assignments.iter().filter(|assignment| {
            declarations[assignment.slot].category == HintReservationCategory::Collection
        }) {
            if let Binding::Live(candidate) = &assignment.binding {
                claims
                    .entry(physical_target(candidate))
                    .or_default()
                    .push(assignment.slot);
            }
        }
        for slots in claims.values().filter(|slots| slots.len() > 1) {
            for &slot in slots {
                assignments[slot].binding = Binding::Unavailable(Reason::Ambiguous);
            }
        }
    }
}
