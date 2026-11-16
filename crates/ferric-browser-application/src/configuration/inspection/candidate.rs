//! Sealed inspection candidates are separate from active sources. They confer
//! no preparation, validation receipt, persistence or publication authority.
use super::{ConfigurationInspectionInputs, ConfigurationOwner, ConfigurationPolicySources};
use crate::{
    BrowserApplication, ConfigurationLayers, ConfigurationRequestCapture,
    PreparedConfigurationUpdate,
};
use ferric_browser_config::ConfigurationApplyBoundary;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigurationInspectionCandidateStage {
    Unprepared,
    Prepared,
    WritePending,
}

#[derive(Clone, Debug)]
pub struct ConfigurationInspectionCandidate {
    pub(super) owner: ConfigurationOwner,
    pub(super) expected_revision: u64,
    pub(super) sources: ConfigurationPolicySources,
    pub(super) publication: Result<ConfigurationInspectionInputs, String>,
    pub(super) stage: ConfigurationInspectionCandidateStage,
}

impl PartialEq for ConfigurationInspectionCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.owner.same_capture(&other.owner)
            && self.expected_revision == other.expected_revision
            && self.sources == other.sources
            && self.publication == other.publication
            && self.stage == other.stage
    }
}

impl ConfigurationInspectionCandidate {
    /// Exact immutable source boundaries of this sealed candidate. Reading
    /// these does not validate CSS or grant publication permission.
    #[must_use]
    pub const fn sources(&self) -> &ConfigurationPolicySources {
        &self.sources
    }

    #[must_use]
    pub const fn stage(&self) -> ConfigurationInspectionCandidateStage {
        self.stage
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.sources.revision()
    }

    pub(crate) fn write_pending(mut self) -> Self {
        self.stage = ConfigurationInspectionCandidateStage::WritePending;
        self
    }
}

impl ConfigurationInspectionInputs {
    /// Captured candidates remain inspectable after replacement, but callers
    /// must separately check their own CSS/context generation before presenting
    /// them as the current staged operation.
    ///
    /// # Errors
    /// Rejects a candidate belonging to a different owner or active revision.
    pub fn inspect_setting_with_candidate(
        &self,
        request: &super::SettingInspectionRequest,
        candidate: &ConfigurationInspectionCandidate,
    ) -> Result<super::SettingInspection, String> {
        self.with_candidate(candidate)?.inspect_setting(request)
    }

    pub(super) fn with_candidate(
        &self,
        candidate: &ConfigurationInspectionCandidate,
    ) -> Result<Self, String> {
        if self.active.revision != candidate.expected_revision
            || !self.owner.same_capture(&candidate.owner)
        {
            return Err("inspection candidate belongs to another configuration capture".into());
        }
        let mut inputs = self.clone();
        inputs.candidate = Some(Arc::new(candidate.clone()));
        Ok(inputs)
    }

    pub(super) const fn desired_boundary(&self) -> super::InspectionBoundary {
        if self.candidate.is_some() {
            super::InspectionBoundary::Candidate
        } else {
            super::InspectionBoundary::Desired
        }
    }
}

impl BrowserApplication {
    /// Captures raw staged layers without reserving or mutating configuration.
    /// Private projection uses the canonical boundary operation. The CSS/context
    /// adapter owns the separate generation check for current applicability.
    /// Run outside the physical input stack: this composes an owned state clone.
    ///
    /// # Errors
    /// Rejects a stale owner/revision or exhausted candidate revision. Invalid
    /// raw input remains a prospective diagnostic, never an active replacement.
    pub fn capture_configuration_inspection_candidate(
        &self,
        capture: &ConfigurationRequestCapture,
        layers: &ConfigurationLayers,
        boundary: ConfigurationApplyBoundary,
    ) -> Result<ConfigurationInspectionCandidate, String> {
        if !self.configuration_request_is_current(capture) {
            return Err("configuration changed before candidate inspection".into());
        }
        let revision = capture
            .revision
            .checked_add(1)
            .ok_or("inspection candidate revision is exhausted")?;
        let mut sources = self.configuration.policy_sources();
        sources.desired = Arc::new(layers.clone());
        sources.revision = revision;
        let mut state = self.configuration.clone();
        let publication = state
            .update_layers_at(layers.clone(), boundary)
            .map(|_| ConfigurationInspectionInputs {
                active: state.snapshot(),
                sources: state.policy_sources(),
                owner: capture.owner.clone(),
                candidate: None,
                core_profile: None,
            })
            .map_err(|error| error.to_string());
        Ok(ConfigurationInspectionCandidate {
            owner: capture.owner.clone(),
            expected_revision: capture.revision,
            sources,
            publication,
            stage: ConfigurationInspectionCandidateStage::Unprepared,
        })
    }

    /// Checks local ownership/revision only. It does not certify CSS generation,
    /// writer completion, validation or publication of this candidate.
    #[must_use]
    pub fn configuration_inspection_candidate_is_current(
        &self,
        candidate: &ConfigurationInspectionCandidate,
    ) -> bool {
        candidate.owner.matches(self)
            && candidate.expected_revision == self.configuration_revision()
    }
}

impl PreparedConfigurationUpdate {
    /// A read-only preview of this exact prepared update. Access does not grant
    /// publication permission and a caller must retain its own stage/generation.
    #[must_use]
    pub fn inspection_candidate(&self) -> ConfigurationInspectionCandidate {
        let sources = self.policy_sources();
        ConfigurationInspectionCandidate {
            owner: self.owner.clone(),
            expected_revision: self.expected_revision,
            publication: Ok(ConfigurationInspectionInputs {
                active: self.snapshot(),
                sources: sources.clone(),
                owner: self.owner.clone(),
                candidate: None,
                core_profile: None,
            }),
            sources,
            stage: ConfigurationInspectionCandidateStage::Prepared,
        }
    }
}
