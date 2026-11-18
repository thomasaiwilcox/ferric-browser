//! Data projection of an already checked browser capture. No renderer input,
//! label strings, commands, receiver selection or new request authority.
use super::{AnnotationError, AnnotationRequest, INTERACTION_ANNOTATION_VERSION, wire};
use ferric_browser_config::{
    InteractionAttributeTest, InteractionCollectionOrder, InteractionPredicates, InteractionRoot,
    InteractionRootStart, InteractionRootStep, InteractionRootStepKind, InteractionSelector,
    InteractionSelectorRelation,
};
use serde::Serialize;

#[derive(Serialize)]
struct Plan<'a> {
    version: u32,
    observations: bool,
    stamp: &'a wire::Stamp,
    rules: Vec<RulePlan<'a>>,
}

/// Each tuple is (ancestry relation, checked simple CSS compound, atom count).
/// Decoded atoms are unnecessary for native compound matching. Keep their
/// count for the same bounded accounting without inflating short selectors.
#[derive(Serialize)]
struct SelectorPlan {
    steps: Vec<(Option<InteractionSelectorRelation>, String, usize)>,
}

#[derive(Serialize)]
struct RuleIdentity<'a> {
    kind: wire::RuleKind,
    id: &'a str,
}

#[derive(Serialize)]
struct RootPlan<'a> {
    start: InteractionRootStart,
    #[serde(skip_serializing_if = "Option::is_none")]
    receiver_origin: Option<&'a str>,
    steps: Vec<StepPlan>,
}

#[derive(Serialize)]
struct StepPlan {
    kind: InteractionRootStepKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    host_selector: Option<SelectorPlan>,
    #[serde(rename = "self", skip_serializing_if = "Option::is_none")]
    host_self: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assigned_selector: Option<SelectorPlan>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum RulePlan<'a> {
    Anchor(AnchorPlan<'a>),
    Collection(CollectionPlan<'a>),
}

#[derive(Serialize)]
struct AnchorPlan<'a> {
    rule: RuleIdentity<'a>,
    root: RootPlan<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope_selector: Option<SelectorPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_selector: Option<SelectorPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    predicates: Option<&'a InteractionPredicates>,
}

#[derive(Serialize)]
struct CollectionPlan<'a> {
    rule: RuleIdentity<'a>,
    root: RootPlan<'a>,
    slot_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    container_selector: Option<SelectorPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    container_self: Option<bool>,
    item_selector: SelectorPlan,
    primary_root_steps: Vec<StepPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    primary_selector: Option<SelectorPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    primary_self: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    item_attributes: Option<&'a [InteractionAttributeTest]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    primary_predicates: Option<&'a InteractionPredicates>,
    #[serde(skip_serializing_if = "Option::is_none")]
    identity_attributes: Option<&'a [String]>,
    order: InteractionCollectionOrder,
    #[serde(skip_serializing_if = "Option::is_none")]
    row_tolerance_px: Option<f64>,
}

fn compiled_selector(source: &str) -> Result<SelectorPlan, AnnotationError> {
    let checked =
        InteractionSelector::parse(source).map_err(|_| AnnotationError::InvalidInventory)?;
    Ok(SelectorPlan {
        steps: checked
            .steps()
            .iter()
            .map(|step| (step.relation, step.compound.clone(), step.tests.len()))
            .collect(),
    })
}

fn selector(source: Option<&str>) -> Result<Option<SelectorPlan>, AnnotationError> {
    source.map(compiled_selector).transpose()
}

fn steps(source: &[InteractionRootStep]) -> Result<Vec<StepPlan>, AnnotationError> {
    source
        .iter()
        .map(|step| {
            Ok(StepPlan {
                kind: step.kind,
                host_selector: selector(step.host_selector.as_deref())?,
                host_self: step.host_self,
                assigned_selector: selector(step.assigned_selector.as_deref())?,
            })
        })
        .collect()
}

fn root(source: Option<&InteractionRoot>) -> Result<RootPlan<'_>, AnnotationError> {
    Ok(RootPlan {
        start: source.map_or(InteractionRootStart::TopDocument, |root| root.start),
        receiver_origin: source.and_then(|root| root.receiver_origin.as_deref()),
        steps: steps(source.map_or(&[], |root| root.steps.as_slice()))?,
    })
}

impl AnnotationRequest {
    /// Compiles selectors for the exact literal invocation and serializes the
    /// original immutable stamp. Receiver context and live authorization must
    /// still come from the retained browser/broker request, never renderer data.
    /// Labels/remaps are deliberately absent; only collection slot counts cross.
    ///
    /// # Errors
    /// Rejects an inconsistent checked selector inventory or serialization
    /// failure. This does not establish a transport-size or live-proof gate.
    pub fn collector_plan_json(&self) -> Result<String, AnnotationError> {
        let mut rules = Vec::new();
        if self.policy.enabled() {
            for anchor in self
                .policy
                .anchors()
                .iter()
                .filter(|anchor| anchor.families.contains(&self.stamp.family))
            {
                rules.push(RulePlan::Anchor(AnchorPlan {
                    rule: RuleIdentity {
                        kind: wire::RuleKind::Anchor,
                        id: &anchor.id,
                    },
                    root: root(anchor.root.as_ref())?,
                    scope_selector: selector(anchor.scope_selector.as_deref())?,
                    target_selector: selector(anchor.target_selector.as_deref())?,
                    predicates: anchor.predicates.as_ref(),
                }));
            }
            for collection in self
                .policy
                .collections()
                .iter()
                .filter(|collection| collection.families.contains(&self.stamp.family))
            {
                rules.push(RulePlan::Collection(CollectionPlan {
                    rule: RuleIdentity {
                        kind: wire::RuleKind::Collection,
                        id: &collection.id,
                    },
                    root: root(collection.root.as_ref())?,
                    slot_count: collection.codes.len(),
                    container_selector: selector(collection.container_selector.as_deref())?,
                    container_self: collection.container_self,
                    item_selector: compiled_selector(&collection.item_selector)?,
                    primary_root_steps: steps(
                        collection.primary_root_steps.as_deref().unwrap_or_default(),
                    )?,
                    primary_selector: selector(collection.primary_selector.as_deref())?,
                    primary_self: collection.primary_self,
                    item_attributes: collection.item_attributes.as_deref(),
                    primary_predicates: collection.primary_predicates.as_ref(),
                    identity_attributes: collection.identity_attributes.as_deref(),
                    order: collection.order,
                    row_tolerance_px: collection.row_tolerance_px,
                }));
            }
        }
        serde_json::to_string(&Plan {
            version: INTERACTION_ANNOTATION_VERSION,
            observations: true,
            stamp: &self.stamp,
            rules,
        })
        .map_err(|_| AnnotationError::InvalidSchema)
    }
}
