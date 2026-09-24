use super::*;

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ferric-browser-storage-{label}-{}-{}.sqlite",
        std::process::id(),
        unix_timestamp()
    ))
}

#[test]
fn migration_configures_durable_profile_storage() {
    let path = temp_path("migration");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    assert_eq!(store.schema_version(), 3);
    let foreign_keys: i64 = store
        .connection
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .expect("pragma");
    let synchronous: i64 = store
        .connection
        .query_row("PRAGMA synchronous", [], |row| row.get(0))
        .expect("pragma");
    assert_eq!(foreign_keys, 1);
    assert_eq!(synchronous, 2);
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}-wal", store.path().display()));
    let _ = std::fs::remove_file(format!("{}-shm", store.path().display()));
}

#[test]
#[allow(clippy::too_many_lines)]
fn migration_exposes_the_specified_logical_schema() {
    use std::collections::BTreeSet;

    let path = temp_path("schema-contract");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    let expected = [
        (
            "schema_migrations",
            ["version", "checksum", "applied_at"].as_slice(),
        ),
        (
            "pages",
            [
                "id",
                "normalized_url",
                "safe_title",
                "first_visit",
                "last_visit",
                "visit_count",
            ]
            .as_slice(),
        ),
        (
            "visits",
            ["id", "page_id", "committed_at", "transition"].as_slice(),
        ),
        (
            "bookmarks",
            ["id", "url", "title", "created_at", "updated_at"].as_slice(),
        ),
        (
            "quickmarks",
            ["name", "url", "created_at", "updated_at"].as_slice(),
        ),
        (
            "command_history",
            ["id", "command", "created_at"].as_slice(),
        ),
        (
            "permission_rules",
            [
                "origin",
                "permission",
                "decision",
                "expires_at",
                "updated_at",
            ]
            .as_slice(),
        ),
        (
            "downloads",
            [
                "id",
                "source_url",
                "destination",
                "state",
                "bytes_received",
                "created_at",
                "completed_at",
            ]
            .as_slice(),
        ),
        (
            "site_preferences",
            ["origin", "preferences_json", "revision"].as_slice(),
        ),
        (
            "journey_nodes",
            [
                "id",
                "profile_id",
                "tab_id",
                "url",
                "title",
                "committed_at",
                "transition",
                "source",
            ]
            .as_slice(),
        ),
        (
            "journey_edges",
            ["source_id", "target_id", "transition", "created_at"].as_slice(),
        ),
        ("journey_current", ["tab_id", "node_id"].as_slice()),
    ];
    for (table, columns) in expected {
        let exists: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .expect("table lookup");
        assert_eq!(exists, 1, "missing logical table {table}");

        let mut statement = store
            .connection
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("table info");
        let actual = statement
            .query_map([], |row| row.get::<_, String>(1))
            .expect("column query")
            .collect::<Result<BTreeSet<_>, _>>()
            .expect("column names");
        let expected = columns.iter().map(|column| (*column).to_owned()).collect();
        assert_eq!(actual, expected, "schema columns for {table}");
    }
    drop(store);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn flush_checkpoints_committed_metadata_for_reopen() {
    let path = temp_path("flush");
    {
        let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
        store
            .record_visit("https://example.test/flush", "Flush", "navigate", 1)
            .expect("record visit");
        store.flush().expect("checkpoint");
        assert_eq!(
            store
                .connection
                .query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |row| row
                    .get::<_, i64>(0))
                .expect("checkpoint status"),
            0
        );
    }
    let reopened = ProfileStore::open(&path, StoreMode::Normal).expect("reopen");
    assert_eq!(
        reopened.history(10).expect("history")[0].url,
        "https://example.test/flush"
    );
    drop(reopened);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn corruption_is_reported_without_replacing_original_bytes() {
    let path = temp_path("corrupt");
    let original = b"this is not a sqlite database";
    std::fs::write(&path, original).expect("write corrupt fixture");
    let error = ProfileStore::open(&path, StoreMode::Normal).expect_err("corrupt open");
    assert!(matches!(error, StoreError::Corrupt(_)));
    assert_eq!(std::fs::read(&path).expect("read original"), original);
    let inspection = inspect_store(&path);
    assert_eq!(inspection.status, "corrupt");
    assert_eq!(inspection.integrity, "failed");
    assert!(inspection.recovery.contains("original"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn sqlite_full_is_reported_as_a_recoverable_disk_full_error() {
    let sqlite_error = rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error {
            code: rusqlite::ErrorCode::DiskFull,
            extended_code: 13,
        },
        Some("database or disk is full".into()),
    );
    let error = StoreError::from(sqlite_error);
    assert!(matches!(error, StoreError::DiskFull));
    assert!(error.to_string().contains("free disk space and retry once"));
}

#[test]
fn newer_schema_is_refused_without_migration_or_replacement() {
    let path = temp_path("newer-schema");
    let connection = Connection::open(&path).expect("create fixture");
    connection
        .execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_at INTEGER NOT NULL);
             INSERT INTO schema_migrations(version, checksum, applied_at) VALUES (99, 'future', 1);",
        )
        .expect("write future schema");
    drop(connection);
    let original = std::fs::read(&path).expect("read fixture");
    let error = ProfileStore::open(&path, StoreMode::Normal).expect_err("future schema");
    assert!(matches!(error, StoreError::UnsupportedSchema(99)));
    assert_eq!(std::fs::read(&path).expect("read original"), original);
    let inspection = inspect_store(&path);
    assert_eq!(inspection.status, "unsupported");
    assert_eq!(inspection.schema_version, Some(99));
    let _ = std::fs::remove_file(path);
}

#[test]
fn older_schema_is_refused_without_migration_or_replacement() {
    let path = temp_path("legacy-schema");
    let connection = Connection::open(&path).expect("create fixture");
    connection
        .execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_at INTEGER NOT NULL);
             INSERT INTO schema_migrations(version, checksum, applied_at) VALUES (0, 'legacy', 1);",
        )
        .expect("write legacy schema");
    drop(connection);

    let original = std::fs::read(&path).expect("read fixture");
    let error = ProfileStore::open(&path, StoreMode::Normal).expect_err("legacy schema");
    assert!(matches!(error, StoreError::LegacySchema(0)));
    assert_eq!(std::fs::read(&path).expect("read original"), original);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn migration_checksum_mismatch_is_refused_without_replacement() {
    let path = temp_path("migration-checksum");
    let connection = Connection::open(&path).expect("create fixture");
    connection
        .execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_at INTEGER NOT NULL);
             INSERT INTO schema_migrations(version, checksum, applied_at) VALUES (3, 'tampered', 1);",
        )
        .expect("write mismatched schema");
    drop(connection);
    let original = std::fs::read(&path).expect("read fixture");
    let error = ProfileStore::open(&path, StoreMode::Normal).expect_err("checksum refusal");
    assert!(matches!(error, StoreError::Corrupt(reason) if reason.contains("checksum")));
    assert_eq!(std::fs::read(&path).expect("read original"), original);
    let _ = std::fs::remove_file(path);
}

