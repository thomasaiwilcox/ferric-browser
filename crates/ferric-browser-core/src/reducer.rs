use std::fmt;

use crate::{
    ApplicationState, DocumentId, ExistenceState, JourneyEdgeKind, JourneyNodeId, LoadingState,
    Mode, PrivacyKind, ProfileId, RendererState, ResourceLifecycle, SearchCase, SearchState, TabId,
    ValidatedUrl, VersioningEngineEffect, VersioningEvent, VersioningRequest, WindowId,
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Target {
    pub tab: TabId,
    pub generation: u64,
    pub document: DocumentId,
}

impl fmt::Debug for Target {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Target([redacted])")
    }
}

/// State copied across same-profile windows while the live engine view is
/// reparented by the UI adapter.
#[derive(Clone, Eq, PartialEq)]
pub struct TabTransfer {
    pub url: Option<String>,
    pub title: String,
    pub loading: LoadingState,
    pub pinned: bool,
    pub muted: bool,
    pub zoom_hundredths: u32,
}

impl fmt::Debug for TabTransfer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TabTransfer")
            .field("has_url", &self.url.is_some())
            .field("title", &"[redacted]")
            .field("loading", &self.loading)
            .field("pinned", &self.pinned)
            .field("muted", &self.muted)
            .field("zoom_hundredths", &self.zoom_hundredths)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum Event {
    Versioning(VersioningEvent),
    VersioningRequest(VersioningRequest),
    CreateProfile {
        label: String,
        privacy: PrivacyKind,
    },
    CreateWindow {
        profile: ProfileId,
    },
    SetWindowContext {
        window: WindowId,
        context: Option<String>,
    },
    OpenTab {
        window: WindowId,
    },
    OpenTabWithNavigation {
        window: WindowId,
        url: ValidatedUrl,
        background: bool,
    },
    FocusWindow {
        window: WindowId,
    },
    /// Selects the runtime target without claiming an OS-level focus event.
    ///
    /// Real focus callbacks must use [`Event::FocusWindow`] so the
    /// `last_focused_window` record advances as well.
    SetActiveWindow {
        window: WindowId,
    },
    ActivateTab {
        window: WindowId,
        tab: TabId,
    },
    OpenPopup {
        opener: Target,
        url: ValidatedUrl,
        user_gesture: bool,
    },
    MoveTab {
        tab: TabId,
        to_window: WindowId,
        index: Option<usize>,
    },
    TransferTabOut {
        tab: TabId,
    },
    TransferTabIn {
        window: WindowId,
        transfer: TabTransfer,
    },
    SetTabPinned {
        tab: TabId,
        pinned: bool,
    },
    SetTabMuted {
        tab: TabId,
        muted: bool,
    },
    SetTabZoom {
        tab: TabId,
        zoom_hundredths: u32,
    },
    StartNavigation {
        target: Target,
        url: ValidatedUrl,
    },
    CommitNavigation {
        target: Target,
        url: ValidatedUrl,
        title: String,
    },
    CommitNavigationWithTransition {
        target: Target,
        url: ValidatedUrl,
        title: String,
        transition: JourneyEdgeKind,
        source: Option<String>,
        parent: Option<JourneyNodeId>,
    },
    SameDocumentNavigation {
        target: Target,
        url: ValidatedUrl,
    },
    CompleteNavigation {
        target: Target,
    },
    FailNavigation {
        target: Target,
    },
    Reload {
        target: Target,
        bypass_cache: bool,
    },
    Stop {
        target: Target,
    },
    TraverseHistory {
        target: Target,
        offset: i32,
    },
    StartSearch {
        target: Target,
        query: String,
        backward: bool,
        case: SearchCase,
    },
    SearchNext {
        target: Target,
        backward: bool,
    },
    EndSearch {
        target: Target,
    },
    RendererTerminated {
        target: Target,
    },
    CloseTab {
        tab: TabId,
    },
    SuspendTab {
        tab: TabId,
    },
    DiscardTab {
        tab: TabId,
    },
    ResumeTab {
        tab: TabId,
    },
    ViewClosed {
        tab: TabId,
        generation: u64,
    },
    PushMode {
        window: WindowId,
        mode: Mode,
    },
    PopMode {
        window: WindowId,
    },
    Escape {
        window: WindowId,
    },
    ClearJourney {
        since: Option<u64>,
        origin: Option<String>,
    },
    RequestShutdown,
}

impl fmt::Debug for Event {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let variant = match self {
            Self::Versioning(_) => "Versioning",
            Self::VersioningRequest(_) => "VersioningRequest",
            Self::CreateProfile { .. } => "CreateProfile",
            Self::CreateWindow { .. } => "CreateWindow",
            Self::SetWindowContext { .. } => "SetWindowContext",
            Self::OpenTab { .. } => "OpenTab",
            Self::OpenTabWithNavigation { .. } => "OpenTabWithNavigation",
            Self::FocusWindow { .. } => "FocusWindow",
            Self::SetActiveWindow { .. } => "SetActiveWindow",
            Self::ActivateTab { .. } => "ActivateTab",
            Self::OpenPopup { .. } => "OpenPopup",
            Self::MoveTab { .. } => "MoveTab",
            Self::TransferTabOut { .. } => "TransferTabOut",
            Self::TransferTabIn { .. } => "TransferTabIn",
            Self::SetTabPinned { .. } => "SetTabPinned",
            Self::SetTabMuted { .. } => "SetTabMuted",
            Self::SetTabZoom { .. } => "SetTabZoom",
            Self::StartNavigation { .. } => "StartNavigation",
            Self::CommitNavigation { .. } => "CommitNavigation",
            Self::CommitNavigationWithTransition { .. } => "CommitNavigationWithTransition",
            Self::SameDocumentNavigation { .. } => "SameDocumentNavigation",
            Self::CompleteNavigation { .. } => "CompleteNavigation",
            Self::FailNavigation { .. } => "FailNavigation",
            Self::Reload { .. } => "Reload",
            Self::Stop { .. } => "Stop",
            Self::TraverseHistory { .. } => "TraverseHistory",
            Self::StartSearch { .. } => "StartSearch",
            Self::SearchNext { .. } => "SearchNext",
            Self::EndSearch { .. } => "EndSearch",
            Self::RendererTerminated { .. } => "RendererTerminated",
            Self::CloseTab { .. } => "CloseTab",
            Self::SuspendTab { .. } => "SuspendTab",
            Self::DiscardTab { .. } => "DiscardTab",
            Self::ResumeTab { .. } => "ResumeTab",
            Self::ViewClosed { .. } => "ViewClosed",
            Self::PushMode { .. } => "PushMode",
            Self::PopMode { .. } => "PopMode",
            Self::Escape { .. } => "Escape",
            Self::ClearJourney { .. } => "ClearJourney",
            Self::RequestShutdown => "RequestShutdown",
        };
        write!(formatter, "Event::{variant}([redacted])")
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum Effect {
    Engine(EngineEffect),
    Persist(PersistEffect),
    Diagnostic(Diagnostic),
}

impl fmt::Debug for Effect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine(_) => formatter.write_str("Effect::Engine([redacted])"),
            Self::Persist(effect) => formatter
                .debug_tuple("Effect::Persist")
                .field(effect)
                .finish(),
            Self::Diagnostic(_) => formatter.write_str("Effect::Diagnostic([redacted])"),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum EngineEffect {
    Versioning(VersioningEngineEffect),
    Navigate {
        target: Target,
        url: ValidatedUrl,
    },
    Reload {
        target: Target,
        bypass_cache: bool,
    },
    Stop {
        target: Target,
    },
    TraverseHistory {
        target: Target,
        offset: i32,
    },
    FindText {
        target: Target,
        query: String,
        backward: bool,
        case: SearchCase,
    },
    ClearFindText {
        target: Target,
    },
    CloseView {
        tab: TabId,
        generation: u64,
    },
    SetTabLifecycle {
        tab: TabId,
        generation: u64,
        state: ResourceLifecycle,
    },
    ReparentView {
        tab: TabId,
        generation: u64,
        window: WindowId,
    },
}

impl fmt::Debug for EngineEffect {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let variant = match self {
            Self::Versioning(_) => "Versioning",
            Self::Navigate { .. } => "Navigate",
            Self::Reload { .. } => "Reload",
            Self::Stop { .. } => "Stop",
            Self::TraverseHistory { .. } => "TraverseHistory",
            Self::FindText { .. } => "FindText",
            Self::ClearFindText { .. } => "ClearFindText",
            Self::CloseView { .. } => "CloseView",
            Self::SetTabLifecycle { .. } => "SetTabLifecycle",
            Self::ReparentView { .. } => "ReparentView",
        };
        write!(formatter, "EngineEffect::{variant}([redacted])")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistEffect {
    Snapshot { revision: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Diagnostic {
    IgnoredStaleTarget { tab: TabId },
    PopupBlocked { opener: TabId },
}

#[derive(Clone, Eq, PartialEq)]
pub enum ReduceError {
    Versioning(crate::BranchError),
    UnknownProfile(ProfileId),
    UnknownWindow(WindowId),
    UnknownTab(TabId),
    WrongProfile,
    TabAlreadyInWindow,
    InvalidActivation,
    TabNotHidden,
    InvalidLifecycle,
    ShutdownInProgress,
    InvalidZoom,
}

impl fmt::Debug for ReduceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let variant = match self {
            Self::Versioning(_) => "Versioning",
            Self::UnknownProfile(_) => "UnknownProfile",
            Self::UnknownWindow(_) => "UnknownWindow",
            Self::UnknownTab(_) => "UnknownTab",
            Self::WrongProfile => "WrongProfile",
            Self::TabAlreadyInWindow => "TabAlreadyInWindow",
            Self::InvalidActivation => "InvalidActivation",
            Self::TabNotHidden => "TabNotHidden",
            Self::InvalidLifecycle => "InvalidLifecycle",
            Self::ShutdownInProgress => "ShutdownInProgress",
            Self::InvalidZoom => "InvalidZoom",
        };
        write!(formatter, "ReduceError::{variant}([redacted])")
    }
}

impl fmt::Display for ReduceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Versioning(_) => "versioning state transition failed",
            Self::UnknownProfile(_) => "profile was not found",
            Self::UnknownWindow(_) => "window was not found",
            Self::UnknownTab(_) => "tab was not found",
            Self::WrongProfile => "operation targets the wrong profile",
            Self::TabAlreadyInWindow => "tab already belongs to a window",
            Self::InvalidActivation => "profile activation is invalid",
            Self::TabNotHidden => "tab is not hidden",
            Self::InvalidLifecycle => "lifecycle transition is invalid",
            Self::ShutdownInProgress => "browser shutdown is in progress",
            Self::InvalidZoom => "zoom value is invalid",
        })
    }
}

