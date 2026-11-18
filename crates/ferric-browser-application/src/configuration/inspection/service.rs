use super::{
    ConfigurationInspectionInputs, ConfigurationPolicySources,
    model::{
        InspectedSettingValue, InspectionBoundary, InspectionOrigin, SettingActivationBoundary,
        SettingExplanation, SettingInspection, SettingInspectionPending, SettingInspectionRequest,
        SettingInspectionValidation,
    },
};
use ferric_browser_config::{
    HintSelectorFieldOrigins, HintSelectorInputField, PolicyFieldOrigin, PolicyRuleLocation,
    PolicySourceIdentity, selector_setting_family,
};
use std::{path::PathBuf, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SemanticOrigin {
    source: PolicySourceIdentity,
    rule: Option<PolicyRuleLocation>,
    document: Option<PathBuf>,
}
impl From<&InspectionOrigin> for SemanticOrigin {
    fn from(value: &InspectionOrigin) -> Self {
        Self {
            source: value.origin.source().clone(),
            rule: value.origin.rule().cloned(),
            document: value.document.clone(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct SemanticInput {
    pub source: SemanticOrigin,
    pub path: Vec<String>,
    pub input: toml::Value,
    pub selectors_explicit: Option<bool>,
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct SettingMeaning {
    pub fields: Vec<(
        Vec<String>,
        Option<SemanticOrigin>,
        Option<HintSelectorInputField>,
    )>,
    pub inputs: Vec<SemanticInput>,
}

pub(super) struct ComposedSetting {
    pub value: InspectedSettingValue,
    pub meaning: Option<SettingMeaning>,
}

impl ConfigurationInspectionInputs {
    /// Inspects this immutable application/profile capture. Desired compilation
    /// uses a private prospective source graph and never refreshes active caches.
    /// Active and desired errors are independent; a failed prospective result
    /// cannot replace an independently known active value.
    ///
    /// # Errors
    /// Requires a captured live profile identity. Source/combination failures
    /// are retained separately inside the report, not substituted with fallbacks.
    pub fn inspect_setting(
        &self,
        request: &SettingInspectionRequest,
    ) -> Result<SettingInspection, String> {
        let profile = self
            .profile()
            .ok_or("configuration inspection has no captured profile")?;
        let instance = self
            .profile_instance()
            .ok_or("configuration inspection has no captured profile instance")?;
        let mut active = self.compose_setting(request, false);
        let mut desired = self.compose_setting(request, true);
        let pending = match (&active, &desired) {
            (Ok(active), Ok(desired)) if active.meaning.is_some() && desired.meaning.is_some() => {
                let value_changed = active.value.value != desired.value.value;
                let source_changed = active.meaning != desired.meaning;
                if value_changed || source_changed {
                    self.pending_boundary(request, desired).map_or(
                        SettingInspectionPending::Unknown,
                        |boundary| SettingInspectionPending::Pending {
                            boundary,
                            value_changed,
                            source_changed,
                        },
                    )
                } else {
                    SettingInspectionPending::Unchanged
                }
            }
            _ => SettingInspectionPending::Unknown,
        };
        if !request.explain {
            for compiled in [&mut active, &mut desired].into_iter().flatten() {
                compiled.value.explanation = None;
            }
        }
        Ok(SettingInspection {
            key: request.key.clone(),
            metadata: request.metadata,
            configuration_revision: self.active.revision,
            reload_revision: self.sources.reload_revision(),
            startup_revision: self.sources.startup_revision(),
            candidate_stage: self.candidate.as_ref().map(|candidate| candidate.stage),
            candidate_revision: self
                .candidate
                .as_ref()
                .map(|candidate| candidate.revision()),
            profile: profile.clone(),
            profile_instance: instance,
            active: active.map(|compiled| compiled.value),
            desired: desired.map(|compiled| compiled.value),
            pending,
        })
    }

    fn pending_boundary(
        &self,
        request: &SettingInspectionRequest,
        desired: &ComposedSetting,
    ) -> Option<SettingActivationBoundary> {
        if let Some(candidate) = &self.candidate {
            let publication = candidate
                .publication
                .as_ref()
                .ok()?
                .compose_setting(request, false)
                .ok()?;
            if publication.value.value == desired.value.value
                && publication.meaning == desired.meaning
            {
                return Some(SettingActivationBoundary::Publication);
            }
        }
        Some(match request.metadata.apply_time {
            "startup" => SettingActivationBoundary::Startup,
            "restart" => SettingActivationBoundary::Restart,
            _ => SettingActivationBoundary::ConfigurationReload,
        })
    }

    fn compose_setting(
        &self,
        request: &SettingInspectionRequest,
        desired: bool,
    ) -> Result<ComposedSetting, String> {
        let profile_name = &self
            .profile()
            .ok_or("inspection profile is unavailable")?
            .name;
        let sources = if desired {
            self.prospective_sources()
        } else {
            self.sources.clone()
        };
        let boundary = if desired {
            self.desired_boundary()
        } else {
            InspectionBoundary::Reload
        };
        let url = request
            .url
            .as_ref()
            .map_or("", ferric_browser_core::ValidatedUrl::as_str);
        if let Some(family) = selector_setting_family(&request.key) {
            let mut inspected = sources
                .inspect_hint_selectors(url, profile_name)
                .map_err(|error| error.to_string())?;
            let policy = inspected
                .policies
                .remove(&family)
                .ok_or("selector result is missing its family")?;
            let trace = inspected
                .trace
                .remove(&family)
                .ok_or("selector result is missing its trace")?;
            let value = policy
                .policy
                .map(toml::Value::try_from)
                .transpose()
                .map_err(|error| error.to_string())?;
            let mut meaning = SettingMeaning::default();
            self.selector_seed_meaning(&trace.seed, boundary, &mut meaning)?;
            for contribution in &trace.contributions {
                let origin = self.origin(boundary, Arc::clone(&contribution.source))?;
                meaning.inputs.push(SemanticInput {
                    source: SemanticOrigin::from(&origin),
                    path: Vec::new(),
                    input: toml::Value::try_from(&contribution.policy)
                        .map_err(|error| error.to_string())?,
                    selectors_explicit: Some(contribution.selectors_explicit),
                });
            }
            return Ok(ComposedSetting {
                value: InspectedSettingValue {
                    value,
                    validation: SettingInspectionValidation::composed(),
                    provenance_diagnostic: None,
                    explanation: Some(SettingExplanation::Selectors {
                        boundary,
                        trace,
                        selection: inspected.selection,
                    }),
                },
                meaning: Some(meaning),
            });
        }
        if request.key == "hints.proxy_targets" {
            return self.compose_proxy(&sources, url, profile_name, boundary);
        }
        super::ordinary::compose(self, request, desired)
    }

    fn compose_proxy(
        &self,
        sources: &ConfigurationPolicySources,
        url: &str,
        profile: &str,
        rules: InspectionBoundary,
    ) -> Result<ComposedSetting, String> {
        let inspected = sources
            .inspect_hint_proxy_targets(url, profile)
            .map_err(|error| error.to_string())?;
        let globals = if rules == InspectionBoundary::Candidate {
            rules
        } else {
            InspectionBoundary::Desired
        };
        let mut meaning = SettingMeaning::default();
        for contribution in inspected
            .trace
            .seed
            .iter()
            .chain(&inspected.trace.contributions)
        {
            let boundary = if contribution.origin.rule().is_some() {
                rules
            } else {
                globals
            };
            let origin = self.origin(boundary, Arc::clone(&contribution.origin))?;
            meaning.inputs.push(SemanticInput {
                source: SemanticOrigin::from(&origin),
                path: Vec::new(),
                input: toml::Value::try_from(&contribution.value)
                    .map_err(|error| error.to_string())?,
                selectors_explicit: None,
            });
        }
        Ok(ComposedSetting {
            value: InspectedSettingValue {
                value: Some(
                    toml::Value::try_from(inspected.effective.policy)
                        .map_err(|error| error.to_string())?,
                ),
                validation: SettingInspectionValidation::composed(),
                provenance_diagnostic: None,
                explanation: Some(SettingExplanation::Proxy {
                    globals,
                    rules,
                    trace: inspected.trace,
                    selection: inspected.selection,
                }),
            },
            meaning: Some(meaning),
        })
    }

    fn selector_seed_meaning(
        &self,
        fields: &HintSelectorFieldOrigins,
        boundary: InspectionBoundary,
        meaning: &mut SettingMeaning,
    ) -> Result<(), String> {
        let entries = fields
            .mode
            .iter()
            .chain(&fields.selectors)
            .chain(&fields.empty_list);
        for value in entries {
            let origin = self.origin(boundary, Arc::clone(&value.source))?;
            meaning.fields.push((
                Vec::new(),
                Some(SemanticOrigin::from(&origin)),
                Some(value.input),
            ));
        }
        Ok(())
    }

    pub(super) fn prospective_sources(&self) -> ConfigurationPolicySources {
        let sources = self
            .candidate
            .as_ref()
            .map_or(&self.sources, |candidate| &candidate.sources);
        sources.prospective_reload()
    }

    pub(super) fn origin(
        &self,
        boundary: InspectionBoundary,
        origin: Arc<PolicyFieldOrigin>,
    ) -> Result<InspectionOrigin, String> {
        let layers = self.layers(boundary);
        let document = if let PolicySourceIdentity::Authored { document_id } = origin.source() {
            Some(
                layers
                    .base_authored
                    .documents()
                    .get(*document_id as usize)
                    .ok_or("inspection source document is outside its captured catalogue")?
                    .path()
                    .to_owned(),
            )
        } else {
            None
        };
        Ok(InspectionOrigin {
            boundary,
            origin,
            document,
        })
    }

    pub(super) fn layers(
        &self,
        boundary: InspectionBoundary,
    ) -> &super::super::ConfigurationLayers {
        match boundary {
            InspectionBoundary::Desired => self.sources.desired_layers(),
            InspectionBoundary::Reload => self.sources.reload_layers(),
            InspectionBoundary::Startup => self.sources.startup_layers(),
            InspectionBoundary::Candidate => self
                .candidate
                .as_ref()
                .expect("candidate boundary requires captured candidate")
                .sources
                .desired_layers(),
        }
    }
}
