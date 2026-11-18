//! Browser-owned collector roles. A native ancestor is not a collector merely
//! because its owner appears in the selected frame's receipt chain.
use std::collections::BTreeSet;

use ferric_browser_config::is_canonical_interaction_origin;

use super::{AnnotationError, validation};

/// Facts captured from the actual selected native receiver. The empty owner
/// means that its Document remains in Top's existing collector namespace.
#[derive(Clone, Debug)]
pub struct AnnotationFocusedReceiver {
    pub owner: String,
    pub origin: String,
    /// Native frame-chain association, independent of shadow-root ancestry.
    pub depth: u8,
    /// Captured native frame hops plus ancestor open-shadow roots. This must
    /// be available before discovery receives its remaining depth allowance.
    pub composed_depth: u8,
}

/// No renderer deserializer exists for this authority. Native callers must
/// first verify selected-view ownership, the full receipt and terminal focus.
#[derive(Clone, Debug, Default)]
pub struct AnnotationDocumentRoles {
    structural_owners: BTreeSet<String>,
    focused: Option<AnnotationFocusedReceiver>,
}

impl AnnotationDocumentRoles {
    /// Top is always the canonical empty-owner collector. Structural owners
    /// may occur only in parent ancestry, never in semantic evidence.
    ///
    /// # Errors
    /// Rejects malformed/duplicate/overlapping owners, more than seven native
    /// ancestors, or a selected receiver without canonical origin/depth facts.
    pub fn capture(
        structural_owners: impl IntoIterator<Item = String>,
        focused: Option<AnnotationFocusedReceiver>,
    ) -> Result<Self, AnnotationError> {
        let mut structural = BTreeSet::new();
        for owner in structural_owners {
            if structural.len() == 7
                || owner.is_empty()
                || !validation::valid_owner(&owner)
                || !structural.insert(owner)
            {
                return Err(AnnotationError::InvalidAssociation);
            }
        }
        if focused.as_ref().is_some_and(|receiver| {
            !validation::valid_owner(&receiver.owner)
                || structural.contains(&receiver.owner)
                || !is_canonical_interaction_origin(&receiver.origin)
                || !(1..=8).contains(&receiver.depth)
                || receiver.composed_depth < receiver.depth
                || receiver.composed_depth > 8
        }) {
            return Err(AnnotationError::InvalidAssociation);
        }
        Ok(Self {
            structural_owners: structural,
            focused,
        })
    }

    pub(super) fn contains(&self, owner: &str) -> bool {
        self.is_collector(owner) || self.structural_owners.contains(owner)
    }

    pub(super) fn is_collector(&self, owner: &str) -> bool {
        owner.is_empty()
            || self
                .focused
                .as_ref()
                .is_some_and(|receiver| receiver.owner == owner)
    }

    pub(super) fn focused(&self) -> Option<&AnnotationFocusedReceiver> {
        self.focused.as_ref()
    }
}