#[test]
fn missing_store_inspection_is_read_only_and_actionable() {
    let path = temp_path("missing-inspection");
    let inspection = inspect_store(&path);
    assert_eq!(inspection.status, "missing");
    assert!(!path.exists());
    assert!(inspection.recovery.contains("new named profile"));
}

#[test]
fn visits_deduplicate_pages_but_retain_visit_events() {
    let path = temp_path("history");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    let first = store
        .record_visit("https://example.test/", "Example", "typed", 1)
        .expect("visit");
    let second = store
        .record_visit("https://example.test/", "Example updated", "link", 2)
        .expect("visit");
    assert_eq!(first.id, second.id);
    assert_eq!(second.visit_count, 2);
    let visits: i64 = store
        .connection
        .query_row("SELECT COUNT(*) FROM visits", [], |row| row.get(0))
        .expect("count");
    assert_eq!(visits, 2);
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn history_retention_and_command_history_cap_are_transactional() {
    let path = temp_path("history-retention");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .record_visit("https://old.example/", "Old", "navigate", 1)
        .expect("old visit");
    let current = DEFAULT_HISTORY_RETENTION_SECONDS + 10;
    store
        .record_visit("https://new.example/", "New", "navigate", current)
        .expect("new visit");
    let history = store.history(10).expect("history");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].url, "https://new.example/");

    store
        .record_visit("https://custom-old.example/", "Custom old", "navigate", 100)
        .expect("custom old visit");
    store
        .record_visit_batch(&[VisitInput {
            url: "https://custom-new.example/".into(),
            title: "Custom new".into(),
            transition: "navigate".into(),
            timestamp: 200,
            retention_before: Some(150),
        }])
        .expect("custom retention batch");
    assert!(
        store
            .history(10)
            .expect("custom history")
            .iter()
            .all(|entry| entry.url != "https://custom-old.example/")
    );
    store
        .record_visit_with_retention(
            "https://sync-old.example/",
            "Sync old",
            "navigate",
            200,
            Some(250),
        )
        .expect("sync old visit");
    store
        .record_visit_with_retention(
            "https://sync-new.example/",
            "Sync new",
            "navigate",
            300,
            Some(250),
        )
        .expect("sync new visit");
    let sync_history = store.history(10).expect("sync history");
    assert!(
        sync_history
            .iter()
            .all(|entry| entry.url != "https://sync-old.example/")
    );
    assert!(
        sync_history
            .iter()
            .any(|entry| entry.url == "https://sync-new.example/")
    );

    for index in 0..=MAX_COMMAND_HISTORY_ROWS {
        store
            .record_command(&format!("open item-{index}"), index, true)
            .expect("command history");
    }
    let command_count: i64 = store
        .connection
        .query_row("SELECT COUNT(*) FROM command_history", [], |row| row.get(0))
        .expect("command count");
    assert_eq!(command_count, MAX_COMMAND_HISTORY_ROWS);
    let oldest_command: i64 = store
        .connection
        .query_row("SELECT MIN(id) FROM command_history", [], |row| row.get(0))
        .expect("oldest command");
    assert_eq!(oldest_command, 2);
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn journey_storage_links_current_nodes_and_keeps_safe_fields() {
    let path = temp_path("journey");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .record_journey_node(
            "journeynodeid-1",
            "profileid-1",
            "tabid-1",
            "https://example.test/one",
            "One Title",
            1,
            "navigate",
            Some("typed"),
        )
        .expect("first node");
    store
        .record_journey_node(
            "journeynodeid-2",
            "profileid-1",
            "tabid-1",
            "https://example.test/two",
            "Two",
            2,
            "branch-after-back",
            None,
        )
        .expect("second node");
    let nodes = store.journey_nodes(10).expect("nodes");
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0].id, "journeynodeid-2");
    assert_eq!(nodes[0].profile_id, "profileid-1");
    assert_eq!(nodes[1].title, "One Title");
    let edges = store.journey_edges(10).expect("edges");
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].source_id, "journeynodeid-1");
    assert_eq!(edges[0].target_id, "journeynodeid-2");
    assert_eq!(edges[0].transition, "branch-after-back");
    assert_eq!(
        store.current_journey_node("tabid-1").expect("current"),
        Some("journeynodeid-2".into())
    );
    assert!(
        store
            .set_current_journey_node_for_url("tabid-1", "https://example.test/one")
            .expect("move current")
    );
    assert_eq!(
        store.current_journey_node("tabid-1").expect("current one"),
        Some("journeynodeid-1".into())
    );
    assert!(
        store
            .set_current_journey_node_by_id("tabid-1", "journeynodeid-2")
            .expect("move current by id")
    );
    assert_eq!(
        store.current_journey_node("tabid-1").expect("current two"),
        Some("journeynodeid-2".into())
    );
    assert!(
        !store
            .set_current_journey_node_for_url("tabid-1", "https://missing.example/")
            .expect("missing current")
    );
    assert_eq!(store.clear_journey().expect("clear"), 2);
    assert!(store.journey_nodes(10).expect("empty nodes").is_empty());
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn journey_storage_rejects_secrets_and_private_mode() {
    let path = temp_path("journey-policy");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    let error = store
        .record_journey_node(
            "node",
            "profile",
            "tab",
            "https://example.test/?token=secret",
            "title",
            1,
            "navigate",
            None,
        )
        .expect_err("secret URL");
    assert!(matches!(error, StoreError::InvalidInput(_)));
    assert!(matches!(
        ProfileStore::open(&path, StoreMode::Private),
        Err(StoreError::PrivateNoDurableState)
    ));
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn journey_storage_links_a_cross_tab_parent() {
    let path = temp_path("journey-parent");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .record_journey_node(
            "opener-node",
            "profileid-1",
            "opener-tab",
            "https://example.test/opener",
            "Opener",
            1,
            "navigate",
            None,
        )
        .expect("opener");
    store
        .record_journey_node_with_retention_and_parent(
            "popup-node",
            "profileid-1",
            "popup-tab",
            "https://example.test/popup",
            "Popup",
            2,
            "popup",
            Some("popup"),
            None,
            Some("opener-node"),
        )
        .expect("popup");
    let edges = store.journey_edges(10).expect("edges");
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].source_id, "opener-node");
    assert_eq!(edges[0].target_id, "popup-node");
    assert_eq!(edges[0].transition, "popup");
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn journey_search_and_neighborhood_queries_are_bounded() {
    let path = temp_path("journey-view");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    for (id, url, title, time) in [
        (
            "journey-view-1",
            "https://example.test/one",
            "Example One",
            1,
        ),
        (
            "journey-view-2",
            "https://example.test/two",
            "Example Two",
            2,
        ),
        (
            "journey-view-3",
            "https://other.test/three",
            "Other Three",
            3,
        ),
    ] {
        store
            .record_journey_node(
                id,
                "profile-view",
                "tab-view",
                url,
                title,
                time,
                "navigate",
                Some("typed"),
            )
            .expect("journey node");
    }
    let matches = store.journey_nodes_search("EXAMPLE", 1).expect("search");
    assert_eq!(matches.len(), 1);
    assert!(matches[0].title.starts_with("Example"));
    let neighbors = store
        .journey_neighbors("journey-view-2", 10)
        .expect("neighbors");
    assert_eq!(neighbors.len(), 2);
    assert!(neighbors.iter().any(|node| node.id == "journey-view-1"));
    assert!(neighbors.iter().any(|node| node.id == "journey-view-3"));
    let edges = store
        .journey_edges_for_nodes(&["journey-view-1".into(), "journey-view-2".into()], 10)
        .expect("visible edges");
    assert_eq!(edges.len(), 1);
    assert!(
        store
            .journey_nodes_search("%", 10)
            .expect("literal search")
            .is_empty()
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn journey_retention_cutoff_removes_old_nodes_in_insert_transaction() {
    let path = temp_path("journey-retention");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .record_journey_node_with_retention(
            "old-node",
            "profile",
            "old-tab",
            "https://old.example/",
            "Old",
            1,
            "navigate",
            None,
            None,
        )
        .expect("old node");
    store
        .record_journey_node_with_retention(
            "new-node",
            "profile",
            "old-tab",
            "https://new.example/",
            "New",
            100,
            "navigate",
            None,
            Some(50),
        )
        .expect("new node");
    let nodes = store.journey_nodes(10).expect("nodes");
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].id, "new-node");
    assert!(
        store
            .current_journey_node("old-tab")
            .expect("old current")
            .is_some()
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn journey_retention_preserves_active_current_nodes() {
    let path = temp_path("journey-retention-current");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .record_journey_node_with_retention(
            "active-old-node",
            "profile",
            "active-tab",
            "https://active.example/",
            "Active",
            1,
            "navigate",
            None,
            None,
        )
        .expect("active node");
    store
        .record_journey_node_with_retention(
            "retention-trigger",
            "profile",
            "other-tab",
            "https://trigger.example/",
            "Trigger",
            100,
            "navigate",
            None,
            Some(50),
        )
        .expect("retention trigger");

    let nodes = store.journey_nodes(10).expect("nodes");
    assert_eq!(nodes.len(), 2);
    assert_eq!(
        store
            .current_journey_node("active-tab")
            .expect("active current"),
        Some("active-old-node".into())
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn private_mode_never_opens_a_database_and_sensitive_commands_are_skipped() {
    let path = temp_path("private");
    assert!(matches!(
        ProfileStore::open(&path, StoreMode::Private),
        Err(StoreError::PrivateNoDurableState)
    ));
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    assert!(
        !store
            .record_command(":set password=secret", 1, false)
            .expect("skip")
    );
    let count: i64 = store
        .connection
        .query_row("SELECT COUNT(*) FROM command_history", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 0);
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn quickmarks_and_bookmarks_round_trip() {
    let path = temp_path("marks");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .set_quickmark("w", "https://work.test/", 1)
        .expect("quickmark");
    assert_eq!(
        store.quickmark("w").expect("query").expect("mark").url,
        "https://work.test/"
    );
    store
        .add_bookmark("bookmark-1", "https://example.test/", "Example", 1)
        .expect("bookmark");
    store
        .apply_mark_write(&MarkWrite::EditBookmark {
            id: "bookmark-1".into(),
            title: "Edited example".into(),
            timestamp: 2,
        })
        .expect("edit bookmark");
    store
        .apply_mark_write(&MarkWrite::EditQuickmark {
            name: "w".into(),
            url: "https://edited.example/".into(),
            timestamp: 2,
        })
        .expect("edit quickmark");
    assert_eq!(
        store
            .quickmark("w")
            .expect("edited quickmark")
            .expect("mark")
            .url,
        "https://edited.example/"
    );
    assert_eq!(
        store.bookmarks(10).expect("edited bookmarks")[0].title,
        "Edited example"
    );
    assert_eq!(store.quickmarks(10).expect("quickmarks").len(), 1);
    assert_eq!(store.bookmarks(10).expect("bookmarks").len(), 1);
    let bookmarks: i64 = store
        .connection
        .query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get(0))
        .expect("count");
    assert_eq!(bookmarks, 1);
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn mark_write_batch_rolls_back_when_a_later_mutation_is_stale() {
    let path = temp_path("mark-batch-rollback");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .add_bookmark("bookmark-1", "https://example.test/", "Example", 1)
        .expect("bookmark");

    let error = store
        .apply_mark_write_batch(&[
            MarkWrite::EditBookmark {
                id: "bookmark-1".into(),
                title: "Changed".into(),
                timestamp: 2,
            },
            MarkWrite::DeleteQuickmark {
                name: "missing".into(),
            },
        ])
        .expect_err("stale later mutation");
    assert!(matches!(error, StoreError::InvalidInput(_)));
    assert_eq!(store.bookmarks(10).expect("bookmarks")[0].title, "Example");
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn history_listing_is_newest_first_and_bounded() {
    let path = temp_path("history-list");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .record_visit("https://old.example/", "Old", "typed", 1)
        .expect("old visit");
    store
        .record_visit("https://new.example/", "New", "typed", 2)
        .expect("new visit");
    let history = store.history(1).expect("history");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].url, "https://new.example/");
    assert_eq!(history[0].last_visit, 2);
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn profile_marks_delete_exactly_and_history_clear_is_bounded() {
    let path = temp_path("clear-history");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .set_quickmark("work", "https://work.example/", 1)
        .expect("quickmark");
    assert!(store.delete_quickmark("work").expect("delete quickmark"));
    assert!(!store.delete_quickmark("work").expect("missing quickmark"));
    store
        .add_bookmark("one", "https://one.example/", "One", 1)
        .expect("bookmark");
    assert!(store.delete_bookmark("one").expect("delete bookmark"));
    assert!(!store.delete_bookmark("one").expect("missing bookmark"));
    store
        .record_visit("https://one.example/path", "One", "typed", 1)
        .expect("one history");
    store
        .record_visit("https://two.example/path", "Two", "typed", 2)
        .expect("two history");
    store
        .record_visit("https://one.example.evil/path", "Lookalike", "typed", 3)
        .expect("lookalike history");
    store
        .record_journey_node(
            "journey-one",
            "profile",
            "tab-one",
            "https://one.example/path",
            "One",
            1,
            "navigate",
            None,
        )
        .expect("one journey");
    store
        .record_journey_node(
            "journey-two",
            "profile",
            "tab-two",
            "https://two.example/path",
            "Two",
            2,
            "navigate",
            None,
        )
        .expect("two journey");
    assert_eq!(
        store
            .clear_history(None, Some("https://one.example"))
            .expect("clear"),
        1
    );
    let history = store.history(10).expect("history");
    assert_eq!(history.len(), 2);
    assert!(
        history
            .iter()
            .any(|record| record.url == "https://one.example.evil/path")
    );
    let journey = store.journey_nodes(10).expect("journey");
    assert_eq!(journey.len(), 1);
    assert_eq!(journey[0].url, "https://two.example/path");
    assert!(matches!(
        store.clear_history(None, Some("https://one.example%")),
        Err(StoreError::InvalidInput(message)) if message.contains("origin")
    ));
    assert!(matches!(
        store.clear_history(Some(-1), None),
        Err(StoreError::InvalidInput(message)) if message.contains("timestamp")
    ));
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn history_rejects_credentials_fragments_and_secret_query_keys() {
    assert!(is_safe_history_url("https://example.test/path?a=1"));
    assert!(!is_safe_history_url("https://example.test/path\n"));
    assert!(!is_safe_history_url(&format!(
        "https://example.test/{}",
        "x".repeat(16 * 1024)
    )));
    assert!(!is_safe_history_url("https://user:pass@example.test/"));
    assert!(!is_safe_history_url(
        "https://example.test/callback?code=abc"
    ));
    for key in [
        "access_token",
        "refresh-token",
        "client_secret",
        "credential",
        "jwt",
        "%61ccess_token",
        "access%5Ftoken",
    ] {
        assert!(
            !is_safe_history_url(&format!("https://example.test/callback?{key}=abc")),
            "secret query key was accepted: {key}"
        );
    }
    assert!(!is_safe_history_url(
        "https://example.test/callback?access%ZZtoken=abc"
    ));
    assert!(!is_safe_history_url(
        "https://example.test/#access_token=abc"
    ));
    let path = temp_path("unsafe");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    assert!(matches!(
        store.record_visit("https://example.test/?token=abc", "", "typed", 1),
        Err(StoreError::UnsafeHistoryUrl)
    ));
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn durable_metadata_rejects_untrusted_text_overflow_and_controls() {
    let path = temp_path("metadata-boundary");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    let oversized_title = "x".repeat(4 * 1024 + 1);
    assert!(matches!(
        store.record_visit("https://example.test/", &oversized_title, "typed", 1),
        Err(StoreError::InvalidInput(_))
    ));
    assert!(matches!(
        store.add_bookmark("bookmark", "https://example.test/", "bad\n", 1),
        Err(StoreError::InvalidInput(_))
    ));
    assert!(matches!(
        store.set_quickmark("bad\n", "https://example.test/", 1),
        Err(StoreError::InvalidInput(_))
    ));
    assert!(matches!(
        store.create_download(
            "download",
            &format!("https://example.test/{}", "x".repeat(16 * 1024)),
            "",
            DownloadState::Offered,
            1
        ),
        Err(StoreError::InvalidInput(_))
    ));
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn downloads_round_trip_lifecycle_and_order() {
    let path = temp_path("downloads");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .create_download(
            "download-1",
            "https://example.test/file.pdf",
            "/tmp/file.pdf",
            DownloadState::SelectingDestination,
            1,
        )
        .expect("create");
    store
        .update_download("download-1", DownloadState::InProgress, 12, None)
        .expect("progress");
    store
        .update_download("download-1", DownloadState::Completed, 42, Some(3))
        .expect("complete");
    let record = store.download("download-1").expect("read").expect("record");
    assert_eq!(record.state, DownloadState::Completed);
    assert_eq!(record.bytes_received, 42);
    assert_eq!(record.completed_at, Some(3));
    assert_eq!(store.downloads().expect("list").len(), 1);
    assert!(store.download("missing").expect("missing").is_none());
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn permission_rules_are_exact_origin_bounded_and_resettable() {
    assert_eq!(
        normalize_permission_origin("HTTPS://Example.test:443").expect("origin"),
        "https://example.test"
    );
    assert_eq!(
        normalize_permission_origin("http://localhost:80").expect("origin"),
        "http://localhost"
    );
    assert_eq!(
        normalize_permission_origin("https://[::1]:443").expect("IPv6 origin"),
        "https://[::1]"
    );
    assert_eq!(
        normalize_permission_origin("https://[2001:db8::1]:8443").expect("IPv6 origin"),
        "https://[2001:db8::1]:8443"
    );
    assert!(normalize_permission_origin("http://example.test").is_err());
    assert!(normalize_permission_origin("https://example.test/path").is_err());

    let path = temp_path("permissions");
    let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
    store
        .set_permission_rule(
            "HTTPS://Example.test:443",
            "notifications",
            "allow",
            None,
            1,
        )
        .expect("set rule");
    let rules = store
        .permission_rules(Some("https://example.test"))
        .expect("list rules");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].origin, "https://example.test");
    assert_eq!(rules[0].decision, "allow");
    assert!(
        store
            .reset_permission_rule("https://example.test:443", "notifications")
            .expect("reset rule")
    );
    assert!(
        store
            .permission_rules(None)
            .expect("list after reset")
            .is_empty()
    );

    store
        .set_permission_rule("https://example.test", "notifications", "allow", None, 2)
        .expect("restore rule");
    let error = store
        .reset_permission_rule_batch(
            "https://example.test",
            &["notifications".into(), "unsupported".into()],
        )
        .expect_err("invalid reset batch");
    assert!(matches!(error, StoreError::InvalidInput(_)));
    assert_eq!(
        store
            .permission_rules(None)
            .expect("list after rejected batch")
            .len(),
        1
    );
    drop(store);
    let _ = std::fs::remove_file(path);
}
