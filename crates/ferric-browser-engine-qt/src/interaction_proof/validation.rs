use super::{
    AnnotationError as E, AnnotationRequest,
    wire::{
        Attribute, Document, Envelope, Fact, Hop, Id, MAX_RECORDS, Node, Rect, ResultFacts, Root,
        RootKind, Rule, RuleKind, SearchStage, Size, UnavailableReason, Witness,
    },
};
use ferric_browser_config::{
    CompiledInteractionAttributeTest, CompiledInteractionPredicates, InteractionAttributeEvidence,
    InteractionNameEvidence, InteractionNameEvidenceError, InteractionPredicateEvidenceError,
    InteractionPredicateFacts, InteractionPredicateWork, InteractionPredicates, InteractionRoot,
    InteractionRootStart, InteractionRootStep, InteractionRootStepKind,
    is_canonical_interaction_origin,
};
use ferric_browser_core::{HintCandidate, HintKind, MAX_HINT_CANDIDATES, MAX_HINT_INSPECTIONS};
use std::collections::{BTreeMap, BTreeSet};

mod discovery;
mod normalization;
pub(super) use normalization::normalize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Classification {
    Match,
    NoMatch,
    Unavailable(UnavailableReason),
}

fn classification(value: Result<bool, InteractionPredicateEvidenceError>) -> Classification {
    match value {
        Ok(true) => Classification::Match,
        Ok(false) => Classification::NoMatch,
        Err(InteractionPredicateEvidenceError::WorkExhausted) => {
            Classification::Unavailable(UnavailableReason::PredicateLimit)
        }
        Err(
            InteractionPredicateEvidenceError::Truncated
            | InteractionPredicateEvidenceError::Name(InteractionNameEvidenceError::Truncated),
        ) => Classification::Unavailable(UnavailableReason::Truncated),
        Err(_) => Classification::Unavailable(UnavailableReason::MissingEvidence),
    }
}

fn require_match(value: Classification) -> Result<(), E> {
    match value {
        Classification::Match => Ok(()),
        Classification::Unavailable(UnavailableReason::PredicateLimit) => Err(E::WorkLimit),
        Classification::NoMatch | Classification::Unavailable(_) => Err(E::IncompleteEvidence),
    }
}

pub(super) fn valid_owner(owner: &str) -> bool {
    owner.is_empty() || owner.strip_prefix("frame-").is_some_and(valid_epoch)
}
fn valid_epoch(epoch: &str) -> bool {
    epoch.len() == 36 && uuid::Uuid::parse_str(epoch).is_ok_and(|id| id.to_string() == epoch)
}

struct Checker<'a> {
    document_roles: &'a super::AnnotationDocumentRoles,
    documents: BTreeMap<Id, &'a Document>,
    roots: BTreeMap<Id, &'a Root>,
    used_roots: BTreeSet<Id>,
    used_documents: BTreeSet<Id>,
    focused_documents: BTreeSet<Id>,
    object_kinds: BTreeMap<Node, RootKind>,
    // Frame association and traversal cost are distinct: ancestor shadow
    // roots consume traversal depth without inventing native frame hops.
    document_depths: BTreeMap<Id, usize>,
    composed_document_depths: BTreeMap<Id, usize>,
    shadow_hosts: BTreeMap<Node, Node>,
    shadow_roots: BTreeMap<Node, Node>,
    frame_documents: BTreeMap<Node, Id>,
    assigned_slots: BTreeMap<Node, Node>,
    records: usize,
    work: InteractionPredicateWork,
    predicate_work: Option<InteractionPredicateWork>,
}

