//! Independent typed consumers of one immutable configuration/context capture.
//! None of these source-compiled values grant live document or input authority.
use crate::{
    CompiledInteractionSnapshot, ConfigurationPolicySources, EffectiveHintProxyTargets,
    InteractionCompilationState,
};
use ferric_browser_config::{
    EffectiveHintSelectorFamily, HintProxyTargets, HintSelectorFamily, InteractionPolicyRevision,
    SiteMatchContext,
};
use ferric_browser_core::ValidatedUrl;
use std::{collections::BTreeMap, sync::Arc};

/// The existing site matcher supplies HTTP(S) context. Other supported browser
/// URLs have global-only discovery, never an invented site-rule match.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SitePolicyMatchContext(Option<SiteMatchContext>);

impl SitePolicyMatchContext {
    #[must_use]
    pub fn for_url(url: &ValidatedUrl) -> Self {
        Self(SiteMatchContext::for_url(url))
    }

    pub(crate) fn site(context: &SiteMatchContext) -> Self {
        Self(Some(context.clone()))
    }

    pub(crate) const fn supports_interaction(&self) -> bool {
        self.0.is_some()
    }
}

/// A failure in one consumer never substitutes another consumer's result or
/// silently turns a missing result into built-in defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledHintDiscovery {
    selectors: Result<BTreeMap<HintSelectorFamily, EffectiveHintSelectorFamily>, String>,
    proxy_targets: Result<EffectiveHintProxyTargets, String>,
}

impl CompiledHintDiscovery {
    pub(crate) fn compile(sources: &ConfigurationPolicySources, url: &str) -> Self {
        Self {
            selectors: sources
                .compile_hint_selectors(url)
                .map_err(|error| error.to_string()),
            proxy_targets: sources
                .compile_hint_proxy_policy(url)
                .map_err(|error| error.to_string()),
        }
    }

    /// # Errors
    /// Returns the captured bounded selector-compilation diagnostic.
    pub fn selectors(
        &self,
    ) -> Result<&BTreeMap<HintSelectorFamily, EffectiveHintSelectorFamily>, &str> {
        self.selectors.as_ref().map_err(String::as_str)
    }

    /// # Errors
    /// Returns the captured bounded proxy-compilation diagnostic.
    pub fn proxy_targets(&self) -> Result<&HintProxyTargets, &str> {
        self.proxy_policy().map(|value| &value.policy)
    }

    /// # Errors
    /// Returns the captured bounded proxy-compilation diagnostic.
    pub fn proxy_policy(&self) -> Result<&EffectiveHintProxyTargets, &str> {
        self.proxy_targets.as_ref().map_err(String::as_str)
    }
}

/// Shared capture, with independent Interaction and ordinary discovery states.
/// Source revisions describe this exact capture. The policy revision changes
/// only with context, winning provenance or consumer meaning, not a no-op read.
#[derive(Clone)]
pub struct CompiledSitePolicySnapshot {
    revision: InteractionPolicyRevision,
    sources: ConfigurationPolicySources,
    interaction: Option<Arc<CompiledInteractionSnapshot>>,
    discovery: CompiledHintDiscovery,
}

impl std::fmt::Debug for CompiledSitePolicySnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledSitePolicySnapshot")
            .field("revision", &self.revision)
            .field(
                "interaction",
                &self.interaction.as_ref().map(|value| value.state()),
            )
            .finish_non_exhaustive()
    }
}

impl CompiledSitePolicySnapshot {
    pub(crate) fn compile(
        sources: ConfigurationPolicySources,
        profile_name: &str,
        revision: InteractionPolicyRevision,
        url: &ValidatedUrl,
        context: &SitePolicyMatchContext,
    ) -> Self {
        let interaction = context.supports_interaction().then(|| {
            let compiled = sources.compile_interaction(profile_name, revision, url.as_str());
            let (policy, diagnostic) = match compiled {
                Ok(policy) => (Some(policy), None),
                Err(error) => (None, Some(error.to_string())),
            };
            Arc::new(CompiledInteractionSnapshot {
                state: InteractionCompilationState::for_policy(policy.as_ref()),
                revision,
                policy,
                diagnostic,
                sources: sources.clone(),
            })
        });
        let discovery = CompiledHintDiscovery::compile(&sources, url.as_str());
        Self {
            revision,
            sources,
            interaction,
            discovery,
        }
    }

    pub(crate) fn same_meaning(&self, other: &Self) -> bool {
        self.discovery == other.discovery
            && match (&self.interaction, &other.interaction) {
                (Some(left), Some(right)) => match (&left.policy, &right.policy) {
                    (Some(left), Some(right)) => left.same_meaning(right),
                    (None, None) => left.diagnostic == right.diagnostic,
                    _ => false,
                },
                (None, None) => true,
                _ => false,
            }
    }

    pub(crate) fn with_policy_revision(mut self, policy_revision: u64) -> Self {
        self.revision.policy_revision = policy_revision;
        if let Some(interaction) = &mut self.interaction {
            let interaction = Arc::make_mut(interaction);
            interaction.revision.policy_revision = policy_revision;
            interaction.policy = interaction
                .policy
                .take()
                .map(|policy| policy.with_policy_revision(policy_revision));
        }
        self
    }

    #[must_use]
    pub const fn revision(&self) -> InteractionPolicyRevision {
        self.revision
    }

    #[must_use]
    pub const fn sources(&self) -> &ConfigurationPolicySources {
        &self.sources
    }

    #[must_use]
    pub fn interaction(&self) -> Option<&Arc<CompiledInteractionSnapshot>> {
        self.interaction.as_ref()
    }

    #[must_use]
    pub const fn discovery(&self) -> &CompiledHintDiscovery {
        &self.discovery
    }
}
