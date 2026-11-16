//! Profile bootstrap has one read/validate/commit coordinator. The adapter may
//! hold a single-use candidate for engine checks without publishing metadata.
use super::{
    AdmissionMode, OpenPhase, PreparedProfile, ProfileOpenCoordinator, ProfileOpenEffect,
    ProfileOpenError, ProfileOpenRequest,
};
use crate::{
    BrowserApplication, ConfigurationLayers, ConfigurationSnapshot,
    configuration::{ConfigurationOwner, ConfigurationState, InteractionValidationProfile},
    profile::ActiveProfile,
    profile_configuration::ProfileOpenContexts,
};
use ferric_browser_config::{Config, ConfigError, RuntimeOverrides};
use ferric_browser_core::{ProfileId, ShutdownState};
use std::sync::{Arc, Weak};

/// Exact, immutable profile inputs awaiting the presentation adapter's engine
/// checks. This handle is neither cloneable nor serializable.
pub struct ProfileOpenCandidate {
    prepared: PreparedProfile,
    seal: ProfileOpenSeal,
}

/// An identity for engine evidence, carrying no storage/publication authority.
#[derive(Clone)]
pub struct ProfileOpenValidationCapture(Weak<()>);

pub(crate) struct PublishedProfileValidation {
    identity: Arc<()>,
    owner: ConfigurationOwner,
}

pub(super) struct ProfileOpenSeal {
    identity: Arc<()>,
    owner: ConfigurationOwner,
    revision: u64,
    generation: u64,
    contexts: ProfileOpenContexts,
    scope: Option<ProfileId>,
    state: ConfigurationState,
    configuration: ConfigurationSnapshot,
}

impl ProfileOpenCandidate {
    #[must_use]
    pub fn layers(&self) -> &ConfigurationLayers {
        self.seal.state.layers()
    }

    #[must_use]
    pub fn configuration(&self) -> &ConfigurationSnapshot {
        &self.seal.configuration
    }

    #[must_use]
    pub fn validation_capture(&self) -> ProfileOpenValidationCapture {
        ProfileOpenValidationCapture(Arc::downgrade(&self.seal.identity))
    }
}

impl ProfileOpenSeal {
    fn check(&self, app: &BrowserApplication) -> Result<(), ProfileOpenError> {
        if !self.owner.belongs_to(app)
            || app.shutdown() != ShutdownState::Running
            || self.revision != app.configuration_revision()
            || self.generation != app.next_profile_generation
            || !app.profile_open_contexts_are_current(&self.contexts)
        {
            return Err(ProfileOpenError::InvalidRequest(
                "The profile owner, configuration or open contexts changed during preparation"
                    .into(),
            ));
        }
        Ok(())
    }
}

impl BrowserApplication {
    /// Opens a profile using application static/effective validation. Adapters
    /// which require engine validation must use the staged entry point instead.
    /// # Errors
    /// Rejects invalid requests, exhausted generations or another pending open.
    pub fn request_profile_open(
        &mut self,
        request: ProfileOpenRequest,
    ) -> Result<(), ProfileOpenError> {
        self.request_profile_open_mode(request, AdmissionMode::Automatic)
    }

    /// Reads profile inputs without admitting durable metadata work. The caller
    /// must validate and admit the exact candidate returned by polling.
    /// # Errors
    /// Returns the same admission errors as [`Self::request_profile_open`].
    pub fn request_profile_open_for_validation(
        &mut self,
        request: ProfileOpenRequest,
    ) -> Result<(), ProfileOpenError> {
        self.request_profile_open_mode(request, AdmissionMode::External)
    }

    fn request_profile_open_mode(
        &mut self,
        request: ProfileOpenRequest,
        mode: AdmissionMode,
    ) -> Result<(), ProfileOpenError> {
        if self.profile_open.is_some() {
            return Err(ProfileOpenError::Busy);
        }
        self.next_profile_generation.checked_add(1).ok_or_else(|| {
            ProfileOpenError::InvalidRequest("profile activation generation is exhausted".into())
        })?;
        let mut coordinator = ProfileOpenCoordinator::spawn()?;
        coordinator.mode = mode;
        coordinator.submit(request)?;
        self.interaction_cache.suspend();
        let mutation = self.begin_shared_policy_mutation();
        self.profile_open = Some(coordinator);
        self.refresh_shared_policy_owner(false);
        Self::finish_shared_source_mutation(mutation);
        Ok(())
    }