impl std::error::Error for ReduceError {}

#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
fn commit_navigation(
    state: &mut ApplicationState,
    effects: &mut Vec<Effect>,
    target: Target,
    url: ValidatedUrl,
    title: String,
    transition: JourneyEdgeKind,
    source: Option<String>,
    parent: Option<JourneyNodeId>,
) {
    let Some(tab) = state.tabs.get_mut(&target.tab).filter(|tab| {
        tab.generation == target.generation
            && tab.document == target.document
            && tab.existence == ExistenceState::Live
    }) else {
        effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget {
            tab: target.tab,
        }));
        return;
    };
    tab.url = Some(url.to_string());
    tab.title.clone_from(&title);
    tab.loading = LoadingState::Committed;
    tab.renderer = RendererState::Healthy;
    let profile = tab.profile;
    let traversed = state.journey.complete_traversal(target.tab).is_some();
    if !traversed || transition != JourneyEdgeKind::Navigate {
        state.journey.record_navigation_from(
            &mut state.ids,
            profile,
            target.tab,
            &url,
            &title,
            state.revision,
            transition,
            source.as_deref(),
            parent,
        );
    }
}

/// Applies one event and returns side effects for adapters to execute.
///
/// # Errors
///
/// Returns a typed error when the event refers to a missing object, violates
/// profile/window ownership, or arrives after shutdown has started. Stale
/// engine callbacks are not errors: they produce a diagnostic effect and are
/// ignored.
///
/// # Panics
///
/// Panics only if a monotonic runtime counter is exhausted or an internal
/// ownership invariant was already corrupted before this call.
#[allow(clippy::too_many_lines)]
pub fn reduce(state: &mut ApplicationState, event: Event) -> Result<Vec<Effect>, ReduceError> {
    let mut effects = Vec::new();
    match event {
        Event::VersioningRequest(request) => {
            ensure_running(state)?;
            if !state.windows.contains_key(&request.window()) {
                return Err(ReduceError::UnknownWindow(request.window()));
            }
            if let Some(target) = request.target() {
                let tab = state
                    .tabs
                    .get(&target.tab)
                    .ok_or(ReduceError::UnknownTab(target.tab))?;
                if tab.window != request.window()
                    || tab.generation != target.generation
                    || tab.document != target.document
                    || tab.existence != ExistenceState::Live
                {
                    return Err(ReduceError::InvalidActivation);
                }
            }
            effects.push(Effect::Engine(EngineEffect::Versioning(
                VersioningEngineEffect::Request(request),
            )));
        }
        Event::Versioning(action) => {
            ensure_running(state)?;
            let identity_members = match &action {
                VersioningEvent::BindStructuralIdentity { window, pages, .. }
                | VersioningEvent::BindRestoredLineage { window, pages } => Some((window, pages)),
                _ => None,
            };
            if let Some((window, pages)) = identity_members {
                let window_state = state
                    .windows
                    .get(window)
                    .ok_or(ReduceError::UnknownWindow(*window))?;
                let profile = state
                    .profiles
                    .get(&window_state.profile)
                    .ok_or(ReduceError::UnknownProfile(window_state.profile))?;
                if profile.privacy != PrivacyKind::Normal {
                    return Err(ReduceError::Versioning(crate::BranchError::WrongProfile));
                }
                for (tab, _) in pages {
                    let page = state.tabs.get(tab).ok_or(ReduceError::UnknownTab(*tab))?;
                    if page.window != *window
                        || page.profile != window_state.profile
                        || !window_state.tabs.contains(tab)
                        || page.existence != ExistenceState::Live
                    {
                        return Err(ReduceError::Versioning(
                            crate::BranchError::RetainedOwnerMismatch,
                        ));
                    }
                }
            }
            if let VersioningEvent::BindTransientChildLineage { window, pages, .. } = &action {
                let window_state = state
                    .windows
                    .get(window)
                    .ok_or(ReduceError::UnknownWindow(*window))?;
                let profile = state
                    .profiles
                    .get(&window_state.profile)
                    .ok_or(ReduceError::UnknownProfile(window_state.profile))?;
                if profile.privacy != PrivacyKind::Ephemeral || pages.is_empty() {
                    return Err(ReduceError::Versioning(crate::BranchError::WrongProfile));
                }
                for (tab, _, _) in pages {
                    let page = state.tabs.get(tab).ok_or(ReduceError::UnknownTab(*tab))?;
                    if page.window != *window
                        || page.profile != window_state.profile
                        || !window_state.tabs.contains(tab)
                        || page.existence != ExistenceState::Live
                    {
                        return Err(ReduceError::Versioning(
                            crate::BranchError::RetainedOwnerMismatch,
                        ));
                    }
                }
            }
            if let VersioningEvent::PrepareBranch { profile, .. } = &action
                && !state.profiles.contains_key(profile)
            {
                return Err(ReduceError::UnknownProfile(*profile));
            }
            if let VersioningEvent::PrepareRetained { owner, .. }
            | VersioningEvent::PrepareStash { owner, .. } = &action
                && !retained_owner_is_current(state, *owner)
            {
                return Err(ReduceError::Versioning(
                    crate::BranchError::RetainedOwnerMismatch,
                ));
            }
            if let VersioningEvent::AcquireRetained { view } = &action
                && let Some(record) = state.versioning.retained().get(view)
                && !retained_owner_is_current(state, record.owner())
            {
                return Err(ReduceError::Versioning(
                    crate::BranchError::RetainedOwnerMismatch,
                ));
            }
            if let VersioningEvent::BeginRetainedTransfer {
                destination,
                window,
                ..
            }
            | VersioningEvent::AdoptRetained {
                destination,
                window,
                ..
            } = &action
                && state
                    .windows
                    .get(window)
                    .is_none_or(|target| target.profile != destination.runtime_profile())
            {
                return Err(ReduceError::Versioning(crate::BranchError::WrongProfile));
            }
            state
                .versioning
                .apply(action)
                .map_err(ReduceError::Versioning)?;
            // Explicit versioning records are not ordinary recovery snapshots.
            state.revision = state.revision.checked_add(1).expect("revision exhausted");
        }
        Event::CreateProfile { label, privacy } => {
            ensure_running(state)?;
            let id = state.ids.profile();
            state
                .profiles
                .insert(id, crate::ProfileState { id, label, privacy });
            persist(state, &mut effects);
        }
        Event::CreateWindow { profile } => {
            ensure_running(state)?;
            if !state.profiles.contains_key(&profile) {
                return Err(ReduceError::UnknownProfile(profile));
            }
            let id = state.ids.window();
            state.windows.insert(
                id,
                crate::WindowState {
                    id,
                    profile,
                    context: None,
                    tabs: Vec::new(),
                    active_tab: None,
                    previous_tab: None,
                    modes: vec![Mode::Normal],
                },
            );
            state.active_window = Some(id);
            state.last_focused_window = Some(id);
            persist(state, &mut effects);
        }
        Event::SetWindowContext { window, context } => {
            ensure_running(state)?;
            let window_state = state
                .windows
                .get_mut(&window)
                .ok_or(ReduceError::UnknownWindow(window))?;
            window_state.context = context;
            persist(state, &mut effects);
        }
        Event::OpenTab { window } => {
            ensure_running(state)?;
            add_tab(state, window, None)?;
            state.active_window = Some(window);
            state.last_focused_window = Some(window);
            persist(state, &mut effects);
        }
        Event::OpenTabWithNavigation {
            window,
            url,
            background,
        } => {
            ensure_running(state)?;
            let previous_active_window = state.active_window;
            let previous_last_focused_window = state.last_focused_window;
            let previous_active_tab = state
                .windows
                .get(&window)
                .ok_or(ReduceError::UnknownWindow(window))?
                .active_tab;
            let previous_recent_tab = state.windows[&window].previous_tab;
            let tab = add_tab(state, window, None)?;
            if background {
                if let Some(active_tab) = previous_active_tab {
                    let window = state
                        .windows
                        .get_mut(&window)
                        .expect("window checked above");
                    window.active_tab = Some(active_tab);
                    window.previous_tab = previous_recent_tab;
                }
                state.active_window = previous_active_window;
                state.last_focused_window = previous_last_focused_window;
            } else {
                state.active_window = Some(window);
                state.last_focused_window = Some(window);
            }
            let tab_state = state.tabs.get_mut(&tab).expect("new tab exists");
            tab_state.generation = tab_state
                .generation
                .checked_add(1)
                .expect("tab generation exhausted");
            tab_state.document = state.ids.document();
            tab_state.loading = LoadingState::Provisional;
            let next_target = Target {
                tab,
                generation: tab_state.generation,
                document: tab_state.document,
            };
            effects.push(Effect::Engine(EngineEffect::Navigate {
                target: next_target,
                url,
            }));
            persist(state, &mut effects);
        }
        Event::FocusWindow { window } => {
            if !state.windows.contains_key(&window) {
                return Err(ReduceError::UnknownWindow(window));
            }
            state.active_window = Some(window);
            state.last_focused_window = Some(window);
        }
        Event::SetActiveWindow { window } => {
            if !state.windows.contains_key(&window) {
                return Err(ReduceError::UnknownWindow(window));
            }
            state.active_window = Some(window);
        }
        Event::OpenPopup {
            opener,
            url,
            user_gesture,
        } => {
            if !user_gesture {
                effects.push(Effect::Diagnostic(Diagnostic::PopupBlocked {
                    opener: opener.tab,
                }));
                return Ok(effects);
            }
            if !target_is_current(state, opener) {
                effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget {
                    tab: opener.tab,
                }));
                return Ok(effects);
            }
            let window = state
                .tabs
                .get(&opener.tab)
                .expect("opener target checked above")
                .window;
            let tab = add_tab(state, window, None)?;
            let tab_state = state.tabs.get_mut(&tab).expect("new tab exists");
            tab_state.generation = 1;
            tab_state.document = state.ids.document();
            tab_state.loading = LoadingState::Provisional;
            let target = Target {
                tab,
                generation: tab_state.generation,
                document: tab_state.document,
            };
            effects.push(Effect::Engine(EngineEffect::Navigate { target, url }));
            persist(state, &mut effects);
        }
        Event::ActivateTab { window, tab } => {
            if !state
                .tabs
                .get(&tab)
                .is_some_and(|tab| tab.existence == ExistenceState::Live)
            {
                return Err(ReduceError::InvalidActivation);
            }
            let window_state = state
                .windows
                .get_mut(&window)
                .ok_or(ReduceError::UnknownWindow(window))?;
            if !window_state.tabs.contains(&tab) {
                return Err(ReduceError::InvalidActivation);
            }
            if window_state.active_tab != Some(tab) {
                window_state.previous_tab = window_state.active_tab;
                window_state.active_tab = Some(tab);
            }
            state.active_window = Some(window);
            state.last_focused_window = Some(window);
        }
        Event::MoveTab {
            tab,
            to_window,
            index,
        } => {
            ensure_running(state)?;
            if state.versioning.owns_tab(tab) {
                return Err(ReduceError::Versioning(
                    crate::BranchError::RetainedWorkOwned,
                ));
            }
            let tab_state = state.tabs.get(&tab).ok_or(ReduceError::UnknownTab(tab))?;
            if tab_state.existence != ExistenceState::Live {
                return Err(ReduceError::InvalidActivation);
            }
            let from_window = tab_state.window;
            let profile = tab_state.profile;
            let generation = tab_state.generation;
            let target_window = state
                .windows
                .get(&to_window)
                .ok_or(ReduceError::UnknownWindow(to_window))?;
            if target_window.profile != profile {
                return Err(ReduceError::WrongProfile);
            }
            let pinned = state
                .tabs
                .get(&tab)
                .expect("tab ownership invariant")
                .pinned;
            let pinned_ids = state
                .tabs
                .iter()
                .filter_map(|(id, tab)| tab.pinned.then_some(*id))
                .collect::<std::collections::BTreeSet<_>>();
            if from_window == to_window {
                let window = state
                    .windows
                    .get_mut(&from_window)
                    .expect("source window checked above");
                let old_index = window
                    .tabs
                    .iter()
                    .position(|candidate| *candidate == tab)
                    .expect("tab ownership invariant");
                window.tabs.remove(old_index);
                insert_tab_in_group(window, tab, pinned, index, &pinned_ids);
                if window.active_tab != Some(tab) {
                    window.previous_tab = window.active_tab;
                    window.active_tab = Some(tab);
                }
            } else {
                let was_active = state
                    .windows
                    .get(&from_window)
                    .is_some_and(|window| window.active_tab == Some(tab));
                let source_tabs = {
                    let source = state
                        .windows
                        .get_mut(&from_window)
                        .expect("source window checked above");
                    source.tabs.retain(|candidate| *candidate != tab);
                    if source.previous_tab == Some(tab) {
                        source.previous_tab = None;
                    }
                    source.tabs.clone()
                };
                if was_active {
                    let active = source_tabs.iter().rev().copied().find(|candidate| {
                        state
                            .tabs
                            .get(candidate)
                            .is_some_and(|tab| tab.existence == ExistenceState::Live)
                    });
                    state
                        .windows
                        .get_mut(&from_window)
                        .expect("source window checked above")
                        .active_tab = active;
                }
                let target = state
                    .windows
                    .get_mut(&to_window)
                    .expect("target window checked above");
                insert_tab_in_group(target, tab, pinned, index, &pinned_ids);
                if target.active_tab != Some(tab) {
                    target.previous_tab = target.active_tab;
                    target.active_tab = Some(tab);
                }
                state.active_window = Some(to_window);
                state.last_focused_window = Some(to_window);
            }
            state
                .tabs
                .get_mut(&tab)
                .expect("tab ownership invariant")
                .window = to_window;
            effects.push(Effect::Engine(EngineEffect::ReparentView {
                tab,
                generation,
                window: to_window,
            }));
            persist(state, &mut effects);
        }
        Event::TransferTabOut { tab } => {
            ensure_running(state)?;
            if state.versioning.owns_tab(tab) {
                return Err(ReduceError::Versioning(
                    crate::BranchError::RetainedWorkOwned,
                ));
            }
            let tab_state = state.tabs.get(&tab).ok_or(ReduceError::UnknownTab(tab))?;
            if tab_state.existence != ExistenceState::Live {
                return Err(ReduceError::InvalidActivation);
            }
            let window_id = tab_state.window;
            let was_active = state
                .windows
                .get(&window_id)
                .is_some_and(|window| window.active_tab == Some(tab));
            let remaining_tabs = {
                let window = state
                    .windows
                    .get_mut(&window_id)
                    .ok_or(ReduceError::UnknownWindow(window_id))?;
                window.tabs.retain(|candidate| *candidate != tab);
                if window.previous_tab == Some(tab) {
                    window.previous_tab = None;
                }
                window.tabs.clone()
            };
            if was_active {
                let active = remaining_tabs.iter().rev().copied().find(|candidate| {
                    state
                        .tabs
                        .get(candidate)
                        .is_some_and(|tab| tab.existence == ExistenceState::Live)
                });
                state
                    .windows
                    .get_mut(&window_id)
                    .expect("source window checked above")
                    .active_tab = active;
            }
            state.tabs.remove(&tab);
            state.versioning.forget_structural_page(tab);
            persist(state, &mut effects);
        }
        Event::TransferTabIn { window, transfer } => {
            ensure_running(state)?;
            if !(25..=500).contains(&transfer.zoom_hundredths) {
                return Err(ReduceError::InvalidZoom);
            }
            let reusable_tab = state
                .windows
                .get(&window)
                .filter(|window| window.tabs.len() == 1)
                .and_then(|window| window.tabs.first().copied())
                .filter(|tab| {
                    state.tabs.get(tab).is_some_and(|tab| {
                        tab.existence == ExistenceState::Live
                            && tab.url.is_none()
                            && tab.title.is_empty()
                            && tab.loading == LoadingState::Idle
                    })
                });
            let tab = if let Some(tab) = reusable_tab {
                state
                    .tabs
                    .get_mut(&tab)
                    .expect("reusable tab exists")
                    .url
                    .clone_from(&transfer.url);
                tab
            } else {
                add_tab(state, window, transfer.url.clone())?
            };
            let tab_state = state.tabs.get_mut(&tab).expect("new tab exists");
            tab_state.title = transfer.title;
            tab_state.loading = transfer.loading;
            tab_state.pinned = transfer.pinned;
            tab_state.muted = transfer.muted;
            tab_state.zoom = f64::from(transfer.zoom_hundredths) / 100.0;
            if transfer.pinned {
                let pinned_ids = state
                    .tabs
                    .iter()
                    .filter_map(|(id, tab)| tab.pinned.then_some(*id))
                    .collect::<std::collections::BTreeSet<_>>();
                let window_state = state
                    .windows
                    .get_mut(&window)
                    .expect("window checked by add_tab");
                window_state.tabs.pop();
                insert_tab_in_group(window_state, tab, true, None, &pinned_ids);
            }
            state.active_window = Some(window);
            state.last_focused_window = Some(window);
            persist(state, &mut effects);
        }
        Event::SetTabPinned { tab, pinned } => {
            ensure_running(state)?;
            let (window_id, old_pinned) = {
                let tab_state = state.tabs.get(&tab).ok_or(ReduceError::UnknownTab(tab))?;
                if tab_state.existence != ExistenceState::Live {
                    return Err(ReduceError::InvalidActivation);
                }
                (tab_state.window, tab_state.pinned)
            };
            if old_pinned != pinned {
                state.tabs.get_mut(&tab).expect("tab checked above").pinned = pinned;
                let pinned_ids = state
                    .tabs
                    .iter()
                    .filter_map(|(id, tab)| tab.pinned.then_some(*id))
                    .collect::<std::collections::BTreeSet<_>>();
                let window = state.windows.get_mut(&window_id).expect("window invariant");
                let old_index = window
                    .tabs
                    .iter()
                    .position(|candidate| *candidate == tab)
                    .expect("tab ownership invariant");
                window.tabs.remove(old_index);
                insert_tab_in_group(window, tab, pinned, None, &pinned_ids);
            }
            persist(state, &mut effects);
        }
        Event::SetTabMuted { tab, muted } => {
            ensure_running(state)?;
            let tab_state = state
                .tabs
                .get_mut(&tab)
                .ok_or(ReduceError::UnknownTab(tab))?;
            if tab_state.existence != ExistenceState::Live {
                return Err(ReduceError::InvalidActivation);
            }
            tab_state.muted = muted;
            persist(state, &mut effects);
        }
        Event::SetTabZoom {
            tab,
            zoom_hundredths,
        } => {
            ensure_running(state)?;
            if !(25..=500).contains(&zoom_hundredths) {
                return Err(ReduceError::InvalidZoom);
            }
            let tab_state = state
                .tabs
                .get_mut(&tab)
                .ok_or(ReduceError::UnknownTab(tab))?;
            if tab_state.existence != ExistenceState::Live {
                return Err(ReduceError::InvalidActivation);
            }
            tab_state.zoom = f64::from(zoom_hundredths) / 100.0;
            persist(state, &mut effects);
        }
        Event::StartNavigation { target, url } => {
            let tab = match state.tabs.get_mut(&target.tab) {
                Some(tab)
                    if tab.generation == target.generation
                        && tab.document == target.document
                        && tab.existence == ExistenceState::Live =>
                {
                    tab
                }
                _ => {
                    effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget {
                        tab: target.tab,
                    }));
                    return Ok(effects);
                }
            };
            let had_search = tab.search.take().is_some();
            tab.renderer = RendererState::Healthy;
            tab.generation = tab
                .generation
                .checked_add(1)
                .expect("tab generation exhausted");
            tab.document = state.ids.document();
            tab.loading = LoadingState::Provisional;
            let next_target = Target {
                tab: tab.id,
                generation: tab.generation,
                document: tab.document,
            };
            mark_retention_lost_for_tab(state, target.tab)?;
            if had_search {
                effects.push(Effect::Engine(EngineEffect::ClearFindText { target }));
            }
            effects.push(Effect::Engine(EngineEffect::Navigate {
                target: next_target,
                url,
            }));
        }
        Event::CommitNavigation { target, url, title } => {
            commit_navigation(
                state,
                &mut effects,
                target,
                url,
                title,
                JourneyEdgeKind::Navigate,
                None,
                None,
            );
        }
        Event::CommitNavigationWithTransition {
            target,
            url,
            title,
            transition,
            source,
            parent,
        } => {
            commit_navigation(
                state,
                &mut effects,
                target,
                url,
                title,
                transition,
                source,
                parent,
            );
        }
        Event::SameDocumentNavigation { target, url } => {
            let tab = match state.tabs.get_mut(&target.tab) {
                Some(tab)
                    if tab.generation == target.generation
                        && tab.document == target.document
                        && tab.existence == ExistenceState::Live =>
                {
                    tab
                }
                _ => {
                    effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget {
                        tab: target.tab,
                    }));
                    return Ok(effects);
                }
            };
            tab.url = Some(url.to_string());
        }
        Event::CompleteNavigation { target } => {
            let tab = match state.tabs.get_mut(&target.tab) {
                Some(tab)
                    if tab.generation == target.generation
                        && tab.document == target.document
                        && tab.existence == ExistenceState::Live =>
                {
                    tab
                }
                _ => {
                    effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget {
                        tab: target.tab,
                    }));
                    return Ok(effects);
                }
            };
            tab.loading = LoadingState::Complete;
        }
        Event::FailNavigation { target } => {
            let tab = match state.tabs.get_mut(&target.tab) {
                Some(tab)
                    if tab.generation == target.generation
                        && tab.document == target.document
                        && tab.existence == ExistenceState::Live =>
                {
                    tab
                }
                _ => {
                    effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget {
                        tab: target.tab,
                    }));
                    return Ok(effects);
                }
            };
            tab.loading = LoadingState::Failed;
        }
        Event::Reload {
            target,
            bypass_cache,
        } => {
            let tab = match state.tabs.get_mut(&target.tab) {
                Some(tab)
                    if tab.generation == target.generation
                        && tab.document == target.document
                        && tab.existence == ExistenceState::Live =>
                {
                    tab
                }
                _ => {
                    effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget {
                        tab: target.tab,
                    }));
                    return Ok(effects);
                }
            };
            tab.loading = LoadingState::Provisional;
            tab.renderer = RendererState::Healthy;
            effects.push(Effect::Engine(EngineEffect::Reload {
                target,
                bypass_cache,
            }));
        }
        Event::Stop { target } => {
            let tab = match state.tabs.get_mut(&target.tab) {
                Some(tab)
                    if tab.generation == target.generation
                        && tab.document == target.document
                        && tab.existence == ExistenceState::Live =>
                {
                    tab
                }
                _ => {
                    effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget {
                        tab: target.tab,
                    }));
                    return Ok(effects);
                }
            };
            tab.loading = LoadingState::Cancelled;
            effects.push(Effect::Engine(EngineEffect::Stop { target }));
        }
        Event::TraverseHistory { target, offset } => {
            if !target_is_current(state, target) {
                effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget {
                    tab: target.tab,
                }));
                return Ok(effects);
            }
            state.journey.request_traversal(target.tab, offset);
            effects.push(Effect::Engine(EngineEffect::TraverseHistory {
                target,
                offset,
            }));
        }
        Event::StartSearch {
            target,
            query,
            backward,
            case,
        } => {
            if !target_is_current(state, target) {
                effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget {
                    tab: target.tab,
                }));
                return Ok(effects);
            }
            let tab = state
                .tabs
                .get_mut(&target.tab)
                .expect("search target checked above");
            if query.is_empty() {
                tab.search = None;
                effects.push(Effect::Engine(EngineEffect::ClearFindText { target }));
            } else {
                tab.search = Some(SearchState {
                    query: query.clone(),
                    backward,
                    case,
                });
                effects.push(Effect::Engine(EngineEffect::FindText {
                    target,
                    query,
                    backward,
                    case,
                }));
            }
        }
        Event::SearchNext { target, backward } => {
            if !target_is_current(state, target) {
                effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget {
                    tab: target.tab,
                }));
                return Ok(effects);
            }
            let Some(search) = state.tabs.get_mut(&target.tab).and_then(|tab| {
                tab.search.as_mut().map(|search| {
                    search.backward = backward;
                    search.clone()
                })
            }) else {
                return Ok(effects);
            };
            effects.push(Effect::Engine(EngineEffect::FindText {
                target,
                query: search.query,
                backward,
                case: search.case,
            }));
        }
        Event::EndSearch { target } => {
            if !target_is_current(state, target) {
                effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget {
                    tab: target.tab,
                }));
                return Ok(effects);
            }
            if let Some(tab) = state.tabs.get_mut(&target.tab) {
                tab.search = None;
            }
            effects.push(Effect::Engine(EngineEffect::ClearFindText { target }));
        }
        Event::RendererTerminated { target } => {
            let tab = match state.tabs.get_mut(&target.tab) {
                Some(tab)
                    if tab.generation == target.generation
                        && tab.document == target.document
                        && tab.existence == ExistenceState::Live =>
                {
                    tab
                }
                _ => {
                    effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget {
                        tab: target.tab,
                    }));
                    return Ok(effects);
                }
            };
            tab.renderer = RendererState::Terminated;
            tab.loading = LoadingState::Failed;
            mark_retention_lost_for_tab(state, target.tab)?;
        }
        Event::CloseTab { tab } => {
            if state.versioning.owns_tab(tab) {
                return Err(ReduceError::Versioning(
                    crate::BranchError::RetainedWorkOwned,
                ));
            }
            let tab_state = state
                .tabs
                .get_mut(&tab)
                .ok_or(ReduceError::UnknownTab(tab))?;
            if tab_state.existence != ExistenceState::Live {
                effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget { tab }));
                return Ok(effects);
            }
            tab_state.existence = ExistenceState::Closing;
            tab_state.resources = ResourceLifecycle::Frozen;
            tab_state.loading = LoadingState::Cancelled;
            let generation = tab_state.generation;
            let window_id = tab_state.window;
            let next_active = {
                let window = state.windows.get(&window_id).expect("tab invariant");
                if window.active_tab == Some(tab) {
                    let index = window
                        .tabs
                        .iter()
                        .position(|candidate| *candidate == tab)
                        .expect("active tab invariant");
                    next_live_tab(state, &window.tabs, index)
                } else {
                    None
                }
            };
            let window = state.windows.get_mut(&window_id).expect("tab invariant");
            if window.active_tab == Some(tab) {
                window.active_tab = next_active;
            }
            effects.push(Effect::Engine(EngineEffect::CloseView { tab, generation }));
            persist(state, &mut effects);
        }
        Event::SuspendTab { tab } => {
            ensure_running(state)?;
            let generation = {
                let tab_state = state.tabs.get(&tab).ok_or(ReduceError::UnknownTab(tab))?;
                if tab_state.existence != ExistenceState::Live {
                    return Err(ReduceError::InvalidLifecycle);
                }
                let window = state
                    .windows
                    .get(&tab_state.window)
                    .ok_or(ReduceError::UnknownWindow(tab_state.window))?;
                if window.active_tab == Some(tab) {
                    return Err(ReduceError::TabNotHidden);
                }
                if tab_state.resources != ResourceLifecycle::Active {
                    return Err(ReduceError::InvalidLifecycle);
                }
                tab_state.generation
            };
            state
                .tabs
                .get_mut(&tab)
                .expect("tab checked above")
                .resources = ResourceLifecycle::Frozen;
            effects.push(Effect::Engine(EngineEffect::SetTabLifecycle {
                tab,
                generation,
                state: ResourceLifecycle::Frozen,
            }));
            persist(state, &mut effects);
        }
        Event::DiscardTab { tab } => {
            ensure_running(state)?;
            if state.versioning.owns_tab(tab) {
                return Err(ReduceError::Versioning(
                    crate::BranchError::RetainedWorkOwned,
                ));
            }
            let generation = {
                let tab_state = state.tabs.get(&tab).ok_or(ReduceError::UnknownTab(tab))?;
                if tab_state.existence != ExistenceState::Live {
                    return Err(ReduceError::InvalidLifecycle);
                }
                let window = state
                    .windows
                    .get(&tab_state.window)
                    .ok_or(ReduceError::UnknownWindow(tab_state.window))?;
                if window.active_tab == Some(tab) {
                    return Err(ReduceError::TabNotHidden);
                }
                if tab_state.resources != ResourceLifecycle::Active {
                    return Err(ReduceError::InvalidLifecycle);
                }
                tab_state.generation
            };
            state
                .tabs
                .get_mut(&tab)
                .expect("tab checked above")
                .resources = ResourceLifecycle::Discarded;
            effects.push(Effect::Engine(EngineEffect::SetTabLifecycle {
                tab,
                generation,
                state: ResourceLifecycle::Discarded,
            }));
            persist(state, &mut effects);
        }
        Event::ResumeTab { tab } => {
            ensure_running(state)?;
            let generation = {
                let tab_state = state.tabs.get(&tab).ok_or(ReduceError::UnknownTab(tab))?;
                if tab_state.existence != ExistenceState::Live {
                    return Err(ReduceError::InvalidLifecycle);
                }
                if tab_state.resources == ResourceLifecycle::Active {
                    return Ok(effects);
                }
                tab_state.generation
            };
            state
                .tabs
                .get_mut(&tab)
                .expect("tab checked above")
                .resources = ResourceLifecycle::Active;
            effects.push(Effect::Engine(EngineEffect::SetTabLifecycle {
                tab,
                generation,
                state: ResourceLifecycle::Active,
            }));
            persist(state, &mut effects);
        }
        Event::ViewClosed { tab, generation } => {
            let Some(tab_state) = state.tabs.get(&tab) else {
                effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget { tab }));
                return Ok(effects);
            };
            if tab_state.generation != generation || tab_state.existence != ExistenceState::Closing
            {
                effects.push(Effect::Diagnostic(Diagnostic::IgnoredStaleTarget { tab }));
                return Ok(effects);
            }
            let window_id = tab_state.window;
            let removed = state.tabs.remove(&tab).expect("tab checked above");
            state.versioning.forget_structural_page(tab);
            debug_assert_eq!(removed.existence, ExistenceState::Closing);
            let window = state.windows.get_mut(&window_id).expect("tab invariant");
            window.tabs.retain(|candidate| *candidate != tab);
            if window.previous_tab == Some(tab) {
                window.previous_tab = None;
            }
            if window.active_tab == Some(tab) {
                window.active_tab = window.tabs.iter().rev().copied().next();
            }
            if window.tabs.is_empty() {
                let blank = add_tab(state, window_id, Some("about:blank".into()))?;
                let blank_state = state.tabs.get_mut(&blank).expect("blank tab exists");
                blank_state.loading = LoadingState::Complete;
                state
                    .windows
                    .get_mut(&window_id)
                    .expect("window invariant")
                    .active_tab = Some(blank);
            }
            persist(state, &mut effects);
        }
        Event::PushMode { window, mode } => {
            let window_state = state
                .windows
                .get_mut(&window)
                .ok_or(ReduceError::UnknownWindow(window))?;
            window_state.modes.push(mode);
        }
        Event::PopMode { window } => {
            let window_state = state
                .windows
                .get_mut(&window)
                .ok_or(ReduceError::UnknownWindow(window))?;
            if window_state.modes.len() > 1 {
                window_state.modes.pop();
            }
        }
        Event::Escape { window } => {
            let window_state = state
                .windows
                .get_mut(&window)
                .ok_or(ReduceError::UnknownWindow(window))?;
            window_state.modes.truncate(1);
        }
        Event::ClearJourney { since, origin } => {
            ensure_running(state)?;
            state.journey.clear_matching(|node| {
                since.is_none_or(|cutoff| node.committed_at < cutoff)
                    && origin.as_deref().is_none_or(|candidate| {
                        crate::canonical_origin(&node.url).as_deref() == Some(candidate)
                    })
            });
            persist(state, &mut effects);
        }
        Event::RequestShutdown => {
            if state.shutdown == crate::ShutdownState::Requested {
                return Ok(effects);
            }
            state.shutdown = crate::ShutdownState::Requested;
            persist(state, &mut effects);
        }
    }
    state
        .validate()
        .map_err(|_| ReduceError::InvalidActivation)?;
    Ok(effects)
}

