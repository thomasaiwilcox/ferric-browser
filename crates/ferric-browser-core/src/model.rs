use std::collections::BTreeMap;

use crate::{DocumentId, IdSource, JourneyGraph, ProfileId, TabId, WindowId};

// The reducer is a child of the state model so it is the sole module with
// mutable access to `ApplicationState` internals. All other subsystems use the
// read-only snapshots below and express changes as typed events.
#[path = "reducer.rs"]
pub(crate) mod reducer;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivacyKind {
    Normal,
    Private,
    Ephemeral,
}

impl PrivacyKind {
    #[must_use]
    pub const fn is_transient(self) -> bool {
        matches!(self, Self::Private | Self::Ephemeral)
    }

    #[must_use]
    pub const fn is_ephemeral(self) -> bool {
        matches!(self, Self::Ephemeral)
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Private => "private",
            Self::Ephemeral => "ephemeral",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Mode {
    Normal,
    Insert,
    Command,
    Search,
    Hint,
    Grid,
    Caret,
    PassThrough,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownState {
    Running,
    Requested,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExistenceState {
    Constructing,
    Live,
    Closing,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadingState {
    Idle,
    Provisional,
    Committed,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererState {
    Healthy,
    Terminated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceLifecycle {
    Active,
    Frozen,
    Discarded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchCase {
    Smart,
    Sensitive,
    Insensitive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchState {
    pub query: String,
    pub backward: bool,
    pub case: SearchCase,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileState {
    pub id: ProfileId,
    pub label: String,
    pub privacy: PrivacyKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TabState {
    pub id: TabId,
    pub window: WindowId,
    pub profile: ProfileId,
    pub generation: u64,
    pub document: DocumentId,
    pub url: Option<String>,
    pub title: String,
    pub existence: ExistenceState,
    pub loading: LoadingState,
    pub renderer: RendererState,
    pub resources: ResourceLifecycle,
    pub search: Option<SearchState>,
    pub pinned: bool,
    pub muted: bool,
    pub zoom: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowState {
    pub id: WindowId,
    pub profile: ProfileId,
    pub context: Option<String>,
    pub tabs: Vec<TabId>,
    pub active_tab: Option<TabId>,
    pub previous_tab: Option<TabId>,
    pub modes: Vec<Mode>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApplicationState {
    profiles: BTreeMap<ProfileId, ProfileState>,
    windows: BTreeMap<WindowId, WindowState>,
    tabs: BTreeMap<TabId, TabState>,
    journey: JourneyGraph,
    active_window: Option<WindowId>,
    last_focused_window: Option<WindowId>,
    revision: u64,
    shutdown: ShutdownState,
    ids: IdSource,
}

impl Default for ApplicationState {
    fn default() -> Self {
        Self {
            profiles: BTreeMap::new(),
            windows: BTreeMap::new(),
            tabs: BTreeMap::new(),
            journey: JourneyGraph::new(),
            active_window: None,
            last_focused_window: None,
            revision: 0,
            shutdown: ShutdownState::Running,
            ids: IdSource::new(),
        }
    }
}

impl ApplicationState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn profiles(&self) -> &BTreeMap<ProfileId, ProfileState> {
        &self.profiles
    }

    #[must_use]
    pub fn windows(&self) -> &BTreeMap<WindowId, WindowState> {
        &self.windows
    }

    #[must_use]
    pub fn tabs(&self) -> &BTreeMap<TabId, TabState> {
        &self.tabs
    }

    #[must_use]
    pub fn journey(&self) -> &JourneyGraph {
        &self.journey
    }

    #[must_use]
    pub const fn active_window(&self) -> Option<WindowId> {
        self.active_window
    }

    #[must_use]
    pub const fn last_focused_window(&self) -> Option<WindowId> {
        self.last_focused_window
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn shutdown(&self) -> ShutdownState {
        self.shutdown
    }

    #[must_use]
    pub fn active_tab(&self) -> Option<&TabState> {
        let window = self.active_window.and_then(|id| self.windows.get(&id))?;
        window
            .active_tab
            .and_then(|id| self.tabs.get(&id))
            .filter(|tab| tab.existence == ExistenceState::Live)
    }

    /// Captures the tab and document generations used by an asynchronous
    /// operation. The target becomes stale after navigation or tab closure.
    #[must_use]
    pub fn capture_target(&self, tab: TabId) -> Option<crate::Target> {
        let tab_state = self.tabs.get(&tab)?;
        if tab_state.existence != ExistenceState::Live {
            return None;
        }
        Some(crate::Target {
            tab,
            generation: tab_state.generation,
            document: tab_state.document,
        })
    }
}
