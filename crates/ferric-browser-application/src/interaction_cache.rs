//! One current source-compilation result. Refresh runs at navigation and
//! publication boundaries; readers never compile or fall back to another site.
use crate::{BrowserApplication, ConfigurationPolicySources, configuration::ConfigurationOwner};
use ferric_browser_config::{
    EffectiveInteractionPolicy, InteractionPolicyRevision, SiteMatchContext,
};
use ferric_browser_core::{ProfileId, TabId, ValidatedUrl, WindowId, canonicalize};
use std::{collections::BTreeMap, sync::Arc};

mod observation;
mod snapshot;
pub use observation::SitePolicyObservation;
pub use snapshot::{CompiledHintDiscovery, CompiledSitePolicySnapshot, SitePolicyMatchContext};

/// Source compilation only. `Ready` still requires codebook, live-document,
/// receiver and native-delivery qualification before any action is permitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionCompilationState {
    Disabled,
    NoOp,
    Conflicted,
    Ready,
}

impl InteractionCompilationState {
    pub(crate) fn for_policy(policy: Option<&EffectiveInteractionPolicy>) -> Self {
        match policy {
            None => Self::Conflicted,
            Some(policy) if !policy.enabled() => Self::Disabled,
            Some(policy)
                if policy.labels() == policy.ordinary_labels()
                    && policy.anchors().is_empty()
                    && policy.collections().is_empty()
                    && policy.hint_page_keys().is_empty()
                    && policy.remaps().is_empty() =>
            {
                Self::NoOp
            }
            Some(_) => Self::Ready,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionCacheUnavailable {
    OwnerNotReady,
    UnsupportedContext,
    RevisionExhausted,
    PendingRefresh,
    RejectedEngineContext,
}

/// An immutable capture; retaining it does not keep it current or grant input
/// authority. Obtain current captures through the context-checked lookup.
#[derive(Clone)]
pub struct CompiledInteractionSnapshot {
    state: InteractionCompilationState,
    revision: InteractionPolicyRevision,
    policy: Option<EffectiveInteractionPolicy>,
    diagnostic: Option<String>,
    sources: ConfigurationPolicySources,
}

impl CompiledInteractionSnapshot {
    #[must_use]
    pub const fn state(&self) -> InteractionCompilationState {
        self.state
    }

    #[must_use]
    pub const fn revision(&self) -> InteractionPolicyRevision {
        self.revision
    }

    #[must_use]
    pub fn policy(&self) -> Option<&EffectiveInteractionPolicy> {
        self.policy.as_ref()
    }

    #[must_use]
    pub fn diagnostic(&self) -> Option<&str> {
        self.diagnostic.as_deref()
    }

    #[must_use]
    pub const fn sources(&self) -> &ConfigurationPolicySources {
        &self.sources
    }
}

impl std::fmt::Debug for CompiledInteractionSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledInteractionSnapshot")
            .field("state", &self.state)
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct CacheEntry {
    owner: ConfigurationOwner,
    window: WindowId,
    tab: TabId,
    profile: ProfileId,
    context: SitePolicyMatchContext,
    url: ValidatedUrl,
    runtime_url: Option<String>,
    runtime_target: Option<ferric_browser_core::Target>,
    snapshot: Arc<CompiledSitePolicySnapshot>,
}

#[derive(Clone)]
struct SuspendedContext {
    window: WindowId,
    tab: TabId,
    url: ValidatedUrl,
    runtime_url: Option<String>,
    runtime_target: Option<ferric_browser_core::Target>,
}

#[derive(Clone, Default)]
pub(crate) struct InteractionPolicyCache {
    entry: Option<CacheEntry>,
    semantic_revision: u64,
    unavailable: Option<InteractionCacheUnavailable>,
    exhausted: bool,
    dirty: bool,
    suspended_context: Option<SuspendedContext>,
    observed_contexts: BTreeMap<TabId, observation::ObservedPolicyContext>,
    observation_generation: u64,
}

impl InteractionPolicyCache {
    pub(crate) fn retire(&mut self) {
        self.clear();
        self.observed_contexts.clear();
    }

    pub(crate) fn clear(&mut self) {
        self.entry = None;
        self.dirty = false;
        self.suspended_context = None;
        self.unavailable = Some(if self.exhausted {
            InteractionCacheUnavailable::RevisionExhausted
        } else {
            InteractionCacheUnavailable::OwnerNotReady
        });
    }

    pub(crate) fn suspend(&mut self) {
        let context = self.entry.as_ref().map(|entry| SuspendedContext {
            window: entry.window,
            tab: entry.tab,
            url: entry.url.clone(),
            runtime_url: entry.runtime_url.clone(),
            runtime_target: entry.runtime_target,
        });
        self.clear();
        self.suspended_context = context;
    }

    fn retain_pending_profile_context(
        &mut self,
        app: &BrowserApplication,
        window: WindowId,
        url: ValidatedUrl,
    ) {
        self.clear();
        if app.profile_open.is_some() {
            self.suspended_context = app
                .windows()
                .get(&window)
                .and_then(|window_state| window_state.active_tab)
                .map(|tab| SuspendedContext {
                    window,
                    tab,
                    url,
                    runtime_url: app.tabs().get(&tab).and_then(|tab| tab.url.clone()),
                    runtime_target: app.capture_target(tab),
                });
        }
    }

    fn current_scope(app: &BrowserApplication, window: WindowId) -> Option<(ProfileId, TabId)> {
        let profile = app.interaction_core_profile()?;
        if app.profile_open.is_some()
            || app.active_window() != Some(window)
            || app.runtime_view().shutdown() != ferric_browser_core::ShutdownState::Running
        {
            return None;
        }
        let window = app.runtime_view().windows().get(&window)?;
        if window.profile != profile {
            return None;
        }
        let tab = window.active_tab?;
        let page = app.tabs().get(&tab)?;
        // Core maintains active_tab membership. Check its actual page domain
        // without scanning the tab vector on every physical-input cache read.
        (page.profile == profile && page.window == window.id).then_some((profile, tab))
    }

    fn refresh(
        &mut self,
        app: &BrowserApplication,
        sources: ConfigurationPolicySources,
        window: WindowId,
        url: ValidatedUrl,
    ) {
        if self.rejected_for(app, window) {
            self.close_rejected_observation();
            return;
        }
        if self.exhausted {
            self.entry = None;
            self.unavailable = Some(InteractionCacheUnavailable::RevisionExhausted);
            self.dirty = false;
            return;
        }
        let context = SitePolicyMatchContext::for_url(&url);
        let Some((profile, tab)) = Self::current_scope(app, window) else {
            self.retain_pending_profile_context(app, window, url);
            return;
        };
        let previous = self.entry.as_ref().filter(|entry| {
            entry.owner.matches(app)
                && entry.window == window
                && entry.tab == tab
                && entry.profile == profile
                && entry.context == context
        });
        if previous.is_some_and(|entry| entry.snapshot.sources().revision() == sources.revision()) {
            let entry = self.entry.as_mut().expect("matching entry exists");
            entry.url = url;
            entry.runtime_url = app.tabs().get(&tab).and_then(|tab| tab.url.clone());
            entry.runtime_target = app.capture_target(tab);
            self.dirty = false;
            self.unavailable = (!context.supports_interaction())
                .then_some(InteractionCacheUnavailable::UnsupportedContext);
            return;
        }
        let active = app.profile_snapshot().expect("current scope has a profile");
        let revision = InteractionPolicyRevision {
            profile,
            // The local profile lifetime, not a peer's durable file revision.
            profile_revision: active.generation,
            configuration_revision: sources.revision(),
            policy_revision: self.semantic_revision,
        };
        #[allow(
            deprecated,
            reason = "Atomic::try_update is unavailable on MSRV Rust 1.88"
        )]
        let _ = app.interaction_compilations.fetch_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |count| Some(count.saturating_add(1)),
        );
        let compiled =
            CompiledSitePolicySnapshot::compile(sources, &active.name, revision, &url, &context);
        let unchanged = previous.is_some_and(|entry| {
            same_catalogues(entry.snapshot.sources(), compiled.sources())
                && entry.snapshot.same_meaning(&compiled)
        });
        if !unchanged {
            let Some(next) = self.semantic_revision.checked_add(1) else {
                self.entry = None;
                self.exhausted = true;
                self.dirty = false;
                self.unavailable = Some(InteractionCacheUnavailable::RevisionExhausted);
                return;
            };
            self.semantic_revision = next;
        }
        let supports_interaction = context.supports_interaction();
        let snapshot = Arc::new(compiled.with_policy_revision(self.semantic_revision));
        self.entry = Some(CacheEntry {
            owner: ConfigurationOwner::capture(app),
            window,
            tab,
            profile,
            context,
            url,
            runtime_url: app.tabs().get(&tab).and_then(|tab| tab.url.clone()),
            runtime_target: app.capture_target(tab),
            snapshot,
        });
        self.unavailable =
            (!supports_interaction).then_some(InteractionCacheUnavailable::UnsupportedContext);
        self.dirty = false;
    }
}