fn ensure_running(state: &ApplicationState) -> Result<(), ReduceError> {
    if state.shutdown == crate::ShutdownState::Requested {
        Err(ReduceError::ShutdownInProgress)
    } else {
        Ok(())
    }
}

fn mark_retention_lost_for_tab(
    state: &mut ApplicationState,
    tab: TabId,
) -> Result<(), ReduceError> {
    let affected = state
        .versioning
        .retained()
        .iter()
        .filter_map(|(view, record)| {
            (record.owner().target().tab == tab
                && !matches!(
                    record.lifecycle(),
                    crate::versioning::RetainedLifecycle::Lost
                        | crate::versioning::RetainedLifecycle::Discarded
                ))
            .then_some(*view)
        })
        .collect::<Vec<_>>();
    for view in affected {
        state
            .versioning
            .apply(VersioningEvent::LoseRetained { view })
            .map_err(ReduceError::Versioning)?;
        state.revision = state.revision.checked_add(1).expect("revision exhausted");
    }
    Ok(())
}

fn retained_owner_is_current(state: &ApplicationState, owner: crate::CapturedOwner) -> bool {
    let target = owner.target();
    let Some(window) = state.windows.get(&owner.window()) else {
        return false;
    };
    let Some(tab) = state.tabs.get(&target.tab) else {
        return false;
    };
    window.profile == owner.native().runtime_profile()
        && window.tabs.contains(&target.tab)
        && tab.profile == window.profile
        && tab.window == owner.window()
        && tab.generation == target.generation
        && tab.document == target.document
        && tab.existence == ExistenceState::Live
        && tab.renderer == RendererState::Healthy
        && tab.resources != ResourceLifecycle::Discarded
}