impl<'a> Checker<'a> {
    fn charge(&mut self, steps: usize) -> Result<(), E> {
        self.work.charge(steps).map_err(|_| E::WorkLimit)
    }
    fn record(&mut self, count: usize) -> Result<(), E> {
        self.records = self.records.checked_add(count).ok_or(E::WorkLimit)?;
        if self.records > MAX_RECORDS {
            return Err(E::WorkLimit);
        }
        self.charge(count)
    }
    fn document(&mut self, id: Id) -> Result<&'a Document, E> {
        self.used_documents.insert(id);
        self.documents.get(&id).copied().ok_or(E::InvalidDocument)
    }
    fn node(&mut self, node: Node) -> Result<&'a Document, E> {
        let document = self.document(node.document)?;
        if !self.document_roles.is_collector(&document.frame_owner) {
            return Err(E::InvalidAssociation);
        }
        Ok(document)
    }
    fn typed_node(&mut self, node: Node, kind: RootKind) -> Result<&'a Document, E> {
        let document = self.node(node)?;
        self.record_node_kind(node, kind, document)
    }
    // Document roots and parent hosts may belong to structural-only native
    // ancestors. No semantic evidence path may use this exception.
    fn ancestry_node(&mut self, node: Node, kind: RootKind) -> Result<&'a Document, E> {
        let document = self.document(node.document)?;
        self.record_node_kind(node, kind, document)
    }
    fn record_node_kind(
        &mut self,
        node: Node,
        kind: RootKind,
        document: &'a Document,
    ) -> Result<&'a Document, E> {
        self.charge(1)?;
        if (node.node == document.root_node && kind != RootKind::Document)
            || self
                .object_kinds
                .get(&node)
                .is_some_and(|previous| *previous != kind)
        {
            return Err(E::InvalidAssociation);
        }
        self.object_kinds.insert(node, kind);
        Ok(document)
    }
    fn root(&mut self, id: Id) -> Result<&'a Root, E> {
        self.used_roots.insert(id);
        self.roots.get(&id).copied().ok_or(E::InvalidRoot)
    }

    fn graph(&mut self, request: &AnnotationRequest) -> Result<(), E> {
        let mut positions = BTreeSet::new();
        // Validate every variable-length field before any ancestry walk can
        // use a later record as a parent (including cross-owner transitions).
        let documents: Vec<_> = self.documents.values().copied().collect();
        for document in documents.iter().copied() {
            self.record(1)?;
            self.charge(document.origin.len())?;
            if !request.document_roles.contains(&document.frame_owner)
                || document.frame_path.len() > 256
                || !crate::hint_payload::valid_hint_frame_path(&document.frame_path)
                || !valid_epoch(&document.epoch)
                || !is_canonical_interaction_origin(&document.origin)
                || !positions.insert((&document.frame_owner, &document.frame_path))
            {
                return Err(E::InvalidDocument);
            }
        }
        for document in documents.iter().copied() {
            self.ancestry_node(
                Node {
                    document: document.id,
                    node: document.root_node,
                },
                RootKind::Document,
            )?;
        }
        for document in documents.iter().copied() {
            if let Some(host) = document.parent_host {
                self.ancestry_node(host, RootKind::Element)?;
                if self
                    .frame_documents
                    .insert(host, document.id)
                    .is_some_and(|previous| previous != document.id)
                {
                    return Err(E::InvalidAssociation);
                }
            }
        }
        for document in documents.iter().copied() {
            let mut seen = BTreeSet::from([document.id]);
            let mut current = document;
            for depth in 0..=8 {
                let Some(parent) = current.parent_host else {
                    if current.frame_path != "0" {
                        return Err(E::InvalidDocument);
                    }
                    break;
                };
                self.charge(1)?;
                if depth == 8 || !seen.insert(parent.document) {
                    return Err(E::InvalidDocument);
                }
                let ancestor = self
                    .documents
                    .get(&parent.document)
                    .copied()
                    .ok_or(E::InvalidDocument)?;
                if current.frame_owner == ancestor.frame_owner {
                    let suffix = current
                        .frame_path
                        .strip_prefix(&format!("{}.", ancestor.frame_path))
                        .ok_or(E::InvalidDocument)?;
                    if suffix.contains('.') || suffix.is_empty() {
                        return Err(E::InvalidDocument);
                    }
                } else if current.frame_path != "0" {
                    return Err(E::InvalidDocument);
                }
                current = ancestor;
            }
            let native_base = request.document_roles.focused().map_or(0, |receiver| {
                if !current.frame_owner.is_empty() && current.frame_owner == receiver.owner {
                    usize::from(receiver.depth)
                } else {
                    0
                }
            });
            let depth = seen.len() - 1 + native_base;
            if depth > 8 {
                return Err(E::InvalidDocument);
            }
            self.document_depths.insert(document.id, depth);
            let extra = request.document_roles.focused().map_or(0, |receiver| {
                if !receiver.owner.is_empty() && document.frame_owner == receiver.owner {
                    usize::from(receiver.composed_depth - receiver.depth)
                } else {
                    0
                }
            });
            if depth + extra > 8 {
                return Err(E::InvalidDocument);
            }
            self.composed_document_depths
                .insert(document.id, depth + extra);
        }
        for root in self.roots.values().copied().collect::<Vec<_>>() {
            self.record(1 + root.hops.len())?;
            if self
                .composed_document_depths
                .get(&root.start.document)
                .ok_or(E::InvalidDocument)?
                + root.hops.len()
                > 8
            {
                return Err(E::InvalidRoot);
            }
            let start_doc = self.typed_node(root.start, root.start_kind)?;
            if root.start_kind == RootKind::Document && start_doc.root_node != root.start.node {
                return Err(E::InvalidRoot);
            }
            let mut current = root.start;
            let mut kind = root.start_kind;
            let mut seen = BTreeSet::from([current]);
            for hop in root.hops.iter() {
                let host = hop.host();
                if host.document != current.document {
                    return Err(E::InvalidRoot);
                }
                let host_doc = self.typed_node(host, RootKind::Element)?;
                let (result, result_kind) = hop.result();
                let result_doc = self.typed_node(result, result_kind)?;
                match hop {
                    Hop::OpenShadow { .. }
                        if result.document == host.document && result != host =>
                    {
                        if self
                            .shadow_hosts
                            .insert(result, host)
                            .is_some_and(|previous| previous != host)
                            || self
                                .shadow_roots
                                .insert(host, result)
                                .is_some_and(|previous| previous != result)
                        {
                            return Err(E::InvalidAssociation);
                        }
                    }
                    Hop::SameOriginFrame { .. }
                        if result_doc.parent_host == Some(host)
                            && host_doc.frame_owner == result_doc.frame_owner
                            && host_doc.origin == result_doc.origin
                            && result.node == result_doc.root_node => {}
                    Hop::AssignedSlot { owner_document, .. }
                        if *owner_document == result.document
                            && result.document == host.document
                            && result != host =>
                    {
                        if self
                            .assigned_slots
                            .insert(result, host)
                            .is_some_and(|previous| previous != host)
                        {
                            return Err(E::InvalidAssociation);
                        }
                    }
                    _ => return Err(E::InvalidRoot),
                }
                if !seen.insert(result) {
                    return Err(E::InvalidRoot);
                }
                current = result;
                kind = result_kind;
            }
            if root.resolved != current || root.resolved_kind != kind {
                return Err(E::InvalidRoot);
            }
        }
        // Graph validation is not evidence that a document/root was requested.
        self.used_documents.clear();
        Ok(())
    }

    fn path(&mut self, id: Id, authored: Option<&InteractionRoot>) -> Result<&'a Root, E> {
        self.path_prefix(id, authored, authored.map_or(0, |root| root.steps.len()))
    }
    fn path_base_depth(&self, root: &Root, authored: Option<&InteractionRoot>) -> Result<usize, E> {
        if authored.is_some_and(|root| root.start == InteractionRootStart::FocusedFrame) {
            return self
                .document_roles
                .focused()
                .map(|receiver| usize::from(receiver.composed_depth))
                .ok_or(E::InvalidRoot);
        }
        self.composed_document_depths
            .get(&root.start.document)
            .copied()
            .ok_or(E::InvalidDocument)
    }
    fn path_prefix(
        &mut self,
        id: Id,
        authored: Option<&InteractionRoot>,
        count: usize,
    ) -> Result<&'a Root, E> {
        let root = self.root(id)?;
        let doc = self.node(root.start)?;
        if root.start_kind != RootKind::Document {
            return Err(E::InvalidRoot);
        }
        match authored.map_or(InteractionRootStart::TopDocument, |root| root.start) {
            InteractionRootStart::TopDocument
                if !doc.frame_owner.is_empty()
                    || doc.frame_path != "0"
                    || doc.parent_host.is_some() =>
            {
                return Err(E::InvalidRoot);
            }
            InteractionRootStart::FocusedFrame => {
                let receiver = self.document_roles.focused().ok_or(E::InvalidRoot)?;
                if authored.and_then(|root| root.receiver_origin.as_deref())
                    != Some(receiver.origin.as_str())
                    || doc.origin != receiver.origin
                    || doc.frame_owner != receiver.owner
                    || self.document_depths.get(&doc.id) != Some(&usize::from(receiver.depth))
                {
                    return Err(E::InvalidRoot);
                }
            }
            InteractionRootStart::TopDocument => {}
        }
        if authored.is_some_and(|root| root.start == InteractionRootStart::FocusedFrame) {
            // Retain the browser policy's required receiver association even
            // when canonical filtering later removes every match/item.
            self.focused_documents.insert(root.start.document);
        }
        let prefix = authored
            .map_or(&[][..], |root| root.steps.as_slice())
            .get(..count)
            .ok_or(E::InvalidRoot)?;
        self.steps(root, prefix)?;
        if self.path_base_depth(root, authored)? + count > 8 {
            return Err(E::InvalidRoot);
        }
        Ok(root)
    }

    fn search(
        &mut self,
        rule: &Rule,
        root: Id,
        stage: SearchStage,
        request: &AnnotationRequest,
    ) -> Result<(), E> {
        let (authored, selector_search) = match rule.kind {
            RuleKind::Anchor => {
                let anchor = request
                    .policy
                    .anchors()
                    .iter()
                    .find(|anchor| anchor.id == rule.id)
                    .ok_or(E::InvalidInventory)?;
                (
                    anchor.root.as_ref(),
                    matches!(stage, SearchStage::AnchorScope {}) && anchor.scope_selector.is_some(),
                )
            }
            RuleKind::Collection => {
                let collection = request
                    .policy
                    .collections()
                    .iter()
                    .find(|collection| collection.id == rule.id)
                    .ok_or(E::InvalidInventory)?;
                (
                    collection.root.as_ref(),
                    matches!(stage, SearchStage::CollectionContainer {})
                        && collection.container_selector.is_some(),
                )
            }
        };
        let step = match stage {
            SearchStage::AnchorScope {} | SearchStage::CollectionContainer {} => {
                if !selector_search {
                    return Err(E::InvalidInventory);
                }
                self.path(root, authored)?;
                return Ok(());
            }
            SearchStage::RootHost { step } | SearchStage::AssignedElement { step, .. } => {
                usize::from(step)
            }
        };
        let authored = authored.ok_or(E::InvalidRoot)?;
        let expected = authored.steps.get(step).ok_or(E::InvalidRoot)?;
        let prefix = self.path_prefix(root, Some(authored), step)?;
        // A valid prefix is insufficient when the attempted next hop itself
        // exceeds the shared focused-frame/root depth budget.
        self.charge(1)?;
        let start_depth = self.path_base_depth(prefix, Some(authored))?;
        if start_depth + prefix.hops.len() + 1 > 8 {
            return Err(E::InvalidRoot);
        }
        match stage {
            SearchStage::RootHost { .. } if expected.host_selector.is_some() => Ok(()),
            SearchStage::AssignedElement { slot, .. }
                if expected.kind == InteractionRootStepKind::AssignedSlot
                    && expected.assigned_selector.is_some() =>
            {
                self.typed_node(slot, RootKind::Element)?;
                if slot.document != prefix.resolved.document
                    || (expected.host_self == Some(true)
                        && (prefix.resolved_kind != RootKind::Element || slot != prefix.resolved))
                {
                    return Err(E::InvalidAssociation);
                }
                Ok(())
            }
            _ => Err(E::InvalidInventory),
        }
    }
    fn steps(&mut self, root: &Root, authored: &[InteractionRootStep]) -> Result<(), E> {
        if root.hops.len() != authored.len() {
            return Err(E::InvalidRoot);
        }
        let mut current = root.start;
        let mut current_kind = root.start_kind;
        self.node(current)?;
        for (hop, expected) in root.hops.iter().zip(authored) {
            self.charge(1)?;
            if hop.kind() != expected.kind
                || (expected.host_self == Some(true)
                    && (current_kind != RootKind::Element || hop.host() != current))
            {
                return Err(E::InvalidRoot);
            }
            self.node(hop.host())?;
            (current, current_kind) = hop.result();
            self.node(current)?;
        }
        Ok(())
    }
    fn attributes(&mut self, values: &[Attribute], expected: &BTreeSet<&str>) -> Result<(), E> {
        self.record(values.len())?;
        let actual: BTreeSet<_> = values.iter().map(|value| value.name.as_str()).collect();
        if actual.len() != values.len() || &actual != expected {
            return Err(E::IncompleteEvidence);
        }
        Ok(())
    }
    fn witness_structure(
        &mut self,
        witness: &Witness,
        root: &Root,
        predicates: Option<&InteractionPredicates>,
        candidates: &[HintCandidate],
        observed: bool,
    ) -> Result<usize, E> {
        self.record(1)?;
        let reference = &witness.candidate;
        let index = usize::from(reference.index);
        let expected = candidates.get(index).ok_or(E::InvalidAssociation)?;
        let document = self.node(root.resolved)?;
        self.typed_node(
            Node {
                document: root.resolved.document,
                node: reference.element_id,
            },
            RootKind::Element,
        )?;
        if let Some(related) = reference.related_element_id {
            self.typed_node(
                Node {
                    document: root.resolved.document,
                    node: related,
                },
                RootKind::Element,
            )?;
        }
        if reference.frame_owner != expected.frame_owner
            || reference.frame_path != expected.frame_path
            || reference.element_id.get() != expected.element_id
            || reference.related_element_id.map(Id::get) != expected.related_element_id
            || reference.frame_owner != document.frame_owner
            || reference.frame_path != document.frame_path
        {
            return Err(E::InvalidAssociation);
        }
        if let Fact::CompletePresent { value } = &witness.href {
            if expected.href.as_deref() != Some(value) {
                return Err(E::InvalidAssociation);
            }
        } else if matches!(witness.href, Fact::CompleteAbsent {}) && expected.href.is_some() {
            return Err(E::InvalidAssociation);
        }
        let border = witness.target_border_box_css;
        let viewport = witness.top_viewport_css;
        let fragment = witness.projected_primary_fragment_css;
        if !valid_size(border, false) || !valid_size(viewport, true) || !valid_rect(fragment) {
            return Err(E::InvalidGeometry);
        }
        let x = (fragment.x + fragment.width / 2.) / viewport.width;
        let y = (fragment.y + fragment.height / 2.) / viewport.height;
        if !x.is_finite() || !y.is_finite() || !(0. ..=1.).contains(&x) || !(0. ..=1.).contains(&y)
        {
            return Err(E::InvalidGeometry);
        }
        let empty = InteractionPredicates::default();
        let predicates = predicates.unwrap_or(&empty);
        self.attributes(
            &witness.attributes,
            &predicates
                .attributes
                .iter()
                .flatten()
                .map(|test| test.name.as_str())
                .collect(),
        )?;
        if let Fact::CompletePresent { value } = &witness.name {
            InteractionNameEvidence::complete(value).map_err(|_| E::IncompleteEvidence)?;
        }
        if let Fact::CompletePresent { value } = &witness.href
            && (value.len() > ferric_browser_config::MAX_INTERACTION_HREF_EVIDENCE_BYTES
                || value.is_empty()
                || value.chars().any(char::is_control))
        {
            return Err(E::IncompleteEvidence);
        }
        if witness.fragment_index.is_some_and(|index| index >= 8)
            || (observed && witness.fragment_index.is_none())
        {
            return Err(E::InvalidGeometry);
        }
        if observed {
            // Use the same rounding tolerance as ordinary fragment parsing.
            let inside = |rect: Rect, bounds: Rect| {
                rect.x >= bounds.x - 1e-6
                    && rect.y >= bounds.y - 1e-6
                    && rect.x + rect.width <= bounds.x + bounds.width + 1e-6
                    && rect.y + rect.height <= bounds.y + bounds.height + 1e-6
            };
            let bounds = Rect {
                x: expected.geometry.x,
                y: expected.geometry.y,
                width: expected.geometry.width,
                height: expected.geometry.height,
            };
            if !valid_rect(bounds)
                || !inside(fragment, bounds)
                || !inside(
                    fragment,
                    Rect {
                        x: 0.,
                        y: 0.,
                        width: viewport.width,
                        height: viewport.height,
                    },
                )
            {
                return Err(E::InvalidGeometry);
            }
        }
        Ok(index)
    }
}

