//! Owned, off-thread preparation of one context configuration-domain update.

use std::{error::Error, fmt};

use ferric_browser_config::{ContextsConfig, validate_contexts_configuration};
use ferric_browser_core::{PrivacyKind, ShutdownState};
use ferric_browser_storage::{ContextDefinitionUpdate, ContextRegistry, ProfileRegistry};

use super::ContextCatalog;
use crate::{BrowserApplication, ProfileSnapshot};

#[derive(Debug)]
pub enum ContextReloadError {
    ProfileUnavailable,
    InvalidConfiguration(ferric_browser_config::ConfigError),
    ProfileCatalog(Box<dyn Error + Send + Sync>),
    UnknownProfile(String),
    Registry(ferric_browser_storage::ContextRegistryError),
    Stale,
}

impl fmt::Display for ContextReloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProfileUnavailable => {
                formatter.write_str("This window's profile is not ready. Try again shortly.")
            }
            Self::InvalidConfiguration(error) => {
                write!(formatter, "Context configuration is invalid: {error}")
            }
            Self::ProfileCatalog(error) => write!(
                formatter,
                "The context profile catalog could not be read: {error}"
            ),
            Self::UnknownProfile(name) => write!(
                formatter,
                "Context profile '{name}' is unavailable. Check contexts.toml."
            ),
            Self::Registry(error) => {
                write!(formatter, "Context metadata could not be updated: {error}")
            }
            Self::Stale => {
                formatter.write_str("The window changed during configuration reload. Try again.")
            }
        }
    }
}
impl Error for ContextReloadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidConfiguration(error) => Some(error),
            Self::ProfileCatalog(error) => Some(error.as_ref()),
            Self::Registry(error) => Some(error),
            _ => None,
        }
    }
}

/// Captures only validated durable inputs; it contains no UI or page reference.
/// The presentation scheduler executes it after all engine validation succeeds.
#[derive(Debug)]
pub struct ContextReloadJob {
    profile: ProfileSnapshot,
    previous_names: Vec<String>,
    configuration: ContextsConfig,
}

/// A committed catalog whose publication still requires its original owner.
#[derive(Debug)]
pub struct ContextReloadEffect {
    profile: ProfileSnapshot,
    catalog: ContextCatalog,
    configuration: ContextsConfig,
}

impl BrowserApplication {
    /// Prepares context metadata work without filesystem I/O or state changes.
    /// Private and temporary sessions never produce a durable context job.
    ///
    /// # Errors
    /// Rejects malformed documents and unavailable profile owners.
    pub fn prepare_context_reload(
        &self,
        previous: &ContextsConfig,
        configuration: ContextsConfig,
    ) -> Result<Option<ContextReloadJob>, ContextReloadError> {
        validate_contexts_configuration(previous)
            .map_err(ContextReloadError::InvalidConfiguration)?;
        validate_contexts_configuration(&configuration)
            .map_err(ContextReloadError::InvalidConfiguration)?;
        if self.profile_open.is_some() || self.runtime.view().shutdown() != ShutdownState::Running {
            return Err(ContextReloadError::ProfileUnavailable);
        }
        let profile = self
            .profile_snapshot()
            .ok_or(ContextReloadError::ProfileUnavailable)?;
        if profile.privacy != PrivacyKind::Normal {
            return Ok(None);
        }
        if !profile.durable || profile.roots.is_none() {
            return Err(ContextReloadError::ProfileUnavailable);
        }
        Ok(Some(ContextReloadJob {
            profile,
            previous_names: previous
                .contexts
                .iter()
                .map(|definition| definition.name.clone())
                .collect(),
            configuration,
        }))
    }

    /// Publishes committed metadata only into the same live profile generation.
    /// This method performs no disk I/O and never changes a window's profile.
    /// The caller publishes the returned routing document with its config result.
    ///
    /// # Errors
    /// Rejects results from an owner that closed, changed profile or is starting.
    pub fn apply_context_reload(
        &mut self,
        effect: ContextReloadEffect,
    ) -> Result<ContextsConfig, ContextReloadError> {
        if self.profile_snapshot().as_ref() != Some(&effect.profile)
            || self.runtime.view().shutdown() != ShutdownState::Running
            || !self.refresh_context_catalog(&effect.catalog)
        {
            return Err(ContextReloadError::Stale);
        }
        Ok(effect.configuration)
    }
}

impl ContextReloadJob {
    /// Applies the complete metadata change under the registry's existing lock.
    /// All referenced profiles must already exist. No security profile is created.
    ///
    /// # Errors
    /// Reports missing profiles, unsafe affinity changes and durable I/O errors.
    pub fn run(self) -> Result<ContextReloadEffect, ContextReloadError> {
        let roots = self
            .profile
            .roots
            .clone()
            .ok_or(ContextReloadError::ProfileUnavailable)?;
        let profiles = ProfileRegistry::open(&roots)
            .map_err(|error| ContextReloadError::ProfileCatalog(Box::new(error)))?;
        for definition in &self.configuration.contexts {
            if !profiles
                .profiles()
                .iter()
                .any(|profile| profile.name == definition.profile)
            {
                return Err(ContextReloadError::UnknownProfile(
                    definition.profile.clone(),
                ));
            }
        }
        let definitions = self
            .configuration
            .contexts
            .iter()
            .map(|definition| ContextDefinitionUpdate {
                name: definition.name.clone(),
                label: definition.label.clone(),
                profile: definition.profile.clone(),
                sessions: definition.sessions.clone(),
                workspace: definition.workspace.clone(),
                accent: definition.accent.clone(),
                default_target: definition.default_target.clone(),
            })
            .collect::<Vec<_>>();
        let mut registry = ContextRegistry::open(&roots).map_err(ContextReloadError::Registry)?;
        registry
            .reconcile_configured(&self.previous_names, &definitions)
            .map_err(ContextReloadError::Registry)?;
        Ok(ContextReloadEffect {
            profile: self.profile,
            catalog: ContextCatalog { roots, registry },
            configuration: self.configuration,
        })
    }
}

#[cfg(test)]
#[path = "context_reload_tests.rs"]
mod tests;