fn add_tab(
    state: &mut ApplicationState,
    window: WindowId,
    url: Option<String>,
) -> Result<TabId, ReduceError> {
    let profile = state
        .windows
        .get(&window)
        .ok_or(ReduceError::UnknownWindow(window))?
        .profile;
    let id = state.ids.tab();
    let document = state.ids.document();
    state.tabs.insert(
        id,
        crate::TabState {
            id,
            window,
            profile,
            generation: 0,
            document,
            url,
            title: String::new(),
            existence: ExistenceState::Live,
            loading: LoadingState::Idle,
            renderer: RendererState::Healthy,
            resources: ResourceLifecycle::Active,
            search: None,
            pinned: false,
            muted: false,
            zoom: 1.0,
        },
    );
    let window_state = state
        .windows
        .get_mut(&window)
        .expect("window checked above");
    window_state.tabs.push(id);
    window_state.previous_tab = window_state.active_tab;
    window_state.active_tab = Some(id);
    Ok(id)
}

fn insert_tab_in_group(
    window: &mut crate::WindowState,
    tab: TabId,
    pinned: bool,
    index: Option<usize>,
    pinned_ids: &std::collections::BTreeSet<TabId>,
) {
    let pinned_count = window
        .tabs
        .iter()
        .filter(|candidate| pinned_ids.contains(candidate))
        .count();
    let group_count = if pinned {
        pinned_count
    } else {
        window.tabs.len().saturating_sub(pinned_count)
    };
    let requested = index.map_or(group_count, |index| {
        if pinned {
            index.min(group_count)
        } else {
            index.saturating_sub(pinned_count).min(group_count)
        }
    });
    let insertion = if pinned {
        requested
    } else {
        pinned_count.saturating_add(requested)
    };
    window.tabs.insert(insertion.min(window.tabs.len()), tab);
}

