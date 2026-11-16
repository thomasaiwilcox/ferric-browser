//! Diagnostic inputs observed by the canonical Interaction composer. These
//! values grant no selection, editing or publication authority.
use super::{InteractionLabelsPatch, InteractionPolicyPatch, PolicyFieldOrigin};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub enum InteractionContributionPatch {
    /// Explicit shared leaves applied to ordinary and enhanced accumulators.
    SharedLabels(InteractionLabelsPatch),
    /// An explicit section applied only to the enhanced accumulator. Empty
    /// sections remain visible as authored no-ops; empty lists remain clears.
    SiteSection(InteractionPolicyPatch),
}

#[derive(Clone, Debug, PartialEq)]
pub struct InteractionContribution {
    pub origin: Arc<PolicyFieldOrigin>,
    pub patch: InteractionContributionPatch,
}

pub type InteractionCompositionTrace = Vec<InteractionContribution>;