    /// Polls only the read/validation stage. Automatic polling cannot take or
    /// admit a candidate belonging to an external engine-validation request.
    pub fn poll_profile_candidate(
        &mut self,
    ) -> Option<Result<ProfileOpenCandidate, ProfileOpenError>> {
        let coordinator = self.profile_open.as_mut()?;
        if coordinator.mode != AdmissionMode::External
            || !matches!(coordinator.phase, OpenPhase::Preparing)
        {
            return None;
        }
        let prepared = coordinator.poll()?;
        let result = prepared.and_then(|prepared| self.make_profile_candidate(prepared));
        match result {
            Ok(candidate) => {
                self.profile_open.as_mut()?.phase =
                    OpenPhase::Awaiting(Arc::downgrade(&candidate.seal.identity));
                Some(Ok(candidate))
            }
            Err(error) => Some(Err(self.retire_profile_open(error))),
        }
    }

    #[must_use]
    pub fn profile_candidate_is_current(&self, candidate: &ProfileOpenCandidate) -> bool {
        self.profile_candidate_matches_request(candidate) && candidate.seal.check(self).is_ok()
    }

    fn profile_candidate_matches_request(&self, candidate: &ProfileOpenCandidate) -> bool {
        candidate.seal.owner.belongs_to(self)
            && self.profile_open.as_ref().is_some_and(|coordinator| {
                matches!(&coordinator.phase, OpenPhase::Awaiting(identity)
                if Weak::ptr_eq(identity, &Arc::downgrade(&candidate.seal.identity)))
            })
    }

    /// Replace the candidate's temporary layer before any engine evidence is
    /// captured. The replacement gets a new, single-use validation identity.
    /// # Errors
    /// Rejects stale candidates or invalid composed source inputs.
    pub fn prepare_profile_candidate_temporary(
        &mut self,
        candidate: ProfileOpenCandidate,
        temporary: RuntimeOverrides,
    ) -> Result<ProfileOpenCandidate, ProfileOpenError> {
        self.replace_profile_candidate(candidate, |prepared| prepared.layers.temporary = temporary)
    }

    /// Select an independently validated built-in fallback after engine failure.
    /// No metadata has been admitted by this operation.
    /// # Errors
    /// Rejects stale candidates or an unavailable fallback context.
    pub fn prepare_profile_candidate_fallback(
        &mut self,
        candidate: ProfileOpenCandidate,
        reason: String,
    ) -> Result<ProfileOpenCandidate, ProfileOpenError> {
        self.replace_profile_candidate(candidate, |prepared| {
            prepared.layers = ConfigurationLayers::new(Config::default());
            prepared.fallback_layers = prepared.layers.clone();
            prepared.configuration_error = Some(reason);
        })
    }

    fn replace_profile_candidate(
        &mut self,
        candidate: ProfileOpenCandidate,
        change: impl FnOnce(&mut PreparedProfile),
    ) -> Result<ProfileOpenCandidate, ProfileOpenError> {
        if !self.profile_candidate_matches_request(&candidate) {
            return Err(stale_candidate());
        }
        if let Err(error) = candidate.seal.check(self) {
            return Err(self.retire_profile_open(error));
        }
        let mut prepared = candidate.prepared;
        change(&mut prepared);
        let candidate = self
            .make_exact_profile_candidate(prepared)
            .map_err(|error| self.retire_profile_open(error))?;
        self.profile_open
            .as_mut()
            .ok_or(ProfileOpenError::WorkerStopped)?
            .phase = OpenPhase::Awaiting(Arc::downgrade(&candidate.seal.identity));
        Ok(candidate)
    }