fn next_live_tab(state: &ApplicationState, tabs: &[TabId], closing_index: usize) -> Option<TabId> {
    tabs.iter()
        .enumerate()
        .skip(closing_index + 1)
        .filter(|(_, tab)| {
            state
                .tabs
                .get(tab)
                .is_some_and(|tab| tab.existence == ExistenceState::Live)
        })
        .map(|(_, tab)| *tab)
        .next()
        .or_else(|| {
            tabs.iter()
                .enumerate()
                .take(closing_index)
                .rev()
                .filter(|(_, tab)| {
                    state
                        .tabs
                        .get(tab)
                        .is_some_and(|tab| tab.existence == ExistenceState::Live)
                })
                .map(|(_, tab)| *tab)
                .next()
        })
}

fn target_is_current(state: &ApplicationState, target: Target) -> bool {
    state.tabs.get(&target.tab).is_some_and(|tab| {
        tab.generation == target.generation
            && tab.document == target.document
            && tab.existence == ExistenceState::Live
    })
}

fn persist(state: &mut ApplicationState, effects: &mut Vec<Effect>) {
    state.revision = state.revision.checked_add(1).expect("revision exhausted");
    effects.push(Effect::Persist(PersistEffect::Snapshot {
        revision: state.revision,
    }));
}

