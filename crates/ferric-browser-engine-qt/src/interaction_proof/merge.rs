//! Combine the original sealed collector worlds before canonical matching.
//! Projection data enters through the native continuation, never a page label.
use super::{
    AnnotationError as E, AnnotationRequest, CheckedAnnotationFacts, CheckedAnnotations,
    bounded::List,
    finish_annotations, validation,
    wire::{self, Hop, Id, Node, Outcome, ResultFacts, Rule, RuleKind, SearchStage, Witness},
};
use ferric_browser_config::InteractionRootStart;
use ferric_browser_core::{HintCandidate, HintGeometry, HintKind, HintPoint};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};

pub(crate) struct MergeWorld {
    pub(crate) owner: String,
    pub(crate) facts: Box<CheckedAnnotationFacts>,
    pub(crate) candidates: Arc<[HintCandidate]>,
}

pub(crate) type WorldProjections = List<WorldProjection, 2>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorldProjection {
    owner: String,
    geometry: Projection,
    #[serde(default)]
    excluded: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Projection {
    viewport: wire::Size,
    mappings: List<Mapping, 8>,
    #[serde(default)]
    targets: List<ProjectedTarget, { ferric_browser_core::MAX_HINT_CANDIDATES }>,
    #[serde(default)]
    limited: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectedTarget {
    index: u16,
    fragment_index: u8,
    rect: wire::Rect,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Mapping {
    left: f64,
    top: f64,
    scale_x: f64,
    scale_y: f64,
    content: Clip,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Clip {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    width: f64,
    height: f64,
}

fn valid_rect(r: &wire::Rect) -> bool {
    [r.x, r.y, r.width, r.height, r.x + r.width, r.y + r.height]
        .iter()
        .all(|value| value.is_finite())
        && r.width > 0.
        && r.height > 0.
}

impl Projection {
    fn validate(&self, foreign: bool) -> Result<(), E> {
        if !self.viewport.width.is_finite()
            || !self.viewport.height.is_finite()
            || self.viewport.width <= 0.
            || self.viewport.height <= 0.
            || foreign == self.mappings.is_empty()
            || self.targets.len() > ferric_browser_core::MAX_HINT_CANDIDATES
        {
            return Err(E::InvalidGeometry);
        }
        for map in self.mappings.iter() {
            let c = &map.content;
            if ![
                map.left,
                map.top,
                map.scale_x,
                map.scale_y,
                c.left,
                c.top,
                c.right,
                c.bottom,
                c.width,
                c.height,
            ]
            .iter()
            .all(|value| value.is_finite())
                || map.scale_x <= 0.
                || map.scale_y <= 0.
                || c.width <= 0.
                || c.height <= 0.
                || c.right <= c.left
                || c.bottom <= c.top
                || (c.right - c.left - c.width).abs() > 1e-6
                || (c.bottom - c.top - c.height).abs() > 1e-6
            {
                return Err(E::InvalidGeometry);
            }
        }
        let mut indices = std::collections::BTreeSet::new();
        for target in self.targets.iter() {
            if target.fragment_index >= 8
                || !valid_rect(&target.rect)
                || !indices.insert(target.index)
                || target.rect.x < 0.
                || target.rect.y < 0.
                || target.rect.x + target.rect.width > self.viewport.width
                || target.rect.y + target.rect.height > self.viewport.height
            {
                return Err(E::InvalidGeometry);
            }
        }
        Ok(())
    }

    fn rect(&self, mut rect: HintGeometry) -> Option<HintGeometry> {
        for map in self.mappings.iter().rev() {
            let left = map.content.left.max(map.left + rect.x * map.scale_x);
            let top = map.content.top.max(map.top + rect.y * map.scale_y);
            let right = map
                .content
                .right
                .min(map.left + (rect.x + rect.width) * map.scale_x);
            let bottom = map
                .content
                .bottom
                .min(map.top + (rect.y + rect.height) * map.scale_y);
            rect = HintGeometry {
                x: left,
                y: top,
                width: right - left,
                height: bottom - top,
            };
            if !valid_rect(&wire::Rect {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
            }) {
                return None;
            }
        }
        Some(rect)
    }

    fn candidate(&self, candidate: &HintCandidate, foreign: bool) -> Option<HintCandidate> {
        if foreign
            && !matches!(
                candidate.kind,
                HintKind::Link
                    | HintKind::Button
                    | HintKind::Input
                    | HintKind::Select
                    | HintKind::Textarea
                    | HintKind::ContentEditable
                    | HintKind::EditorFocus
                    | HintKind::LabelControl
                    | HintKind::Aria
                    | HintKind::Focusable
                    | HintKind::Frame
            )
        {
            return None;
        }
        let mut result = candidate.clone();
        result.geometry = self.rect(candidate.geometry)?;
        if let Some(presentation) = &mut result.presentation {
            presentation.fragments = presentation
                .fragments
                .iter()
                .filter_map(|rect| self.rect(*rect))
                .collect();
            let mut anchor = Some(presentation.anchor);
            for map in self.mappings.iter().rev() {
                anchor = anchor.and_then(|point| {
                    let point = HintPoint {
                        x: map.left + point.x * map.scale_x,
                        y: map.top + point.y * map.scale_y,
                    };
                    (point.x.is_finite()
                        && point.y.is_finite()
                        && point.x >= map.content.left
                        && point.y >= map.content.top
                        && point.x <= map.content.right
                        && point.y <= map.content.bottom)
                        .then_some(point)
                });
            }
            let primary = presentation
                .fragments
                .iter()
                .min_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)))?;
            presentation.anchor = anchor
                .filter(|point| {
                    presentation.fragments.iter().any(|rect| {
                        point.x >= rect.x
                            && point.y >= rect.y
                            && point.x <= rect.x + rect.width
                            && point.y <= rect.y + rect.height
                    })
                })
                .unwrap_or(HintPoint {
                    x: primary.x + primary.width / 2.,
                    y: primary.y + primary.height / 2.,
                });
        }
        Some(result)
    }
}

struct Remap<'a> {
    documents: BTreeMap<Id, Id>,
    roots: BTreeMap<Id, Id>,
    candidates: Vec<Option<u16>>,
    projection: &'a Projection,
    targets: BTreeMap<u16, &'a ProjectedTarget>,
}