// Moved from structural validation into semantic normalization. Keeping both
// counters preserves producer-work subtraction and the original remaining budget.
pub(super) struct NormalizationWork {
    work: InteractionPredicateWork,
    verification_limit: usize,
    predicate_work: Option<InteractionPredicateWork>,
}

impl NormalizationWork {
    pub(super) fn verification_steps(&self) -> usize {
        self.verification_limit - self.work.remaining()
    }
    fn matching_work(&mut self) -> &mut InteractionPredicateWork {
        self.predicate_work.as_mut().unwrap_or(&mut self.work)
    }
    fn charge(&mut self, steps: usize) -> Result<(), E> {
        self.work.charge(steps).map_err(|_| E::WorkLimit)
    }
    fn classify_attributes(
        &mut self,
        values: &[Attribute],
        tests: &[CompiledInteractionAttributeTest],
    ) -> Result<Classification, E> {
        for test in tests {
            let fact = values
                .iter()
                .find(|fact| fact.name == test.name())
                .ok_or(E::IncompleteEvidence)?;
            let matched = classification(test.matches(evidence(&fact.fact), self.matching_work()));
            if matched != Classification::Match {
                return Ok(matched);
            }
        }
        Ok(Classification::Match)
    }
    fn classify_witness(
        &mut self,
        witness: &Witness,
        candidate: &HintCandidate,
        compiled: &CompiledInteractionPredicates,
    ) -> Result<Classification, E> {
        let name = match &witness.name {
            Fact::CompletePresent { value } => {
                InteractionNameEvidence::complete(value).map_err(|_| E::IncompleteEvidence)?
            }
            Fact::Truncated {} => InteractionNameEvidence::truncated(),
            Fact::Missing {} | Fact::CompleteAbsent {} => InteractionNameEvidence::missing(),
        };
        let fragment = witness.projected_primary_fragment_css;
        let viewport = witness.top_viewport_css;
        let facts = InteractionPredicateFacts {
            hint_name: name,
            href: evidence(&witness.href),
            role: evidence(&witness.role),
            is_link: Some(candidate.kind == HintKind::Link),
            width_px: Some(witness.target_border_box_css.width),
            height_px: Some(witness.target_border_box_css.height),
            viewport_x: Some((fragment.x + fragment.width / 2.) / viewport.width),
            viewport_y: Some((fragment.y + fragment.height / 2.) / viewport.height),
        };
        Ok(classification(compiled.matches(
            &facts,
            |name| {
                witness
                    .attributes
                    .iter()
                    .find(|fact| fact.name == name)
                    .map_or(InteractionAttributeEvidence::Missing, |fact| {
                        evidence(&fact.fact)
                    })
            },
            self.matching_work(),
        )))
    }
}

