//! Schema-4 versioning index. The profile metadata worker owns this connection.

use rusqlite::Connection;

pub(crate) const VERSION: i64 = 4;
pub(crate) const CHECKSUM: &str = "versioning-index-schema-4";

pub(crate) fn is_present(connection: &Connection) -> Result<bool, rusqlite::Error> {
    for table in [
        "vb_commits",
        "vb_objects",
        "vb_commit_objects",
        "vb_refs",
        "vb_journal",
        "vb_operations",
    ] {
        let count: i64 = connection.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |row| row.get(0),
        )?;
        if count != 1 {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Runs inside the caller's immediate migration transaction.
pub(crate) fn install(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "CREATE TABLE vb_commits (
             id TEXT PRIMARY KEY NOT NULL,
             profile_uuid TEXT NOT NULL,
             parent_id TEXT,
             created_at INTEGER NOT NULL,
             message TEXT NOT NULL CHECK (length(message) <= 1024),
             manifest_schema INTEGER NOT NULL CHECK (manifest_schema = 1),
             manifest_bytes BLOB NOT NULL CHECK (length(manifest_bytes) BETWEEN 1 AND 8388608),
             manifest_hash TEXT NOT NULL CHECK (length(manifest_hash) = 64),
             pinned INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
             UNIQUE (profile_uuid, id),
             FOREIGN KEY (profile_uuid, parent_id) REFERENCES vb_commits(profile_uuid, id) ON DELETE RESTRICT
         );
         CREATE INDEX vb_commits_profile_time ON vb_commits(profile_uuid, created_at, id);
         CREATE TABLE vb_objects (
             hash TEXT PRIMARY KEY NOT NULL CHECK (length(hash) = 64),
             kind TEXT NOT NULL CHECK (length(kind) BETWEEN 1 AND 32),
             bytes INTEGER NOT NULL CHECK (bytes >= 0),
             state TEXT NOT NULL CHECK (state IN ('ready', 'missing', 'quarantined')),
             created_at INTEGER NOT NULL
         );
         CREATE TABLE vb_commit_objects (
             commit_id TEXT NOT NULL REFERENCES vb_commits(id) ON DELETE CASCADE,
             object_hash TEXT NOT NULL REFERENCES vb_objects(hash) ON DELETE RESTRICT,
             role TEXT NOT NULL CHECK (length(role) BETWEEN 1 AND 32),
             logical_page_id TEXT CHECK (logical_page_id IS NULL OR length(logical_page_id) BETWEEN 1 AND 64)
         );
         CREATE UNIQUE INDEX vb_commit_objects_unique
             ON vb_commit_objects(commit_id, object_hash, role, coalesce(logical_page_id, ''));
         CREATE INDEX vb_commit_objects_hash ON vb_commit_objects(object_hash);
         CREATE TABLE vb_refs (
             id TEXT PRIMARY KEY NOT NULL,
             profile_uuid TEXT NOT NULL,
             kind TEXT NOT NULL CHECK (length(kind) BETWEEN 1 AND 32),
             name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 128),
             commit_id TEXT,
             updated_at INTEGER NOT NULL,
             FOREIGN KEY (profile_uuid, commit_id) REFERENCES vb_commits(profile_uuid, id) ON DELETE RESTRICT,
             UNIQUE (profile_uuid, kind, name)
         );
         CREATE TABLE vb_operations (
             id TEXT PRIMARY KEY NOT NULL,
             profile_uuid TEXT NOT NULL,
             kind TEXT NOT NULL CHECK (length(kind) BETWEEN 1 AND 32),
             state TEXT NOT NULL CHECK (length(state) BETWEEN 1 AND 32),
             safe_recovery_metadata TEXT NOT NULL CHECK (length(safe_recovery_metadata) <= 4096),
             UNIQUE (profile_uuid, id)
         );
         CREATE TABLE vb_journal (
             sequence INTEGER PRIMARY KEY,
             profile_uuid TEXT NOT NULL,
             time INTEGER NOT NULL,
             operation_id TEXT,
             kind TEXT NOT NULL CHECK (length(kind) BETWEEN 1 AND 32),
             safe_summary TEXT NOT NULL CHECK (length(safe_summary) <= 1024),
             FOREIGN KEY (profile_uuid, operation_id) REFERENCES vb_operations(profile_uuid, id) ON DELETE RESTRICT
         );
         CREATE INDEX vb_journal_profile_time ON vb_journal(profile_uuid, time, sequence);
         INSERT INTO schema_migrations(version, checksum, applied_at)
             VALUES (4, 'versioning-index-schema-4', strftime('%s','now'));",
    )
}
