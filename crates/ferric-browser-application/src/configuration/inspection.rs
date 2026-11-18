//! Immutable inputs for read-only inspection. Capturing them neither composes
//! a prospective policy nor installs a consumer or application revision.

use super::{ConfigurationOwner, ConfigurationPolicySources, ConfigurationSnapshot};
use ferric_browser_config::ConfigError;
use std::sync::Arc;

mod candidate;
#[cfg(test)]
mod historical_tests;
mod interaction;
mod model;
mod ordinary;
mod service;
pub use candidate::{ConfigurationInspectionCandidate, ConfigurationInspectionCandidateStage};
pub use interaction::{
    InspectedInteractionContribution, InspectedInteractionValue, InspectionProfile,
    InteractionInspection, InteractionInspectionExplanation, InteractionInspectionRequest,
    InteractionInspectionValues,
};
pub use model::{
    InspectedSettingContribution, InspectedSettingValue, InspectionBoundary, InspectionOrigin,
    InspectionValidationStage, SettingActivationBoundary, SettingExplanation, SettingInspection,
    SettingInspectionPending, SettingInspectionRequest, SettingInspectionValidation,
};

/// An active view and its exact source boundaries captured in one application
/// borrow. Validation and prospective compilation can run on these owned inputs
/// outside the physical command stack; they grant no publication permission.
#[derive(Clone, Debug)]
pub struct ConfigurationInspectionInputs {
    active: ConfigurationSnapshot,
    sources: ConfigurationPolicySources,
    owner: ConfigurationOwner,
    candidate: Option<Arc<ConfigurationInspectionCandidate>>,
    /// Context metadata only. Global configuration ownership never depends on
    /// a selected page, and prepared candidates do not manufacture this ID.
    core_profile: Option<ferric_browser_core::ProfileId>,
}

impl PartialEq for ConfigurationInspectionInputs {
    fn eq(&self, other: &Self) -> bool {
        self.active == other.active
            && self.sources == other.sources
            && Arc::ptr_eq(&self.owner.marker, &other.owner.marker)
            && self.owner.profile == other.owner.profile
            && self.owner.instance == other.owner.instance
            && self.candidate == other.candidate
            && self.core_profile == other.core_profile
    }
}

impl ConfigurationInspectionInputs {
    #[must_use]
    pub fn profile(&self) -> Option<&crate::ProfileSnapshot> {
        self.owner.profile.as_ref()
    }

    #[must_use]
    pub const fn profile_instance(&self) -> Option<crate::ProfileInstanceId> {
        self.owner.instance
    }
    #[must_use]
    pub const fn active(&self) -> &ConfigurationSnapshot {
        &self.active
    }

    #[must_use]
    pub const fn sources(&self) -> &ConfigurationPolicySources {
        &self.sources
    }

    /// Validates captured catalogues against their bases and runs the same raw
    /// static validation as the policy compilers. This does not establish engine
    /// CSS validation, a URL-specific effective result or live-document proof.
    ///
    /// # Errors
    /// Rejects inconsistent catalogues, invalid raw settings/rules or unsupported
    /// source shapes. The active application and every consumer remain untouched.
    pub fn validate_sources(&self) -> Result<(), ConfigError> {
        self.sources.validate_raw_sources()
    }
}

impl crate::BrowserApplication {
    #[must_use]
    pub fn configuration_inspection_inputs(&self) -> ConfigurationInspectionInputs {
        ConfigurationInspectionInputs {
            active: self.configuration.snapshot(),
            sources: self.configuration.policy_sources(),
            owner: ConfigurationOwner::capture(self),
            candidate: None,
            core_profile: self.interaction_core_profile(),
        }
    }

    /// Presentation freshness only; this grants no mutation authority. Old
    /// captures remain inspectable but cannot replace another owner's display.
    #[must_use]
    pub fn configuration_inspection_is_current(
        &self,
        inputs: &ConfigurationInspectionInputs,
    ) -> bool {
        inputs.owner.matches(self) && inputs.active.revision == self.configuration_revision()
    }
}

#[cfg(test)]
mod tests {
    use crate::{BrowserApplication, ConfigurationLayers};
    use ferric_browser_config::{Config, load};

    #[test]
    fn inspection_validation_rejects_a_valid_but_mismatched_base_without_effects() {
        let loaded = load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../ferric-browser-config/tests/fixtures/site-policy/includes/root.toml"
        ))
        .unwrap();
        let mut layers = ConfigurationLayers::new(loaded.config);
        layers.base_authored = loaded.authored;
        let mut app = BrowserApplication::new(Config::default());
        app.activate_configuration(layers.clone()).unwrap();
        let captured = app.configuration_inspection_inputs();
        captured.validate_sources().unwrap();
        assert_eq!(captured.active().revision, captured.sources().revision());
        layers.base.ui.font_size_pt = 20.0;
        let valid = app.configuration_snapshot();
        assert!(app.activate_configuration(layers.clone()).is_err());
        assert_eq!(app.configuration_snapshot(), valid);
        // Historical corruption remains inspectable, but no public activation
        // method needs to accept inconsistent metadata to construct this case.
        app.configuration.activate_layers(layers).unwrap();
        let before = app.configuration_snapshot();
        let sources = app.configuration_policy_sources();
        let count = app.interaction_compilation_count();
        let invalid = app.configuration_inspection_inputs();
        assert!(
            invalid
                .validate_sources()
                .unwrap_err()
                .to_string()
                .contains("ui.font_size_pt")
        );
        assert_eq!(app.configuration_snapshot(), before);
        assert_eq!(app.configuration_policy_sources(), sources);
        assert_eq!(app.interaction_compilation_count(), count);
        // Later publication cannot change an earlier immutable inspection.
        captured.validate_sources().unwrap();
        assert!(captured.active().revision < invalid.active().revision);
    }
}
