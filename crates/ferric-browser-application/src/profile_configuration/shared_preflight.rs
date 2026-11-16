//! Weak membership and exact validation captures for actual sharing owners.
//! Existing configuration admission remains the only publication authority.
use super::SharedConfiguration;
use crate::{
    BrowserApplication, ConfigurationPolicySources, PreparedConfigurationUpdate,
    configuration::InteractionValidationProfile,
};
use ferric_browser_config::ConfigError;
use ferric_browser_core::{ProfileId, ShutdownState};
use ferric_browser_runtime::{RuntimeEffect, RuntimeInput};
use std::sync::{Arc, Mutex, Weak};

mod contexts;
mod pending_root;
use contexts::{ContextCapture, ContextStore};
pub(crate) use pending_root::PendingRootOwner;

#[derive(Clone, Eq, PartialEq)]
struct ProfileContext {
    scope: Option<ProfileId>,
    name: String,
    generation: u64,
}
impl ProfileContext {
    fn borrowed(&self) -> InteractionValidationProfile<'_> {
        InteractionValidationProfile {
            scope: self.scope,
            name: &self.name,
            generation: self.generation,
        }
    }
}

struct OwnerInputs {
    sources: Option<ConfigurationPolicySources>,
    profile: Option<ProfileContext>,
    pending: bool,
}
struct OwnerState {
    inputs: Arc<OwnerInputs>,
    contexts: ContextStore,
    busy: usize,
    dirty: bool,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
}
struct Entry {
    owner: Weak<()>,
    state: Arc<Mutex<OwnerState>>,
}
#[derive(Default)]
pub(crate) struct SharedOwnerRegistry {
    entries: Vec<Entry>,
    membership: Arc<()>,
}

/// Only the application owns the marker. Captures never prolong membership.
pub(crate) struct OwnerRegistration {
    group: Weak<SharedConfiguration>,
    marker: Arc<()>,
    state: Arc<Mutex<OwnerState>>,
}

/// A cold profile-open capture reuses the existing incremental context epoch.
/// Weak identities do not keep a retired application or shared root alive.
pub(crate) struct ProfileOpenContexts(ProfileOpenContextKind);

enum ProfileOpenContextKind {
    Registered {
        owner: Weak<()>,
        state: Weak<Mutex<OwnerState>>,
        epoch: Arc<()>,
    },
    Unregistered(u64),
}

impl BrowserApplication {
    pub(crate) fn capture_profile_open_contexts(&self) -> Result<ProfileOpenContexts, ConfigError> {
        self.prepare_shared_policy_contexts();
        let Some(registration) = self.shared_policy_owner.as_ref() else {
            return Ok(ProfileOpenContexts(ProfileOpenContextKind::Unregistered(
                self.revision(),
            )));
        };
        let state = registration.state.lock().map_err(|_| unavailable())?;
        if state.busy != 0 || state.dirty {
            return Err(unavailable());
        }
        Ok(ProfileOpenContexts(ProfileOpenContextKind::Registered {
            owner: Arc::downgrade(&registration.marker),
            state: Arc::downgrade(&registration.state),
            epoch: Arc::clone(&state.contexts.epoch),
        }))
    }

    pub(crate) fn profile_open_contexts_are_current(&self, capture: &ProfileOpenContexts) -> bool {
        match &capture.0 {
            ProfileOpenContextKind::Unregistered(revision) => {
                self.shared_policy_owner.is_none() && *revision == self.revision()
            }
            ProfileOpenContextKind::Registered {
                owner,
                state,
                epoch,
            } => {
                let Some(registration) = self.shared_policy_owner.as_ref() else {
                    return false;
                };
                if !Weak::ptr_eq(owner, &Arc::downgrade(&registration.marker))
                    || !Weak::ptr_eq(state, &Arc::downgrade(&registration.state))
                {
                    return false;
                }
                registration.state.lock().is_ok_and(|state| {
                    state.busy == 0 && !state.dirty && Arc::ptr_eq(epoch, &state.contexts.epoch)
                })
            }
        }
    }
}
impl Drop for OwnerRegistration {
    fn drop(&mut self) {
        if let Some(group) = self.group.upgrade() {
            let mut owners = group
                .owners
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let marker = Arc::downgrade(&self.marker);
            owners
                .entries
                .retain(|entry| !Weak::ptr_eq(&entry.owner, &marker));
            owners.membership = Arc::new(());
        }
    }
}