    /// Admit the exact engine-checked candidate on the existing worker. This
    /// is the only production transition from preparation to metadata I/O.
    /// # Errors
    /// Rejects stale owners/contexts/shared settings or an unavailable worker.
    pub fn admit_profile_candidate(
        &mut self,
        candidate: ProfileOpenCandidate,
    ) -> Result<(), ProfileOpenError> {
        if !self.profile_candidate_matches_request(&candidate) {
            return Err(stale_candidate());
        }
        if let Err(error) = candidate.seal.check(self) {
            return Err(self.retire_profile_open(error));
        }
        if let Some(owner) = candidate.prepared.pending_policy_owner.as_ref() {
            owner
                .check_revision()
                .map_err(|error| self.retire_profile_open(invalid(&error)))?;
        }
        if let Some(proposal) = candidate.prepared.durable.as_ref() {
            proposal.metadata.check_current().map_err(|error| {
                self.retire_profile_open(ProfileOpenError::InvalidRequest(error.to_string()))
            })?;
        }
        let ProfileOpenCandidate { prepared, seal } = candidate;
        self.profile_open
            .as_mut()
            .ok_or(ProfileOpenError::WorkerStopped)?
            .submit_commit(prepared)
            .map_err(|error| self.retire_profile_open(error))?;
        self.profile_open
            .as_mut()
            .ok_or(ProfileOpenError::WorkerStopped)?
            .phase = OpenPhase::Committing(Box::new(seal));
        Ok(())
    }

