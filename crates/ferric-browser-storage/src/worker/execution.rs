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
    paths.into_iter().try_fold(
        SessionRestoreSnapshot {
            entries: Vec::new(),
            closed_tabs: Vec::new(),
        },
        |mut restored, path| {
            let snapshot = load_session(&path).map_err(|error| error.to_string())?;
            let mut entries = snapshot.restore_plan().map_err(|error| error.to_string())?;
            restored.entries.append(&mut entries);
            if restored.closed_tabs.is_empty() {
                restored.closed_tabs = snapshot.closed_tabs;
            }
            Ok(restored)
        },
    )
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