/// Published before mutation under the same synchronization as final admission.
/// No lock or owner lifetime lease is held while Core/application code runs.
pub(crate) struct OwnerMutationGuard {
    group: Weak<SharedConfiguration>,
    state: Arc<Mutex<OwnerState>>,
    completed: bool,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
}
impl OwnerMutationGuard {
    fn finish(mut self, update: impl FnOnce(&mut OwnerState)) {
        if let Some(group) = self.group.upgrade() {
            let _owners = group
                .owners
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            update(&mut state);
            state.busy -= 1;
        }
        self.completed = true;
    }
}
impl Drop for OwnerMutationGuard {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        if let Some(group) = self.group.upgrade() {
            let _owners = group
                .owners
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // A Dispatch error can occur after earlier events mutated Core.
            // Neither an error nor unwinding can expose obsolete ready facts.
            state.dirty = true;
            state.busy -= 1;
        }
        if let Some(wake) = &self.wake {
            wake();
        }
    }
}

struct CapturedEntry {
    owner: Weak<()>,
    state: Arc<Mutex<OwnerState>>,
    inputs: Arc<OwnerInputs>,
    contexts: ContextCapture,
}
pub(crate) struct SharedContextPreflight {
    group: Weak<SharedConfiguration>,
    membership: Arc<()>,
    entries: Vec<CapturedEntry>,
}
impl SharedContextPreflight {
    /// Called at existing admission under the shared-state mutex. The registry
    /// stays locked across all comparisons, including readiness and busy state.
    pub(crate) fn check_current(
        &self,
        actual: &Arc<SharedConfiguration>,
    ) -> Result<(), ConfigError> {
        if !Weak::ptr_eq(&self.group, &Arc::downgrade(actual)) {
            return Err(stale());
        }
        let owners = actual
            .owners
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !Arc::ptr_eq(&self.membership, &owners.membership) {
            return Err(stale());
        }
        let mut entries = owners
            .entries
            .iter()
            .filter(|entry| entry.owner.strong_count() != 0);
        for captured in &self.entries {
            let Some(current) = entries.next() else {
                return Err(stale());
            };
            if captured.owner.strong_count() == 0
                || !Weak::ptr_eq(&captured.owner, &current.owner)
                || !Arc::ptr_eq(&captured.state, &current.state)
            {
                return Err(stale());
            }
            let state = current
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.busy != 0
                || state.dirty
                || state.inputs.pending
                || !Arc::ptr_eq(&captured.inputs, &state.inputs)
                || !Arc::ptr_eq(&captured.contexts.epoch, &state.contexts.epoch)
            {
                return Err(stale());
            }
        }
        if entries.next().is_some() {
            return Err(stale());
        }
        Ok(())
    }
}
fn stale() -> ConfigError {
    ConfigError::validation(format_args!(
        "Sharing owners or open policy contexts changed before settings admission. Try again."
    ))
}
fn unavailable() -> ConfigError {
    ConfigError::validation(format_args!(
        "A sharing profile is opening or preparing its current policy contexts. Retry after it is ready."
    ))
}

impl SharedConfiguration {
    /// Copy actual owners' existing runtime wakers under short locks, then
    /// invoke after unlocking. Captures and retired owners grant no wake target.
    pub(crate) fn wake_policy_owners(&self) {
        let wakes = {
            let owners = self
                .owners
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            owners
                .entries
                .iter()
                .filter(|entry| entry.owner.strong_count() != 0)
                .filter_map(|entry| {
                    entry
                        .state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .wake
                        .clone()
                })
                .collect::<Vec<_>>()
        };
        for wake in wakes {
            wake();
        }
    }
}

impl BrowserApplication {
    fn validation_profile_context(&self) -> Option<ProfileContext> {
        self.active_profile.as_ref().map(|profile| ProfileContext {
            scope: self.interaction_core_profile(),
            name: profile.name().to_owned(),
            generation: profile.generation,
        })
    }

    pub(crate) fn register_shared_policy_owner(&mut self) {
        let outgoing = self.shared_policy_owner.take();
        let Some(profile) = self.active_profile.as_ref() else {
            return;
        };
        let group = Arc::clone(&profile.session.0.configuration);
        let profile = self.validation_profile_context();
        let state = Arc::new(Mutex::new(OwnerState {
            contexts: ContextStore::capture(self, profile.as_ref()),
            inputs: Arc::new(OwnerInputs {
                sources: Some(self.configuration_policy_sources()),
                profile,
                pending: self.profile_open.is_some() || self.policy_owner_preparing,
            }),
            busy: 0,
            dirty: false,
            wake: self.policy_context_waker.clone(),
        }));
        let registration = OwnerRegistration {
            group: Arc::downgrade(&group),
            marker: Arc::new(()),
            state: Arc::clone(&state),
        };
        {
            let mut owners = group
                .owners
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            owners.entries.push(Entry {
                owner: Arc::downgrade(&registration.marker),
                state,
            });
            owners.membership = Arc::new(());
        }
        self.shared_policy_owner = Some(registration);
        drop(outgoing);
    }

