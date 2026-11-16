//! Legacy discovery consumers use the same source boundaries and selected rule
//! feed as Interaction. A pending rule/selector is never an active input.
use super::{ConfigurationLayers, ConfigurationPolicySources, policy_compilation::checked};
use ferric_browser_config::{
    ConfigError, EffectiveHintSelectorFamily, HintSelectorCompositionTrace, HintSelectorFamily,
    HintSelectorPolicyBuilder, HintSelectorSeedOrigins, PolicyFieldOrigin, PolicySourceIdentity,
    SelectedSitePolicyRules, SitePolicySelectionTrace,
};
use std::{collections::BTreeMap, sync::Arc};

mod proxy;
pub use proxy::{
    EffectiveHintProxyTargets, HintProxyCompositionTrace, HintProxyContribution,
    InspectedHintProxyTargets,
};

type SelectorPolicies = BTreeMap<HintSelectorFamily, EffectiveHintSelectorFamily>;

/// Read-only diagnostic output from the same active-boundary selector compiler.
#[derive(Clone, Debug, PartialEq)]
pub struct InspectedHintSelectors {
    pub policies: SelectorPolicies,
    pub trace: HintSelectorCompositionTrace,
    pub selection: SitePolicySelectionTrace,
}

struct SelectorCompilation {
    policies: SelectorPolicies,
    inspection: Option<(HintSelectorCompositionTrace, SitePolicySelectionTrace)>,
}

#[derive(Clone, Copy)]
enum GlobalSettingSource {
    Profile,
    Generated,
    CommandLine,
    Temporary,
}

#[derive(Clone, Copy)]
enum SettingSource<'a> {
    Global(GlobalSettingSource),
    Site(&'a PolicyFieldOrigin),
}

impl SettingSource<'_> {
    fn origin(
        self,
        profile_name: &str,
        revision: u64,
    ) -> Result<Arc<PolicyFieldOrigin>, ConfigError> {
        let source = match self {
            Self::Site(origin) => return Ok(Arc::new(origin.clone())),
            Self::Global(GlobalSettingSource::Profile) => PolicySourceIdentity::Profile {
                name: profile_name.into(),
            },
            Self::Global(GlobalSettingSource::Generated) => PolicySourceIdentity::Generated,
            Self::Global(GlobalSettingSource::CommandLine) => PolicySourceIdentity::CommandLine,
            Self::Global(GlobalSettingSource::Temporary) => PolicySourceIdentity::Temporary,
        };
        PolicyFieldOrigin::shared(source, revision)
            .map(Arc::new)
            .map_err(|error| invalid(&error))
    }
}

impl ConfigurationPolicySources {
    /// Compiles selectors entirely from the captured reload boundary. Base
    /// include tables have already merged by leaf and are seeded exactly once.
    ///
    /// # Errors
    /// Rejects invalid raw sources, inconsistent catalogues and effective limits.
    pub fn compile_hint_selectors(
        &self,
        url: &str,
    ) -> Result<BTreeMap<HintSelectorFamily, EffectiveHintSelectorFamily>, ConfigError> {
        self.compile_selectors(url, None)
            .map(|compiled| compiled.policies)
    }

    /// Observes active-boundary composition without installing a cache or
    /// promoting desired rules. Profile identity comes from the captured owner.
    ///
    /// # Errors
    /// Rejects invalid/mismatched source catalogues and effective conflicts.
    pub fn inspect_hint_selectors(
        &self,
        url: &str,
        profile_name: &str,
    ) -> Result<InspectedHintSelectors, ConfigError> {
        let compiled = self.compile_selectors(url, Some(profile_name))?;
        let (trace, selection) = compiled
            .inspection
            .ok_or_else(|| invalid("selector inspection trace is unavailable"))?;
        Ok(InspectedHintSelectors {
            policies: compiled.policies,
            trace,
            selection,
        })
    }

    fn compile_selectors(
        &self,
        url: &str,
        profile_name: Option<&str>,
    ) -> Result<SelectorCompilation, ConfigError> {
        checked(self.desired_layers(), self.revision())?;
        let reload = checked(self.reload_layers(), self.reload_revision())?;
        let (selected, selection) = if profile_name.is_some() {
            let (selected, trace) = reload.rules.select_with_trace(url);
            (selected, Some(trace))
        } else {
            (reload.rules.select(url), None)
        };
        let globals = self.reload_layers();
        let mut builder = if profile_name.is_some() {
            self.desired_layers()
                .base_authored
                .validate_base_settings(&self.desired_layers().base)?;
            globals
                .base_authored
                .validate_base_settings(&globals.base)?;
            let seeds = selector_seed_origins(globals, self.reload_revision())?;
            HintSelectorPolicyBuilder::from_base_traced(
                &globals.base.hints.selectors,
                "base",
                &seeds,
            )
        } else {
            HintSelectorPolicyBuilder::from_base(&globals.base.hints.selectors, "base")
        }
        .map_err(|message| invalid(&message))?;
        apply_settings_in_source_order(globals, &selected, |settings, label, source| {
            if let Some(profile_name) = profile_name {
                builder.apply_settings_with_origin(
                    settings,
                    label,
                    &source.origin(profile_name, self.reload_revision())?,
                )
            } else {
                builder.apply_settings(settings, label)
            }
            .map_err(|message| invalid(&message))
        })?;
        if let Some(selection) = selection {
            let (policies, trace) = builder
                .finish_with_trace()
                .map_err(|message| invalid(&message))?;
            Ok(SelectorCompilation {
                policies,
                inspection: Some((trace, selection)),
            })
        } else {
            Ok(SelectorCompilation {
                policies: builder.finish().map_err(|message| invalid(&message))?,
                inspection: None,
            })
        }
    }
}

