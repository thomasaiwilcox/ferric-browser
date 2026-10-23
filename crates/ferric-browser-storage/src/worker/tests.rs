use super::*;
use crate::{
    DownloadState, RestoreSafety, SessionSnapshot, SnapshotTabInput, SnapshotWindowInput,
    StoreMode, named_session_path, unix_timestamp,
};

fn temp_path(suffix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ferric-browser-storage-worker-{}-{}-{suffix}.sqlite",
        std::process::id(),
        unix_timestamp()
    ))
}

#[test]
fn worker_reads_library_off_thread_with_single_flight_backpressure() {
    let path = temp_path("read");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .record_visit("https://example.test/worker", "Worker", "navigate", 1)
        .expect("visit");
    store
        .add_bookmark("bookmark-1", "https://example.test/worker", "Worker", 1)
        .expect("bookmark");
    store
        .set_quickmark("w", "https://example.test/worker", 1)
        .expect("quickmark");
    store
        .set_permission_rule("https://example.test", "notifications", "allow", None, 1)
        .expect("permission");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker.request_library(10).expect("request");
    assert_eq!(worker.request_library(10), Err(StorageWorkerError::Busy));
    let snapshot = (0..100).find_map(|_| {
        let value = worker.poll_library();
        if value.is_none() {
            thread::sleep(std::time::Duration::from_millis(1));
        }
        value
    });
    let snapshot = snapshot.expect("worker response").expect("library");
    assert_eq!(snapshot.history.len(), 1);
    assert_eq!(snapshot.bookmarks.len(), 1);
    assert_eq!(snapshot.quickmarks.len(), 1);
    assert_eq!(snapshot.permissions.len(), 1);
    drop(worker);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn completion_batch_collects_each_ready_response_exactly_once() {
    let path = temp_path("completion-batch");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker.request_library(10).expect("request");
    let snapshot = (0..100).find_map(|_| {
        let completion = worker
            .poll_events()
            .into_iter()
            .find_map(|completion| match completion {
                StorageCompletion::Library(result) => Some(result),
                _ => None,
            });
        if completion.is_none() {
            thread::sleep(std::time::Duration::from_millis(1));
        }
        completion
    });
    assert!(snapshot.is_some_and(|result| result.is_ok()));
    assert!(worker.poll_events().is_empty());
    drop(worker);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn completion_events_preserve_the_operation_when_storage_fails() {
    let completion =
        StorageCompletion::failed(StorageOperation::HistoryClear, "database unavailable");
    assert!(matches!(
        completion,
        StorageCompletion::HistoryClear(Err(StorageOperationError {
            operation: StorageOperation::HistoryClear,
            message,
        })) if message == "database unavailable"
    ));
}

#[test]
fn worker_lists_named_sessions_off_thread() {
    let path = temp_path("session-list");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    drop(store);
    let root = std::env::temp_dir().join(format!(
        "ferric-browser-session-root-{}-{}",
        std::process::id(),
        unix_timestamp()
    ));
    let profile_id = uuid::Uuid::new_v4();
    std::fs::create_dir_all(root.join("sessions").join(profile_id.to_string()))
        .expect("session directory");

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .request_session_list(root.clone(), profile_id)
        .expect("session list request");
    assert_eq!(
        worker.request_session_list(root.clone(), profile_id),
        Err(StorageWorkerError::Busy)
    );
    let sessions = (0..100).find_map(|_| {
        let value = worker.poll_session_list();
        if value.is_none() {
            thread::sleep(std::time::Duration::from_millis(1));
        }
        value
    });
    assert_eq!(sessions, Some(Ok(Vec::new())));
    let session_path = named_session_path(&root, profile_id, "work").expect("session path");
    let tab_id = uuid::Uuid::new_v4();
    let mut snapshot = SessionSnapshot::new(
        uuid::Uuid::new_v4(),
        "work",
        profile_id,
        1,
        "1",
        vec![SnapshotWindowInput {
            selected_tab: Some(tab_id),
            workspace: None,
            tabs: vec![SnapshotTabInput {
                id: tab_id,
                url: "https://example.test/worker".into(),
                safety: RestoreSafety::SafeGet,
                private: false,
                pinned: false,
                muted: false,
                zoom: 1.0,
                scroll_position: None,
            }],
        }],
    )
    .expect("snapshot");
    snapshot.closed_tabs.push(crate::ClosedTabSnapshot {
        id: uuid::Uuid::new_v4(),
        url: "https://closed.example.test/".into(),
        title: "Closed worker tab".into(),
        closed_at: 2,
        group_index: Some(0),
        pinned: true,
        muted: true,
        zoom_hundredths: 125,
    });
    let current_path = root
        .join("sessions")
        .join(profile_id.to_string())
        .join("current.json");
    worker
        .request_session_save(session_path.clone(), snapshot)
        .expect("session save request");
    let saved = (0..100).find_map(|_| {
        let value = worker.poll_session_save();
        if value.is_none() {
            thread::sleep(std::time::Duration::from_millis(1));
        }
        value
    });
    assert_eq!(saved, Some(Ok(())));
    worker
        .request_session_save(
            current_path.clone(),
            SessionSnapshot::new(
                uuid::Uuid::new_v4(),
                "last-session",
                profile_id,
                1,
                "1",
                vec![],
            )
            .expect("current snapshot"),
        )
        .expect("current session save request");
    let current_saved = (0..100).find_map(|_| {
        let value = worker.poll_session_save();
        if value.is_none() {
            thread::sleep(std::time::Duration::from_millis(1));
        }
        value
    });
    assert_eq!(current_saved, Some(Ok(())));
    worker
        .request_session_restore(session_path.clone())
        .expect("session restore request");
    let plan = (0..100).find_map(|_| {
        let value = worker.poll_session_restore();
        if value.is_none() {
            thread::sleep(std::time::Duration::from_millis(1));
        }
        value
    });
    let plan = plan.expect("restore response").expect("restore plan");
    assert_eq!(plan.entries.len(), 1);
    assert_eq!(
        plan.entries[0].url.as_deref(),
        Some("https://example.test/worker")
    );
    assert_eq!(plan.closed_tabs.len(), 1);
    assert_eq!(plan.closed_tabs[0].url, "https://closed.example.test/");
    worker
        .request_session_delete(root.clone(), profile_id, "work".into())
        .expect("session delete request");
    let deleted = (0..100).find_map(|_| {
        let value = worker.poll_session_delete();
        if value.is_none() {
            thread::sleep(std::time::Duration::from_millis(1));
        }
        value
    });
    assert_eq!(deleted, Some(Ok(())));
    assert!(!session_path.exists());
    worker
        .request_session_checkpoint_clear(root.clone(), profile_id)
        .expect("checkpoint clear request");
    let cleared = (0..100).find_map(|_| {
        let value = worker.poll_session_checkpoint_clear();
        if value.is_none() {
            thread::sleep(std::time::Duration::from_millis(1));
        }
        value
    });
    assert_eq!(cleared, Some(Ok(())));
    assert!(!current_path.exists());
    drop(worker);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn worker_commits_history_batch_atomically_off_thread() {
    let path = temp_path("write");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    let visits = vec![
        VisitInput {
            url: "https://example.test/one".into(),
            title: "One".into(),
            transition: "navigate".into(),
            timestamp: 1,
            retention_before: None,
        },
        VisitInput {
            url: "https://example.test/two".into(),
            title: "Two".into(),
            transition: "link".into(),
            timestamp: 2,
            retention_before: None,
        },
    ];
    worker
        .request_history_batch(visits)
        .expect("history batch request");
    let committed = worker.wait_history_batch(Duration::from_secs(1));
    assert_eq!(committed, Some(Ok(2)));

    worker.request_library(10).expect("library request");
    let snapshot = (0..100).find_map(|_| {
        let value = worker.poll_library();
        if value.is_none() {
            thread::sleep(std::time::Duration::from_millis(1));
        }
        value
    });
    let snapshot = snapshot.expect("worker response").expect("library");
    assert_eq!(snapshot.history.len(), 2);
    drop(worker);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_records_command_and_returns_it_in_library_snapshot() {
    let path = temp_path("command-history");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    drop(store);
    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .try_submit(StorageCommand::CommandRecord {
            command: "open https://example.test".into(),
            timestamp: 42,
        })
        .expect("record request");
    let completion = worker.wait_operation(StorageOperation::CommandRecord, Duration::from_secs(1));
    assert!(matches!(
        completion,
        Some(StorageCompletion::CommandRecord(Ok(true)))
    ));
    worker.request_library(10).expect("library request");
    let snapshot = (0..100)
        .find_map(|_| {
            let value = worker.poll_library();
            if value.is_none() {
                thread::sleep(Duration::from_millis(1));
            }
            value
        })
        .expect("library response")
        .expect("library");
    assert_eq!(snapshot.command_history, ["open https://example.test"]);
    drop(worker);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_commits_permission_batch_off_thread() {
    let path = temp_path("permission");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .request_permission_batch(vec![PermissionRule {
            origin: "https://example.test".into(),
            permission: "notifications".into(),
            decision: "allow".into(),
            expires_at: None,
            updated_at: 1,
        }])
        .expect("permission batch request");
    let committed = (0..100).find_map(|_| {
        let value = worker.poll_permission_batch();
        if value.is_none() {
            thread::sleep(std::time::Duration::from_millis(1));
        }
        value
    });
    assert_eq!(committed, Some(Ok(1)));
    drop(worker);

    let store = ProfileStore::open(&path, StoreMode::Normal).expect("reopen");
    let rules = store.permission_rules(None).expect("rules");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].permission, "notifications");
    assert_eq!(rules[0].decision, "allow");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_resets_permission_rules_off_thread() {
    let path = temp_path("permission-reset");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .set_permission_rule_batch(&[PermissionRule {
            origin: "https://example.test".into(),
            permission: "notifications".into(),
            decision: "allow".into(),
            expires_at: None,
            updated_at: 1,
        }])
        .expect("permission rule");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .request_permission_reset("https://example.test".into(), vec!["notifications".into()])
        .expect("permission reset request");
    let reset = (0..100).find_map(|_| {
        let value = worker.poll_permission_reset();
        if value.is_none() {
            thread::sleep(std::time::Duration::from_millis(1));
        }
        value
    });
    assert_eq!(reset, Some(Ok(true)), "reset response");
    drop(worker);

    let store = ProfileStore::open(&path, StoreMode::Normal).expect("reopen");
    assert_eq!(
        store.permission_rules(None).expect("rules"),
        Vec::<crate::permissions::PermissionRule>::new()
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_flushes_the_profile_checkpoint_off_thread() {
    let path = temp_path("flush");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker.request_flush().expect("flush request");
    let flushed = (0..100).find_map(|_| {
        let value = worker.wait_flush(Duration::from_millis(10));
        if value.is_none() {
            thread::sleep(std::time::Duration::from_millis(1));
        }
        value
    });
    assert_eq!(flushed, Some(Ok(())));
    drop(worker);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_permission_batch_rolls_back_when_one_rule_is_invalid() {
    let path = temp_path("permission-rollback");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .request_permission_batch(vec![
            PermissionRule {
                origin: "https://example.test".into(),
                permission: "notifications".into(),
                decision: "allow".into(),
                expires_at: None,
                updated_at: 1,
            },
            PermissionRule {
                origin: "https://example.test/path".into(),
                permission: "camera".into(),
                decision: "allow".into(),
                expires_at: None,
                updated_at: 1,
            },
        ])
        .expect("permission batch request");
    let result = (0..100).find_map(|_| {
        let value = worker.poll_permission_batch();
        if value.is_none() {
            thread::sleep(std::time::Duration::from_millis(1));
        }
        value
    });
    assert_eq!(
        result,
        Some(Err("permission origin authority is invalid".into()))
    );
    drop(worker);

    let store = ProfileStore::open(&path, StoreMode::Normal).expect("reopen");
    assert_eq!(
        store
            .permission_rules(Some("https://example.test"))
            .expect("rules"),
        Vec::<crate::permissions::PermissionRule>::new()
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_commits_download_batch_atomically_off_thread() {
    let path = temp_path("download");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .create_download(
            "download-1",
            "https://example.test/file",
            "/tmp/file",
            DownloadState::Offered,
            1,
        )
        .expect("download");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .request_download_batch(vec![DownloadUpdate {
            id: "download-1".into(),
            state: DownloadState::Completed,
            bytes_received: 42,
            completed_at: Some(2),
        }])
        .expect("download batch request");
    assert_eq!(
        worker.wait_download_batch(Duration::from_secs(1)),
        Some(Ok(1))
    );
    drop(worker);

    let store = ProfileStore::open(&path, StoreMode::Normal).expect("reopen");
    let download = store
        .download("download-1")
        .expect("download row")
        .expect("download exists");
    assert_eq!(download.state, DownloadState::Completed);
    assert_eq!(download.bytes_received, 42);
    assert_eq!(download.completed_at, Some(2));
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_commits_download_destination_before_acknowledging() {
    let path = temp_path("download-destination");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .create_download(
            "download-1",
            "https://example.test/file",
            "",
            DownloadState::Offered,
            1,
        )
        .expect("download");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .request_download_destination("download-1".into(), "/tmp/file".into())
        .expect("destination request");
    assert_eq!(
        worker.wait_download_destination(Duration::from_secs(1)),
        Some(Ok(()))
    );
    drop(worker);

    let store = ProfileStore::open(&path, StoreMode::Normal).expect("reopen");
    assert_eq!(
        store
            .download("download-1")
            .expect("download row")
            .expect("download exists")
            .destination,
        "/tmp/file"
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_creates_download_index_before_acknowledging() {
    let path = temp_path("download-create");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .request_download_create(
            "download-1".into(),
            "https://example.test/file".into(),
            String::new(),
            DownloadState::Offered,
            1,
        )
        .expect("create request");
    assert_eq!(
        worker.wait_download_create(Duration::from_secs(1)),
        Some(Ok(()))
    );
    drop(worker);

    let store = ProfileStore::open(&path, StoreMode::Normal).expect("reopen");
    let download = store
        .download("download-1")
        .expect("download row")
        .expect("download exists");
    assert_eq!(download.source_url, "https://example.test/file");
    assert_eq!(download.state, DownloadState::Offered);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_download_batch_rolls_back_when_one_download_is_missing() {
    let path = temp_path("download-rollback");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .create_download(
            "download-1",
            "https://example.test/file",
            "/tmp/file",
            DownloadState::Offered,
            1,
        )
        .expect("download");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .request_download_batch(vec![
            DownloadUpdate {
                id: "download-1".into(),
                state: DownloadState::Completed,
                bytes_received: 42,
                completed_at: Some(2),
            },
            DownloadUpdate {
                id: "missing".into(),
                state: DownloadState::Cancelled,
                bytes_received: 0,
                completed_at: None,
            },
        ])
        .expect("download batch request");
    assert_eq!(
        worker.wait_download_batch(Duration::from_secs(1)),
        Some(Err("download ID was not found".into()))
    );
    drop(worker);

    let store = ProfileStore::open(&path, StoreMode::Normal).expect("reopen");
    let download = store
        .download("download-1")
        .expect("download row")
        .expect("download exists");
    assert_eq!(download.state, DownloadState::Offered);
    assert_eq!(download.bytes_received, 0);
    assert_eq!(download.completed_at, None);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_commits_mark_write_before_acknowledging() {
    let path = temp_path("mark");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .request_mark_write(MarkWrite::AddBookmark {
            id: "bookmark-1".into(),
            url: "https://example.test/bookmark".into(),
            title: "Example".into(),
            timestamp: 1,
        })
        .expect("mark request");
    assert_eq!(worker.wait_mark_write(Duration::from_secs(1)), Some(Ok(1)));
    worker
        .request_mark_write(MarkWrite::SetQuickmark {
            name: "work".into(),
            url: "https://work.example/".into(),
            timestamp: 1,
        })
        .expect("quickmark request");
    assert_eq!(worker.wait_mark_write(Duration::from_secs(1)), Some(Ok(1)));
    worker
        .request_mark_write(MarkWrite::EditBookmark {
            id: "bookmark-1".into(),
            title: "Edited example".into(),
            timestamp: 2,
        })
        .expect("bookmark edit request");
    assert_eq!(worker.wait_mark_write(Duration::from_secs(1)), Some(Ok(1)));
    worker
        .request_mark_write(MarkWrite::EditQuickmark {
            name: "work".into(),
            url: "https://edited.example/".into(),
            timestamp: 3,
        })
        .expect("quickmark edit request");
    assert_eq!(worker.wait_mark_write(Duration::from_secs(1)), Some(Ok(1)));
    drop(worker);

    let store = ProfileStore::open(&path, StoreMode::Normal).expect("reopen");
    assert_eq!(
        store.bookmarks(10).expect("bookmarks")[0].title,
        "Edited example"
    );
    assert_eq!(
        store
            .quickmark("work")
            .expect("quickmark")
            .expect("mark")
            .url,
        "https://edited.example/"
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_mark_batch_rolls_back_when_a_later_mutation_is_stale() {
    let path = temp_path("mark-batch-rollback");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .add_bookmark("bookmark-1", "https://example.test/", "Example", 1)
        .expect("bookmark");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .request_mark_writes(vec![
            MarkWrite::EditBookmark {
                id: "bookmark-1".into(),
                title: "Changed".into(),
                timestamp: 2,
            },
            MarkWrite::DeleteQuickmark {
                name: "missing".into(),
            },
        ])
        .expect("mark batch request");
    assert_eq!(
        worker.wait_mark_write(Duration::from_secs(1)),
        Some(Err("quickmark was not found".into()))
    );
    drop(worker);

    let store = ProfileStore::open(&path, StoreMode::Normal).expect("reopen");
    assert_eq!(store.bookmarks(10).expect("bookmarks")[0].title, "Example");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_clears_history_before_acknowledging() {
    let path = temp_path("history-clear");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .record_visit("https://example.test/one", "One", "navigate", 1)
        .expect("one visit");
    store
        .record_visit("https://example.test/two", "Two", "navigate", 2)
        .expect("two visits");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .request_history_clear(None, None)
        .expect("history clear request");
    assert_eq!(
        worker.wait_history_clear(Duration::from_secs(1)),
        Some(Ok(2))
    );
    drop(worker);

    let store = ProfileStore::open(&path, StoreMode::Normal).expect("reopen");
    assert_eq!(
        store.history(10).expect("history"),
        Vec::<HistoryRecord>::new()
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_commits_journey_node_and_current_pointer_before_acknowledging() {
    let path = temp_path("journey");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .request_journey_write(JourneyWrite::RecordNode {
            id: "journey-1".into(),
            profile_id: "profile-1".into(),
            tab_id: "tab-1".into(),
            url: "https://example.test/journey".into(),
            title: "Journey".into(),
            committed_at: 1,
            transition: "navigate".into(),
            source: None,
            retention_before: None,
            parent_id: None,
        })
        .expect("journey request");
    assert_eq!(
        worker.wait_journey_write(Duration::from_secs(1)),
        Some(Ok(true))
    );
    worker
        .request_journey_write(JourneyWrite::SetCurrentById {
            tab_id: "tab-1".into(),
            node_id: "journey-1".into(),
        })
        .expect("current pointer request");
    assert_eq!(
        worker.wait_journey_write(Duration::from_secs(1)),
        Some(Ok(true))
    );
    drop(worker);

    let store = ProfileStore::open(&path, StoreMode::Normal).expect("reopen");
    assert_eq!(store.journey_nodes(10).expect("nodes").len(), 1);
    assert_eq!(
        store.current_journey_node("tab-1").expect("current"),
        Some("journey-1".into())
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_reads_a_journey_node_off_thread() {
    let path = temp_path("journey-node-read");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .record_journey_node(
            "journey-lookup",
            "profile-1",
            "tab-1",
            "https://example.test/journey",
            "Journey",
            1,
            "navigate",
            None,
        )
        .expect("journey node");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .request_journey_node("journey-lookup".into())
        .expect("journey node request");
    let node = worker
        .wait_journey_node(Duration::from_secs(1))
        .expect("journey node response")
        .expect("journey node read")
        .expect("journey node exists");
    assert_eq!(node.id, "journey-lookup");
    assert_eq!(node.url, "https://example.test/journey");
    drop(worker);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_reads_a_bounded_journey_query_off_thread() {
    let path = temp_path("journey-query");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .record_journey_node(
            "journey-query",
            "profile-1",
            "tab-1",
            "https://example.test/query",
            "Query",
            1,
            "navigate",
            None,
        )
        .expect("journey node");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .request_journey_query(None, Some("example.test".into()), None)
        .expect("journey query request");
    let snapshot = worker
        .wait_journey_query(Duration::from_secs(1))
        .expect("journey query response")
        .expect("journey query read");
    assert_eq!(snapshot.nodes.len(), 1);
    assert_eq!(snapshot.nodes[0].id, "journey-query");
    assert_eq!(snapshot.edges, Vec::<crate::JourneyEdgeRecord>::new());
    drop(worker);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn worker_reads_a_bounded_journey_export_off_thread() {
    let path = temp_path("journey-export");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .record_journey_node(
            "journey-export",
            "profile-1",
            "tab-1",
            "https://example.test/export",
            "Export",
            1,
            "navigate",
            None,
        )
        .expect("journey node");
    drop(store);

    let mut worker = ProfileStoreWorker::spawn(&path).expect("worker");
    worker
        .request_journey_export()
        .expect("journey export request");
    let snapshot = worker
        .wait_journey_export(Duration::from_secs(1))
        .expect("journey export response")
        .expect("journey export read");
    assert_eq!(snapshot.nodes.len(), 1);
    assert_eq!(snapshot.nodes[0].id, "journey-export");
    drop(worker);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn invalid_payloads_are_rejected_before_admission_and_do_not_occupy_the_worker() {
    let path = temp_path("invalid-admission");
    let mut worker = ProfileStoreWorker::spawn(&path).unwrap();
    let invalid = StorageCommand::HistoryBatch {
        visits: vec![VisitInput {
            url: "https://example.test/".into(),
            title: "x".repeat(4097),
            transition: "navigate".into(),
            timestamp: 1,
            retention_before: None,
        }],
    };
    assert_eq!(
        worker.try_submit(invalid),
        Err(StorageWorkerError::InvalidBatch)
    );
    assert!(worker.poll_events().is_empty());
    worker
        .try_submit(StorageCommand::HistoryBatch {
            visits: vec![VisitInput {
                url: "https://example.test/".into(),
                title: "valid".into(),
                transition: "navigate".into(),
                timestamp: 1,
                retention_before: None,
            }],
        })
        .unwrap();
    drop(worker);
    let store = ProfileStore::open(&path, StoreMode::Normal).unwrap();
    assert_eq!(store.history(10).unwrap()[0].title, "valid");
    drop(store);
    let _ = std::fs::remove_file(path);
}