impl ApplicationState {
    /// Checks the cross-object ownership invariants required by the reducer.
    ///
    /// # Errors
    ///
    /// Returns a static explanation of the first broken invariant.
    pub fn validate(&self) -> Result<(), &'static str> {
        for (id, window) in &self.windows {
            if !self.profiles.contains_key(&window.profile) {
                return Err("window references missing profile");
            }
            if window.active_tab.is_some_and(|tab| {
                !window.tabs.contains(&tab)
                    || !self
                        .tabs
                        .get(&tab)
                        .is_some_and(|tab| tab.existence == ExistenceState::Live)
            }) {
                return Err("active tab is not in window");
            }
            for tab_id in &window.tabs {
                let tab = self
                    .tabs
                    .get(tab_id)
                    .ok_or("window references missing tab")?;
                if tab.window != *id || tab.profile != window.profile {
                    return Err("tab crosses window/profile boundary");
                }
            }
        }
        for tab in self.tabs.values() {
            let window = self
                .windows
                .get(&tab.window)
                .ok_or("tab references missing window")?;
            if window.profile != tab.profile || !window.tabs.contains(&tab.id) {
                return Err("tab ownership invariant violated");
            }
        }
        if self
            .active_window
            .is_some_and(|window| !self.windows.contains_key(&window))
        {
            return Err("active window is missing");
        }
        self.journey.validate_profiles(&self.profiles)?;
        self.versioning.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reducer_error_diagnostics_redact_runtime_ids() {
        let errors = [
            ReduceError::UnknownProfile(ProfileId::from_raw(0x51_7e_11)),
            ReduceError::UnknownWindow(WindowId::from_raw(0x51_7e_12)),
            ReduceError::UnknownTab(TabId::from_raw(0x51_7e_13)),
        ];

        for error in errors {
            for diagnostic in [format!("{error:?}"), error.to_string()] {
                assert!(!diagnostic.contains("517e"));
                assert!(!diagnostic.contains("profile-"));
                assert!(!diagnostic.contains("window-"));
                assert!(!diagnostic.contains("tab-"));
            }
        }
    }

    fn url(value: &str) -> ValidatedUrl {
        ValidatedUrl::parse(value).expect("test URL")
    }

    fn setup() -> (ApplicationState, ProfileId, WindowId, TabId, Target) {
        let mut state = ApplicationState::new();
        reduce(
            &mut state,
            Event::CreateProfile {
                label: "default".into(),
                privacy: PrivacyKind::Normal,
            },
        )
        .unwrap();
        let profile = state.profiles.keys().next().copied().unwrap();
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let window = state.windows.keys().next().copied().unwrap();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let tab = state.tabs.keys().next().copied().unwrap();
        let target = state.capture_target(tab).expect("test tab exists");
        (state, profile, window, tab, target)
    }

    #[test]
    fn creates_owned_objects_and_captures_current_target() {
        let (state, profile, window, tab, target) = setup();
        assert_eq!(state.windows[&window].profile, profile);
        assert_eq!(state.tabs[&tab].window, window);
        assert_eq!(state.active_tab().map(|tab| tab.id), Some(tab));
        assert_eq!(target.tab, tab);
        assert!(state.validate().is_ok());
    }

    #[test]
    fn event_and_transfer_debug_redact_page_data() {
        let sentinel = "private-event-page-data-sentinel";
        let (_, _, window, _, target) = setup();
        let page_url = url(&format!("https://example.test/{sentinel}?query=private"));
        let transfer = TabTransfer {
            url: Some(page_url.to_string()),
            title: sentinel.into(),
            loading: LoadingState::Committed,
            pinned: false,
            muted: false,
            zoom_hundredths: 100,
        };
        let events = [
            Event::CommitNavigation {
                target,
                url: page_url.clone(),
                title: sentinel.into(),
            },
            Event::StartSearch {
                target,
                query: sentinel.into(),
                backward: false,
                case: SearchCase::Sensitive,
            },
            Event::TransferTabIn { window, transfer },
            Event::ClearJourney {
                since: None,
                origin: Some(sentinel.into()),
            },
        ];

        for event in events {
            let debug = format!("{event:?}");
            assert!(!debug.contains(sentinel));
            assert!(!debug.contains("query=private"));
        }
        assert_eq!(format!("{target:?}"), "Target([redacted])");
    }

    #[test]
    fn journey_clear_is_reduced_and_publishes_a_persist_revision() {
        let (mut state, _, _, tab, target) = setup();
        reduce(
            &mut state,
            Event::StartNavigation {
                target,
                url: url("https://example.test/path"),
            },
        )
        .expect("start navigation");
        let committed_target = state.capture_target(tab).expect("current target");
        reduce(
            &mut state,
            Event::CommitNavigation {
                target: committed_target,
                url: url("https://example.test/path"),
                title: "Example".into(),
            },
        )
        .expect("commit navigation");
        assert_eq!(state.journey.nodes().count(), 1);

        let previous_revision = state.revision;
        let effects = reduce(
            &mut state,
            Event::ClearJourney {
                since: None,
                origin: Some("https://example.test".into()),
            },
        )
        .expect("clear journey");

        assert_eq!(state.journey.nodes().count(), 0);
        assert_eq!(state.revision, previous_revision + 1);
        assert_eq!(
            effects,
            vec![Effect::Persist(PersistEffect::Snapshot {
                revision: state.revision,
            })]
        );
    }

    #[test]
    fn ephemeral_profile_is_distinct_and_transient() {
        assert!(PrivacyKind::Ephemeral.is_transient());
        assert!(PrivacyKind::Ephemeral.is_ephemeral());
        assert!(PrivacyKind::Private.is_transient());
        assert!(!PrivacyKind::Private.is_ephemeral());
        assert!(!PrivacyKind::Normal.is_transient());

        let mut state = ApplicationState::new();
        reduce(
            &mut state,
            Event::CreateProfile {
                label: "Ephemeral · 1234abcd".into(),
                privacy: PrivacyKind::Ephemeral,
            },
        )
        .expect("ephemeral profile creates");
        let profile = state.profiles.values().next().expect("profile exists");
        assert_eq!(profile.privacy, PrivacyKind::Ephemeral);
        assert_eq!(profile.label, "Ephemeral · 1234abcd");
    }

    #[test]
    fn pinning_keeps_pinned_tabs_first_and_mute_is_state_only() {
        let (mut state, _, window, first, _) = setup();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let second = state.windows[&window].active_tab.unwrap();
        reduce(
            &mut state,
            Event::SetTabPinned {
                tab: second,
                pinned: true,
            },
        )
        .unwrap();
        assert_eq!(state.windows[&window].tabs, vec![second, first]);
        reduce(
            &mut state,
            Event::SetTabMuted {
                tab: first,
                muted: true,
            },
        )
        .unwrap();
        assert!(state.tabs[&first].muted);
        assert!(state.tabs[&second].pinned);
        assert!(state.validate().is_ok());
    }

    #[test]
    fn moving_tabs_cannot_cross_the_pinned_boundary() {
        let (mut state, _, window, first, _) = setup();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let second = state.windows[&window].active_tab.unwrap();
        reduce(
            &mut state,
            Event::SetTabPinned {
                tab: first,
                pinned: true,
            },
        )
        .unwrap();
        reduce(
            &mut state,
            Event::MoveTab {
                tab: second,
                to_window: window,
                index: Some(0),
            },
        )
        .unwrap();
        assert_eq!(state.windows[&window].tabs, vec![first, second]);
        assert!(state.validate().is_ok());
    }

