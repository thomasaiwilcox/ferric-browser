//! Owned, off-thread preparation of one context configuration-domain update.

use std::{error::Error, fmt};

use ferric_browser_config::{ContextsConfig, validate_contexts_configuration};
use ferric_browser_core::{PrivacyKind, ShutdownState};
use ferric_browser_storage::{ContextDefinitionUpdate, ContextRegistry, ProfileRegistry};

use super::ContextCatalog;
use crate::{BrowserApplication, ProfileSnapshot, configuration::ConfigurationOwner};

#[derive(Debug)]
pub enum ContextReloadError {
    ProfileUnavailable,
    NotAdmitted,
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
            Self::NotAdmitted => formatter.write_str("The context reload was not admitted."),
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
    owner: ConfigurationOwner,
    expected_revision: u64,
    profile_revision: u64,
    reservation: Option<crate::ConfigurationReservation>,
    preparation_identity: Option<std::sync::Arc<()>>,
    profile: ProfileSnapshot,
    profile_lock: Option<ferric_browser_storage::ProfileLock>,
    previous_names: Vec<String>,
    configuration: ContextsConfig,
}

/// A committed catalog whose publication still requires its original owner.
#[derive(Debug)]
pub struct ContextReloadEffect {
    owner: ConfigurationOwner,
    expected_revision: u64,
    profile_revision: u64,
    reservation: Option<crate::ConfigurationReservation>,
    preparation_identity: Option<std::sync::Arc<()>>,
    _profile_lock: Option<ferric_browser_storage::ProfileLock>,
    catalog: ContextCatalog,
    configuration: ContextsConfig,
}

