use std::collections::{BTreeMap, BTreeSet};

use crate::{IdSource, JourneyNodeId, ProfileId, TabId, ValidatedUrl};

pub const MAX_JOURNEY_NODES: usize = 50_000;
pub const MAX_JOURNEY_EDGES: usize = 100_000;
const MAX_TITLE_BYTES: usize = 4 * 1024;
const MAX_SOURCE_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JourneyEdgeKind {
    Navigate,
    Redirect,
    Opener,
    Popup,
    Hint,
    SessionRestore,
    Reopen,
    BranchAfterBack,
}

impl JourneyEdgeKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Navigate => "navigate",
            Self::Redirect => "redirect",
            Self::Opener => "opener",
            Self::Popup => "popup",
            Self::Hint => "hint",
            Self::SessionRestore => "session-restore",
            Self::Reopen => "reopen",
            Self::BranchAfterBack => "branch-after-back",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JourneyNode {
    pub id: JourneyNodeId,
    pub profile: ProfileId,
    pub url: String,
    pub title: String,
    pub committed_at: u64,
    pub transition: JourneyEdgeKind,
    pub source: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JourneyEdge {
    pub source: JourneyNodeId,
    pub target: JourneyNodeId,
    pub kind: JourneyEdgeKind,
    pub created_at: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct JourneyGraph {
    nodes: BTreeMap<JourneyNodeId, JourneyNode>,
    edges: Vec<JourneyEdge>,
    current_by_tab: BTreeMap<TabId, JourneyNodeId>,
    paths_by_tab: BTreeMap<TabId, JourneyPath>,
    pending_traversals: BTreeMap<TabId, i32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct JourneyPath {
    nodes: Vec<JourneyNodeId>,
    cursor: usize,
}

impl JourneyGraph {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one safe committed navigation and links it to the tab's prior
    /// current node. The caller supplies a deterministic timestamp, so this
    /// remains a pure reducer-owned data boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn record_navigation(
        &mut self,
        ids: &mut IdSource,
        profile: ProfileId,
        tab: TabId,
        url: &ValidatedUrl,
        title: &str,
        committed_at: u64,
        transition: JourneyEdgeKind,
        source: Option<&str>,
    ) -> JourneyNodeId {
        self.record_navigation_from(
            ids,
            profile,
            tab,
            url,
            title,
            committed_at,
            transition,
            source,
            None,
        )
    }

    /// Records one safe committed navigation and optionally links it to a
    /// validated node from another tab, such as an opener or popup source.
    #[allow(clippy::too_many_arguments)]
    pub fn record_navigation_from(
        &mut self,
        ids: &mut IdSource,
        profile: ProfileId,
        tab: TabId,
        url: &ValidatedUrl,
        title: &str,
        committed_at: u64,
        transition: JourneyEdgeKind,
        source: Option<&str>,
        parent: Option<JourneyNodeId>,
    ) -> JourneyNodeId {
        let id = ids.journey_node();
        let branched = self
            .paths_by_tab
            .get(&tab)
            .is_some_and(|path| path.cursor + 1 < path.nodes.len());
        let transition = if branched && transition == JourneyEdgeKind::Navigate {
            JourneyEdgeKind::BranchAfterBack
        } else {
            transition
        };
        let node = JourneyNode {
            id,
            profile,
            url: url.to_string(),
            title: bounded_text(title, MAX_TITLE_BYTES),
            committed_at,
            transition,
            source: source.map(|value| bounded_text(value, MAX_SOURCE_BYTES)),
        };
        let previous = self.current_by_tab.insert(tab, id);
        self.nodes.insert(id, node);
        let path = self.paths_by_tab.entry(tab).or_insert_with(|| JourneyPath {
            nodes: Vec::new(),
            cursor: 0,
        });
        if branched {
            path.nodes.truncate(path.cursor + 1);
        }
        path.nodes.push(id);
        path.cursor = path.nodes.len().saturating_sub(1);
        let parent = parent.filter(|parent| {
            self.nodes
                .get(parent)
                .is_some_and(|node| node.profile == profile)
        });
        if let Some(source) = parent.or(previous) {
            self.edges.push(JourneyEdge {
                source,
                target: id,
                kind: transition,
                created_at: committed_at,
            });
        }
        self.prune();
        id
    }

    /// Requests an engine history traversal. The current node is not changed
    /// until the engine reports a matching committed navigation.
    pub fn request_traversal(&mut self, tab: TabId, offset: i32) {
        let Some(path) = self.paths_by_tab.get(&tab) else {
            return;
        };
        if offset == 0 {
            return;
        }
        let next = i64::try_from(path.cursor)
            .unwrap_or(i64::MAX)
            .saturating_add(i64::from(offset));
        if (0..i64::try_from(path.nodes.len()).unwrap_or(i64::MAX)).contains(&next) {
            self.pending_traversals.insert(tab, offset);
        }
    }

    /// Completes a previously requested traversal and moves the current node
    /// without creating a new durable relationship.
    pub fn complete_traversal(&mut self, tab: TabId) -> Option<JourneyNodeId> {
        let offset = self.pending_traversals.remove(&tab)?;
        let path = self.paths_by_tab.get_mut(&tab)?;
        let next = i64::try_from(path.cursor)
            .ok()?
            .checked_add(i64::from(offset))?;
        if !(0..i64::try_from(path.nodes.len()).ok()?).contains(&next) {
            return None;
        }
        path.cursor = usize::try_from(next).ok()?;
        let node = path.nodes.get(path.cursor).copied()?;
        self.current_by_tab.insert(tab, node);
        Some(node)
    }

    #[must_use]
    pub fn node(&self, id: JourneyNodeId) -> Option<&JourneyNode> {
        self.nodes.get(&id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &JourneyNode> {
        self.nodes.values()
    }

    #[must_use]
    pub fn edges(&self) -> &[JourneyEdge] {
        &self.edges
    }

    #[must_use]
    pub fn current_node(&self, tab: TabId) -> Option<JourneyNodeId> {
        self.current_by_tab.get(&tab).copied()
    }

    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Removes nodes matching a completed history-clear operation and repairs
    /// all graph indexes in place. The caller supplies the same validated
    /// time/origin predicate used by the durable store.
    pub fn clear_matching<F>(&mut self, mut matches: F) -> usize
    where
        F: FnMut(&JourneyNode) -> bool,
    {
        let removed = self
            .nodes
            .values()
            .filter(|node| matches(node))
            .map(|node| node.id)
            .collect::<BTreeSet<_>>();
        if removed.is_empty() {
            return 0;
        }
        for id in &removed {
            self.nodes.remove(id);
        }
        self.edges
            .retain(|edge| !removed.contains(&edge.source) && !removed.contains(&edge.target));
        self.current_by_tab
            .retain(|_, node| !removed.contains(node));

        let mut empty_tabs = BTreeSet::new();
        for (tab, path) in &mut self.paths_by_tab {
            let cursor_node = path.nodes.get(path.cursor).copied();
            path.nodes.retain(|node| !removed.contains(node));
            if path.nodes.is_empty() {
                empty_tabs.insert(*tab);
                continue;
            }
            path.cursor = cursor_node
                .and_then(|node| path.nodes.iter().position(|candidate| *candidate == node))
                .unwrap_or_else(|| path.cursor.min(path.nodes.len() - 1));
        }
        for tab in empty_tabs {
            self.paths_by_tab.remove(&tab);
            self.pending_traversals.remove(&tab);
        }
        removed.len()
    }

    /// Checks that graph records remain profile-local and never point at a
    /// pruned node. The application reducer uses this with its live profiles.
    pub(crate) fn validate_profiles(
        &self,
        profiles: &BTreeMap<ProfileId, crate::ProfileState>,
    ) -> Result<(), &'static str> {
        for node in self.nodes.values() {
            if !profiles.contains_key(&node.profile) {
                return Err("journey node references missing profile");
            }
        }
        for edge in &self.edges {
            if !self.nodes.contains_key(&edge.source) || !self.nodes.contains_key(&edge.target) {
                return Err("journey edge references missing node");
            }
        }
        for node in self.current_by_tab.values() {
            if !self.nodes.contains_key(node) {
                return Err("journey current node was pruned");
            }
        }
        for path in self.paths_by_tab.values() {
            if path.nodes.is_empty() || path.cursor >= path.nodes.len() {
                return Err("journey path cursor is invalid");
            }
            if path.nodes.iter().any(|node| !self.nodes.contains_key(node)) {
                return Err("journey path references missing node");
            }
        }
        Ok(())
    }

    fn prune(&mut self) {
        self.prune_to(MAX_JOURNEY_NODES, MAX_JOURNEY_EDGES);
    }

    fn prune_to(&mut self, max_nodes: usize, max_edges: usize) {
        while self.nodes.len() > max_nodes {
            let path_candidate = self
                .paths_by_tab
                .iter()
                .filter_map(|(tab, path)| {
                    let (node, from_front) = if path.cursor > 0 {
                        (path.nodes.first().copied()?, true)
                    } else if path.cursor + 1 < path.nodes.len() {
                        (path.nodes.last().copied()?, false)
                    } else {
                        return None;
                    };
                    if self.current_by_tab.values().any(|current| *current == node) {
                        return None;
                    }
                    let committed_at = self.nodes.get(&node)?.committed_at;
                    Some((committed_at, node, *tab, from_front))
                })
                .min_by_key(|(committed_at, node, tab, _)| (*committed_at, *node, *tab));
            if let Some((_, node, tab, from_front)) = path_candidate {
                if let Some(path) = self.paths_by_tab.get_mut(&tab) {
                    if from_front {
                        path.nodes.remove(0);
                        path.cursor = path.cursor.saturating_sub(1);
                    } else {
                        path.nodes.pop();
                    }
                }
                self.nodes.remove(&node);
                self.edges
                    .retain(|edge| edge.source != node && edge.target != node);
                continue;
            }
            let mut protected = self
                .current_by_tab
                .values()
                .copied()
                .collect::<BTreeSet<_>>();
            for path in self.paths_by_tab.values() {
                protected.extend(path.nodes.iter().copied());
            }
            let Some(id) = self
                .nodes
                .values()
                .filter(|node| !protected.contains(&node.id))
                .min_by_key(|node| (node.committed_at, node.id))
                .map(|node| node.id)
            else {
                break;
            };
            self.nodes.remove(&id);
            self.edges
                .retain(|edge| edge.source != id && edge.target != id);
        }
        if self.edges.len() > max_edges {
            let excess = self.edges.len() - max_edges;
            let mut remove = (0..self.edges.len()).collect::<Vec<_>>();
            remove.sort_by_key(|index| {
                let edge = &self.edges[*index];
                (edge.created_at, edge.source, edge.target)
            });
            let remove = remove.into_iter().take(excess).collect::<BTreeSet<_>>();
            self.edges = self
                .edges
                .drain(..)
                .enumerate()
                .filter_map(|(index, edge)| (!remove.contains(&index)).then_some(edge))
                .collect();
        }
    }
}

fn bounded_text(value: &str, limit: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .scan(0usize, |bytes, character| {
            let next = *bytes + character.len_utf8();
            if next > limit {
                None
            } else {
                *bytes = next;
                Some(character)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ApplicationState, Event, PrivacyKind, reduce};

    fn url(value: &str) -> ValidatedUrl {
        ValidatedUrl::parse(value).expect("test URL")
    }

    #[test]
    fn records_profile_local_typed_edges_and_preserves_branch_nodes() {
        let mut state = ApplicationState::new();
        reduce(
            &mut state,
            Event::CreateProfile {
                label: "normal".into(),
                privacy: PrivacyKind::Normal,
            },
        )
        .expect("profile");
        let profile = *state.profiles.keys().next().expect("profile id");
        reduce(&mut state, Event::CreateWindow { profile }).expect("window");
        let window = *state.windows.keys().next().expect("window id");
        reduce(&mut state, Event::OpenTab { window }).expect("tab");
        let tab = *state.tabs.keys().next().expect("tab id");
        let first = state.journey.record_navigation(
            &mut state.ids,
            profile,
            tab,
            &url("https://example.test/one"),
            "One\nunsafe",
            1,
            JourneyEdgeKind::Navigate,
            Some("typed-initial-url"),
        );
        let second = state.journey.record_navigation(
            &mut state.ids,
            profile,
            tab,
            &url("https://example.test/two"),
            &"x".repeat(MAX_TITLE_BYTES + 16),
            2,
            JourneyEdgeKind::BranchAfterBack,
            Some("open"),
        );
        assert_eq!(state.journey.current_node(tab), Some(second));
        assert_eq!(state.journey.node(first).expect("first").title, "Oneunsafe");
        assert_eq!(
            state.journey.node(second).expect("second").title.len(),
            MAX_TITLE_BYTES
        );
        assert_eq!(
            state.journey.edges()[0].kind,
            JourneyEdgeKind::BranchAfterBack
        );
        assert_eq!(state.journey.edges()[0].source, first);
        assert!(state.validate().is_ok());
    }

    #[test]
    fn graph_rejects_cross_profile_records() {
        let mut graph = JourneyGraph::new();
        let mut ids = IdSource::new();
        let profile = ids.profile();
        let tab = ids.tab();
        graph.record_navigation(
            &mut ids,
            profile,
            tab,
            &url("https://example.test/"),
            "Example",
            1,
            JourneyEdgeKind::Navigate,
            None,
        );
        let profiles = BTreeMap::new();
        assert_eq!(
            graph.validate_profiles(&profiles),
            Err("journey node references missing profile")
        );
    }

    #[test]
    fn traversal_moves_current_after_commit_and_new_navigation_branches() {
        let mut graph = JourneyGraph::new();
        let mut ids = IdSource::new();
        let profile = ids.profile();
        let tab = ids.tab();
        let first = graph.record_navigation(
            &mut ids,
            profile,
            tab,
            &url("https://example.test/one"),
            "One",
            1,
            JourneyEdgeKind::Navigate,
            None,
        );
        let second = graph.record_navigation(
            &mut ids,
            profile,
            tab,
            &url("https://example.test/two"),
            "Two",
            2,
            JourneyEdgeKind::Navigate,
            None,
        );
        let third = graph.record_navigation(
            &mut ids,
            profile,
            tab,
            &url("https://example.test/three"),
            "Three",
            3,
            JourneyEdgeKind::Navigate,
            None,
        );
        assert_eq!(graph.current_node(tab), Some(third));
        graph.request_traversal(tab, -1);
        assert_eq!(graph.current_node(tab), Some(third));
        assert_eq!(graph.complete_traversal(tab), Some(second));
        assert_eq!(graph.current_node(tab), Some(second));

        let branch = graph.record_navigation(
            &mut ids,
            profile,
            tab,
            &url("https://example.test/branch"),
            "Branch",
            4,
            JourneyEdgeKind::Navigate,
            Some("typed"),
        );
        assert_eq!(
            graph.node(branch).expect("branch").transition,
            JourneyEdgeKind::BranchAfterBack
        );
        assert!(graph.node(first).is_some());
        assert!(graph.node(third).is_some());
        assert_eq!(graph.current_node(tab), Some(branch));
    }

    #[test]
    fn cross_tab_parent_creates_a_typed_relationship() {
        let mut graph = JourneyGraph::new();
        let mut ids = IdSource::new();
        let profile = ids.profile();
        let opener = ids.tab();
        let popup = ids.tab();
        let opener_node = graph.record_navigation(
            &mut ids,
            profile,
            opener,
            &url("https://example.test/opener"),
            "Opener",
            1,
            JourneyEdgeKind::Navigate,
            None,
        );
        let popup_node = graph.record_navigation_from(
            &mut ids,
            profile,
            popup,
            &url("https://example.test/popup"),
            "Popup",
            2,
            JourneyEdgeKind::Popup,
            Some("popup"),
            Some(opener_node),
        );
        assert_eq!(graph.edges().len(), 1);
        assert_eq!(graph.edges()[0].source, opener_node);
        assert_eq!(graph.edges()[0].target, popup_node);
        assert_eq!(graph.edges()[0].kind, JourneyEdgeKind::Popup);
    }

    #[test]
    fn pruning_removes_oldest_traversable_branch_first() {
        let mut graph = JourneyGraph::new();
        let mut ids = IdSource::new();
        let profile = ids.profile();
        let first_tab = ids.tab();
        let second_tab = ids.tab();

        let first_old = graph.record_navigation(
            &mut ids,
            profile,
            first_tab,
            &url("https://example.test/first-old"),
            "First old",
            100,
            JourneyEdgeKind::Navigate,
            None,
        );
        let first_current = graph.record_navigation(
            &mut ids,
            profile,
            first_tab,
            &url("https://example.test/first-current"),
            "First current",
            300,
            JourneyEdgeKind::Navigate,
            None,
        );
        let second_old = graph.record_navigation(
            &mut ids,
            profile,
            second_tab,
            &url("https://example.test/second-old"),
            "Second old",
            200,
            JourneyEdgeKind::Navigate,
            None,
        );
        let second_current = graph.record_navigation(
            &mut ids,
            profile,
            second_tab,
            &url("https://example.test/second-current"),
            "Second current",
            400,
            JourneyEdgeKind::Navigate,
            None,
        );
        graph.prune_to(3, MAX_JOURNEY_EDGES);

        assert!(graph.node(first_old).is_none());
        assert!(graph.node(second_old).is_some());
        assert_eq!(graph.current_node(first_tab), Some(first_current));
        assert_eq!(graph.current_node(second_tab), Some(second_current));
    }

    #[test]
    fn pruning_never_removes_the_active_traversal_node() {
        let mut graph = JourneyGraph::new();
        let mut ids = IdSource::new();
        let profile = ids.profile();
        let tab = ids.tab();
        let old = graph.record_navigation(
            &mut ids,
            profile,
            tab,
            &url("https://example.test/old"),
            "Old",
            1,
            JourneyEdgeKind::Navigate,
            None,
        );
        graph.record_navigation(
            &mut ids,
            profile,
            tab,
            &url("https://example.test/current"),
            "Current",
            2,
            JourneyEdgeKind::Navigate,
            None,
        );
        graph.request_traversal(tab, -1);
        assert_eq!(graph.complete_traversal(tab), Some(old));

        graph.prune_to(0, MAX_JOURNEY_EDGES);

        assert_eq!(graph.current_node(tab), Some(old));
        assert!(graph.node(old).is_some());
        assert_eq!(graph.node_count(), 1);
    }

    #[test]
    fn clear_matching_repairs_current_paths_and_edges() {
        let mut graph = JourneyGraph::new();
        let mut ids = IdSource::new();
        let profile = ids.profile();
        let tab = ids.tab();
        let old = graph.record_navigation(
            &mut ids,
            profile,
            tab,
            &url("https://example.test/old"),
            "Old",
            1,
            JourneyEdgeKind::Navigate,
            None,
        );
        let current = graph.record_navigation(
            &mut ids,
            profile,
            tab,
            &url("https://example.test/current"),
            "Current",
            2,
            JourneyEdgeKind::Navigate,
            None,
        );

        assert_eq!(graph.clear_matching(|node| node.id == old), 1);
        assert!(graph.node(old).is_none());
        assert_eq!(graph.current_node(tab), Some(current));
        assert!(graph.edges().is_empty());
        assert_eq!(graph.node_count(), 1);

        assert_eq!(graph.clear_matching(|node| node.id == current), 1);
        assert_eq!(graph.current_node(tab), None);
        assert_eq!(graph.node_count(), 0);
    }
}
