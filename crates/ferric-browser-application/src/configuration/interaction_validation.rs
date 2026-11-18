//! Effective checks for the owner's known open pages, before write admission.
//! Later contexts still compile independently and can become Conflicted.

use super::{
    ConfigurationLayers, ConfigurationPolicySources, PreparedConfigurationUpdate,
    policy_compilation::InteractionSourceCompiler,
};
use crate::BrowserApplication;
use ferric_browser_config::{ConfigError, InteractionPolicyRevision, SiteMatchContext};
use ferric_browser_core::{ExistenceState, ProfileId, ValidatedUrl};
use std::collections::BTreeSet;

/// Browser-owned metadata for the profile whose configuration is being staged.
/// Incoming activation never borrows an outgoing profile's contextual identity.
#[derive(Clone, Copy)]
pub(crate) struct InteractionValidationProfile<'a> {
    pub(crate) scope: Option<ProfileId>,
    pub(crate) name: &'a str,
    pub(crate) generation: u64,
}

impl BrowserApplication {
    /// Checks this exact candidate against both its active-after-publication
    /// sources and prospective reload sources for this owner's open HTTP(S)
    /// pages, including background and suspended pages. This installs nothing
    /// and grants no engine CSS or live-document proof. Publication and writer
    /// admission recheck current contexts through the existing coordinator.
    /// Success means candidate admissibility, not that every open policy is
    /// conflict-free: unchanged pre-existing conflicts can remain closed.
    ///
    /// # Errors
    /// Rejects a stale candidate, invalid sources, an unavailable association
    /// for relevant open contexts, or a newly introduced effective conflict.
    /// Unchanged existing conflicts stay closed so ordinary edits and pending
    /// repairs remain available; they never supply a usable policy.
    pub fn validate_configuration_interaction_contexts(
        &self,
        prepared: &PreparedConfigurationUpdate,
    ) -> Result<(), ConfigError> {
        self.ensure_configuration_writable()?;
        self.check_configuration_preparation(prepared, self.profile_configuration_revision()?)?;
        self.preflight_shared_policy_contexts(prepared).map(|_| ())
    }

    pub(crate) fn validate_open_interaction_sources(
        &self,
        sources: &ConfigurationPolicySources,
    ) -> Result<(), ConfigError> {
        let previous_sources = self.configuration_policy_sources();
        let owner = self
            .active_profile
            .as_ref()
            .map(|profile| InteractionValidationProfile {
                scope: self.interaction_core_profile(),
                name: profile.name(),
                generation: profile.generation,
            });
        self.validate_open_interaction_sources_for_profile(sources, owner, Some(&previous_sources))
    }

    pub(crate) fn validate_open_interaction_sources_for_profile(
        &self,
        sources: &ConfigurationPolicySources,
        owner: Option<InteractionValidationProfile<'_>>,
        previous_sources: Option<&ConfigurationPolicySources>,
    ) -> Result<(), ConfigError> {
        let contexts = CapturedInteractionContexts::capture(self, owner);
        contexts.validate(sources, owner, previous_sources)
    }
}

/// Cold, immutable facts for all relevant open pages. A snapshot grants no
/// action authority and is independent of focus, renderer or resource state.
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct CapturedInteractionContexts {
    coherent: bool,
    pages: Vec<(ferric_browser_core::TabId, Result<ValidatedUrl, ()>)>,
}

impl CapturedInteractionContexts {
    pub(crate) fn capture(
        app: &BrowserApplication,
        owner: Option<InteractionValidationProfile<'_>>,
    ) -> Self {
        let profile = owner.and_then(|owner| owner.scope);
        Self::from_raw_pages(
            Self::membership_is_coherent(app, profile),
            app.tabs()
                .values()
                .filter(|tab| {
                    profile.is_none_or(|profile| tab.profile == profile)
                        && matches!(
                            tab.existence,
                            ExistenceState::Constructing | ExistenceState::Live
                        )
                })
                .filter_map(|tab| tab.url.as_deref().map(|url| (tab.id, url))),
        )
    }

    pub(crate) fn membership_is_coherent(
        app: &BrowserApplication,
        profile: Option<ProfileId>,
    ) -> bool {
        let mut members = BTreeSet::new();
        for window in app
            .windows()
            .values()
            .filter(|window| profile.is_none_or(|profile| window.profile == profile))
        {
            for id in &window.tabs {
                let Some(tab) = app.tabs().get(id) else {
                    return false;
                };
                if tab.window != window.id || tab.profile != window.profile || !members.insert(*id)
                {
                    return false;
                }
            }
        }
        app.tabs()
            .values()
            .filter(|tab| {
                profile.is_none_or(|profile| tab.profile == profile)
                    && matches!(
                        tab.existence,
                        ExistenceState::Constructing | ExistenceState::Live
                    )
            })
            .all(|tab| members.contains(&tab.id))
    }