fn apply_settings_in_source_order(
    globals: &ConfigurationLayers,
    selected: &SelectedSitePolicyRules<'_>,
    mut apply: impl FnMut(
        &BTreeMap<String, toml::Value>,
        &str,
        SettingSource<'_>,
    ) -> Result<(), ConfigError>,
) -> Result<(), ConfigError> {
    apply(
        &globals.profile.settings,
        "profile",
        SettingSource::Global(GlobalSettingSource::Profile),
    )?;
    for layer in 0..3 {
        for rule in selected.selected().iter().filter(|rule| {
            matches!(
                (layer, rule.origin().source()),
                (
                    0,
                    PolicySourceIdentity::InMemory | PolicySourceIdentity::Authored { .. }
                ) | (1, PolicySourceIdentity::Generated)
                    | (2, PolicySourceIdentity::Temporary)
            )
        }) {
            let source = match rule.origin().source() {
                PolicySourceIdentity::InMemory => "in-memory".to_owned(),
                PolicySourceIdentity::Authored { document_id } => format!("authored:{document_id}"),
                PolicySourceIdentity::Generated => "generated".to_owned(),
                PolicySourceIdentity::Temporary => "temporary".to_owned(),
                PolicySourceIdentity::Profile { .. } | PolicySourceIdentity::CommandLine => {
                    unreachable!("unsupported site source rejected during validation")
                }
            };
            apply(
                &rule.rule().set,
                &format!("{source}:site-rule:{}", rule.rule().id),
                SettingSource::Site(rule.origin()),
            )?;
        }
        match layer {
            0 => {}
            1 => {
                apply(
                    &globals.runtime.settings,
                    "generated",
                    SettingSource::Global(GlobalSettingSource::Generated),
                )?;
                apply(
                    &globals.command_line.settings,
                    "command-line",
                    SettingSource::Global(GlobalSettingSource::CommandLine),
                )?;
            }
            2 => apply(
                &globals.temporary.settings,
                "temporary",
                SettingSource::Global(GlobalSettingSource::Temporary),
            )?,
            _ => unreachable!("three supported site layers"),
        }
    }
    Ok(())
}

fn selector_seed_origins(
    globals: &ConfigurationLayers,
    revision: u64,
) -> Result<BTreeMap<HintSelectorFamily, HintSelectorSeedOrigins>, ConfigError> {
    let mut seeds = BTreeMap::new();
    for family in globals.base.hints.selectors.keys() {
        let mut mode = None;
        let mut selectors = None;
        if globals.base_authored.documents().is_empty() {
            let origin = Arc::new(
                PolicyFieldOrigin::shared(PolicySourceIdentity::InMemory, revision)
                    .map_err(|error| invalid(&error))?,
            );
            mode = Some(Arc::clone(&origin));
            selectors = Some(origin);
        } else {
            for (index, document) in globals.base_authored.documents().iter().enumerate() {
                let Some(policy) = document
                    .hints()
                    .and_then(|hints| hints.selectors.as_ref())
                    .and_then(|policies| policies.get(family))
                else {
                    continue;
                };
                if policy.mode.is_none() && policy.selectors.is_none() {
                    continue;
                }
                let document_id = u32::try_from(index)
                    .map_err(|_| invalid("authored source catalogue is too large"))?;
                let origin = Arc::new(
                    PolicyFieldOrigin::shared(
                        PolicySourceIdentity::Authored { document_id },
                        revision,
                    )
                    .map_err(|error| invalid(&error))?,
                );
                if policy.mode.is_some() {
                    mode = Some(Arc::clone(&origin));
                }
                if policy.selectors.is_some() {
                    selectors = Some(origin);
                }
            }
        }
        seeds.insert(
            *family,
            HintSelectorSeedOrigins {
                mode: mode
                    .ok_or_else(|| invalid("configured selector mode has no captured source"))?,
                selectors,
            },
        );
    }
    Ok(seeds)
}

fn invalid(message: &str) -> ConfigError {
    ConfigError::validation(format_args!("{message}"))
}
