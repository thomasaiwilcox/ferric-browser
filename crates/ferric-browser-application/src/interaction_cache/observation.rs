//! Rejected actual engine observations outlive cache refreshes and document
//! requests. Only a valid observation for the current target clears the latch.
//! Rejecting a user command before issuing navigation must not call this API.
use super::{
    BrowserApplication, ConfigurationOwner, InteractionCacheUnavailable, InteractionPolicyCache,
};
use ferric_browser_core::{ShutdownState, TabId, Target, WindowId};

/// Browser-owned capture of application/profile, window and document lifetime.
/// The trusted adapter captures it with the observation and retains it across
/// any asynchronous work. It is not page data or an action permit.
#[derive(Clone)]
pub struct SitePolicyObservation {
    owner: ConfigurationOwner,
    window: WindowId,
    target: Target,
    generation: u64,
}

/// Adoption can reuse a Core placeholder Target. Keep its observation lifetime
/// after recovery so a pre-adoption capture cannot release a later closure.
#[derive(Clone)]
pub(super) struct ObservedPolicyContext {
    owner: ConfigurationOwner,
    generation: u64,
    rejected: bool,
}

impl std::fmt::Debug for SitePolicyObservation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SitePolicyObservation")
            .field("window", &self.window)
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

impl InteractionPolicyCache {
    pub(super) fn rejected_for(&self, app: &BrowserApplication, window: WindowId) -> bool {
        app.windows()
            .get(&window)
            .and_then(|window| window.active_tab)
            .and_then(|tab| self.observed_contexts.get(&tab))
            .is_some_and(|context| context.rejected && context.owner.belongs_to(app))
    }

    pub(super) fn close_rejected_observation(&mut self) {
        self.clear();
        self.unavailable = Some(InteractionCacheUnavailable::RejectedEngineContext);
    }
}

fn current_target(app: &BrowserApplication, window: WindowId, target: Target) -> bool {
    app.shutdown() == ShutdownState::Running
        && app.capture_target(target.tab) == Some(target)
        && app
            .tabs()
            .get(&target.tab)
            .is_some_and(|tab| tab.window == window)
}

impl BrowserApplication {
    /// Captures a current observation subject. A callback must retain this
    /// capture rather than recapture a retired source after owner replacement.
    #[must_use]
    pub fn capture_site_policy_observation(
        &self,
        window: WindowId,
        target: Target,
    ) -> Option<SitePolicyObservation> {
        current_target(self, window, target).then(|| SitePolicyObservation {
            owner: ConfigurationOwner::capture(self),
            window,
            target,
            generation: self.observation_generation(target.tab),
        })
    }

    pub(crate) fn retire_observed_policy_targets(
        &mut self,
        effects: &[ferric_browser_runtime::RuntimeEffect],
        moved_from: Option<(TabId, WindowId)>,
    ) {
        use ferric_browser_core::Event;
        use ferric_browser_runtime::RuntimeEffect;
        for effect in effects {
            match effect {
                RuntimeEffect::AppliedEvent(Event::TransferTabIn { window, .. }) => {
                    // TransferTabIn selects the imported/reused tab. No adapter
                    // sees effects until this new observation lifetime is closed.
                    if let Some(tab) = self
                        .windows()
                        .get(window)
                        .and_then(|window| window.active_tab)
                    {
                        self.close_adopted_policy_context(*window, tab);
                    }
                }
                RuntimeEffect::AppliedEvent(Event::MoveTab { tab, to_window, .. })
                    if moved_from
                        .is_some_and(|(moved, from)| moved == *tab && from != *to_window) =>
                {
                    self.close_adopted_policy_context(*to_window, *tab);
                }
                RuntimeEffect::AppliedEvent(
                    Event::CloseTab { tab } | Event::TransferTabOut { tab },
                ) => {
                    self.interaction_cache.observed_contexts.remove(tab);
                }
                RuntimeEffect::AppliedEvent(Event::ViewClosed { tab, .. })
                    if !self.tabs().contains_key(tab) =>
                {
                    self.interaction_cache.observed_contexts.remove(tab);
                }
                RuntimeEffect::AppliedEvent(Event::RequestShutdown) => {
                    self.interaction_cache.observed_contexts.clear();
                }
                _ => {}
            }
        }
    }