impl Remap<'_> {
    fn node(&self, node: &mut Node) -> Result<(), E> {
        node.document = *self
            .documents
            .get(&node.document)
            .ok_or(E::InvalidDocument)?;
        Ok(())
    }
    fn root(&self, root: &mut Id) -> Result<(), E> {
        *root = *self.roots.get(root).ok_or(E::InvalidRoot)?;
        Ok(())
    }
    fn witness(&self, witness: &mut Witness) -> bool {
        let original = witness.candidate.index;
        let Some(index) = self
            .candidates
            .get(usize::from(original))
            .copied()
            .flatten()
        else {
            return false;
        };
        if !self.projection.mappings.is_empty() {
            let Some(target) = self.targets.get(&original) else {
                return false;
            };
            witness.projected_primary_fragment_css = target.rect;
            witness.fragment_index = Some(target.fragment_index);
        }
        witness.candidate.index = index;
        witness.top_viewport_css = self.projection.viewport;
        true
    }
    fn outcome(&self, outcome: &mut Outcome) -> Result<(), E> {
        let mut complete = true;
        match &mut outcome.result {
            ResultFacts::Unavailable { .. } => {}
            ResultFacts::SearchComplete { root, stage, .. } => {
                self.root(root)?;
                if let SearchStage::AssignedElement { slot, .. } = stage {
                    self.node(slot)?;
                }
            }
            ResultFacts::AnchorComplete {
                root,
                scope,
                matches: targets,
                ..
            }
            | ResultFacts::AnchorObserved {
                root,
                scope,
                targets,
                ..
            } => {
                self.root(root)?;
                self.node(scope)?;
                for witness in targets.iter_mut() {
                    complete &= self.witness(witness);
                }
            }
            ResultFacts::CollectionComplete {
                root,
                container,
                items,
                ..
            } => {
                self.root(root)?;
                self.node(container)?;
                for item in items.iter_mut() {
                    self.node(&mut item.logical_node)?;
                    self.root(&mut item.primary_root)?;
                    complete &= self.witness(&mut item.primary);
                }
            }
            ResultFacts::CollectionObserved {
                root,
                container,
                items,
                ..
            } => {
                self.root(root)?;
                self.node(container)?;
                for item in items.iter_mut() {
                    self.node(&mut item.logical_node)?;
                    if let Some(parent) = &mut item.parent_item {
                        self.node(parent)?;
                    }
                    if let wire::ObservedPrimary::TargetsComplete { root, targets } =
                        &mut item.primary
                    {
                        self.root(root)?;
                        for witness in targets.iter_mut() {
                            complete &= self.witness(witness);
                        }
                    }
                }
            }
        }
        if !complete {
            outcome.result = ResultFacts::Unavailable {
                reason: if self.projection.limited {
                    wire::UnavailableReason::Truncated
                } else {
                    wire::UnavailableReason::MissingEvidence
                },
            };
        }
        Ok(())
    }
}

