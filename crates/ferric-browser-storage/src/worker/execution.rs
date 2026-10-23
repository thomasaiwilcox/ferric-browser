//! Worker-thread database and session execution.
use super::{
    JourneyQuerySnapshot, PathBuf, ProfileLibrarySnapshot, ProfileStore, Receiver,
    SessionRestoreSnapshot, StorageCommand, StorageCompletion, StorageOperation,
    StorageOperationError, StoreMode, SyncSender, WorkerMessage, clear_current_session_checkpoints,
    current_session_paths, delete_named_session, list_named_sessions, load_session,
    save_session_atomic,
};

#[allow(clippy::needless_pass_by_value)]
pub(super) fn run_worker(
    path: PathBuf,
    requests: Receiver<WorkerMessage>,
    completions: SyncSender<StorageCompletion>,
) {
    let store = ProfileStore::open(path, StoreMode::Normal).map_err(|error| error.to_string());
    while let Ok(message) = requests.recv() {
        let WorkerMessage::Execute(request) = message else {
            break;
        };
        let completion = match *request {
            StorageCommand::StartPage {
                now,
                retention_before,
                rank,
            } => {
                let result = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .start_page_snapshot_with_ranking(now, retention_before, rank)
                            .map_err(|error| error.to_string())
                    },
                );
                StorageCompletion::StartPage(storage_result(
                    StorageOperation::StartPage,
                    result.map(Box::new),
                ))
            }
            StorageCommand::StartPageWrite {
                mutation,
                timestamp,
            } => {
                let result = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .mutate_start_page(&mutation, timestamp)
                            .map_err(|error| error.to_string())
                    },
                );
                StorageCompletion::StartPageWrite(storage_result(
                    StorageOperation::StartPageWrite,
                    result,
                ))
            }
            StorageCommand::Library { limit } => StorageCompletion::Library(storage_result(
                StorageOperation::Library,
                store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| load_library(store, limit),
                ),
            )),
            StorageCommand::SessionList { root, profile_id } => {
                StorageCompletion::SessionList(storage_result(
                    StorageOperation::SessionList,
                    list_named_sessions(&root, profile_id).map_err(|error| error.to_string()),
                ))
            }
            StorageCommand::SessionRestore { paths } => {
                let snapshot = restore_session_paths(paths);
                StorageCompletion::SessionRestore(storage_result(
                    StorageOperation::SessionRestore,
                    snapshot,
                ))
            }
            StorageCommand::SessionRecovery { root, profile_id } => {
                let snapshot = restore_session_paths(current_session_paths(&root, profile_id));
                StorageCompletion::SessionRestore(storage_result(
                    StorageOperation::SessionRestore,
                    snapshot,
                ))
            }
            StorageCommand::SessionSave { path, snapshot } => {
                StorageCompletion::SessionSave(storage_result(
                    StorageOperation::SessionSave,
                    save_session_atomic(&path, &snapshot).map_err(|error| error.to_string()),
                ))
            }
            StorageCommand::SessionCheckpointClear { root, profile_id } => {
                StorageCompletion::SessionCheckpointClear(storage_result(
                    StorageOperation::SessionCheckpointClear,
                    clear_current_session_checkpoints(&root, profile_id)
                        .map_err(|error| error.to_string()),
                ))
            }
            StorageCommand::SessionDelete {
                root,
                profile_id,
                name,
            } => StorageCompletion::SessionDelete(storage_result(
                StorageOperation::SessionDelete,
                delete_named_session(&root, profile_id, &name).map_err(|error| error.to_string()),
            )),
            StorageCommand::HistoryBatch { visits } => {
                let committed = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .record_visit_batch(&visits)
                            .map(|()| visits.len())
                            .map_err(|error| error.to_string())
                    },
                );
                StorageCompletion::HistoryBatch(storage_result(
                    StorageOperation::HistoryBatch,
                    committed,
                ))
            }
            StorageCommand::CommandRecord { command, timestamp } => {
                let committed = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .record_command(&command, timestamp, true)
                            .map_err(|error| error.to_string())
                    },
                );
                StorageCompletion::CommandRecord(storage_result(
                    StorageOperation::CommandRecord,
                    committed,
                ))
            }
            StorageCommand::PermissionLookup { origin } => {
                let loaded = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .permission_rules_snapshot(Some(&origin))
                            .map_err(|error| error.to_string())
                    },
                );
                StorageCompletion::PermissionLookup(storage_result(
                    StorageOperation::PermissionLookup,
                    loaded,
                ))
            }
            StorageCommand::PermissionBatch { rules } => {
                let committed = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .set_permission_rule_batch(&rules)
                            .map(|()| rules.len())
                            .map_err(|error| error.to_string())
                    },
                );
                StorageCompletion::PermissionBatch(storage_result(
                    StorageOperation::PermissionBatch,
                    committed,
                ))
            }
            StorageCommand::PermissionReset {
                origin,
                permissions,
            } => {
                let removed = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .reset_permission_rule_batch(&origin, &permissions)
                            .map_err(|error| error.to_string())
                    },
                );
                StorageCompletion::PermissionReset(storage_result(
                    StorageOperation::PermissionReset,
                    removed,
                ))
            }
            StorageCommand::DownloadSave { records } => {
                let saved = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .save_download_batch(&records)
                            .map(|()| records.len())
                            .map_err(|error| error.to_string())
                    },
                );
                StorageCompletion::DownloadSave(storage_result(
                    StorageOperation::DownloadSave,
                    saved,
                ))
            }
            StorageCommand::DownloadBatch { updates } => {
                let committed = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .update_download_batch(&updates)
                            .map(|()| updates.len())
                            .map_err(|error| error.to_string())
                    },
                );
                StorageCompletion::DownloadBatch(storage_result(
                    StorageOperation::DownloadBatch,
                    committed,
                ))
            }
            StorageCommand::DownloadDestination { id, destination } => {
                let updated = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .set_download_destination(&id, &destination)
                            .map_err(|error| error.to_string())
                    },
                );
                StorageCompletion::DownloadDestination(storage_result(
                    StorageOperation::DownloadDestination,
                    updated,
                ))
            }
            StorageCommand::DownloadCreate {
                id,
                source_url,
                destination,
                state,
                created_at,
            } => {
                let created = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .create_download(&id, &source_url, &destination, state, created_at)
                            .map_err(|error| error.to_string())
                    },
                );
                StorageCompletion::DownloadCreate(storage_result(
                    StorageOperation::DownloadCreate,
                    created,
                ))
            }
            StorageCommand::JourneyWrite { write } => {
                let applied = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .apply_journey_write(&write)
                            .map_err(|error| error.to_string())
                    },
                );
                StorageCompletion::JourneyWrite(storage_result(
                    StorageOperation::JourneyWrite,
                    applied,
                ))
            }
            StorageCommand::JourneyNode { id } => {
                let node = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| store.journey_node(&id).map_err(|error| error.to_string()),
                );
                StorageCompletion::JourneyNode(storage_result(StorageOperation::JourneyNode, node))
            }
            StorageCommand::JourneyQuery {
                current_tab_id,
                search,
                expand,
            } => {
                let result = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        load_journey_query(
                            store,
                            current_tab_id.as_deref(),
                            search.as_deref(),
                            expand.as_deref(),
                        )
                    },
                );
                StorageCompletion::JourneyQuery(storage_result(
                    StorageOperation::JourneyQuery,
                    result,
                ))
            }
            StorageCommand::JourneyExport => {
                let result = store
                    .as_ref()
                    .map_or_else(|error| Err(error.clone()), load_journey_export);
                StorageCompletion::JourneyExport(storage_result(
                    StorageOperation::JourneyExport,
                    result,
                ))
            }
            StorageCommand::MarkBatch { writes } => {
                let committed = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .apply_mark_write_batch(&writes)
                            .map(|()| writes.len())
                            .map_err(|error| error.to_string())
                    },
                );
                StorageCompletion::MarkWrite(storage_result(StorageOperation::MarkWrite, committed))
            }
            StorageCommand::HistoryClear { since, origin } => {
                let deleted = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .clear_history(since, origin.as_deref())
                            .map_err(|error| error.to_string())
                    },
                );
                StorageCompletion::HistoryClear(storage_result(
                    StorageOperation::HistoryClear,
                    deleted,
                ))
            }
            StorageCommand::Flush => {
                let flushed = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| store.flush().map_err(|error| error.to_string()),
                );
                StorageCompletion::Flush(storage_result(StorageOperation::Flush, flushed))
            }
        };
        if completions.try_send(completion).is_err() {
            break;
        }
    }
}