    #[test]
    fn navigation_invalidates_old_document_and_emits_engine_effect() {
        let (mut state, _, _, tab, old_target) = setup();
        let effects = reduce(
            &mut state,
            Event::StartNavigation {
                target: old_target,
                url: url("https://example.test/"),
            },
        )
        .unwrap();
        let new_target = match &effects[0] {
            Effect::Engine(EngineEffect::Navigate { target, .. }) => *target,
            effect => panic!("unexpected effect: {effect:?}"),
        };
        assert_ne!(new_target.document, old_target.document);
        assert_eq!(state.tabs[&tab].loading, LoadingState::Provisional);
        let stale_effects = reduce(
            &mut state,
            Event::CommitNavigation {
                target: old_target,
                url: url("https://stale.test/"),
                title: "stale".into(),
            },
        )
        .unwrap();
        assert_eq!(
            stale_effects,
            vec![Effect::Diagnostic(Diagnostic::IgnoredStaleTarget { tab })]
        );
        reduce(
            &mut state,
            Event::CommitNavigation {
                target: new_target,
                url: url("https://example.test/"),
                title: "Example".into(),
            },
        )
        .unwrap();
        assert_eq!(state.tabs[&tab].title, "Example");
        assert_eq!(state.tabs[&tab].loading, LoadingState::Committed);
        reduce(&mut state, Event::CompleteNavigation { target: new_target }).unwrap();
        assert_eq!(state.tabs[&tab].loading, LoadingState::Complete);
    }

    #[test]
    fn renderer_termination_is_a_failed_live_document() {
        let (mut state, _, _, tab, target) = setup();
        reduce(
            &mut state,
            Event::StartNavigation {
                target,
                url: url("https://example.test/"),
            },
        )
        .unwrap();
        let current = state.capture_target(tab).unwrap();
        reduce(&mut state, Event::RendererTerminated { target: current }).unwrap();
        assert_eq!(state.tabs[&tab].renderer, RendererState::Terminated);
        assert_eq!(state.tabs[&tab].loading, LoadingState::Failed);
        assert_eq!(state.tabs[&tab].existence, ExistenceState::Live);
        assert_eq!(state.tabs[&tab].resources, ResourceLifecycle::Active);
    }

    #[test]
    fn explicit_reload_and_new_commit_restore_renderer_health() {
        let (mut state, _, _, tab, target) = setup();
        reduce(&mut state, Event::RendererTerminated { target }).unwrap();
        assert_eq!(state.tabs[&tab].renderer, RendererState::Terminated);

        reduce(
            &mut state,
            Event::Reload {
                target,
                bypass_cache: false,
            },
        )
        .unwrap();
        assert_eq!(state.tabs[&tab].renderer, RendererState::Healthy);
        let current = state.capture_target(tab).unwrap();
        reduce(
            &mut state,
            Event::CommitNavigation {
                target: current,
                url: url("https://example.test/recovered"),
                title: "Recovered".into(),
            },
        )
        .unwrap();
        assert_eq!(state.tabs[&tab].renderer, RendererState::Healthy);
    }

    #[test]
    fn engine_control_effects_preserve_captured_target() {
        let (mut state, _, _, tab, target) = setup();
        assert_eq!(
            reduce(
                &mut state,
                Event::Reload {
                    target,
                    bypass_cache: true,
                },
            )
            .unwrap(),
            vec![Effect::Engine(EngineEffect::Reload {
                target,
                bypass_cache: true,
            })]
        );
        assert_eq!(state.tabs[&tab].loading, LoadingState::Provisional);
        assert_eq!(
            reduce(&mut state, Event::Stop { target }).unwrap(),
            vec![Effect::Engine(EngineEffect::Stop { target })]
        );
        assert_eq!(state.tabs[&tab].loading, LoadingState::Cancelled);
        assert_eq!(
            reduce(&mut state, Event::TraverseHistory { target, offset: -1 },).unwrap(),
            vec![Effect::Engine(EngineEffect::TraverseHistory {
                target,
                offset: -1,
            })]
        );
        let stale_target = Target {
            tab,
            generation: target.generation + 1,
            document: target.document,
        };
        assert_eq!(
            reduce(
                &mut state,
                Event::Stop {
                    target: stale_target,
                },
            )
            .unwrap(),
            vec![Effect::Diagnostic(Diagnostic::IgnoredStaleTarget { tab })]
        );
    }

    #[test]
    fn search_is_targeted_retained_and_cleared_explicitly() {
        let (mut state, _, _, tab, target) = setup();
        assert_eq!(
            reduce(
                &mut state,
                Event::StartSearch {
                    target,
                    query: "needle".into(),
                    backward: false,
                    case: SearchCase::Smart,
                },
            )
            .unwrap(),
            vec![Effect::Engine(EngineEffect::FindText {
                target,
                query: "needle".into(),
                backward: false,
                case: SearchCase::Smart,
            })]
        );
        assert_eq!(state.tabs[&tab].search.as_ref().unwrap().query, "needle");
        assert_eq!(
            reduce(
                &mut state,
                Event::SearchNext {
                    target,
                    backward: true,
                },
            )
            .unwrap(),
            vec![Effect::Engine(EngineEffect::FindText {
                target,
                query: "needle".into(),
                backward: true,
                case: SearchCase::Smart,
            })]
        );
        assert!(state.tabs[&tab].search.as_ref().unwrap().backward);
        assert_eq!(
            reduce(&mut state, Event::EndSearch { target }).unwrap(),
            vec![Effect::Engine(EngineEffect::ClearFindText { target })]
        );
        assert!(state.tabs[&tab].search.is_none());
    }

    #[test]
    fn close_freezes_tab_until_view_closed_and_stale_callbacks_do_nothing() {
        let (mut state, _, window, tab, target) = setup();
        let effects = reduce(&mut state, Event::CloseTab { tab }).unwrap();
        assert!(matches!(
            effects.as_slice(),
            [Effect::Engine(EngineEffect::CloseView { tab: closed, generation: 0 }), Effect::Persist(_)]
                if *closed == tab
        ));
        assert_eq!(state.tabs[&tab].existence, ExistenceState::Closing);
        assert_eq!(state.tabs[&tab].resources, ResourceLifecycle::Frozen);
        assert_eq!(state.windows[&window].active_tab, None);
        let stale_effects = reduce(&mut state, Event::RendererTerminated { target }).unwrap();
        assert_eq!(
            stale_effects,
            vec![Effect::Diagnostic(Diagnostic::IgnoredStaleTarget { tab })]
        );
        reduce(
            &mut state,
            Event::ViewClosed {
                tab,
                generation: target.generation,
            },
        )
        .unwrap();
        assert!(!state.tabs.contains_key(&tab));
        let blank = state.windows[&window].active_tab.expect("blank tab");
        assert_ne!(blank, tab);
        assert_eq!(state.tabs[&blank].url.as_deref(), Some("about:blank"));
        assert_eq!(state.tabs[&blank].loading, LoadingState::Complete);
        assert!(state.validate().is_ok());
    }

    #[test]
    fn closing_active_tab_in_two_tab_window_selects_and_keeps_the_neighbor() {
        let (mut state, _, window, first, _) = setup();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let second = state.windows[&window].active_tab.expect("second tab");
        let generation = state.tabs[&second].generation;

        reduce(&mut state, Event::CloseTab { tab: second }).unwrap();
        assert_eq!(state.windows[&window].active_tab, Some(first));
        assert_eq!(state.tabs[&second].existence, ExistenceState::Closing);

        reduce(
            &mut state,
            Event::ViewClosed {
                tab: second,
                generation,
            },
        )
        .unwrap();
        assert_eq!(state.windows[&window].tabs, vec![first]);
        assert_eq!(state.windows[&window].active_tab, Some(first));
        assert!(!state.tabs.contains_key(&second));
        assert!(state.validate().is_ok());
    }

    #[test]
    fn suspension_only_freezes_hidden_live_tabs_and_resume_restores_them() {
        let (mut state, _, window, active_tab, _) = setup();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let suspended_tab = state
            .windows
            .get(&window)
            .and_then(|window| window.tabs.last().copied())
            .expect("new tab");
        reduce(
            &mut state,
            Event::ActivateTab {
                window,
                tab: active_tab,
            },
        )
        .unwrap();
        assert_ne!(active_tab, suspended_tab);
        let effects = reduce(&mut state, Event::SuspendTab { tab: suspended_tab }).unwrap();
        assert!(matches!(
            effects.first(),
            Some(Effect::Engine(EngineEffect::SetTabLifecycle {
                tab,
                state: ResourceLifecycle::Frozen,
                ..
            })) if *tab == suspended_tab
        ));
        assert_eq!(
            state.tabs[&suspended_tab].resources,
            ResourceLifecycle::Frozen
        );
        assert_eq!(
            reduce(&mut state, Event::SuspendTab { tab: active_tab }),
            Err(ReduceError::TabNotHidden)
        );
        let effects = reduce(&mut state, Event::ResumeTab { tab: suspended_tab }).unwrap();
        assert!(matches!(
            effects.first(),
            Some(Effect::Engine(EngineEffect::SetTabLifecycle {
                tab,
                state: ResourceLifecycle::Active,
                ..
            })) if *tab == suspended_tab
        ));
        assert_eq!(
            state.tabs[&suspended_tab].resources,
            ResourceLifecycle::Active
        );
        assert!(state.validate().is_ok());
    }

