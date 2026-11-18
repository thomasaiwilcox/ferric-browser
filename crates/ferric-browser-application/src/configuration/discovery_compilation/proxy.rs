//! Scalar proxy inspection observes the production assignment path. Global
//! inputs are live; selected site inputs retain their own reload revision.
use super::{ConfigurationPolicySources, apply_settings_in_source_order, checked, invalid};
use ferric_browser_config::{
    ConfigError, HintProxyTargets, PolicyFieldOrigin, PolicySourceIdentity,
    SitePolicySelectionTrace,
};
use std::sync::Arc;

/// Explicit proxy contributions in precedence order, ending with the winner.
/// Defaults have no authored contribution. Source catalogues remain owned by
/// the immutable configuration capture, rather than by these legacy labels.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveHintProxyTargets {
    pub policy: HintProxyTargets,
    pub provenance: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HintProxyContribution {
    pub origin: Arc<PolicyFieldOrigin>,
    pub value: HintProxyTargets,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HintProxyCompositionTrace {
    /// Explicit leaves retained by the loader, in include application order.
    /// The merged base is seeded once, rather than replaying these inputs.
    pub seed: Vec<HintProxyContribution>,
    /// Actual scalar replacements observed after the base seed.
    pub contributions: Vec<HintProxyContribution>,
}

impl HintProxyCompositionTrace {
    /// None denotes a typed default, not an explicit `off` contribution.
    #[must_use]
    pub fn winning_origin(&self) -> Option<&PolicyFieldOrigin> {
        self.contributions
            .last()
            .or_else(|| self.seed.last())
            .map(|contribution| contribution.origin.as_ref())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InspectedHintProxyTargets {
    pub effective: EffectiveHintProxyTargets,
    pub trace: HintProxyCompositionTrace,
    pub selection: SitePolicySelectionTrace,
}

struct ProxyCompilation {
    effective: EffectiveHintProxyTargets,
    inspection: Option<(HintProxyCompositionTrace, SitePolicySelectionTrace)>,
}

impl ConfigurationPolicySources {
    /// Compiles live explicit proxy globals around reload-scoped matching rules.
    /// Absent global leaves contribute nothing, including default-valued leaves.
    ///
    /// # Errors
    /// Rejects invalid raw sources, inconsistent catalogues or malformed values.
    pub fn compile_hint_proxy_targets(&self, url: &str) -> Result<HintProxyTargets, ConfigError> {
        self.compile_hint_proxy_policy(url)
            .map(|value| value.policy)
    }

    /// Captures proxy values and explicit legacy source labels together.
    ///
    /// # Errors
    /// Rejects the same invalid inputs as `compile_hint_proxy_targets`.
    pub fn compile_hint_proxy_policy(
        &self,
        url: &str,
    ) -> Result<EffectiveHintProxyTargets, ConfigError> {
        self.compile_proxy(url, None).map(|value| value.effective)
    }

    /// Observes the canonical proxy compiler without publishing configuration.
    /// Profile identity must come from the same captured application owner.
    ///
    /// # Errors
    /// Rejects invalid sources and inconsistent catalogue/base pairing.
    pub fn inspect_hint_proxy_targets(
        &self,
        url: &str,
        profile_name: &str,
    ) -> Result<InspectedHintProxyTargets, ConfigError> {
        let compiled = self.compile_proxy(url, Some(profile_name))?;
        let (trace, selection) = compiled
            .inspection
            .ok_or_else(|| invalid("proxy inspection trace is unavailable"))?;
        Ok(InspectedHintProxyTargets {
            effective: compiled.effective,
            trace,
            selection,
        })
    }

    fn compile_proxy(
        &self,
        url: &str,
        profile_name: Option<&str>,
    ) -> Result<ProxyCompilation, ConfigError> {
        checked(self.desired_layers(), self.revision())?;
        let reload = checked(self.reload_layers(), self.reload_revision())?;
        let (selected, selection) = if profile_name.is_some() {
            let (selected, trace) = reload.rules.select_with_trace(url);
            (selected, Some(trace))
        } else {
            (reload.rules.select(url), None)
        };
        let globals = self.desired_layers();
        let mut trace = if profile_name.is_some() {
            globals
                .base_authored
                .validate_base_settings(&globals.base)?;
            self.reload_layers()
                .base_authored
                .validate_base_settings(&self.reload_layers().base)?;
            Some(self.proxy_seed_trace()?)
        } else {
            None
        };
        let mut policy = globals.base.hints.proxy_targets.clone();
        let mut provenance = if globals.base_authored.documents().is_empty() {
            vec!["base".to_owned()]
        } else {
            globals
                .base_authored
                .documents()
                .iter()
                .enumerate()
                .filter(|(_, document)| {
                    document
                        .hints()
                        .is_some_and(|hints| hints.proxy_targets.is_some())
                })
                .map(|(index, _)| format!("authored:{index}"))
                .collect()
        };
        apply_settings_in_source_order(globals, &selected, |settings, label, source| {
            if let Some(value) = settings.get("hints.proxy_targets") {
                policy = value.clone().try_into().map_err(|error| {
                    ConfigError::validation(format_args!(
                        "{label} has an invalid hints.proxy_targets value: {error}"
                    ))
                })?;
                provenance.push(label.to_owned());
                if let (Some(trace), Some(profile_name)) = (&mut trace, profile_name) {
                    trace.contributions.push(HintProxyContribution {
                        origin: source.origin(profile_name, self.revision())?,
                        value: policy.clone(),
                    });
                }
            }
            Ok(())
        })?;
        Ok(ProxyCompilation {
            effective: EffectiveHintProxyTargets { policy, provenance },
            inspection: trace.zip(selection),
        })
    }

    fn proxy_seed_trace(&self) -> Result<HintProxyCompositionTrace, ConfigError> {
        let globals = self.desired_layers();
        let mut trace = HintProxyCompositionTrace::default();
        if globals.base_authored.documents().is_empty() {
            trace.seed.push(HintProxyContribution {
                origin: Arc::new(
                    PolicyFieldOrigin::shared(PolicySourceIdentity::InMemory, self.revision())
                        .map_err(|error| invalid(&error))?,
                ),
                value: globals.base.hints.proxy_targets.clone(),
            });
        } else {
            for (index, document) in globals.base_authored.documents().iter().enumerate() {
                let Some(value) = document
                    .hints()
                    .and_then(|hints| hints.proxy_targets.as_ref())
                else {
                    continue;
                };
                let document_id = u32::try_from(index)
                    .map_err(|_| invalid("authored source catalogue is too large"))?;
                trace.seed.push(HintProxyContribution {
                    origin: Arc::new(
                        PolicyFieldOrigin::shared(
                            PolicySourceIdentity::Authored { document_id },
                            self.revision(),
                        )
                        .map_err(|error| invalid(&error))?,
                    ),
                    value: value.clone(),
                });
            }
        }
        Ok(trace)
    }
}
