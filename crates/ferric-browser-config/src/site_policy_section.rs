//! Compile-time consumer contract over the common resolved rule feed. There is
//! no runtime registry, unknown-section storage, or consumer-owned URL matcher.

use std::sync::Arc;

use crate::{PolicyFieldOrigin, SelectedSitePolicyRule, SelectedSitePolicyRules};

/// A concrete section supplies typed patches and an unpublished accumulator.
/// Validation covers its complete captured raw input, including entries which
/// are shadowed, disabled, nonmatching or removed by include-array replacement.
/// An external typed catalogue must verify exact raw source/revision/rule
/// membership before projecting any patch onto a selected rule.
pub trait SitePolicySection {
    type Patch;
    type Accumulator;
    type Snapshot;

    fn validate_inputs(&self, selected: &SelectedSitePolicyRules<'_>) -> Result<(), String>;
    fn initial(&self) -> Self::Accumulator;
    fn patch<'a>(&'a self, selected: &'a SelectedSitePolicyRule<'_>) -> Option<&'a Self::Patch>;
    fn apply(
        &self,
        accumulator: &mut Self::Accumulator,
        patch: &Self::Patch,
        origin: &Arc<PolicyFieldOrigin>,
    ) -> Result<(), String>;
    fn finish(&self, accumulator: Self::Accumulator) -> Result<Self::Snapshot, String>;
}

/// Validates raw inputs, composes in the common resolver's order, and returns
/// only a fully checked consumer snapshot. No partially composed state escapes
/// on an error. The caller's seed and the selected feed remain immutable.
///
/// # Errors
/// Returns the consumer's raw-input, composition or effective-validation error.
pub fn compose_site_policy_section<S: SitePolicySection>(
    selected: &SelectedSitePolicyRules<'_>,
    section: &S,
) -> Result<S::Snapshot, String> {
    section.validate_inputs(selected)?;
    let mut accumulator = section.initial();
    for rule in selected.selected() {
        if let Some(patch) = section.patch(rule) {
            section.apply(&mut accumulator, patch, rule.shared_origin())?;
        }
    }
    section.finish(accumulator)
}
