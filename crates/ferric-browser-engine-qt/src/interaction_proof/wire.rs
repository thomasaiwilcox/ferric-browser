//! Untrusted facts only. No command, transport, label, selector override or permit.
use super::bounded::{List, Revision};
use ferric_browser_config::{InteractionFamily, InteractionRootStepKind};
use ferric_browser_core::MAX_HINT_CANDIDATES;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

const MAX_OUTCOMES: usize = ferric_browser_config::MAX_INTERACTION_ANCHORS
    + ferric_browser_config::MAX_INTERACTION_COLLECTIONS;

fn explicit_nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

fn present_option<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

pub(crate) type Id = NonZeroU32;
pub(crate) const MAX_RECORDS: usize = 50_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Stamp {
    pub profile: String,
    pub window: String,
    pub tab: String,
    pub document: String,
    pub profile_revision: Revision,
    pub configuration_revision: Revision,
    pub policy_revision: Revision,
    pub target_generation: Revision,
    pub selector_revision: Revision,
    pub view_generation: Revision,
    pub viewport_generation: Revision,
    pub collection_epoch: Revision,
    pub provider_epoch: Revision,
    pub request_id: Revision,
    pub scope_lease: String,
    pub family: InteractionFamily,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Envelope {
    pub version: u32,
    pub stamp: Stamp,
    pub documents: List<Document, MAX_RECORDS>,
    pub roots: List<Root, MAX_RECORDS>,
    pub outcomes: List<Outcome, MAX_OUTCOMES>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub discovery_work: Option<DiscoveryWork>,
}

/// Producer work already spent in the same collection. Structural decoding has
/// its own bounded allowance; canonical predicates share what remains of this.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DiscoveryWork {
    pub predicate_steps: u32,
    pub inspections: u32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Node {
    // One per-document DOM-object namespace, extending the existing Element
    // identity registry to Document/ShadowRoot without reusing Element IDs.
    pub document: Id,
    pub node: Id,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Document {
    pub id: Id,
    pub frame_owner: String,
    pub frame_path: String,
    pub epoch: String,
    pub origin: String,
    #[serde(rename = "document_node")]
    pub root_node: Id,
    #[serde(deserialize_with = "explicit_nullable")]
    pub parent_host: Option<Node>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RootKind {
    Document,
    OpenShadow,
    Element,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum Hop {
    OpenShadow {
        host: Node,
        root: Node,
    },
    SameOriginFrame {
        host: Node,
        document: Node,
    },
    AssignedSlot {
        slot: Node,
        assigned: Node,
        owner_document: Id,
    },
}

impl Hop {
    pub fn kind(&self) -> InteractionRootStepKind {
        match self {
            Self::OpenShadow { .. } => InteractionRootStepKind::OpenShadow,
            Self::SameOriginFrame { .. } => InteractionRootStepKind::SameOriginFrame,
            Self::AssignedSlot { .. } => InteractionRootStepKind::AssignedSlot,
        }
    }
    pub fn host(&self) -> Node {
        match self {
            Self::OpenShadow { host, .. } | Self::SameOriginFrame { host, .. } => *host,
            Self::AssignedSlot { slot, .. } => *slot,
        }
    }
    pub fn result(&self) -> (Node, RootKind) {
        match self {
            Self::OpenShadow { root, .. } => (*root, RootKind::OpenShadow),
            Self::SameOriginFrame { document, .. } => (*document, RootKind::Document),
            Self::AssignedSlot { assigned, .. } => (*assigned, RootKind::Element),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Root {
    pub id: Id,
    pub start: Node,
    pub start_kind: RootKind,
    pub hops: List<Hop, 8>,
    pub resolved: Node,
    pub resolved_kind: RootKind,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RuleKind {
    Anchor,
    Collection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Rule {
    pub kind: RuleKind,
    pub id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum UnavailableReason {
    Truncated,
    InspectionLimit,
    PredicateLimit,
    AnnotationLimit,
    UnsupportedRoot,
    Detached,
    MissingEvidence,
    AmbiguousRoot,
    AmbiguousPrimary,
    NestedItems,
    ChangedDocument,
    InvalidPredicate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum Fact {
    Missing {},
    Truncated {},
    CompleteAbsent {},
    CompletePresent { value: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Attribute {
    pub name: String,
    pub fact: Fact,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Candidate {
    pub index: u16,
    pub frame_owner: String,
    pub frame_path: String,
    pub element_id: Id,
    #[serde(deserialize_with = "explicit_nullable")]
    pub related_element_id: Option<Id>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Witness {
    pub candidate: Candidate,
    pub name: Fact,
    pub href: Fact,
    pub role: Fact,
    pub attributes: List<Attribute, 4>,
    pub target_border_box_css: Size,
    pub projected_primary_fragment_css: Rect,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub fragment_index: Option<u8>,
    pub top_viewport_css: Size,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Item {
    #[serde(rename = "item")]
    pub logical_node: Node,
    pub primary_root: Id,
    pub primary: Witness,
    pub attributes: List<Attribute, 4>,
    pub identity_attributes: List<Attribute, 4>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum ObservedPrimary {
    TargetsComplete {
        root: Id,
        targets: List<Witness, MAX_HINT_CANDIDATES>,
    },
    Unavailable {
        reason: UnavailableReason,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ObservedItem {
    #[serde(rename = "item")]
    pub logical_node: Node,
    #[serde(deserialize_with = "explicit_nullable")]
    pub parent_item: Option<Node>,
    pub attributes: List<Attribute, 4>,
    pub identity_attributes: List<Attribute, 4>,
    pub primary: ObservedPrimary,
}

/// A completed negative search classification, never a live binding. Absence
/// requires exhaustive bounded search; ambiguity means at least two distinct
/// physical matches and can be decided without enumerating every match.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SearchCardinality {
    Absent,
    Ambiguous,
}

/// Identifies the authored search without inventing its missing result node.
/// Root searches refer to the successfully resolved prefix before `step`.
/// Per-item primary paths have a separate context and are not represented here.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum SearchStage {
    AnchorScope {},
    CollectionContainer {},
    RootHost { step: u8 },
    AssignedElement { step: u8, slot: Node },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum ResultFacts {
    SearchComplete {
        root: Id,
        stage: SearchStage,
        cardinality: SearchCardinality,
    },
    AnchorComplete {
        root: Id,
        scope: Node,
        scope_kind: RootKind,
        matches: List<Witness, MAX_HINT_CANDIDATES>,
    },
    CollectionComplete {
        root: Id,
        container: Node,
        container_kind: RootKind,
        items: List<Item, MAX_RECORDS>,
    },
    AnchorObserved {
        root: Id,
        scope: Node,
        scope_kind: RootKind,
        targets: List<Witness, MAX_HINT_CANDIDATES>,
    },
    CollectionObserved {
        root: Id,
        container: Node,
        container_kind: RootKind,
        items: List<ObservedItem, MAX_RECORDS>,
    },
    Unavailable {
        reason: UnavailableReason,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Outcome {
    pub rule: Rule,
    pub result: ResultFacts,
}