    #[test]
    fn discard_only_discards_hidden_active_tabs_and_resume_restores_them() {
        let (mut state, _, window, active_tab, _) = setup();
        reduce(&mut state, Event::OpenTab { window }).unwrap();
        let discarded_tab = state
            .windows
            .get(&window)
            .and_then(|window| window.tabs.last().copied())
            .expect("new tab");
        reduce(
            &mut state,
            Event::ActivateTab {
                window,
                tab: active_tab,
            },
        )
        .unwrap();
        let effects = reduce(&mut state, Event::DiscardTab { tab: discarded_tab }).unwrap();
        assert!(matches!(
            effects.first(),
            Some(Effect::Engine(EngineEffect::SetTabLifecycle {
                tab,
                state: ResourceLifecycle::Discarded,
                ..
            })) if *tab == discarded_tab
        ));
        assert_eq!(
            state.tabs[&discarded_tab].resources,
            ResourceLifecycle::Discarded
        );
        assert_eq!(
            reduce(&mut state, Event::DiscardTab { tab: active_tab }),
            Err(ReduceError::TabNotHidden)
        );
        reduce(&mut state, Event::ResumeTab { tab: discarded_tab }).unwrap();
        assert_eq!(
            state.tabs[&discarded_tab].resources,
            ResourceLifecycle::Active
        );
        assert!(state.validate().is_ok());
    }

    #[test]
    fn popups_require_gesture_and_inherit_opener_profile() {
        let (mut state, profile, window, opener_tab, opener_target) = setup();
        let blocked = reduce(
            &mut state,
            Event::OpenPopup {
                opener: opener_target,
                url: url("https://popup.example/"),
                user_gesture: false,
            },
        )
        .unwrap();
        assert_eq!(
            blocked,
            vec![Effect::Diagnostic(Diagnostic::PopupBlocked {
                opener: opener_tab
            })]
        );
        assert_eq!(state.tabs.len(), 1);

        let effects = reduce(
            &mut state,
            Event::OpenPopup {
                opener: opener_target,
                url: url("https://popup.example/"),
                user_gesture: true,
            },
        )
        .unwrap();
        let popup_target = match &effects[0] {
            Effect::Engine(EngineEffect::Navigate { target, .. }) => *target,
            effect => panic!("unexpected effect: {effect:?}"),
        };
        let popup = &state.tabs[&popup_target.tab];
        assert_eq!(popup.window, window);
        assert_eq!(popup.profile, profile);
        assert_eq!(popup.generation, 1);
        assert_eq!(popup.loading, LoadingState::Provisional);
        assert!(state.validate().is_ok());
    }

    #[test]
    fn same_profile_tab_move_reparents_without_replacing_document() {
        let (mut state, profile, source_window, tab, target) = setup();
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let destination = state
            .windows
            .keys()
            .copied()
            .find(|window| *window != source_window)
            .expect("second window");

        let effects = reduce(
            &mut state,
            Event::MoveTab {
                tab,
                to_window: destination,
                index: None,
            },
        )
        .unwrap();
        assert_eq!(
            effects.first(),
            Some(&Effect::Engine(EngineEffect::ReparentView {
                tab,
                generation: target.generation,
                window: destination,
            }))
        );
        assert_eq!(state.tabs[&tab].window, destination);
        assert_eq!(state.tabs[&tab].generation, target.generation);
        assert_eq!(state.tabs[&tab].document, target.document);
        assert_eq!(state.windows[&source_window].active_tab, None);
        assert_eq!(state.windows[&destination].active_tab, Some(tab));
        assert!(state.validate().is_ok());
    }

    #[test]
    fn transfer_out_removes_live_tab_without_closing_engine_view() {
        let (mut state, _, window, tab, target) = setup();
        let effects = reduce(&mut state, Event::TransferTabOut { tab }).unwrap();
        assert_eq!(state.tabs.get(&tab), None);
        assert!(state.windows[&window].tabs.is_empty());
        assert_eq!(state.windows[&window].active_tab, None);
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, Effect::Engine(_)))
        );
        assert!(state.validate().is_ok());
        assert_eq!(target.tab, tab);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn transfer_in_restores_tab_metadata_into_a_live_window() {
        let (mut state, _, window, _, _) = setup();
        let effects = reduce(
            &mut state,
            Event::TransferTabIn {
                window,
                transfer: TabTransfer {
                    url: Some("https://example.test/".into()),
                    title: "Example".into(),
                    loading: LoadingState::Complete,
                    pinned: true,
                    muted: true,
                    zoom_hundredths: 125,
                },
            },
        )
        .unwrap();
        let tab = state.windows[&window].active_tab.expect("adopted tab");
        let tab_state = &state.tabs[&tab];
        assert_eq!(tab_state.url.as_deref(), Some("https://example.test/"));
        assert_eq!(tab_state.title, "Example");
        assert_eq!(tab_state.loading, LoadingState::Complete);
        assert!(tab_state.pinned);
        assert!(tab_state.muted);
        assert_eq!(tab_state.zoom, 1.25);
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, Effect::Persist(_)))
        );
        assert!(state.validate().is_ok());
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn tab_zoom_updates_only_the_live_tab_and_persists() {
        let (mut state, _, _, tab, _) = setup();
        let effects = reduce(
            &mut state,
            Event::SetTabZoom {
                tab,
                zoom_hundredths: 125,
            },
        )
        .unwrap();
        assert_eq!(state.tabs[&tab].zoom, 1.25);
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, Effect::Persist(_)))
        );
        assert_eq!(
            reduce(
                &mut state,
                Event::SetTabZoom {
                    tab,
                    zoom_hundredths: 501,
                },
            ),
            Err(ReduceError::InvalidZoom)
        );
        assert_eq!(state.tabs[&tab].zoom, 1.25);
        assert!(state.validate().is_ok());
    }

    #[test]
    fn tab_move_between_profiles_is_refused() {
        let (mut state, _, source_window, tab, _) = setup();
        reduce(
            &mut state,
            Event::CreateProfile {
                label: "isolated".into(),
                privacy: PrivacyKind::Normal,
            },
        )
        .unwrap();
        let other_profile = state
            .profiles
            .keys()
            .copied()
            .find(|profile| *profile != state.windows[&source_window].profile)
            .expect("second profile");
        reduce(
            &mut state,
            Event::CreateWindow {
                profile: other_profile,
            },
        )
        .unwrap();
        let destination = *state
            .windows
            .keys()
            .find(|window| **window != source_window)
            .expect("other window");
        assert_eq!(
            reduce(
                &mut state,
                Event::MoveTab {
                    tab,
                    to_window: destination,
                    index: None,
                },
            ),
            Err(ReduceError::WrongProfile)
        );
        assert_eq!(state.tabs[&tab].window, source_window);
        assert!(state.validate().is_ok());
    }

    #[test]
    fn profile_boundaries_and_shutdown_are_enforced() {
        let (mut state, _, window, _, _) = setup();
        reduce(&mut state, Event::RequestShutdown).unwrap();
        let error = reduce(&mut state, Event::OpenTab { window }).unwrap_err();
        assert_eq!(error, ReduceError::ShutdownInProgress);
    }

    #[test]
    fn escape_returns_from_overlay_to_normal_without_page_effect() {
        let (mut state, _, window, _, _) = setup();
        reduce(
            &mut state,
            Event::PushMode {
                window,
                mode: Mode::Command,
            },
        )
        .unwrap();
        let effects = reduce(&mut state, Event::Escape { window }).unwrap();
        assert!(effects.is_empty());
        assert_eq!(state.windows[&window].modes, vec![Mode::Normal]);
    }

    #[test]
    fn same_document_navigation_updates_url_without_new_document() {
        let (mut state, _, _, tab, target) = setup();
        reduce(
            &mut state,
            Event::CommitNavigation {
                target,
                url: url("https://example.test/page#one"),
                title: "Page".into(),
            },
        )
        .unwrap();
        reduce(
            &mut state,
            Event::SameDocumentNavigation {
                target,
                url: url("https://example.test/page#two"),
            },
        )
        .unwrap();
        let tab_state = &state.tabs[&tab];
        assert_eq!(
            tab_state.url.as_deref(),
            Some("https://example.test/page#two")
        );
        assert_eq!(tab_state.generation, target.generation);
        assert_eq!(tab_state.document, target.document);
    }

    #[test]
    fn window_context_assignment_is_persistent_and_profile_neutral() {
        let (mut state, _, window, _, _) = setup();
        let before = state.revision;
        let effects = reduce(
            &mut state,
            Event::SetWindowContext {
                window,
                context: Some("work".into()),
            },
        )
        .unwrap();
        assert!(matches!(effects.first(), Some(Effect::Persist(_))));
        assert_eq!(state.windows[&window].context.as_deref(), Some("work"));
        assert!(state.revision > before);
        reduce(
            &mut state,
            Event::SetWindowContext {
                window,
                context: None,
            },
        )
        .unwrap();
        assert_eq!(state.windows[&window].context, None);
        assert!(state.validate().is_ok());
    }
}
