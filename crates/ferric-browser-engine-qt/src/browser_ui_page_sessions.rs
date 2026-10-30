//! Process-local, typed snapshots of actual window owners. Raw navigation stays
//! inside Rust and is sanitized by `SessionSnapshot` before durable storage.
use super::{
    CxxQtType, ExistenceState, RestoreSafety, SnapshotTabInput, SnapshotWindowInput, Uuid, qobject,
};
use ferric_browser_application::ProfileInstanceId;

#[derive(Clone)]
pub(super) struct RestoredPageWindow {
    pub(super) group: String,
    pub(super) context: Option<String>,
}

#[derive(Clone)]
pub(super) struct LivePageCheckpoint {
    pub(super) session_id: Uuid,
    pub(super) path: Option<std::path::PathBuf>,
    pub(super) recovered: Vec<Uuid>,
}

#[derive(Clone)]
pub(super) struct PageSessionOwner {
    pub(super) profile: ProfileInstanceId,
    pub(super) groups: Vec<(String, SnapshotWindowInput)>,
    pub(super) checkpoint: LivePageCheckpoint,
    pub(super) page_ids: Vec<(Uuid, super::TabId)>,
}

impl PageSessionOwner {
    pub(super) fn capture(ui: &qobject::BrowserUi) -> Option<Self> {
        let rust = ui.rust();
        let state = rust.state.as_ref()?;
        if !rust.page_window_ordinary
            || !rust.view_alive
            || rust.page_window_preparing
            || rust.profile_bootstrap_pending
            || state.configuration().tabs.presentation
                != ferric_browser_config::TabsPresentation::Windows
        {
            return None;
        }
        let window = state.windows().get(&rust.window?)?;
        let profile = state.profiles().get(&window.profile)?;
        if profile.privacy != super::PrivacyKind::Normal {
            return None;
        }
        let mut groups: Vec<(String, SnapshotWindowInput)> = Vec::new();
        let mut page_ids = Vec::new();
        for id in &window.tabs {
            let page = state.tabs().get(id)?;
            if page.existence != ExistenceState::Live || page.profile != window.profile {
                continue;
            }
            let restored = rust
                .restored_page_windows
                .get(id)
                .filter(|_| !rust.interaction.prompt_view_identities.contains_key(id));
            let group = restored.map_or_else(
                || {
                    if rust.page_window_group.is_empty() {
                        rust.window_token.to_string()
                    } else {
                        rust.page_window_group.clone()
                    }
                },
                |metadata| metadata.group.clone(),
            );
            let context = restored.map_or_else(
                || window.context.clone(),
                |metadata| metadata.context.clone(),
            );
            let index = groups
                .iter()
                .position(|(key, data)| *key == group && data.context == context)
                .unwrap_or_else(|| {
                    groups.push((
                        group,
                        SnapshotWindowInput {
                            group: Some(Uuid::new_v4()),
                            context,
                            selected_tab: None,
                            workspace: None,
                            tabs: Vec::new(),
                        },
                    ));
                    groups.len() - 1
                });
            let durable_id = Uuid::new_v4();
            page_ids.push((durable_id, *id));
            if window.active_tab == Some(*id) {
                groups[index].1.selected_tab = Some(durable_id);
            }
            groups[index].1.tabs.push(SnapshotTabInput {
                id: durable_id,
                url: page.url.clone().unwrap_or_else(|| "about:blank".into()),
                safety: RestoreSafety::SafeGet,
                private: false,
                pinned: page.pinned,
                muted: page.muted,
                zoom: page.zoom,
                scroll_position: rust.navigation.scroll_positions.get(id).copied(),
            });
        }
        (!groups.is_empty()).then_some(Self {
            profile: state.profile_instance_id()?,
            groups,
            checkpoint: LivePageCheckpoint {
                session_id: rust.session_id,
                path: rust.session_path.clone(),
                recovered: rust.recovered_session_ids.clone(),
            },
            page_ids,
        })
    }
}

pub(super) fn aggregate(
    mut owners: Vec<(u64, u64, PageSessionOwner)>,
    positions: &std::collections::HashMap<Uuid, usize>,
) -> Vec<SnapshotWindowInput> {
    owners.sort_by_key(|(order, _, _)| *order);
    let mut groups: Vec<(String, u64, SnapshotWindowInput)> = Vec::new();
    for (_, focused, owner) in owners {
        for (key, window) in owner.groups {
            if let Some((_, last_focused, merged)) = groups
                .iter_mut()
                .find(|(group, _, input)| *group == key && input.context == window.context)
            {
                if focused >= *last_focused && window.selected_tab.is_some() {
                    merged.selected_tab = window.selected_tab;
                    *last_focused = focused;
                }
                merged.tabs.extend(window.tabs);
            } else {
                groups.push((key, focused, window));
            }
        }
    }
    // Each logical group retains its pinned partition across native owners.
    groups
        .into_iter()
        .map(|(_, _, mut group)| {
            group.tabs.sort_by_key(|tab| {
                (
                    !tab.pinned,
                    positions.get(&tab.id).copied().unwrap_or(usize::MAX),
                )
            });
            group
        })
        .collect()
}