fn storage_result<T>(
    operation: StorageOperation,
    result: Result<T, String>,
) -> Result<T, StorageOperationError> {
    result.map_err(|message| StorageOperationError { operation, message })
}

fn restore_session_paths(paths: Vec<PathBuf>) -> Result<SessionRestoreSnapshot, String> {
    let mut snapshots = paths
        .into_iter()
        .map(|path| load_session(path).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    // A retained copy of the same session is not another window. Prefer its
    // newest valid revision before resolving recovery ancestry.
    snapshots.sort_by_key(|snapshot| (snapshot.session_id, std::cmp::Reverse(snapshot.revision)));
    snapshots.dedup_by_key(|snapshot| snapshot.session_id);
    let indices = snapshots
        .iter()
        .enumerate()
        .map(|(index, snapshot)| (snapshot.session_id, index))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut incoming = vec![0_usize; snapshots.len()];
    for snapshot in &snapshots {
        for id in &snapshot.recovered_session_ids {
            if let Some(index) = indices.get(id) {
                incoming[*index] += 1;
            }
        }
    }
    let roots = incoming
        .iter()
        .enumerate()
        .filter_map(|(index, count)| (*count == 0).then_some(index))
        .collect::<Vec<_>>();
    let mut pending = roots.clone();
    let mut visited = 0;
    while let Some(index) = pending.pop() {
        visited += 1;
        for id in &snapshots[index].recovered_session_ids {
            if let Some(index) = indices.get(id) {
                incoming[*index] -= 1;
                if incoming[*index] == 0 {
                    pending.push(*index);
                }
            }
        }
    }
    if visited != snapshots.len() {
        return Err("recovery checkpoint ancestry is cyclic".into());
    }
    let mut restored = SessionRestoreSnapshot {
        source_session_ids: snapshots
            .iter()
            .map(|snapshot| snapshot.session_id)
            .collect(),
        entries: Vec::new(),
        closed_tabs: Vec::new(),
    };
    for index in roots {
        let snapshot = &snapshots[index];
        restored
            .entries
            .extend(snapshot.restore_plan().map_err(|error| error.to_string())?);
        restored
            .closed_tabs
            .extend(snapshot.closed_tabs.iter().cloned());
    }
    restored
        .closed_tabs
        .sort_by_key(|tab| std::cmp::Reverse(tab.closed_at));
    let mut seen = std::collections::BTreeSet::new();
    restored.closed_tabs.retain(|tab| seen.insert(tab.id));
    restored.closed_tabs.truncate(100);
    Ok(restored)
}

fn load_library(store: &ProfileStore, limit: usize) -> Result<ProfileLibrarySnapshot, String> {
    let permissions = store
        .permission_rules_snapshot(None)
        .map_err(|error| error.to_string())?;
    Ok(ProfileLibrarySnapshot {
        history: store.history(limit).map_err(|error| error.to_string())?,
        command_history: store
            .command_history(limit)
            .map_err(|error| error.to_string())?,
        bookmarks: store.bookmarks(limit).map_err(|error| error.to_string())?,
        quickmarks: store.quickmarks(limit).map_err(|error| error.to_string())?,
        downloads: store
            .downloads()
            .map_err(|error| error.to_string())?
            .into_iter()
            .take(limit)
            .collect(),
        permissions: permissions.rules,
        permissions_complete: permissions.complete,
    })
}

fn load_journey_query(
    store: &ProfileStore,
    current_tab_id: Option<&str>,
    search: Option<&str>,
    expand: Option<&str>,
) -> Result<JourneyQuerySnapshot, String> {
    let mut nodes = if let Some(node_id) = expand {
        let center = store
            .journey_node(node_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "journey node is stale or unavailable".to_owned())?;
        let mut nodes = vec![center];
        nodes.extend(
            store
                .journey_neighbors(node_id, 999)
                .map_err(|error| error.to_string())?,
        );
        nodes
    } else if let Some(query) = search {
        store
            .journey_nodes_search(query, 1_000)
            .map_err(|error| error.to_string())?
    } else if let Some(tab_id) = current_tab_id {
        let current = store
            .current_journey_node(tab_id)
            .map_err(|error| error.to_string())?;
        match current {
            Some(node_id) => store
                .journey_node(&node_id)
                .map_err(|error| error.to_string())?
                .into_iter()
                .collect(),
            None => Vec::new(),
        }
    } else {
        store
            .journey_nodes(1_000)
            .map_err(|error| error.to_string())?
    };
    nodes.sort_by(|left, right| {
        right
            .committed_at
            .cmp(&left.committed_at)
            .then_with(|| right.id.cmp(&left.id))
    });
    let node_ids = nodes.iter().map(|node| node.id.clone()).collect::<Vec<_>>();
    let edges = store
        .journey_edges_for_nodes(&node_ids, 1_000)
        .map_err(|error| error.to_string())?;
    Ok(JourneyQuerySnapshot { nodes, edges })
}

fn load_journey_export(store: &ProfileStore) -> Result<JourneyQuerySnapshot, String> {
    Ok(JourneyQuerySnapshot {
        nodes: store
            .journey_nodes(50_000)
            .map_err(|error| error.to_string())?,
        edges: store
            .journey_edges(100_000)
            .map_err(|error| error.to_string())?,
    })
}

#[cfg(test)]
mod recovery_tests {
    use super::restore_session_paths;
    use crate::{
        RestoreSafety, SessionSnapshot, SnapshotTabInput, SnapshotWindowInput, save_session_atomic,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
    };
    use uuid::Uuid;

    fn checkpoint(root: &Path, name: &str, urls: &[&str]) -> (PathBuf, SessionSnapshot) {
        let id = Uuid::new_v4();
        let snapshot = SessionSnapshot::new(
            id,
            name,
            Uuid::nil(),
            1,
            "now",
            vec![SnapshotWindowInput {
                selected_tab: None,
                workspace: None,
                tabs: urls
                    .iter()
                    .map(|url| SnapshotTabInput {
                        id: Uuid::new_v4(),
                        url: (*url).into(),
                        safety: RestoreSafety::SafeGet,
                        private: false,
                        pinned: false,
                        muted: false,
                        zoom: 1.0,
                        scroll_position: None,
                    })
                    .collect(),
            }],
        )
        .unwrap();
        let path = root.join(format!("current-{id}.json"));
        save_session_atomic(&path, &snapshot).unwrap();
        (path, snapshot)
    }

    #[test]
    fn durable_recovery_lineage_prevents_duplicate_restore_after_another_crash() {
        let root = std::env::temp_dir().join(format!("ferric-lineage-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let (old_path, mut old) = checkpoint(&root, "old", &["https://example.test/old"]);
        old.closed_tabs = [2_u128, 1]
            .into_iter()
            .map(|id| crate::ClosedTabSnapshot {
                id: Uuid::from_u128(id),
                url: format!("https://example.test/closed-{id}"),
                title: String::new(),
                closed_at: 42,
                group_index: None,
                pinned: false,
                muted: false,
                zoom_hundredths: 100,
            })
            .collect();
        save_session_atomic(&old_path, &old).unwrap();
        let (new_path, mut combined) =
            checkpoint(&root, "combined", &["https://example.test/current"]);
        combined.windows[0]
            .tabs
            .push(old.windows[0].tabs[0].clone());
        combined.closed_tabs.clone_from(&old.closed_tabs);
        combined.revision = 2;
        combined.recovered_session_ids = vec![old.session_id];
        save_session_atomic(&new_path, &combined).unwrap();
        let restored = restore_session_paths(vec![old_path.clone(), new_path.clone()]).unwrap();
        assert_eq!(restored.entries.len(), 2);
        assert_eq!(restored.source_session_ids.len(), 2);
        assert_eq!(
            restored.closed_tabs, old.closed_tabs,
            "same-second Undo order must remain stable"
        );
        assert!(restored.source_session_ids.contains(&old.session_id));
        assert!(restored.source_session_ids.contains(&combined.session_id));
        // Corrupting the replacement falls back to its previous generation.
        // The retained old source must still supply any work that generation
        // had not incorporated.
        fs::write(&new_path, b"interrupted replacement").unwrap();
        let fallback = restore_session_paths(vec![old_path, new_path]).unwrap();
        assert_eq!(fallback.entries.len(), 2);
        assert_eq!(
            fallback
                .entries
                .iter()
                .filter(|entry| entry.url.as_deref() == Some("https://example.test/old"))
                .count(),
            1
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_lineage_cycles_are_rejected_even_with_an_independent_checkpoint() {
        let root = std::env::temp_dir().join(format!("ferric-lineage-cycle-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let (a_path, mut a) = checkpoint(&root, "a", &["https://example.test/a"]);
        let (b_path, mut b) = checkpoint(&root, "b", &["https://example.test/b"]);
        let (c_path, _) = checkpoint(&root, "c", &["https://example.test/c"]);
        a.recovered_session_ids = vec![b.session_id];
        b.recovered_session_ids = vec![a.session_id];
        save_session_atomic(&a_path, &a).unwrap();
        save_session_atomic(&b_path, &b).unwrap();
        assert!(
            restore_session_paths(vec![a_path, b_path, c_path])
                .unwrap_err()
                .contains("cyclic")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
