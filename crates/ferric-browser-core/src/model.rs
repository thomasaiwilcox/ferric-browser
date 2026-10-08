use std::{collections::BTreeMap, fmt};

use crate::{DocumentId, IdSource, JourneyGraph, ProfileId, TabId, VersioningState, WindowId};

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

#[derive(Clone, Eq, PartialEq)]
pub struct SearchState {
    pub query: String,
    pub backward: bool,
    pub case: SearchCase,
}

impl fmt::Debug for SearchState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchState")
            .field("query", &"[redacted]")
            .field("backward", &self.backward)
            .field("case", &self.case)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ProfileState {
    pub id: ProfileId,
    pub label: String,
    pub privacy: PrivacyKind,
}

impl fmt::Debug for ProfileState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProfileState")
            .field("id", &"[redacted]")
            .field("label", &"[redacted]")
            .field("privacy", &self.privacy)
            .finish()
    }
}

#[derive(Clone, PartialEq)]
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

impl fmt::Debug for TabState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TabState")
            .field("id", &"[redacted]")
            .field("window", &"[redacted]")
            .field("profile", &"[redacted]")
            .field("generation", &self.generation)
            .field("document", &"[redacted]")
            .field("url", &"[redacted]")
            .field("title", &"[redacted]")
            .field("existence", &self.existence)
            .field("loading", &self.loading)
            .field("renderer", &self.renderer)
            .field("resources", &self.resources)
            .field("search", &self.search.as_ref().map(|_| "[redacted]"))
            .field("pinned", &self.pinned)
            .field("muted", &self.muted)
            .field("zoom", &self.zoom)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct WindowState {
    pub id: WindowId,
    pub profile: ProfileId,
    pub context: Option<String>,
    pub tabs: Vec<TabId>,
    pub active_tab: Option<TabId>,
    pub previous_tab: Option<TabId>,
    pub modes: Vec<Mode>,
}

impl fmt::Debug for WindowState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WindowState")
            .field("id", &"[redacted]")
            .field("profile", &"[redacted]")
            .field("context", &self.context.as_ref().map(|_| "[redacted]"))
            .field("tab_count", &self.tabs.len())
            .field("has_active_tab", &self.active_tab.is_some())
            .field("has_previous_tab", &self.previous_tab.is_some())
            .field("modes", &self.modes)
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct ApplicationState {
    profiles: BTreeMap<ProfileId, ProfileState>,
    windows: BTreeMap<WindowId, WindowState>,
    tabs: BTreeMap<TabId, TabState>,
    journey: JourneyGraph,
    versioning: VersioningState,
    active_window: Option<WindowId>,
    last_focused_window: Option<WindowId>,
    revision: u64,
    shutdown: ShutdownState,
    ids: IdSource,
}

impl fmt::Debug for ApplicationState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationState")
            .field("profile_count", &self.profiles.len())
            .field("window_count", &self.windows.len())
            .field("tab_count", &self.tabs.len())
            .field("journey_node_count", &self.journey.node_count())
            .field("journey_edge_count", &self.journey.edge_count())
            .field("revision", &self.revision)
            .field("shutdown", &self.shutdown)
            .finish()
    }
}

impl Default for ApplicationState {
    fn default() -> Self {
        Self {
            profiles: BTreeMap::new(),
            windows: BTreeMap::new(),
            tabs: BTreeMap::new(),
            journey: JourneyGraph::new(),
            versioning: VersioningState::default(),
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

    /// Creates a state whose runtime identities are unique across separately
    /// bootstrapped browser-window owners in this process.
    #[must_use]
    pub fn new_process_unique() -> Self {
        Self {
            ids: IdSource::process_shared(),
            ..Self::default()
        }
    }

    /// Clones reducer state for a non-mutating identity projection. The
    /// resulting state allocates locally from a snapshot of the ID source;
    /// it must never be installed as the live state.
    #[must_use]
    pub fn clone_for_identity_preview(&self) -> Self {
        let mut preview = self.clone();
        preview.ids = self.ids.preview_fork();
        preview
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
    pub fn versioning(&self) -> &VersioningState {
        &self.versioning
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

#[cfg(test)]
mod debug_tests {
    use super::*;

    #[test]
    fn state_debug_redacts_profile_tab_search_and_window_data() {
        let sentinel = "private-page-sentinel-3db471";
        let mut ids = IdSource::new();
        let profile = ids.profile();
        let window = ids.window();
        let tab = ids.tab();
        let document = ids.document();
        let profile_state = ProfileState {
            id: profile,
            label: sentinel.into(),
            privacy: PrivacyKind::Normal,
        };
        let tab_state = TabState {
            id: tab,
            window,
            profile,
            generation: 1,
            document,
            url: Some(format!("https://example.test/?q={sentinel}")),
            title: sentinel.into(),
            existence: ExistenceState::Live,
            loading: LoadingState::Complete,
            renderer: RendererState::Healthy,
            resources: ResourceLifecycle::Active,
            search: Some(SearchState {
                query: sentinel.into(),
                backward: false,
                case: SearchCase::Sensitive,
            }),
            pinned: false,
            muted: false,
            zoom: 1.0,
        };
        let window_state = WindowState {
            id: window,
            profile,
            context: Some(sentinel.into()),
            tabs: vec![tab],
            active_tab: Some(tab),
            previous_tab: None,
            modes: vec![Mode::Normal],
        };
        let mut state = ApplicationState::default();
        state.profiles.insert(profile, profile_state.clone());
        state.tabs.insert(tab, tab_state.clone());
        state.windows.insert(window, window_state.clone());

        for projection in [
            format!("{profile_state:?}"),
            format!("{:?}", tab_state.search),
            format!("{tab_state:?}"),
            format!("{window_state:?}"),
            format!("{state:?}"),
        ] {
            assert!(!projection.contains(sentinel), "{projection}");
        }
    }
}