fn rule_owner<'a>(request: &'a AnnotationRequest, rule: &Rule) -> Result<&'a str, E> {
    let root = match rule.kind {
        RuleKind::Anchor => request
            .policy
            .anchors()
            .iter()
            .find(|a| a.id == rule.id)
            .map(|a| a.root.as_ref()),
        RuleKind::Collection => request
            .policy
            .collections()
            .iter()
            .find(|c| c.id == rule.id)
            .map(|c| c.root.as_ref()),
    }
    .ok_or(E::InvalidInventory)?;
    if root.is_some_and(|root| root.start == InteractionRootStart::FocusedFrame) {
        Ok(request
            .selected_collector()
            .map_or("", |receiver| receiver.owner.as_str()))
    } else {
        Ok("")
    }
}

fn remap_graph(wire: &mut wire::Envelope, map: &Remap<'_>) -> Result<(), E> {
    for document in wire.documents.iter_mut() {
        document.id = map.documents[&document.id];
        if let Some(host) = &mut document.parent_host {
            map.node(host)?;
        }
    }
    for root in wire.roots.iter_mut() {
        map.root(&mut root.id)?;
        map.node(&mut root.start)?;
        map.node(&mut root.resolved)?;
        for hop in root.hops.iter_mut() {
            match hop {
                Hop::OpenShadow { host, root } => {
                    map.node(host)?;
                    map.node(root)?;
                }
                Hop::SameOriginFrame { host, document } => {
                    map.node(host)?;
                    map.node(document)?;
                }
                Hop::AssignedSlot {
                    slot,
                    assigned,
                    owner_document,
                } => {
                    map.node(slot)?;
                    map.node(assigned)?;
                    *owner_document = *map
                        .documents
                        .get(owner_document)
                        .ok_or(E::InvalidDocument)?;
                }
            }
        }
    }
    Ok(())
}

fn append_world(
    request: &AnnotationRequest,
    world: MergeWorld,
    world_projection: &WorldProjection,
    candidates: &mut Vec<HintCandidate>,
    documents: &mut Vec<wire::Document>,
    roots: &mut Vec<wire::Root>,
    outcomes: &mut BTreeMap<(RuleKind, String), Outcome>,
) -> Result<(), E> {
    let excluded = world_projection.excluded;
    let projection = &world_projection.geometry;
    if world.facts.wire.stamp != request.stamp
        || !world.facts.belongs_to_world(&world.owner)
        || !Arc::ptr_eq(&world.facts.candidates, &world.candidates)
    {
        return Err(E::InvalidAssociation);
    }
    if world
        .candidates
        .iter()
        .any(|candidate| candidate.frame_owner != world.owner)
        || projection
            .targets
            .iter()
            .any(|target| usize::from(target.index) >= world.candidates.len())
        || (!world.owner.is_empty()
            && request.selected_collector().is_none_or(|receiver| {
                receiver.owner != world.owner
                    || usize::from(receiver.depth) != projection.mappings.len()
            }))
    {
        return Err(E::InvalidAssociation);
    }
    let mut map = Remap {
        documents: BTreeMap::new(),
        roots: BTreeMap::new(),
        candidates: Vec::new(),
        projection,
        targets: projection
            .targets
            .iter()
            .map(|target| (target.index, target))
            .collect(),
    };
    for candidate in world.candidates.iter() {
        let projected = if excluded {
            None
        } else {
            projection.candidate(candidate, !world.owner.is_empty())
        };
        let index = if let Some(projected) = projected {
            let index = u16::try_from(candidates.len()).map_err(|_| E::WorkLimit)?;
            if candidates.len() == ferric_browser_core::MAX_HINT_CANDIDATES {
                return Err(E::WorkLimit);
            }
            candidates.push(projected);
            Some(index)
        } else {
            None
        };
        map.candidates.push(index);
    }
    let mut wire = world.facts.wire;
    if documents.len() + wire.documents.len() > wire::MAX_RECORDS
        || roots.len() + wire.roots.len() > wire::MAX_RECORDS
    {
        return Err(E::WorkLimit);
    }
    map.documents = remapped_ids(
        wire.documents.iter().map(|document| document.id),
        documents.len(),
    )?;
    map.roots = remapped_ids(wire.roots.iter().map(|root| root.id), roots.len())?;
    remap_graph(&mut wire, &map)?;
    for mut outcome in wire.outcomes.iter().cloned() {
        if rule_owner(request, &outcome.rule)? != world.owner {
            continue;
        }
        if excluded {
            outcome.result = ResultFacts::Unavailable {
                reason: wire::UnavailableReason::UnsupportedRoot,
            };
        } else {
            map.outcome(&mut outcome)?;
        }
        if outcomes
            .insert((outcome.rule.kind, outcome.rule.id.clone()), outcome)
            .is_some()
        {
            return Err(E::InvalidInventory);
        }
    }
    documents.extend(wire.documents.iter().cloned());
    roots.extend(wire.roots.iter().cloned());
    Ok(())
}