    pub(crate) fn from_raw_pages<'a>(
        coherent: bool,
        pages: impl IntoIterator<Item = (ferric_browser_core::TabId, &'a str)>,
    ) -> Self {
        let pages = pages
            .into_iter()
            .filter_map(|(id, url)| match ValidatedUrl::parse(url) {
                Ok(url) if SiteMatchContext::for_url(&url).is_some() => Some((id, Ok(url))),
                Ok(_) => None,
                Err(_) => Some((id, Err(()))),
            })
            .collect();
        Self { coherent, pages }
    }

    pub(crate) fn validate(
        &self,
        sources: &ConfigurationPolicySources,
        owner: Option<InteractionValidationProfile<'_>>,
        previous_sources: Option<&ConfigurationPolicySources>,
    ) -> Result<(), ConfigError> {
        let active = InteractionSourceCompiler::new(sources, true)?;
        let prospective_sources = sources.prospective_reload();
        let prospective = InteractionSourceCompiler::new(&prospective_sources, true)?;
        if !has_interaction(sources.desired_layers()) && !has_interaction(sources.reload_layers()) {
            // Unassociated/headless owners retain ordinary global settings.
            // There is no contextual Interaction section to compose here.
            return Ok(());
        }
        let previous_prospective_sources =
            previous_sources.map(ConfigurationPolicySources::prospective_reload);
        // Old sources only supply optional evidence for an unchanged closed
        // conflict. Corrupt historical metadata cannot veto a valid repair.
        let previous_active =
            previous_sources.and_then(|sources| InteractionSourceCompiler::new(sources, true).ok());
        let previous_prospective = previous_prospective_sources
            .as_ref()
            .and_then(|sources| InteractionSourceCompiler::new(sources, true).ok());

        if !self.coherent {
            return Err(unassociated());
        }
        let profile = owner.and_then(|owner| owner.scope);
        for (tab, url) in &self.pages {
            let url = url.as_ref().map_err(|()| {
                ConfigError::validation(format_args!(
                    "An open Interaction context has an invalid URL"
                ))
            })?;
            let Some(profile) = profile else {
                // Raw presence is insufficient: same-ID replacement and URL
                // selection can remove every contextual contribution. Ask the
                // canonical selected feed without inventing a profile ID.
                if active.has_applicable_interaction(url.as_str())
                    || prospective.has_applicable_interaction(url.as_str())
                {
                    return Err(unassociated());
                }
                continue;
            };
            let owner = owner.ok_or_else(unassociated)?;
            let revision = InteractionPolicyRevision {
                profile,
                profile_revision: owner.generation,
                configuration_revision: sources.revision(),
                // Uninstalled source compilation, never a consumer revision.
                policy_revision: 0,
            };
            for (boundary, compiler, previous) in [
                ("active", &active, previous_active.as_ref()),
                (
                    "prospective reload",
                    &prospective,
                    previous_prospective.as_ref(),
                ),
            ] {
                if let Err(error) = compiler.compile(owner.name, revision, url.as_str()) {
                    if previous
                        .is_some_and(|previous| compiler.same_inputs_for(previous, url.as_str()))
                    {
                        // Existing first-use conflicts remain explicitly closed.
                        // An unrelated ordinary edit or valid pending repair
                        // must not require activating the old conflicting rules.
                        continue;
                    }
                    // Identify the actual Core page without retaining its URL,
                    // private query, fragment or title in the diagnostic.
                    return Err(ConfigError::validation(format_args!(
                        "Interaction {boundary} policy conflicts for open tab {tab}: {error}"
                    )));
                }
            }
        }
        Ok(())
    }
}

fn has_interaction(layers: &ConfigurationLayers) -> bool {
    layers
        .base
        .site_rules
        .iter()
        .chain(&layers.runtime.site_rules)
        .chain(&layers.temporary.site_rules)
        .any(|rule| rule.interaction.is_some())
}

fn unassociated() -> ConfigError {
    ConfigError::validation(format_args!(
        "Open Interaction contexts have no coherent associated runtime profile"
    ))
}

#[cfg(test)]
mod tests;