    pub(crate) fn begin_shared_policy_mutation(&self) -> Option<OwnerMutationGuard> {
        let registration = self.shared_policy_owner.as_ref()?;
        let group = registration.group.upgrade()?;
        let _owners = group
            .owners
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut state = registration
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.busy = state
            .busy
            .checked_add(1)
            .expect("simultaneous owner mutation count exhausted");
        Some(OwnerMutationGuard {
            group: Arc::downgrade(&group),
            state: Arc::clone(&registration.state),
            completed: false,
            wake: self.policy_context_waker.clone(),
        })
    }

    pub(crate) fn begin_shared_runtime_mutation(
        &self,
        input: &RuntimeInput,
    ) -> Option<OwnerMutationGuard> {
        contexts::may_change_context(input)
            .then(|| self.begin_shared_policy_mutation())
            .flatten()
    }

    pub(crate) fn finish_shared_runtime_mutation(
        &mut self,
        guard: Option<OwnerMutationGuard>,
        effects: &[RuntimeEffect],
    ) {
        // Retire under the still-published busy state. A shutdown cannot expose
        // unchanged URL facts as ready between guard completion and retirement.
        if self.shutdown() != ShutdownState::Running {
            self.shared_policy_owner.take();
        }
        if let Some(guard) = guard {
            guard.finish(|state| {
                state
                    .contexts
                    .apply(self, state.inputs.profile.as_ref(), effects);
            });
        }
    }

    pub(crate) fn finish_shared_source_mutation(guard: Option<OwnerMutationGuard>) {
        if let Some(guard) = guard {
            guard.finish(|_| {});
        }
    }

    /// Direct bootstrap/compatibility source changes stage outside locks, then
    /// recheck the shared revision/gate before their assignment becomes visible.
    pub(crate) fn begin_shared_source_mutation(
        &self,
        expected: u64,
        runtime: &ferric_browser_config::RuntimeOverrides,
    ) -> Result<Option<OwnerMutationGuard>, ConfigError> {
        let Some(profile) = self.active_profile.as_ref() else {
            return Ok(None);
        };
        let shared = profile
            .session
            .0
            .configuration
            .state
            .lock()
            .map_err(|_| stale())?;
        self.ensure_configuration_writable()?;
        if shared.revision != expected {
            return Err(stale());
        }
        // Direct activation changes only this owner's graph, unlike the
        // prepared publisher that also commits the authoritative Runtime.
        // Preparing owners may select safe bootstrap/fallback layers; ready
        // owners must not silently discard already committed shared settings.
        if !self.policy_owner_preparing
            && shared
                .runtime
                .as_ref()
                .is_some_and(|committed| committed != runtime)
        {
            return Err(ConfigError::validation(format_args!(
                "Configuration activation must retain the committed shared profile settings."
            )));
        }
        Ok(self.begin_shared_policy_mutation())
    }

    /// Shared source Arcs and readiness only; never walks tabs or parses URLs.
    pub(crate) fn refresh_shared_policy_owner(&self, sources_changed: bool) {
        let Some(registration) = self.shared_policy_owner.as_ref() else {
            return;
        };
        let Some(group) = registration.group.upgrade() else {
            return;
        };
        let profile = self.validation_profile_context();
        let pending = self.profile_open.is_some() || self.policy_owner_preparing;
        let sources = sources_changed.then(|| self.configuration_policy_sources());
        let _owners = group
            .owners
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut state = registration
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if sources.is_none() && state.inputs.profile == profile && state.inputs.pending == pending {
            return;
        }
        state.inputs = Arc::new(OwnerInputs {
            sources: sources.or_else(|| state.inputs.sources.clone()),
            profile,
            pending,
        });
    }

    pub(crate) fn update_shared_policy_waker(&self) {
        let Some(registration) = self.shared_policy_owner.as_ref() else {
            return;
        };
        let Some(group) = registration.group.upgrade() else {
            return;
        };
        let _owners = group
            .owners
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registration
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .wake
            .clone_from(&self.policy_context_waker);
    }