fn remapped_ids(values: impl Iterator<Item = Id>, offset: usize) -> Result<BTreeMap<Id, Id>, E> {
    values
        .enumerate()
        .map(|(index, id)| {
            let mapped = offset
                .checked_add(index)
                .and_then(|n| n.checked_add(1))
                .ok_or(E::WorkLimit)?;
            Ok((
                id,
                Id::new(u32::try_from(mapped).map_err(|_| E::WorkLimit)?).ok_or(E::WorkLimit)?,
            ))
        })
        .collect()
}

pub(crate) fn merge_worlds(
    request: &AnnotationRequest,
    worlds: Vec<MergeWorld>,
    projections: &WorldProjections,
    inspections: usize,
    predicate_steps: usize,
    verification_limit: usize,
) -> Result<(Vec<HintCandidate>, CheckedAnnotations), E> {
    let spent = wire::DiscoveryWork {
        inspections: u32::try_from(inspections).map_err(|_| E::WorkLimit)?,
        predicate_steps: u32::try_from(predicate_steps).map_err(|_| E::WorkLimit)?,
    };
    let mut projection_by_owner = BTreeMap::new();
    let mut viewport = None;
    for projection in projections.iter() {
        projection.geometry.validate(!projection.owner.is_empty())?;
        if projection.excluded && !projection.owner.is_empty() {
            return Err(E::InvalidAssociation);
        }
        if viewport.is_some_and(|size| size != projection.geometry.viewport) {
            return Err(E::InvalidGeometry);
        }
        viewport = Some(projection.geometry.viewport);
        if projection_by_owner
            .insert(projection.owner.as_str(), projection)
            .is_some()
        {
            return Err(E::InvalidAssociation);
        }
    }
    if projection_by_owner.len() != worlds.len() {
        return Err(E::InvalidAssociation);
    }
    let mut candidates = Vec::new();
    let mut documents = Vec::new();
    let mut roots = Vec::new();
    let mut outcomes = BTreeMap::new();
    for world in worlds {
        let projection = projection_by_owner
            .remove(world.owner.as_str())
            .ok_or(E::InvalidAssociation)?;
        append_world(
            request,
            world,
            projection,
            &mut candidates,
            &mut documents,
            &mut roots,
            &mut outcomes,
        )?;
    }
    let mut inventory = Vec::new();
    for (kind, id) in request
        .policy
        .anchors()
        .iter()
        .filter(|a| request.policy.enabled() && a.families.contains(&request.stamp.family))
        .map(|a| (RuleKind::Anchor, &a.id))
        .chain(
            request
                .policy
                .collections()
                .iter()
                .filter(|c| request.policy.enabled() && c.families.contains(&request.stamp.family))
                .map(|c| (RuleKind::Collection, &c.id)),
        )
    {
        inventory.push(
            outcomes
                .remove(&(kind, id.clone()))
                .unwrap_or_else(|| Outcome {
                    rule: Rule {
                        kind,
                        id: id.clone(),
                    },
                    result: ResultFacts::Unavailable {
                        reason: wire::UnavailableReason::UnsupportedRoot,
                    },
                }),
        );
    }
    let mut wire = wire::Envelope {
        version: super::INTERACTION_ANNOTATION_VERSION,
        stamp: request.stamp.clone(),
        documents: List::from_vec(documents).ok_or(E::WorkLimit)?,
        roots: List::from_vec(roots).ok_or(E::WorkLimit)?,
        outcomes: List::from_vec(inventory).ok_or(E::WorkLimit)?,
        discovery_work: Some(spent),
    };
    validation::retain_normalized_graph(&mut wire)?;
    let (work, focused) =
        validation::validate_structure(&wire, request, &candidates, verification_limit)?;
    let original = serde_json::to_string(&wire).map_err(|_| E::InvalidSchema)?;
    if original.len() > super::MAX_INTERACTION_ANNOTATION_BYTES {
        return Err(E::TooLarge);
    }
    let checked = finish_annotations(wire, request, &candidates, work, focused, original)?;
    Ok((candidates, checked))
}