fn evidence(fact: &Fact) -> InteractionAttributeEvidence<'_> {
    match fact {
        Fact::Missing {} => InteractionAttributeEvidence::Missing,
        Fact::Truncated {} => InteractionAttributeEvidence::Truncated,
        Fact::CompleteAbsent {} => InteractionAttributeEvidence::CompleteAbsent,
        Fact::CompletePresent { value } => InteractionAttributeEvidence::CompletePresent(value),
    }
}
fn valid_size(size: Size, positive: bool) -> bool {
    size.width.is_finite()
        && size.height.is_finite()
        && size.width >= 0.
        && size.height >= 0.
        && (!positive || (size.width > 0. && size.height > 0.))
}
fn valid_rect(rect: Rect) -> bool {
    rect.x.is_finite()
        && rect.y.is_finite()
        && valid_size(
            Size {
                width: rect.width,
                height: rect.height,
            },
            true,
        )
        && (rect.x + rect.width).is_finite()
        && (rect.y + rect.height).is_finite()
}

pub(super) fn validate_structure(
    wire: &Envelope,
    request: &AnnotationRequest,
    candidates: &[HintCandidate],
    verification_limit: usize,
) -> Result<(NormalizationWork, BTreeSet<Id>), E> {
    let verification_limit =
        verification_limit.min(ferric_browser_config::MAX_INTERACTION_PREDICATE_STEPS);
    if verification_limit == 0 {
        return Err(E::WorkLimit);
    }
    let mut work = InteractionPredicateWork::default();
    work.charge(ferric_browser_config::MAX_INTERACTION_PREDICATE_STEPS - verification_limit)
        .map_err(|_| E::WorkLimit)?;
    if candidates.len() > MAX_HINT_CANDIDATES {
        return Err(E::InvalidAssociation);
    }
    let unique: BTreeSet<_> = candidates
        .iter()
        .map(|candidate| (&candidate.frame_owner, candidate.element_id))
        .collect();
    if unique.len() != candidates.len()
        || candidates.iter().any(|candidate| candidate.element_id == 0)
    {
        return Err(E::InvalidAssociation);
    }
    let mut checker = Checker {
        document_roles: &request.document_roles,
        documents: wire
            .documents
            .iter()
            .map(|document| (document.id, document))
            .collect(),
        roots: wire.roots.iter().map(|root| (root.id, root)).collect(),
        used_roots: BTreeSet::new(),
        used_documents: BTreeSet::new(),
        focused_documents: BTreeSet::new(),
        object_kinds: BTreeMap::new(),
        document_depths: BTreeMap::new(),
        composed_document_depths: BTreeMap::new(),
        shadow_hosts: BTreeMap::new(),
        shadow_roots: BTreeMap::new(),
        frame_documents: BTreeMap::new(),
        assigned_slots: BTreeMap::new(),
        records: 0,
        work,
        predicate_work: None,
    };
    let observed = wire.outcomes.iter().any(|outcome| {
        matches!(
            outcome.result,
            ResultFacts::AnchorObserved { .. } | ResultFacts::CollectionObserved { .. }
        )
    });
    if observed && wire.discovery_work.is_none() {
        return Err(E::IncompleteEvidence);
    }
    if let Some(spent) = wire.discovery_work {
        if usize::try_from(spent.inspections).map_err(|_| E::WorkLimit)? > MAX_HINT_INSPECTIONS
            || usize::try_from(spent.predicate_steps).map_err(|_| E::WorkLimit)?
                > ferric_browser_config::MAX_INTERACTION_PREDICATE_STEPS
        {
            return Err(E::WorkLimit);
        }
        let mut work = InteractionPredicateWork::default();
        work.charge(usize::try_from(spent.predicate_steps).map_err(|_| E::WorkLimit)?)
            .map_err(|_| E::WorkLimit)?;
        checker.predicate_work = Some(work);
    }
    if checker.documents.len() != wire.documents.len() || checker.roots.len() != wire.roots.len() {
        return Err(E::InvalidAssociation);
    }
    checker.graph(request)?;
    let mut expected = BTreeSet::new();
    if request.policy.enabled() {
        expected.extend(
            request
                .policy
                .anchors()
                .iter()
                .filter(|anchor| anchor.families.contains(&request.stamp.family))
                .map(|anchor| (RuleKind::Anchor, anchor.id.as_str())),
        );
        expected.extend(
            request
                .policy
                .collections()
                .iter()
                .filter(|collection| collection.families.contains(&request.stamp.family))
                .map(|collection| (RuleKind::Collection, collection.id.as_str())),
        );
    }
    let actual: BTreeSet<_> = wire
        .outcomes
        .iter()
        .map(|outcome| (outcome.rule.kind, outcome.rule.id.as_str()))
        .collect();
    if expected != actual || actual.len() != wire.outcomes.len() {
        return Err(E::InvalidInventory);
    }
    for outcome in wire.outcomes.iter() {
        checker.record(1)?;
        match (&outcome.rule.kind, &outcome.result) {
            (_, ResultFacts::Unavailable { .. }) => {}
            (_, ResultFacts::SearchComplete { root, stage, .. }) => {
                checker.search(&outcome.rule, *root, *stage, request)?;
            }
            (RuleKind::Anchor, ResultFacts::AnchorObserved { .. }) => {
                discovery::anchor_structure(&mut checker, outcome, request, candidates)?;
            }
            (RuleKind::Collection, ResultFacts::CollectionObserved { .. }) => {
                discovery::collection_structure(&mut checker, outcome, request, candidates)?;
            }
            (
                RuleKind::Anchor,
                ResultFacts::AnchorComplete {
                    root,
                    scope,
                    scope_kind,
                    matches,
                },
            ) => {
                let authored = request
                    .policy
                    .anchors()
                    .iter()
                    .find(|anchor| anchor.id == outcome.rule.id)
                    .ok_or(E::InvalidInventory)?;
                let root = checker.path(*root, authored.root.as_ref())?;
                checker.typed_node(*scope, *scope_kind)?;
                if scope.document != root.resolved.document
                    || (authored.scope_selector.is_none()
                        && (*scope != root.resolved || *scope_kind != root.resolved_kind))
                    || (authored.scope_selector.is_some() && *scope_kind != RootKind::Element)
                {
                    return Err(E::InvalidAssociation);
                }
                let mut seen = BTreeSet::new();
                for witness in matches.iter() {
                    let index = checker.witness_structure(
                        witness,
                        root,
                        authored.predicates.as_ref(),
                        candidates,
                        false,
                    )?;
                    if !seen.insert(index) {
                        return Err(E::InvalidAssociation);
                    }
                }
            }
            (
                RuleKind::Collection,
                ResultFacts::CollectionComplete {
                    root,
                    container,
                    container_kind,
                    items,
                },
            ) => {
                let authored = request
                    .policy
                    .collections()
                    .iter()
                    .find(|collection| collection.id == outcome.rule.id)
                    .ok_or(E::InvalidInventory)?;
                let root = checker.path(*root, authored.root.as_ref())?;
                checker.typed_node(*container, *container_kind)?;
                if container.document != root.resolved.document
                    || (authored.container_self == Some(true)
                        && (*container != root.resolved || *container_kind != root.resolved_kind))
                    || (authored.container_self != Some(true)
                        && *container_kind != RootKind::Element)
                {
                    return Err(E::InvalidAssociation);
                }
                let mut seen_items = BTreeSet::new();
                let mut seen_primaries = BTreeSet::new();
                for item in items.iter() {
                    checker.record(1)?;
                    checker.typed_node(item.logical_node, RootKind::Element)?;
                    if item.logical_node.document != container.document
                        || !seen_items.insert(item.logical_node)
                    {
                        return Err(E::InvalidAssociation);
                    }
                    checker.attributes(
                        &item.attributes,
                        &authored
                            .item_attributes
                            .iter()
                            .flatten()
                            .map(|test| test.name.as_str())
                            .collect(),
                    )?;
                    checker.attributes(
                        &item.identity_attributes,
                        &authored
                            .identity_attributes
                            .iter()
                            .flatten()
                            .map(String::as_str)
                            .collect(),
                    )?;
                    if item
                        .identity_attributes
                        .iter()
                        .any(|attribute| !matches!(attribute.fact, Fact::CompletePresent { .. }))
                    {
                        return Err(E::IncompleteEvidence);
                    }
                    let primary = checker.root(item.primary_root)?;
                    if primary.start != item.logical_node
                        || primary.start_kind != RootKind::Element
                        || checker.path_base_depth(root, authored.root.as_ref())?
                            + root.hops.len()
                            + primary.hops.len()
                            > 8
                    {
                        return Err(E::InvalidRoot);
                    }
                    checker.steps(
                        primary,
                        authored.primary_root_steps.as_deref().unwrap_or_default(),
                    )?;
                    let index = checker.witness_structure(
                        &item.primary,
                        primary,
                        authored.primary_predicates.as_ref(),
                        candidates,
                        false,
                    )?;
                    if !seen_primaries.insert(index) {
                        return Err(E::InvalidAssociation);
                    }
                    if authored.primary_self == Some(true)
                        && (primary.resolved_kind != RootKind::Element
                            || primary.resolved.node != item.primary.candidate.element_id)
                    {
                        return Err(E::InvalidAssociation);
                    }
                }
            }
            _ => return Err(E::InvalidInventory),
        }
    }
    // Parent documents may be needed even if the only live root starts at a
    // descendant. No orphan facts may masquerade as requested capture data.
    for id in checker.used_documents.clone() {
        let mut current = checker.document(id)?;
        while let Some(parent) = current.parent_host {
            checker.charge(1)?;
            current = checker.document(parent.document)?;
        }
    }
    if checker.used_roots.len() != checker.roots.len()
        || checker.used_documents.len() != checker.documents.len()
    {
        return Err(E::InvalidAssociation);
    }
    Ok((
        NormalizationWork {
            work: checker.work,
            verification_limit,
            predicate_work: checker.predicate_work,
        },
        checker.focused_documents,
    ))
}