    /// Trusted adapters mark pending source/CSS validation before exposing an
    /// incoming owner and clear it only after installing the validated sources.
    pub fn set_policy_owner_preparing(&mut self, preparing: bool) {
        if self.policy_owner_preparing != preparing {
            let mutation = self.begin_shared_policy_mutation();
            self.policy_owner_preparing = preparing;
            self.refresh_shared_policy_owner(false);
            Self::finish_shared_source_mutation(mutation);
        }
    }

    /// Retires an adapter whose view has actually closed, even when its
    /// application and durable profile backend remain alive. Existing admitted
    /// jobs retain their leases; only prospective policy membership is removed.
    /// Repeated retirement is harmless.
    pub fn retire_shared_policy_observer(&mut self) {
        self.policy_owner_preparing = false;
        self.shared_policy_owner.take();
    }

    /// Whether an error left this owner's validation facts awaiting cold repair.
    #[must_use]
    pub fn shared_policy_contexts_pending(&self) -> bool {
        self.shared_policy_owner
            .as_ref()
            .is_some_and(|registration| {
                let state = registration
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state.dirty && state.busy == 0
            })
    }

    /// Rebuild unavailable validation facts in the scheduled preparation turn.
    /// Normal dispatch performs affected-entry updates and needs no scan here.
    /// This grants neither a CSS receipt nor live action authority.
    pub fn prepare_shared_policy_contexts(&self) {
        let Some(registration) = self.shared_policy_owner.as_ref() else {
            return;
        };
        let Some(group) = registration.group.upgrade() else {
            return;
        };
        let inputs = {
            let _owners = group
                .owners
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let state = registration
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !state.dirty || state.busy != 0 {
                return;
            }
            Arc::clone(&state.inputs)
        };
        let contexts = ContextStore::capture(self, inputs.profile.as_ref());
        let _owners = group
            .owners
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut state = registration
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.busy == 0 && Arc::ptr_eq(&inputs, &state.inputs) {
            state.contexts = contexts;
            state.dirty = false;
        }
    }

    /// Capture cold facts without holding membership during parsing/compilation.
    pub(crate) fn preflight_shared_policy_contexts(
        &self,
        prepared: &PreparedConfigurationUpdate,
    ) -> Result<Option<SharedContextPreflight>, ConfigError> {
        self.prepare_shared_policy_contexts();
        self.validate_open_interaction_sources(&prepared.policy_sources())?;
        if prepared.state.layers().runtime == self.configuration.layers().runtime {
            return Ok(None);
        }
        let Some(profile) = self.active_profile.as_ref() else {
            return Ok(None);
        };
        let group = &profile.session.0.configuration;
        let registration = self.shared_policy_owner.as_ref().ok_or_else(stale)?;
        if !Weak::ptr_eq(&registration.group, &Arc::downgrade(group)) {
            return Err(stale());
        }
        let (membership, entries) = {
            let owners = group
                .owners
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let entries = owners
                .entries
                .iter()
                .filter(|entry| entry.owner.strong_count() != 0)
                .map(|entry| Entry {
                    owner: entry.owner.clone(),
                    state: Arc::clone(&entry.state),
                })
                .collect::<Vec<_>>();
            (Arc::clone(&owners.membership), entries)
        };
        let own = Arc::downgrade(&registration.marker);
        if !entries.iter().any(|entry| Weak::ptr_eq(&entry.owner, &own)) {
            return Err(stale());
        }
        let mut captured = Vec::with_capacity(entries.len());
        for entry in entries {
            let (inputs, contexts) = {
                let state = entry
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if state.busy != 0 || state.dirty || state.inputs.pending {
                    return Err(unavailable());
                }
                (Arc::clone(&state.inputs), state.contexts.snapshot())
            };
            if !Weak::ptr_eq(&entry.owner, &own) {
                let previous = inputs.sources.as_ref().ok_or_else(unavailable)?;
                let candidate = previous.with_shared_runtime(&prepared.state.layers().runtime)?;
                contexts.compile_inputs().validate(
                    &candidate,
                    inputs.profile.as_ref().map(ProfileContext::borrowed),
                    Some(previous),
                )?;
            }
            captured.push(CapturedEntry {
                owner: entry.owner,
                state: entry.state,
                inputs,
                contexts,
            });
        }
        Ok(Some(SharedContextPreflight {
            group: Arc::downgrade(group),
            membership,
            entries: captured,
        }))
    }
}

#[cfg(test)]
mod tests;
