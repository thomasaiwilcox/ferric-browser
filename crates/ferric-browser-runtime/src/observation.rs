//! Facts captured at reduction time, before a later command changes state.

use ferric_browser_core::{
    ApplicationState, DocumentId, Event, ExistenceState, LoadingState, Mode, ProfileId,
    RendererState, ResourceLifecycle, TabId, Target, WindowId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabObservation {
    pub id: TabId,
    pub window: WindowId,
    pub profile: ProfileId,
    pub generation: u64,
    pub document: DocumentId,
    pub existence: ExistenceState,
    pub loading: LoadingState,
    pub renderer: RendererState,
    pub resources: ResourceLifecycle,
    pub selected: bool,
    pub pinned: bool,
    pub muted: bool,
}

/// Captured event facts; tab metadata excludes page text and navigation URLs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeObservation {
    pub event: Event,
    pub tab: Option<TabObservation>,
    pub window: Option<WindowId>,
    pub mode: Mode,
    pub active_window: Option<WindowId>,
    pub window_count: usize,
}

impl RuntimeObservation {
    pub(crate) fn capture(event: &Event, state: &ApplicationState) -> Self {
        let requested_tab = match event {
            Event::OpenTab { window } | Event::OpenTabWithNavigation { window, .. } => state
                .windows()
                .get(window)
                .and_then(|window| window.tabs.last().copied()),
            Event::ActivateTab { tab, .. }
            | Event::MoveTab { tab, .. }
            | Event::TransferTabOut { tab }
            | Event::CloseTab { tab }
            | Event::SuspendTab { tab }
            | Event::DiscardTab { tab }
            | Event::ResumeTab { tab }
            | Event::ViewClosed { tab, .. }
            | Event::SetTabPinned { tab, .. }
            | Event::SetTabMuted { tab, .. }
            | Event::SetTabZoom { tab, .. } => Some(*tab),
            _ => event_target(event).map(|target| target.tab),
        };
        let tab = requested_tab
            .and_then(|id| state.tabs().get(&id))
            .map(|tab| TabObservation {
                id: tab.id,
                window: tab.window,
                profile: tab.profile,
                generation: tab.generation,
                document: tab.document,
                existence: tab.existence,
                loading: tab.loading,
                renderer: tab.renderer,
                resources: tab.resources,
                pinned: tab.pinned,
                muted: tab.muted,
                selected: state
                    .windows()
                    .get(&tab.window)
                    .is_some_and(|window| window.active_tab == Some(tab.id)),
            });
        let window = match event {
            Event::CreateWindow { .. } => state.active_window(),
            Event::CreateProfile { .. }
            | Event::ClearJourney { .. }
            | Event::ClearWindowContexts { .. }
            | Event::RequestShutdown => None,
            Event::FocusWindow { window }
            | Event::SetActiveWindow { window }
            | Event::SetWindowContext { window, .. }
            | Event::TransferTabIn { window, .. }
            | Event::OpenTab { window }
            | Event::OpenTabWithNavigation { window, .. }
            | Event::ActivateTab { window, .. }
            | Event::PushMode { window, .. }
            | Event::PopMode { window }
            | Event::Escape { window } => Some(*window),
            _ => tab.as_ref().map(|tab| tab.window),
        };
        Self {
            event: event.clone(),
            tab,
            window,
            mode: window
                .and_then(|id| state.windows().get(&id))
                .and_then(|window| window.modes.last().copied())
                .unwrap_or(Mode::Normal),
            active_window: state.active_window(),
            window_count: state.windows().len(),
        }
    }
}

fn event_target(event: &Event) -> Option<Target> {
    match event {
        Event::OpenPopup { opener: target, .. }
        | Event::StartNavigation { target, .. }
        | Event::CommitNavigation { target, .. }
        | Event::CommitNavigationWithTransition { target, .. }
        | Event::SameDocumentNavigation { target, .. }
        | Event::CompleteNavigation { target }
        | Event::FailNavigation { target }
        | Event::Reload { target, .. }
        | Event::Stop { target }
        | Event::TraverseHistory { target, .. }
        | Event::StartSearch { target, .. }
        | Event::SearchNext { target, .. }
        | Event::EndSearch { target }
        | Event::RendererTerminated { target }
        | Event::ClipboardEdit { target, .. } => Some(*target),
        _ => None,
    }
}
