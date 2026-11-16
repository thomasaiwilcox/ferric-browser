use crate::{HintProxyCompositionTrace, ProfileInstanceId, ProfileSnapshot};
use ferric_browser_config::{
    HintSelectorFamilyTrace, PolicyFieldOrigin, SettingMetadata, SitePolicySelectionTrace,
    setting_metadata, setting_supports_site_scope,
};
use ferric_browser_core::ValidatedUrl;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

/// Source catalogue identity. A source document ID is local to this boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionBoundary {
    Desired,
    Reload,
    Startup,
    Candidate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectionOrigin {
    pub boundary: InspectionBoundary,
    pub origin: Arc<PolicyFieldOrigin>,
    pub(super) document: Option<PathBuf>,
}
impl InspectionOrigin {
    /// Available only in an explicitly requested explanation. Presentation
    /// adapters may omit filesystem paths and use catalogue document IDs.
    #[must_use]
    pub fn document_path(&self) -> Option<&Path> {
        self.document.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InspectedSettingContribution {
    pub origin: InspectionOrigin,
    pub path: Vec<String>,
    pub input: toml::Value,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingExplanation {
    Selectors {
        boundary: InspectionBoundary,
        trace: HintSelectorFamilyTrace,
        selection: SitePolicySelectionTrace,
    },
    Proxy {
        globals: InspectionBoundary,
        rules: InspectionBoundary,
        trace: HintProxyCompositionTrace,
        selection: SitePolicySelectionTrace,
    },
    Ordinary {
        /// Literal paths relative to the requested key. None is a typed default.
        fields: BTreeMap<Vec<String>, Option<InspectionOrigin>>,
        contributions: Vec<InspectedSettingContribution>,
        selection: Option<SitePolicySelectionTrace>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionValidationStage {
    Passed,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingInspectionValidation {
    pub raw_static: InspectionValidationStage,
    pub effective_combination: InspectionValidationStage,
    pub engine_css: InspectionValidationStage,
    pub live_document: InspectionValidationStage,
}
impl SettingInspectionValidation {
    pub(super) const fn unknown() -> Self {
        Self {
            raw_static: InspectionValidationStage::Unknown,
            effective_combination: InspectionValidationStage::Unknown,
            engine_css: InspectionValidationStage::Unknown,
            live_document: InspectionValidationStage::Unknown,
        }
    }
    pub(super) const fn composed() -> Self {
        Self {
            raw_static: InspectionValidationStage::Passed,
            effective_combination: InspectionValidationStage::Passed,
            engine_css: InspectionValidationStage::Unknown,
            live_document: InspectionValidationStage::Unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InspectedSettingValue {
    /// None means a known absent/unconfigured value, such as a selector family.
    pub value: Option<toml::Value>,
    pub validation: SettingInspectionValidation,
    pub provenance_diagnostic: Option<String>,
    pub explanation: Option<SettingExplanation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingActivationBoundary {
    Publication,
    ConfigurationReload,
    Startup,
    Restart,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettingInspectionPending {
    Unchanged,
    Pending {
        boundary: SettingActivationBoundary,
        value_changed: bool,
        source_changed: bool,
    },
    /// An unavailable active or desired combination cannot establish equality.
    Unknown,
}

#[derive(Clone, Debug)]
pub struct SettingInspectionRequest {
    pub(super) key: String,
    pub(super) url: Option<ValidatedUrl>,
    pub(super) explain: bool,
    pub(super) metadata: &'static SettingMetadata,
}
impl SettingInspectionRequest {
    /// Validates query syntax and the same canonical URL grammar as consumers.
    /// This performs no source composition or application mutation.
    ///
    /// # Errors
    /// Rejects unknown/unsafe keys, invalid URLs and unsupported site scope.
    pub fn new(key: &str, url: Option<&str>, explain: bool) -> Result<Self, String> {
        if key.is_empty() || key.len() > 256 || key.chars().any(char::is_control) {
            return Err("inspection requires a nonempty, bounded setting key".into());
        }
        let metadata = setting_metadata(key).ok_or_else(|| format!("unknown setting: {key}"))?;
        let url = url
            .map(|url| -> Result<ValidatedUrl, String> {
                if url.len() > 8192 {
                    return Err("inspection URL exceeds 8192 bytes".into());
                }
                let url = ValidatedUrl::parse(url).map_err(|error| error.to_string())?;
                if !url.as_str().starts_with("https://") && !url.as_str().starts_with("http://") {
                    return Err("inspection requires an HTTP or HTTPS URL".into());
                }
                Ok(url)
            })
            .transpose()?;
        if url.is_some() && !setting_supports_site_scope(key) {
            return Err(format!("{key} does not support site scope"));
        }
        Ok(Self {
            key: key.into(),
            url,
            explain,
            metadata,
        })
    }
}

/// Read-only source compilation, not an engine installation or action receipt.
#[derive(Clone, Debug)]
pub struct SettingInspection {
    pub key: String,
    pub metadata: &'static SettingMetadata,
    pub configuration_revision: u64,
    pub reload_revision: u64,
    pub startup_revision: u64,
    pub candidate_stage: Option<super::candidate::ConfigurationInspectionCandidateStage>,
    pub candidate_revision: Option<u64>,
    pub profile: ProfileSnapshot,
    pub profile_instance: ProfileInstanceId,
    pub active: Result<InspectedSettingValue, String>,
    pub desired: Result<InspectedSettingValue, String>,
    pub pending: SettingInspectionPending,
}