    fn observation_generation(&self, tab: TabId) -> u64 {
        self.interaction_cache
            .observed_contexts
            .get(&tab)
            .filter(|context| context.owner.belongs_to(self))
            .map_or(0, |context| context.generation)
    }

    fn observation_is_current(&self, observation: &SitePolicyObservation) -> bool {
        observation.owner.belongs_to(self)
            && current_target(self, observation.window, observation.target)
            && observation.generation == self.observation_generation(observation.target.tab)
    }

    fn close_adopted_policy_context(&mut self, window: WindowId, tab: TabId) {
        if let Some(target) = self.capture_target(tab) {
            let _ = self.begin_site_policy_association(window, target);
        }
    }

    /// Starts a new native association observation, closing reads before any
    /// adapter property evaluation. Even an identical reused Core Target gets
    /// a fresh opaque observation lifetime. This grants no positive eligibility.
    #[must_use]
    pub fn begin_site_policy_association(
        &mut self,
        window: WindowId,
        target: Target,
    ) -> Option<SitePolicyObservation> {
        if !current_target(self, window, target) {
            return None;
        }
        let tab = target.tab;
        let Some(generation) = self.interaction_cache.observation_generation.checked_add(1) else {
            self.interaction_cache.exhausted = true;
            self.interaction_cache.clear();
            return None;
        };
        self.interaction_cache.observation_generation = generation;
        self.interaction_cache.observed_contexts.insert(
            tab,
            ObservedPolicyContext {
                owner: ConfigurationOwner::capture(self),
                generation,
                rejected: true,
            },
        );
        let observation = self.capture_site_policy_observation(window, target)?;
        self.reject_observed_interaction_context(&observation);
        Some(observation)
    }

    /// Records an actual engine context which failed browser URL validation.
    /// Returns true only when this closes the active context. Background tabs
    /// retain their own rejection without changing the active capture.
    /// No URL payload is retained and no policy compilation is performed.
    pub fn reject_observed_interaction_context(
        &mut self,
        observation: &SitePolicyObservation,
    ) -> bool {
        let (window, target) = (observation.window, observation.target);
        if !self.observation_is_current(observation) {
            return false;
        }
        // Keep at most one entry for each current tab/profile owner; never
        // accumulate a history of rejected observations or retired tab IDs.
        let mut rejected = std::mem::take(&mut self.interaction_cache.observed_contexts);
        rejected
            .retain(|tab, context| self.tabs().contains_key(tab) && context.owner.belongs_to(self));
        rejected.insert(
            target.tab,
            ObservedPolicyContext {
                owner: observation.owner.clone(),
                generation: observation.generation,
                rejected: true,
            },
        );
        self.interaction_cache.observed_contexts = rejected;
        let active = self.active_window() == Some(window)
            && self
                .windows()
                .get(&window)
                .is_some_and(|window| window.active_tab == Some(target.tab));
        if active {
            self.interaction_cache.close_rejected_observation();
        }
        active
    }

    /// A trusted adapter calls this only after validating a current engine
    /// observation. Requests, focus changes and untrusted/stale probe results
    /// cannot establish observed eligibility by merely naming the old URL.
    /// True means this observation released a rejection for this exact owner.
    pub fn accept_observed_interaction_context(
        &mut self,
        observation: &SitePolicyObservation,
    ) -> bool {
        let (window, target) = (observation.window, observation.target);
        if !self.observation_is_current(observation) {
            return false;
        }
        let context = self
            .interaction_cache
            .observed_contexts
            .get_mut(&target.tab);
        let released = context.is_some_and(|context| {
            let released = context.rejected;
            context.rejected = false;
            released
        });
        if released
            && self.active_window() == Some(window)
            && self
                .windows()
                .get(&window)
                .is_some_and(|window| window.active_tab == Some(target.tab))
        {
            self.invalidate_interaction_snapshot();
        }
        released
    }
}