fn same_catalogues(left: &ConfigurationPolicySources, right: &ConfigurationPolicySources) -> bool {
    [
        (left.desired_layers(), right.desired_layers()),
        (left.reload_layers(), right.reload_layers()),
    ]
    .into_iter()
    .all(|(left, right)| {
        left.base_authored
            .documents()
            .iter()
            .map(ferric_browser_config::ConfigurationSourceDocument::path)
            .eq(right
                .base_authored
                .documents()
                .iter()
                .map(ferric_browser_config::ConfigurationSourceDocument::path))
    })
}

impl BrowserApplication {
    /// Opt-in diagnostic count of shared source compilations, including
    /// rejected consumer stages. The historical name is retained for callers.
    /// No key/page data is recorded and this counter grants no authority.
    #[must_use]
    pub fn interaction_compilation_count(&self) -> u64 {
        self.interaction_compilations
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Close reads immediately while retaining the last comparison capture.
    /// The adapter schedules refresh on its existing runtime-work timer; this
    /// method never compiles and may be used by a physical key handler.
    pub fn invalidate_interaction_snapshot(&mut self) {
        self.interaction_cache.dirty = true;
        self.interaction_cache.unavailable = Some(InteractionCacheUnavailable::PendingRefresh);
    }

    pub(crate) fn restore_interaction_cache_after_profile_failure(&mut self) {
        if let Some(suspended) = self.interaction_cache.suspended_context.take() {
            let current = self
                .windows()
                .get(&suspended.window)
                .and_then(|window| window.active_tab)
                .and_then(|tab| self.tabs().get(&tab));
            // Preserve the trusted adapter address only while its complete
            // observation survives. During provisional navigation the model
            // still contains the previous document's committed URL.
            let url = current.and_then(|tab| {
                if tab.id == suspended.tab
                    && tab.url == suspended.runtime_url
                    && self.capture_target(tab.id) == suspended.runtime_target
                {
                    Some(suspended.url.as_str().to_owned())
                } else if tab.loading == ferric_browser_core::LoadingState::Provisional {
                    None
                } else {
                    tab.url.clone()
                }
            });
            if let Some(url) = url {
                self.refresh_interaction_snapshot(suspended.window, &url);
            }
        }
    }

    /// Navigation/SPA/active-tab boundary. Invalid URLs clear the current
    /// result; supported non-site URLs retain global discovery. Never call
    /// this from a physical key handler.
    pub fn refresh_site_policy_snapshot(
        &mut self,
        window: WindowId,
        url: &str,
    ) -> Option<Arc<CompiledSitePolicySnapshot>> {
        // Check the existing browser URL bound before allocating the input.
        let url = canonicalize(url)
            .ok()
            .and_then(|url| ValidatedUrl::parse(url).ok());
        let Some(url) = url else {
            self.interaction_cache.entry = None;
            self.interaction_cache.dirty = false;
            self.interaction_cache.suspended_context = None;
            self.interaction_cache.unavailable =
                Some(InteractionCacheUnavailable::UnsupportedContext);
            return None;
        };
        let mut cache = std::mem::take(&mut self.interaction_cache);
        cache.refresh(self, self.configuration_policy_sources(), window, url);
        let snapshot = cache
            .entry
            .as_ref()
            .map(|entry| Arc::clone(&entry.snapshot));
        self.interaction_cache = cache;
        snapshot
    }

    /// Refreshes the Interaction projection through the shared typed cache.
    /// Unsupported site contexts can still have valid global discovery.
    pub fn refresh_interaction_snapshot(
        &mut self,
        window: WindowId,
        url: &str,
    ) -> Option<Arc<CompiledInteractionSnapshot>> {
        self.refresh_site_policy_snapshot(window, url)
            .and_then(|snapshot| snapshot.interaction().cloned())
    }

    /// Read-only, context-checked lookup. Never performs URL parsing, policy
    /// compilation, I/O, DOM work or fallback to a previous context.
    #[must_use]
    pub fn cached_site_policy_snapshot(
        &self,
        window: WindowId,
        context: &SitePolicyMatchContext,
    ) -> Option<&Arc<CompiledSitePolicySnapshot>> {
        if self.interaction_cache.dirty || self.interaction_cache.rejected_for(self, window) {
            return None;
        }
        let entry = self.interaction_cache.entry.as_ref()?;
        let (profile, tab) = InteractionPolicyCache::current_scope(self, window)?;
        (entry.owner.matches(self)
            && entry.window == window
            && entry.tab == tab
            && entry.profile == profile
            && self.tabs().get(&tab)?.url == entry.runtime_url
            && self.capture_target(tab) == entry.runtime_target
            && &entry.context == context
            && entry.snapshot.sources().revision() == self.configuration_revision())
        .then_some(&entry.snapshot)
    }

    /// Checked read of the Interaction consumer from the same shared capture.
    #[must_use]
    pub fn cached_interaction_snapshot(
        &self,
        window: WindowId,
        context: &SiteMatchContext,
    ) -> Option<&Arc<CompiledInteractionSnapshot>> {
        self.cached_site_policy_snapshot(window, &SitePolicyMatchContext::site(context))?
            .interaction()
    }

    #[must_use]
    pub fn interaction_cache_unavailable(&self) -> Option<InteractionCacheUnavailable> {
        if self
            .active_window()
            .is_some_and(|window| self.interaction_cache.rejected_for(self, window))
        {
            Some(InteractionCacheUnavailable::RejectedEngineContext)
        } else {
            self.interaction_cache.unavailable
        }
    }

    /// Stage the current consumer before persistence; installation after the
    /// common publisher's assignment is infallible, including closed conflicts.
    pub(crate) fn stage_interaction_cache(
        &self,
        sources: ConfigurationPolicySources,
    ) -> InteractionPolicyCache {
        let mut cache = self.interaction_cache.clone();
        if self
            .active_window()
            .is_some_and(|window| cache.rejected_for(self, window))
        {
            cache.close_rejected_observation();
            return cache;
        }
        // Engine-originated changes may arrive inside a physical key handler.
        // Keep reads closed and let the existing runtime turn compile the
        // latest sources, retaining the old semantic comparator for no-op work.
        if cache.dirty {
            return cache;
        }
        if let Some(entry) = &cache.entry {
            // A tab/URL transition can precede the adapter's eager refresh.
            // Do not compile the old URL under the newly selected tab/route.
            if self
                .cached_site_policy_snapshot(entry.window, &entry.context)
                .is_none()
            {
                cache.clear();
                return cache;
            }
            let (window, url) = (entry.window, entry.url.clone());
            cache.refresh(self, sources, window, url);
        }
        cache
    }
}

#[cfg(test)]
mod tests;