/// The original graph was checked for orphan records before classification.
/// Remove only records made unused by a valid normalization result.
pub(super) fn retain_normalized_graph(wire: &mut Envelope) -> Result<(), E> {
    let mut roots = BTreeSet::new();
    for outcome in wire.outcomes.iter() {
        match &outcome.result {
            ResultFacts::AnchorComplete { root, .. }
            | ResultFacts::AnchorObserved { root, .. }
            | ResultFacts::SearchComplete { root, .. } => {
                roots.insert(*root);
            }
            ResultFacts::CollectionComplete { root, items, .. } => {
                roots.insert(*root);
                roots.extend(items.iter().map(|item| item.primary_root));
            }
            ResultFacts::Unavailable { .. } => {}
            ResultFacts::CollectionObserved { root, items, .. } => {
                roots.insert(*root);
                roots.extend(items.iter().filter_map(|item| match &item.primary {
                    super::wire::ObservedPrimary::TargetsComplete { root, .. } => Some(*root),
                    super::wire::ObservedPrimary::Unavailable { .. } => None,
                }));
            }
        }
    }
    wire.roots.retain(|root| roots.contains(&root.id));
    let mut documents = BTreeSet::new();
    for root in wire.roots.iter() {
        documents.insert(root.start.document);
        documents.insert(root.resolved.document);
        for hop in root.hops.iter() {
            documents.insert(hop.host().document);
            documents.insert(hop.result().0.document);
        }
    }
    let parents: BTreeMap<_, _> = wire
        .documents
        .iter()
        .map(|doc| (doc.id, doc.parent_host.map(|node| node.document)))
        .collect();
    let mut pending: Vec<_> = documents.iter().copied().collect();
    let mut index = 0;
    while index < pending.len() {
        if let Some(parent) = parents.get(&pending[index]).ok_or(E::InvalidDocument)?
            && documents.insert(*parent)
        {
            pending.push(*parent);
        }
        index += 1;
    }
    wire.documents.retain(|doc| documents.contains(&doc.id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn empty<'a>() -> Checker<'a> {
        static ROLES: std::sync::LazyLock<super::super::AnnotationDocumentRoles> =
            std::sync::LazyLock::new(super::super::AnnotationDocumentRoles::default);
        Checker {
            document_roles: &ROLES,
            documents: BTreeMap::new(),
            roots: BTreeMap::new(),
            used_roots: BTreeSet::new(),
            used_documents: BTreeSet::new(),
            focused_documents: BTreeSet::new(),
            object_kinds: BTreeMap::new(),
            document_depths: BTreeMap::new(),
            composed_document_depths: BTreeMap::new(),
            shadow_hosts: BTreeMap::new(),
            shadow_roots: BTreeMap::new(),
            frame_documents: BTreeMap::new(),
            assigned_slots: BTreeMap::new(),
            records: 0,
            work: InteractionPredicateWork::default(),
            predicate_work: None,
        }
    }
    #[test]
    fn aggregate_record_count_does_not_reset_between_tables() {
        for count in [MAX_RECORDS - 1, MAX_RECORDS, MAX_RECORDS + 1] {
            assert_eq!(empty().record(count).is_ok(), count <= MAX_RECORDS);
        }
        let mut checker = empty();
        checker.record(MAX_RECORDS - 1).unwrap();
        checker.record(1).unwrap();
        assert_eq!(checker.record(1), Err(E::WorkLimit));
        assert_eq!(checker.record(0), Err(E::WorkLimit));
    }
    #[test]
    fn metadata_and_predicates_charge_the_same_work_counter() {
        let mut checker = empty();
        checker
            .work
            .charge(ferric_browser_config::MAX_INTERACTION_PREDICATE_STEPS - 1)
            .unwrap();
        checker.record(1).unwrap();
        assert_eq!(checker.charge(1), Err(E::WorkLimit));
        assert_eq!(checker.work.remaining(), 0);
    }
}
