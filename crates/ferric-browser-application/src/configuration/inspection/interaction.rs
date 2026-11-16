//! Captured source diagnostics. Installed policy revisions and compiled input
//! authority never escape this report or change while it is being produced.
use super::{
    ConfigurationInspectionCandidate, ConfigurationInspectionCandidateStage,
    ConfigurationInspectionInputs, InspectionBoundary, InspectionOrigin, SettingActivationBoundary,
    SettingInspectionPending, SettingInspectionValidation, service::SemanticOrigin,
};
use crate::{InteractionCompilationState, ProfileInstanceId};
use ferric_browser_config::{
    EffectiveInteractionLabels, EffectiveInteractionPolicy, InteractionAnchor,
    InteractionCollection, InteractionContributionPatch, InteractionFamily, InteractionField,
    InteractionLabelField, InteractionPageReceiver, InteractionPolicyRevision, InteractionRemap,
    PolicyFieldOrigin, SiteMatchContext, SitePolicySelectionTrace,
};
use ferric_browser_core::{ProfileId, ValidatedUrl};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug)]
pub struct InteractionInspectionRequest {
    url: ValidatedUrl,
    context: SiteMatchContext,
    explain: bool,
}

impl InteractionInspectionRequest {
    /// Validates a bounded match context without compiling sources. Query and
    /// fragment bytes retain the canonical matcher's existing semantics.
    ///
    /// # Errors
    /// Requires an HTTP(S) context understood by the existing site matcher.
    pub fn new(url: &str, explain: bool) -> Result<Self, String> {
        if url.len() > 8192 || url.chars().any(char::is_control) {
            return Err("Interaction inspection requires a bounded page address".into());
        }
        let url = ValidatedUrl::parse(url).map_err(|error| error.to_string())?;
        let context = SiteMatchContext::for_url(&url)
            .ok_or("Interaction inspection requires an HTTP or HTTPS page address")?;
        Ok(Self {
            url,
            context,
            explain,
        })
    }
}

/// Profile lifetime metadata, with no storage roots or filesystem identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectionProfile {
    pub name: String,
    pub generation: u64,
    pub instance: ProfileInstanceId,
}

/// Plain validated values, not an installed policy or compiled key permit.
#[derive(Clone, Debug, PartialEq)]
pub struct InteractionInspectionValues {
    pub enabled: bool,
    pub labels: EffectiveInteractionLabels,
    pub ordinary_labels: EffectiveInteractionLabels,
    pub anchors: Vec<InteractionAnchor>,
    pub collections: Vec<InteractionCollection>,
    pub hint_page_keys: Vec<String>,
    pub page_receiver: InteractionPageReceiver,
    pub remaps: Vec<InteractionRemap>,
}