    /// Poll automatic preparation or an already admitted commit. A request in
    /// external mode remains read-only until its exact candidate is admitted.
    pub fn poll_profile_open(&mut self) -> Option<Result<ProfileOpenEffect, ProfileOpenError>> {
        let coordinator = self.profile_open.as_mut()?;
        match &coordinator.phase {
            OpenPhase::Preparing if coordinator.mode == AdmissionMode::External => return None,
            OpenPhase::Awaiting(identity) => {
                return (identity.strong_count() == 0)
                    .then(|| Err(self.retire_profile_open(stale_candidate())));
            }
            _ => {}
        }
        let committing = matches!(coordinator.phase, OpenPhase::Committing(_));
        let prepared = coordinator.poll()?;
        let mut prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                let error = if committing {
                    ProfileOpenError::AdmittedWorkFailed(error.to_string())
                } else {
                    error
                };
                return Some(Err(self.retire_profile_open(error)));
            }
        };
        if !committing {
            let candidate = match self.make_profile_candidate(prepared) {
                Ok(candidate) => candidate,
                Err(error) => return Some(Err(self.retire_profile_open(error))),
            };
            self.profile_open.as_mut()?.phase =
                OpenPhase::Awaiting(Arc::downgrade(&candidate.seal.identity));
            return match self.admit_profile_candidate(candidate) {
                Ok(()) => None,
                Err(error) => Some(Err(error)),
            };
        }
        if let Some(error) = prepared.preparation_error.take() {
            return Some(Err(
                self.retire_profile_open(ProfileOpenError::InvalidRequest(error))
            ));
        }
        let coordinator = self.profile_open.take()?;
        let OpenPhase::Committing(seal) = coordinator.phase else {
            unreachable!("commit phase checked");
        };
        let result: Result<ProfileOpenEffect, ProfileOpenError> = (|| {
            seal.check(self)?;
            if let Some(owner) = prepared.pending_policy_owner.as_ref() {
                owner.check_revision().map_err(|error| invalid(&error))?;
            }
            // Do not choose a different fallback after admitted metadata I/O.
            self.prepare_profile_configuration(
                prepared.layers.clone(),
                InteractionValidationProfile {
                    scope: seal.scope,
                    name: &prepared.activation.name,
                    generation: seal.generation,
                },
            )
            .map_err(|error| invalid(&error))?;
            let identity = Arc::clone(&seal.identity);
            let effect = self.install_chosen_profile(
                prepared,
                seal.state,
                seal.configuration,
                seal.scope,
                seal.generation,
            )?;
            self.profile_open_validation = Some(PublishedProfileValidation {
                identity,
                owner: ConfigurationOwner::capture(self),
            });
            Ok(effect)
        })();
        Some(result.map_err(|error| {
            self.retire_profile_open(ProfileOpenError::AdmittedWorkFailed(error.to_string()))
        }))
    }

    #[must_use]
    pub fn profile_open_validation_is_current(
        &self,
        capture: &ProfileOpenValidationCapture,
    ) -> bool {
        self.profile_open_validation
            .as_ref()
            .is_some_and(|published| {
                Weak::ptr_eq(&capture.0, &Arc::downgrade(&published.identity))
                    && published.owner.matches(self)
            })
    }

    fn retire_profile_open(&mut self, error: ProfileOpenError) -> ProfileOpenError {
        let mutation = self.begin_shared_policy_mutation();
        self.profile_open = None;
        self.refresh_shared_policy_owner(false);
        self.restore_interaction_cache_after_profile_failure();
        Self::finish_shared_source_mutation(mutation);
        error
    }

    fn make_profile_candidate(
        &self,
        mut prepared: PreparedProfile,
    ) -> Result<ProfileOpenCandidate, ProfileOpenError> {
        if let Some(error) = prepared.preparation_error.take() {
            return Err(ProfileOpenError::InvalidRequest(error));
        }
        let generation = self.next_profile_generation;
        generation.checked_add(1).ok_or_else(|| {
            ProfileOpenError::InvalidRequest("profile activation generation is exhausted".into())
        })?;
        let scope = crate::interaction_scope::capture_core_profile_scope(
            self.runtime.view(),
            prepared.activation.privacy,
        );
        let incoming = InteractionValidationProfile {
            scope,
            name: &prepared.activation.name,
            generation,
        };
        let (state, configuration) =
            match self.prepare_profile_configuration(prepared.layers.clone(), incoming) {
                Ok(candidate) => candidate,
                Err(error) => {
                    prepared
                        .configuration_error
                        .get_or_insert_with(|| error.to_string());
                    self.prepare_profile_configuration(prepared.fallback_layers.clone(), incoming)
                        .map_err(|error| invalid(&error))?
                }
            };
        self.seal_profile_candidate(prepared, state, configuration, scope)
    }

    fn make_exact_profile_candidate(
        &self,
        prepared: PreparedProfile,
    ) -> Result<ProfileOpenCandidate, ProfileOpenError> {
        let scope = crate::interaction_scope::capture_core_profile_scope(
            self.runtime.view(),
            prepared.activation.privacy,
        );
        let (state, configuration) = self
            .prepare_profile_configuration(
                prepared.layers.clone(),
                InteractionValidationProfile {
                    scope,
                    name: &prepared.activation.name,
                    generation: self.next_profile_generation,
                },
            )
            .map_err(|error| invalid(&error))?;
        self.seal_profile_candidate(prepared, state, configuration, scope)
    }

    fn seal_profile_candidate(
        &self,
        mut prepared: PreparedProfile,
        state: ConfigurationState,
        configuration: ConfigurationSnapshot,
        scope: Option<ProfileId>,
    ) -> Result<ProfileOpenCandidate, ProfileOpenError> {
        let generation = self.next_profile_generation;
        generation.checked_add(1).ok_or_else(|| {
            ProfileOpenError::InvalidRequest("profile activation generation is exhausted".into())
        })?;
        prepared.layers = state.layers().clone();
        Ok(ProfileOpenCandidate {
            prepared,
            seal: ProfileOpenSeal {
                identity: Arc::new(()),
                owner: ConfigurationOwner::capture(self),
                revision: self.configuration_revision(),
                generation,
                contexts: self
                    .capture_profile_open_contexts()
                    .map_err(|error| invalid(&error))?,
                scope,
                state,
                configuration,
            },
        })
    }

    fn prepare_profile_configuration(
        &self,
        layers: ConfigurationLayers,
        profile: InteractionValidationProfile<'_>,
    ) -> Result<(ConfigurationState, ConfigurationSnapshot), ConfigError> {
        let mut state = self.configuration.clone();
        let snapshot = state.activate_layers(layers)?;
        self.validate_open_interaction_sources_for_profile(
            &state.policy_sources(),
            Some(profile),
            None,
        )?;
        Ok((state, snapshot))
    }

    #[cfg(test)]
    pub(crate) fn install_prepared_profile(
        &mut self,
        prepared: PreparedProfile,
    ) -> Result<ProfileOpenEffect, ProfileOpenError> {
        let candidate = self.make_profile_candidate(prepared)?;
        let prepared = super::commit_profile(candidate.prepared);
        self.install_chosen_profile(
            prepared,
            candidate.seal.state,
            candidate.seal.configuration,
            candidate.seal.scope,
            candidate.seal.generation,
        )
    }
}