impl ContextReloadEffect {
    #[must_use]
    pub fn configuration(&self) -> &ContextsConfig {
        &self.configuration
    }
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
            owner: ConfigurationOwner::capture(self),
            expected_revision: self.configuration.revision(),
            profile_revision: self
                .profile_configuration_revision()
                .map_err(ContextReloadError::InvalidConfiguration)?,
            reservation: None,
            preparation_identity: None,
            profile_lock: self
                .active_profile
                .as_ref()
                .and_then(crate::profile::ActiveProfile::persistence_lease),
            profile,
            previous_names: previous
                .contexts
                .iter()
                .map(|definition| definition.name.clone())
                .collect(),
            configuration,
        }))
    }

    /// Admits context disk work under the configuration domain's existing gate.
    /// The job and its completion retain that gate when the observer closes.
    ///
    /// # Errors
    /// Rejects foreign/stale preparations and jobs before filesystem mutation.
    pub fn admit_context_reload(
        &self,
        prepared: &crate::PreparedConfigurationUpdate,
        mut job: ContextReloadJob,
    ) -> Result<ContextReloadJob, ferric_browser_config::ConfigError> {
        self.ensure_configuration_writable()?;
        self.check_configuration_owner(&job.owner)?;
        prepared.admission.ensure_fresh()?;
        if job.reservation.is_some()
            || prepared.expected_revision != job.expected_revision
            || prepared.profile_revision != job.profile_revision
        {
            return Err(ferric_browser_config::ConfigError::validation(
                format_args!("The configuration changed before context reload could start."),
            ));
        }
        let shared_contexts = self.preflight_shared_policy_contexts(prepared)?;
        let reservation = self.reserve_configuration_reload()?;
        let session = self
            .active_profile
            .as_ref()
            .ok_or_else(|| {
                ferric_browser_config::ConfigError::validation(format_args!(
                    "The context profile is unavailable."
                ))
            })?
            .session
            .clone();
        let shared = session.0.configuration.state.lock().map_err(|_| {
            ferric_browser_config::ConfigError::validation(format_args!(
                "Profile settings are unavailable."
            ))
        })?;
        self.check_configuration_preparation(prepared, shared.revision)?;
        shared.revision_for(&prepared.state.layers().runtime)?;
        if let Some(contexts) = shared_contexts {
            contexts.check_current(&session.0.configuration)?;
        }
        job.preparation_identity = Some(prepared.preparation_identity.clone());
        job.reservation = Some(reservation);
        Ok(job)
    }

    /// Publishes validated configuration and committed context metadata together.
    /// All freshness checks precede the in-memory update; this performs no I/O.
    /// Returns the configuration snapshot and runtime effects for invalidated
    /// context bindings; adapters must observe those checkpoint/IPC effects.
    /// # Errors
    /// Rejects stale or reserved configuration and results from other owners.
    pub fn publish_context_configuration(
        &mut self,
        mut prepared: crate::PreparedConfigurationUpdate,
        mut effect: ContextReloadEffect,
    ) -> Result<
        (
            crate::ConfigurationSnapshot,
            Vec<ferric_browser_runtime::RuntimeEffect>,
        ),
        ferric_browser_config::ConfigError,
    > {
        if prepared.expected_revision != effect.expected_revision
            || prepared.profile_revision != effect.profile_revision
            || effect.preparation_identity.as_ref().is_none_or(|identity| {
                !std::sync::Arc::ptr_eq(identity, &prepared.preparation_identity)
            })
        {
            return Err(ferric_browser_config::ConfigError::validation(
                format_args!("The configuration changed during context reload. Try again."),
            ));
        }
        if let Some(reservation) = effect.reservation.take() {
            self.release_configuration_reload(reservation)?;
        }
        // This exact immutable candidate was checked before metadata I/O.
        // A later page can be Conflicted without revoking the committed effect.
        prepared.admission = crate::configuration::ConfigurationAdmission::ContextReload;
        let ContextReloadEffect { owner, catalog, .. } = effect;
        self.publish_configuration_transaction(prepared, |app| {
            app.check_configuration_owner(&owner)?;
            app.refresh_context_catalog(&catalog).ok_or_else(|| {
                ferric_browser_config::ConfigError::validation(format_args!(
                    "The context catalog changed during settings reload. Try again."
                ))
            })
        })
    }

    /// Publishes committed metadata only into its original configuration and profile.
    /// This method performs no disk I/O and never changes a window's profile.
    /// The caller publishes the returned routing document with its config result
    /// and observes the accompanying runtime effects.
    ///
    /// # Errors
    /// Rejects results from an owner that closed, changed profile or is starting.
    pub fn apply_context_reload(
        &mut self,
        mut effect: ContextReloadEffect,
    ) -> Result<(ContextsConfig, Vec<ferric_browser_runtime::RuntimeEffect>), ContextReloadError>
    {
        if let Some(reservation) = effect.reservation.take() {
            self.release_configuration_reload(reservation)
                .map_err(|_| ContextReloadError::Stale)?;
        }
        let session = self
            .active_profile
            .as_ref()
            .ok_or(ContextReloadError::Stale)?
            .session
            .clone();
        let shared = session
            .0
            .configuration
            .state
            .lock()
            .map_err(|_| ContextReloadError::Stale)?;
        if !effect.owner.matches(self)
            || self.ensure_configuration_writable().is_err()
            || self.configuration.revision() != effect.expected_revision
            || shared.revision != effect.profile_revision
        {
            return Err(ContextReloadError::Stale);
        }
        let effects = self
            .refresh_context_catalog(&effect.catalog)
            .ok_or(ContextReloadError::Stale)?;
        Ok((effect.configuration, effects))
    }
}

impl ContextReloadJob {
    /// Applies the complete metadata change under the registry's existing lock.
    /// All referenced profiles must already exist. No security profile is created.
    ///
    /// # Errors
    /// Reports missing profiles, unsafe affinity changes and durable I/O errors.
    pub fn run(self) -> Result<ContextReloadEffect, ContextReloadError> {
        if self.reservation.is_none() || self.preparation_identity.is_none() {
            return Err(ContextReloadError::NotAdmitted);
        }
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
            owner: self.owner,
            expected_revision: self.expected_revision,
            profile_revision: self.profile_revision,
            reservation: self.reservation,
            preparation_identity: self.preparation_identity,
            _profile_lock: self.profile_lock,
            catalog: ContextCatalog { roots, registry },
            configuration: self.configuration,
        })
    }
}

#[cfg(test)]
#[path = "context_reload_tests.rs"]
mod tests;