impl From<&EffectiveInteractionPolicy> for InteractionInspectionValues {
    fn from(policy: &EffectiveInteractionPolicy) -> Self {
        Self {
            enabled: policy.enabled(),
            labels: policy.labels().clone(),
            ordinary_labels: policy.ordinary_labels().clone(),
            anchors: policy.anchors().to_vec(),
            collections: policy.collections().to_vec(),
            hint_page_keys: policy.hint_page_keys().to_vec(),
            page_receiver: policy.page_receiver().clone(),
            remaps: policy.remaps().to_vec(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InspectedInteractionContribution {
    pub origin: InspectionOrigin,
    pub patch: InteractionContributionPatch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InteractionInspectionExplanation {
    pub values: InteractionInspectionValues,
    /// None is a typed default, distinct from an explicitly supplied default.
    pub fields: BTreeMap<InteractionField, Option<InspectionOrigin>>,
    pub ordinary_labels: BTreeMap<InteractionLabelField, Option<InspectionOrigin>>,
    pub contributions: Vec<InspectedInteractionContribution>,
    pub selection: SitePolicySelectionTrace,
    pub rules_boundary: InspectionBoundary,
    pub globals_boundary: InspectionBoundary,
    /// Checked namespaces only. These carry neither discovered targets nor
    /// installed policy revisions or permission to act on a page.
    pub codebooks: Vec<(InteractionFamily, ferric_browser_core::CheckedHintCodebook)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InspectedInteractionValue {
    pub state: InteractionCompilationState,
    pub enabled: bool,
    pub anchor_count: usize,
    pub collection_count: usize,
    pub hint_page_key_count: usize,
    pub remap_count: usize,
    /// Configured intent only, never proof of codebook/live/action availability.
    pub enhanced_hint_families: Vec<InteractionFamily>,
    pub validation: SettingInspectionValidation,
    /// Detailed key, selector and patch data is retained only on explicit opt-in.
    pub explanation: Option<InteractionInspectionExplanation>,
}

/// Read-only source compilation. There is deliberately no installed semantic
/// revision, `EffectiveInteractionPolicy`, target or action permit in this DTO.
#[derive(Clone, Debug)]
pub struct InteractionInspection {
    pub profile: InspectionProfile,
    pub core_profile: ProfileId,
    pub context: SiteMatchContext,
    pub configuration_revision: u64,
    pub reload_revision: u64,
    pub startup_revision: u64,
    pub candidate_stage: Option<ConfigurationInspectionCandidateStage>,
    pub candidate_revision: Option<u64>,
    pub active: Result<InspectedInteractionValue, String>,
    pub desired: Result<InspectedInteractionValue, String>,
    pub pending: SettingInspectionPending,
}

impl InteractionInspection {
    #[must_use]
    pub fn active_state(&self) -> InteractionCompilationState {
        self.active
            .as_ref()
            .map_or(InteractionCompilationState::Conflicted, |value| value.state)
    }
    #[must_use]
    pub fn desired_state(&self) -> InteractionCompilationState {
        self.desired
            .as_ref()
            .map_or(InteractionCompilationState::Conflicted, |value| value.state)
    }
    #[must_use]
    pub fn active_validation(&self) -> SettingInspectionValidation {
        self.active.as_ref().map_or_else(
            |_| SettingInspectionValidation::unknown(),
            |value| value.validation.clone(),
        )
    }
    #[must_use]
    pub fn desired_validation(&self) -> SettingInspectionValidation {
        self.desired.as_ref().map_or_else(
            |_| SettingInspectionValidation::unknown(),
            |value| value.validation.clone(),
        )
    }
}

#[derive(PartialEq)]
struct InteractionMeaning {
    fields: BTreeMap<InteractionField, Option<SemanticOrigin>>,
    ordinary: BTreeMap<InteractionLabelField, Option<SemanticOrigin>>,
    inputs: Vec<(SemanticOrigin, InteractionContributionPatch)>,
}

struct ComposedInteraction {
    summary: InspectedInteractionValue,
    values: InteractionInspectionValues,
    meaning: InteractionMeaning,
}

impl ConfigurationInspectionInputs {
    /// Compiles independent active and private prospective outcomes over this
    /// immutable capture. This neither refreshes the cache nor mints semantic
    /// revisions, target authority, validation receipts or publication rights.
    ///
    /// # Errors
    /// Requires a real captured application/profile/core context. Compilation
    /// failures remain independent outcomes inside the returned report.
    pub fn inspect_interaction(
        &self,
        request: &InteractionInspectionRequest,
    ) -> Result<InteractionInspection, String> {
        let profile = self
            .profile()
            .ok_or("Interaction inspection has no captured profile")?;
        let instance = self
            .profile_instance()
            .ok_or("Interaction inspection has no profile instance")?;
        let core_profile = self
            .core_profile
            .ok_or("Interaction inspection has no captured runtime profile")?;
        let active = self.compose_interaction(request, false, core_profile);
        let desired = self.compose_interaction(request, true, core_profile);
        let pending = match (&active, &desired) {
            (Ok(active), Ok(desired)) => {
                let value_changed = active.values != desired.values;
                let source_changed = active.meaning != desired.meaning;
                if value_changed || source_changed {
                    self.interaction_pending_boundary(request, desired, core_profile)
                        .map_or(SettingInspectionPending::Unknown, |boundary| {
                            SettingInspectionPending::Pending {
                                boundary,
                                value_changed,
                                source_changed,
                            }
                        })
                } else {
                    SettingInspectionPending::Unchanged
                }
            }
            _ => SettingInspectionPending::Unknown,
        };
        Ok(InteractionInspection {
            profile: InspectionProfile {
                name: profile.name.clone(),
                generation: profile.generation,
                instance,
            },
            core_profile,
            context: request.context.clone(),
            configuration_revision: self.active.revision,
            reload_revision: self.sources.reload_revision(),
            startup_revision: self.sources.startup_revision(),
            candidate_stage: self.candidate.as_ref().map(|candidate| candidate.stage),
            candidate_revision: self
                .candidate
                .as_ref()
                .map(|candidate| candidate.revision()),
            active: active.map(|value| value.summary),
            desired: desired.map(|value| value.summary),
            pending,
        })
    }

    /// # Errors
    /// Requires the same sealed owner and expected active revision as ordinary
    /// setting inspection. Candidate stages confer no extra validation proof.
    pub fn inspect_interaction_with_candidate(
        &self,
        request: &InteractionInspectionRequest,
        candidate: &ConfigurationInspectionCandidate,
    ) -> Result<InteractionInspection, String> {
        self.with_candidate(candidate)?.inspect_interaction(request)
    }

    fn interaction_pending_boundary(
        &self,
        request: &InteractionInspectionRequest,
        desired: &ComposedInteraction,
        core_profile: ProfileId,
    ) -> Option<SettingActivationBoundary> {
        if let Some(candidate) = &self.candidate {
            // with_candidate already proved the same owner/revision. Prepared
            // updates are profile-global, so only this outer read supplies the
            // actual captured Core identity to their private projection.
            let publication = candidate
                .publication
                .as_ref()
                .ok()?
                .compose_interaction(request, false, core_profile)
                .ok()?;
            if publication.values == desired.values && publication.meaning == desired.meaning {
                return Some(SettingActivationBoundary::Publication);
            }
        }
        Some(SettingActivationBoundary::ConfigurationReload)
    }

    fn compose_interaction(
        &self,
        request: &InteractionInspectionRequest,
        desired: bool,
        core_profile: ProfileId,
    ) -> Result<ComposedInteraction, String> {
        let profile = self
            .profile()
            .ok_or("Interaction inspection profile is unavailable")?;
        let sources = if desired {
            self.prospective_sources()
        } else {
            self.sources.clone()
        };
        let inspected = sources
            .inspect_interaction(
                &profile.name,
                InteractionPolicyRevision {
                    profile: core_profile,
                    profile_revision: profile.generation,
                    configuration_revision: sources.revision(),
                    // Internal uninstalled compilation stamp. Neither it nor the
                    // EffectiveInteractionPolicy escapes the plain-value report.
                    policy_revision: 0,
                },
                request.url.as_str(),
            )
            .map_err(|error| error.to_string())?;
        self.project_interaction(inspected, desired, request.explain)
    }

    fn project_interaction(
        &self,
        inspected: crate::InspectedInteractionPolicy,
        desired: bool,
        explain: bool,
    ) -> Result<ComposedInteraction, String> {
        let policy = inspected.policy;
        let fields = InteractionField::ALL
            .into_iter()
            .map(|field| {
                let origin = policy
                    .origin(field)
                    .map(|origin| self.interaction_origin(origin, desired))
                    .transpose()?;
                Ok((field, origin))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let ordinary = InteractionLabelField::ALL
            .into_iter()
            .map(|field| {
                let origin = policy
                    .ordinary_label_origin(field)
                    .map(|origin| self.interaction_origin(origin, desired))
                    .transpose()?;
                Ok((field, origin))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        let contributions = inspected
            .trace
            .into_iter()
            .map(|entry| {
                Ok(InspectedInteractionContribution {
                    origin: self.interaction_origin(&entry.origin, desired)?,
                    patch: entry.patch,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let meaning = InteractionMeaning {
            fields: fields
                .iter()
                .map(|(field, origin)| (*field, origin.as_ref().map(SemanticOrigin::from)))
                .collect(),
            ordinary: ordinary
                .iter()
                .map(|(field, origin)| (*field, origin.as_ref().map(SemanticOrigin::from)))
                .collect(),
            inputs: contributions
                .iter()
                .map(|entry| (SemanticOrigin::from(&entry.origin), entry.patch.clone()))
                .collect(),
        };
        let values = InteractionInspectionValues::from(&policy);
        let codebooks = inspect_codebooks(&policy, explain)?;
        let summary = InspectedInteractionValue {
            state: InteractionCompilationState::for_policy(Some(&policy)),
            enabled: policy.enabled(),
            anchor_count: policy.anchors().len(),
            collection_count: policy.collections().len(),
            hint_page_key_count: policy.hint_page_keys().len(),
            remap_count: policy.remaps().len(),
            enhanced_hint_families: InteractionFamily::ALL
                .into_iter()
                .filter(|family| policy.enhanced_requested_for(*family))
                .collect(),
            validation: SettingInspectionValidation::composed(),
            explanation: explain.then(|| InteractionInspectionExplanation {
                values: values.clone(),
                fields,
                ordinary_labels: ordinary,
                contributions,
                selection: inspected.selection,
                rules_boundary: if desired {
                    self.desired_boundary()
                } else {
                    InspectionBoundary::Reload
                },
                globals_boundary: if desired {
                    self.desired_boundary()
                } else {
                    InspectionBoundary::Desired
                },
                codebooks,
            }),
        };
        Ok(ComposedInteraction {
            summary,
            values,
            meaning,
        })
    }

    fn interaction_origin(
        &self,
        origin: &PolicyFieldOrigin,
        desired: bool,
    ) -> Result<InspectionOrigin, String> {
        let boundary = if desired {
            self.desired_boundary()
        } else if origin.rule().is_some() {
            InspectionBoundary::Reload
        } else {
            InspectionBoundary::Desired
        };
        self.origin(boundary, Arc::new(origin.clone()))
    }
}

fn inspect_codebooks(
    policy: &EffectiveInteractionPolicy,
    explain: bool,
) -> Result<Vec<(InteractionFamily, ferric_browser_core::CheckedHintCodebook)>, String> {
    if !explain {
        return Ok(Vec::new());
    }
    InteractionFamily::ALL
        .into_iter()
        .filter_map(|family| match policy.compile_hint_codebook(family) {
            Ok(Some(compiled)) => Some(Ok((family, compiled.codebook().clone()))),
            Ok(None) => None,
            Err(error) => Some(Err(error.to_string())),
        })
        .collect()
}