impl BrowserApplication {
    fn install_chosen_profile(
        &mut self,
        mut prepared: PreparedProfile,
        state: ConfigurationState,
        configuration: ConfigurationSnapshot,
        core_profile_scope: Option<ProfileId>,
        generation: u64,
    ) -> Result<ProfileOpenEffect, ProfileOpenError> {
        let next_generation = generation.checked_add(1).ok_or_else(|| {
            ProfileOpenError::InvalidRequest("profile activation generation is exhausted".into())
        })?;
        // Mirrors must describe the selected validated state, including a
        // fallback that discarded rejected profile/runtime overlays.
        let profile_overrides = state.layers().profile.clone();
        let runtime_overrides = state.layers().runtime.clone();
        let _incoming_policy_owner = prepared.pending_policy_owner.take().or_else(|| {
            crate::profile_configuration::PendingRootOwner::for_activation(&prepared.activation)
        });
        let mutation = self.begin_shared_policy_mutation();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        self.drain_session_downloads(deadline);
        if let Err(error) = self
            .storage
            .replace_until(prepared.activation.storage_path.as_deref(), deadline)
        {
            prepared
                .storage_error
                .get_or_insert_with(|| error.to_string());
            prepared.activation.storage_path = None;
            self.storage.close();
        }
        self.page_scripts = prepared.page_scripts;
        self.userscript_root = prepared.userscript_root;
        self.userscript_worker = None;
        self.configuration = state;
        self.next_profile_generation = next_generation;
        self.core_profile_scope = core_profile_scope;
        self.active_profile = Some(ActiveProfile::activate(generation, prepared.activation));
        self.configuration_reservation = self
            .active_profile
            .as_ref()
            .unwrap()
            .session
            .0
            .configuration_gate
            .clone();
        self.register_shared_policy_owner();
        Self::finish_shared_source_mutation(mutation);
        if let Err(error) = self.initialize_profile_configuration_from_runtime(
            prepared
                .saved_runtime_overrides
                .as_ref()
                .unwrap_or(&runtime_overrides),
        ) {
            prepared.configuration_error = Some(error.to_string());
        }
        let profile = self
            .profile_snapshot()
            .ok_or(ProfileOpenError::WorkerStopped)?;
        // Independently opened windows share the first owner's startup source
        // set. Later bootstrap scans can already contain this run's files.
        let recovery_paths = self
            .active_profile
            .as_ref()
            .unwrap()
            .session
            .0
            .recovery_paths
            .get_or_init(|| prepared.recovery_paths)
            .clone();
        Ok(ProfileOpenEffect {
            configuration,
            profile,
            config_sources: prepared.config_sources,
            profile_overrides,
            runtime_overrides,
            session_path: prepared.session_path,
            session_state_root: prepared.session_state_root,
            recovery_paths,
            storage_error: prepared.storage_error,
            configuration_error: prepared.configuration_error,
            context_error: prepared.context_error,
        })
    }
}

fn invalid(error: &ConfigError) -> ProfileOpenError {
    ProfileOpenError::InvalidRequest(error.to_string())
}
fn stale_candidate() -> ProfileOpenError {
    ProfileOpenError::InvalidRequest("The prepared profile candidate is no longer current".into())
}
