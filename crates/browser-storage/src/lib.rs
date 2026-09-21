//! Profile-local durable metadata storage.
//!
//! `QtWebEngine` owns cookies, cache, service workers, and its native browsing
//! data. This crate owns only `RustBrowser` metadata and refuses durable opening
//! for private sessions.

#![allow(clippy::missing_errors_doc, clippy::too_many_arguments)]

mod contexts;
mod downloads;
mod durable;
mod lifecycle;
mod permissions;
mod profiles;
mod roots;
mod sessions;
mod worker;

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OpenFlags, OptionalExtension, params, params_from_iter};
use serde::Serialize;

pub use contexts::{
    ContextMember, ContextRecord, ContextRegistry, ContextRegistryError, ContextTabDescriptor,
};
pub use downloads::{
    CollisionPolicy, DownloadPathError, choose_download_path, create_download_file,
    sanitize_download_filename,
};
pub use lifecycle::{CrashMarker, CrashMarkerError, crash_diagnostics, transient_marker_scan};
pub use permissions::{PermissionRule, normalize_permission_origin};
pub use profiles::{
    ProfileDataError, ProfileDeletionError, ProfileDeletionOutcome, ProfileLock, ProfileLockError,
    ProfilePrivacy, ProfileRecord, ProfileRegistry, RegistryError, delete_profile_data,
    delete_profile_transaction,
};
pub use roots::{RootSpec, StorageRoots, StorageRootsError};
pub use sessions::{
    ClosedTabSnapshot, RestoreMethod, RestorePlanEntry, RestoreSafety, SessionError,
    SessionSnapshot, SessionTab, SnapshotTabInput, SnapshotWindowInput,
    clear_current_session_checkpoints, current_session_paths, delete_named_session,
    list_named_sessions, load_session, named_session_path, save_session_atomic,
};
pub use worker::JourneyQuerySnapshot;
pub use worker::{
    ProfileLibrarySnapshot, ProfileStoreWorker, SessionRestoreSnapshot, StorageWorkerError,
};

const SCHEMA_VERSION: i64 = 1;
const SCHEMA_MIGRATION_CHECKSUM: &str = "builtin-schema-1";
const MAX_PRE_MIGRATION_BACKUP_BYTES: u64 = 512 * 1024 * 1024;
const DEFAULT_HISTORY_RETENTION_SECONDS: i64 = 90 * 24 * 60 * 60;
const MAX_COMMAND_HISTORY_ROWS: i64 = 1_000;
const MAX_STORED_TITLE_BYTES: usize = 4 * 1024;
const MAX_STORED_IDENTIFIER_BYTES: usize = 256;

fn valid_stored_text(value: &str, limit: usize) -> bool {
    !value.is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}

fn valid_journey_transition(value: &str) -> bool {
    matches!(
        value,
        "navigate"
            | "redirect"
            | "opener"
            | "popup"
            | "hint"
            | "session-restore"
            | "reopen"
            | "branch-after-back"
    )
}

fn bounded_stored_text(value: &str, limit: usize) -> bool {
    value.len() <= limit && !value.chars().any(char::is_control)
}

fn valid_history_origin(origin: &str) -> bool {
    if origin.is_empty()
        || origin.len() > 512
        || origin.chars().any(char::is_control)
        || origin.contains(['%', '_', '\\'])
    {
        return false;
    }
    let Some((scheme, authority)) = origin.split_once("://") else {
        return false;
    };
    matches!(scheme, "http" | "https")
        && !authority.is_empty()
        && !authority.contains(['/', '?', '#', '@'])
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreMode {
    Normal,
    Private,
}

#[derive(Debug)]
pub enum StoreError {
    PrivateNoDurableState,
    Corrupt(String),
    UnsupportedSchema(i64),
    DiskFull,
    CheckpointBusy,
    MigrationBackup(String),
    Sql(rusqlite::Error),
    InvalidInput(&'static str),
    UnsafeHistoryUrl,
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrivateNoDurableState => {
                formatter.write_str("private sessions cannot open durable metadata")
            }
            Self::Corrupt(reason) => write!(
                formatter,
                "profile database integrity check failed; original data was preserved; use read-only inspection or create a new named profile ({reason})"
            ),
            Self::UnsupportedSchema(version) => write!(
                formatter,
                "profile database schema version {version} is newer than this application; original data was preserved; use a newer RustBrowser or create a new named profile"
            ),
            Self::DiskFull => formatter.write_str(
                "profile storage is full; no replacement was written; free disk space and retry once",
            ),
            Self::CheckpointBusy => formatter.write_str(
                "profile storage could not be checkpointed because another connection is active",
            ),
            Self::MigrationBackup(reason) => write!(
                formatter,
                "profile storage migration backup could not be retained; original data was preserved ({reason})"
            ),
            Self::Sql(error) => write!(formatter, "SQLite error: {error}"),
            Self::InvalidInput(message) => formatter.write_str(message),
            Self::UnsafeHistoryUrl => {
                formatter.write_str("history URL contains credentials or an obvious secret")
            }
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        match &error {
            rusqlite::Error::SqliteFailure(code, _)
                if code.code == rusqlite::ErrorCode::DiskFull =>
            {
                Self::DiskFull
            }
            _ => Self::Sql(error),
        }
    }
}

/// Read-only health information for one Rust-owned profile database.
///
/// This probe never creates, migrates, repairs, or replaces the target file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StoreInspection {
    pub status: String,
    pub schema_version: Option<i64>,
    pub integrity: String,
    pub reason: String,
    pub recovery: String,
}

#[derive(Debug)]
pub struct ProfileStore {
    path: PathBuf,
    connection: Connection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageRecord {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub visit_count: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryRecord {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub visit_count: i64,
    pub last_visit: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisitInput {
    pub url: String,
    pub title: String,
    pub transition: String,
    pub timestamp: i64,
    pub retention_before: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JourneyNodeRecord {
    pub id: String,
    pub profile_id: String,
    pub tab_id: String,
    pub url: String,
    pub title: String,
    pub committed_at: i64,
    pub transition: String,
    pub source: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JourneyEdgeRecord {
    pub source_id: String,
    pub target_id: String,
    pub transition: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BookmarkRecord {
    pub id: String,
    pub url: String,
    pub title: String,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Quickmark {
    pub name: String,
    pub url: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DownloadState {
    Offered,
    SelectingDestination,
    InProgress,
    Paused,
    Completed,
    Interrupted,
    Cancelled,
}

impl DownloadState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Offered => "offered",
            Self::SelectingDestination => "selecting-destination",
            Self::InProgress => "in-progress",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Interrupted => "interrupted",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "offered" => Self::Offered,
            "selecting-destination" => Self::SelectingDestination,
            "in-progress" => Self::InProgress,
            "paused" => Self::Paused,
            "completed" => Self::Completed,
            "interrupted" => Self::Interrupted,
            "cancelled" => Self::Cancelled,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadRecord {
    pub id: String,
    pub source_url: String,
    pub destination: String,
    pub state: DownloadState,
    pub bytes_received: i64,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

/// One durable download lifecycle update applied by the metadata worker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadUpdate {
    pub id: String,
    pub state: DownloadState,
    pub bytes_received: i64,
    pub completed_at: Option<i64>,
}

/// One durable bookmark or quickmark mutation owned by the storage worker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarkWrite {
    AddBookmark {
        id: String,
        url: String,
        title: String,
        timestamp: i64,
    },
    DeleteBookmark {
        id: String,
    },
    EditBookmark {
        id: String,
        title: String,
        timestamp: i64,
    },
    SetQuickmark {
        name: String,
        url: String,
        timestamp: i64,
    },
    DeleteQuickmark {
        name: String,
    },
    EditQuickmark {
        name: String,
        url: String,
        timestamp: i64,
    },
}

/// One durable journey mutation owned by the storage worker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JourneyWrite {
    RecordNode {
        id: String,
        profile_id: String,
        tab_id: String,
        url: String,
        title: String,
        committed_at: i64,
        transition: String,
        source: Option<String>,
        retention_before: Option<i64>,
        parent_id: Option<String>,
    },
    SetCurrentById {
        tab_id: String,
        node_id: String,
    },
    SetCurrentForUrl {
        tab_id: String,
        url: String,
    },
}

impl ProfileStore {
    /// Opens or creates a normal profile database and applies migrations.
    ///
    /// # Errors
    ///
    /// Returns an error if the mode is private, SQLite cannot be configured,
    /// or a migration fails.
    pub fn open(path: impl AsRef<Path>, mode: StoreMode) -> Result<Self, StoreError> {
        if mode == StoreMode::Private {
            return Err(StoreError::PrivateNoDurableState);
        }
        let path = path.as_ref().to_owned();
        let existing_version =
            if fs::metadata(&path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0) {
                inspect_existing_database(&path)?
            } else {
                None
            };
        if let Some(version) = existing_version.filter(|version| *version < SCHEMA_VERSION) {
            create_pre_migration_backup(&path, version)?;
        }
        let connection = Connection::open(&path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL;",
        )?;
        migrate(&connection)?;
        Ok(Self { path, connection })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Flushes committed metadata from SQLite's WAL into the profile database.
    ///
    /// This is used at an orderly application shutdown boundary after the
    /// final metadata transaction has committed. A busy checkpoint is treated
    /// as a failure so callers do not report durable shutdown completion while
    /// another connection may still hold the WAL open.
    ///
    /// # Errors
    ///
    /// Returns an error if SQLite cannot run the checkpoint or reports that
    /// another connection still has the WAL busy.
    pub fn flush(&self) -> Result<(), StoreError> {
        let mut statement = self.connection.prepare("PRAGMA wal_checkpoint(FULL)")?;
        let mut rows = statement.query([])?;
        let Some(row) = rows.next()? else {
            return Err(StoreError::InvalidInput(
                "profile storage checkpoint returned no result",
            ));
        };
        let busy: i64 = row.get(0)?;
        if busy != 0 {
            return Err(StoreError::CheckpointBusy);
        }
        Ok(())
    }

    /// Records one committed safe navigation and deduplicates its page row.
    /// The caller must pass the safe URL selected by the navigation policy.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL/title is empty or the transaction fails.
    pub fn record_visit(
        &self,
        url: &str,
        title: &str,
        transition: &str,
        timestamp: i64,
    ) -> Result<PageRecord, StoreError> {
        self.record_visit_with_retention(url, title, transition, timestamp, None)
    }

    /// Records one committed visit while applying an explicit retention
    /// cutoff in the same transaction. `None` uses the 90-day default.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL/title is unsafe or the transaction fails.
    pub fn record_visit_with_retention(
        &self,
        url: &str,
        title: &str,
        transition: &str,
        timestamp: i64,
        retention_before: Option<i64>,
    ) -> Result<PageRecord, StoreError> {
        if url.is_empty() {
            return Err(StoreError::InvalidInput("history URL cannot be empty"));
        }
        if !is_safe_history_url(url) {
            return Err(StoreError::UnsafeHistoryUrl);
        }
        if !bounded_stored_text(title, MAX_STORED_TITLE_BYTES) {
            return Err(StoreError::InvalidInput(
                "history title is too large or contains a control character",
            ));
        }
        if !valid_stored_text(transition, 64) {
            return Err(StoreError::InvalidInput(
                "history transition is empty, too large, or contains a control character",
            ));
        }
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO pages (normalized_url, safe_title, first_visit, last_visit, visit_count)
             VALUES (?1, ?2, ?3, ?3, 1)
             ON CONFLICT(normalized_url) DO UPDATE SET safe_title = excluded.safe_title,
             last_visit = excluded.last_visit, visit_count = pages.visit_count + 1",
            params![url, title, timestamp],
        )?;
        let id: i64 = transaction.query_row(
            "SELECT id FROM pages WHERE normalized_url = ?1",
            [url],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO visits (page_id, committed_at, transition) VALUES (?1, ?2, ?3)",
            params![id, timestamp, transition],
        )?;
        let record = transaction.query_row(
            "SELECT id, normalized_url, safe_title, visit_count FROM pages WHERE id = ?1",
            [id],
            |row| {
                Ok(PageRecord {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    visit_count: row.get(3)?,
                })
            },
        )?;
        prune_history(
            &transaction,
            retention_before
                .unwrap_or_else(|| timestamp.saturating_sub(DEFAULT_HISTORY_RETENTION_SECONDS)),
        )?;
        transaction.commit()?;
        Ok(record)
    }

    /// Commits a bounded batch of safe navigations atomically.
    ///
    /// This is the worker-facing high-volume history path. The whole batch is
    /// rolled back if one input or SQL operation fails, so a caller can retry
    /// the batch without duplicating visit rows.
    ///
    /// # Errors
    ///
    /// Returns an error if any URL/title/transition is unsafe or the
    /// transaction fails.
    pub fn record_visit_batch(&self, visits: &[VisitInput]) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        for visit in visits {
            if visit.url.is_empty() {
                return Err(StoreError::InvalidInput("history URL cannot be empty"));
            }
            if !is_safe_history_url(&visit.url) {
                return Err(StoreError::UnsafeHistoryUrl);
            }
            if !bounded_stored_text(&visit.title, MAX_STORED_TITLE_BYTES) {
                return Err(StoreError::InvalidInput(
                    "history title is too large or contains a control character",
                ));
            }
            if !valid_stored_text(&visit.transition, 64) {
                return Err(StoreError::InvalidInput(
                    "history transition is empty, too large, or contains a control character",
                ));
            }
            transaction.execute(
                "INSERT INTO pages (normalized_url, safe_title, first_visit, last_visit, visit_count)
                 VALUES (?1, ?2, ?3, ?3, 1)
                 ON CONFLICT(normalized_url) DO UPDATE SET safe_title = excluded.safe_title,
                 last_visit = excluded.last_visit, visit_count = pages.visit_count + 1",
                params![visit.url, visit.title, visit.timestamp],
            )?;
            let id: i64 = transaction.query_row(
                "SELECT id FROM pages WHERE normalized_url = ?1",
                [&visit.url],
                |row| row.get(0),
            )?;
            transaction.execute(
                "INSERT INTO visits (page_id, committed_at, transition) VALUES (?1, ?2, ?3)",
                params![id, visit.timestamp, visit.transition],
            )?;
            prune_history(
                &transaction,
                visit.retention_before.unwrap_or_else(|| {
                    visit
                        .timestamp
                        .saturating_sub(DEFAULT_HISTORY_RETENTION_SECONDS)
                }),
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Stores a bookmark with a caller-owned stable ID.
    ///
    /// # Errors
    ///
    /// Returns an error for empty IDs/URLs or a database failure.
    pub fn add_bookmark(
        &self,
        id: &str,
        url: &str,
        title: &str,
        timestamp: i64,
    ) -> Result<(), StoreError> {
        if !valid_stored_text(id, MAX_STORED_IDENTIFIER_BYTES)
            || !bounded_stored_text(title, MAX_STORED_TITLE_BYTES)
            || !valid_stored_text(url, 16 * 1024)
        {
            return Err(StoreError::InvalidInput(
                "bookmark ID and URL must be bounded nonempty text; title must be bounded text",
            ));
        }
        self.connection.execute(
            "INSERT INTO bookmarks (id, url, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(id) DO UPDATE SET url = excluded.url, title = excluded.title, updated_at = excluded.updated_at",
            params![id, url, title, timestamp],
        )?;
        Ok(())
    }

    /// Deletes one bookmark by its stable caller-owned ID.
    ///
    /// # Errors
    ///
    /// Returns an error if SQLite fails.
    pub fn delete_bookmark(&self, id: &str) -> Result<bool, StoreError> {
        Ok(self
            .connection
            .execute("DELETE FROM bookmarks WHERE id = ?1", [id])?
            != 0)
    }

    /// Inserts or updates a case-sensitive quickmark name.
    ///
    /// # Errors
    ///
    /// Returns an error for empty names/URLs or a database failure.
    pub fn set_quickmark(&self, name: &str, url: &str, timestamp: i64) -> Result<(), StoreError> {
        if !valid_stored_text(name, MAX_STORED_IDENTIFIER_BYTES)
            || !valid_stored_text(url, 16 * 1024)
        {
            return Err(StoreError::InvalidInput(
                "quickmark name and URL must be bounded nonempty text",
            ));
        }
        self.connection.execute(
            "INSERT INTO quickmarks (name, url, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(name) DO UPDATE SET url = excluded.url, updated_at = excluded.updated_at",
            params![name, url, timestamp],
        )?;
        Ok(())
    }

    /// Deletes one quickmark by its exact name.
    ///
    /// # Errors
    ///
    /// Returns an error if SQLite fails.
    pub fn delete_quickmark(&self, name: &str) -> Result<bool, StoreError> {
        Ok(self
            .connection
            .execute("DELETE FROM quickmarks WHERE name = ?1", [name])?
            != 0)
    }

    /// Applies one bounded bookmark/quickmark mutation atomically.
    ///
    /// Delete mutations fail when their target is absent, preserving the
    /// command layer's stale-entry semantics.
    pub fn apply_mark_write(&self, write: &MarkWrite) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        Self::apply_mark_write_in_transaction(&transaction, write)?;
        transaction.commit()?;
        Ok(())
    }

    /// Applies a bounded group of bookmark/quickmark mutations atomically.
    ///
    /// Delete mutations fail when their target is absent, preserving the
    /// command layer's stale-entry semantics. If any mutation fails, none of
    /// the preceding mutations are committed.
    pub fn apply_mark_write_batch(&self, writes: &[MarkWrite]) -> Result<(), StoreError> {
        if writes.is_empty() {
            return Err(StoreError::InvalidInput("mark write batch cannot be empty"));
        }
        let transaction = self.connection.unchecked_transaction()?;
        for write in writes {
            Self::apply_mark_write_in_transaction(&transaction, write)?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn apply_mark_write_in_transaction(
        transaction: &rusqlite::Transaction<'_>,
        write: &MarkWrite,
    ) -> Result<(), StoreError> {
        match write {
            MarkWrite::AddBookmark {
                id,
                url,
                title,
                timestamp,
            } => {
                if !valid_stored_text(id, MAX_STORED_IDENTIFIER_BYTES)
                    || !bounded_stored_text(title, MAX_STORED_TITLE_BYTES)
                    || !valid_stored_text(url, 16 * 1024)
                {
                    return Err(StoreError::InvalidInput(
                        "bookmark ID and URL must be bounded nonempty text; title must be bounded text",
                    ));
                }
                transaction.execute(
                    "INSERT INTO bookmarks (id, url, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)
                     ON CONFLICT(id) DO UPDATE SET url = excluded.url, title = excluded.title, updated_at = excluded.updated_at",
                    params![id, url, title, timestamp],
                )?;
            }
            MarkWrite::DeleteBookmark { id } => {
                if !valid_stored_text(id, MAX_STORED_IDENTIFIER_BYTES) {
                    return Err(StoreError::InvalidInput("bookmark ID is invalid"));
                }
                if transaction.execute("DELETE FROM bookmarks WHERE id = ?1", [id])? == 0 {
                    return Err(StoreError::InvalidInput("bookmark was not found"));
                }
            }
            MarkWrite::EditBookmark {
                id,
                title,
                timestamp,
            } => {
                if !valid_stored_text(id, MAX_STORED_IDENTIFIER_BYTES)
                    || !bounded_stored_text(title, MAX_STORED_TITLE_BYTES)
                {
                    return Err(StoreError::InvalidInput(
                        "bookmark ID and title must be bounded valid text",
                    ));
                }
                if transaction.execute(
                    "UPDATE bookmarks SET title = ?2, updated_at = ?3 WHERE id = ?1",
                    params![id, title, timestamp],
                )? == 0
                {
                    return Err(StoreError::InvalidInput("bookmark was not found"));
                }
            }
            MarkWrite::SetQuickmark {
                name,
                url,
                timestamp,
            } => {
                if !valid_stored_text(name, MAX_STORED_IDENTIFIER_BYTES)
                    || !valid_stored_text(url, 16 * 1024)
                {
                    return Err(StoreError::InvalidInput(
                        "quickmark name and URL must be bounded nonempty text",
                    ));
                }
                transaction.execute(
                    "INSERT INTO quickmarks (name, url, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)
                     ON CONFLICT(name) DO UPDATE SET url = excluded.url, updated_at = excluded.updated_at",
                    params![name, url, timestamp],
                )?;
            }
            MarkWrite::DeleteQuickmark { name } => {
                if !valid_stored_text(name, MAX_STORED_IDENTIFIER_BYTES) {
                    return Err(StoreError::InvalidInput("quickmark name is invalid"));
                }
                if transaction.execute("DELETE FROM quickmarks WHERE name = ?1", [name])? == 0 {
                    return Err(StoreError::InvalidInput("quickmark was not found"));
                }
            }
            MarkWrite::EditQuickmark {
                name,
                url,
                timestamp,
            } => {
                if !valid_stored_text(name, MAX_STORED_IDENTIFIER_BYTES)
                    || !valid_stored_text(url, 16 * 1024)
                {
                    return Err(StoreError::InvalidInput(
                        "quickmark name and URL must be bounded nonempty text",
                    ));
                }
                if transaction.execute(
                    "UPDATE quickmarks SET url = ?2, updated_at = ?3 WHERE name = ?1",
                    params![name, url, timestamp],
                )? == 0
                {
                    return Err(StoreError::InvalidInput("quickmark was not found"));
                }
            }
        }
        Ok(())
    }

    /// Applies one durable journey mutation.
    ///
    /// # Errors
    ///
    /// Returns an error when the journey fields are invalid or SQLite fails.
    pub fn apply_journey_write(&self, write: &JourneyWrite) -> Result<bool, StoreError> {
        match write {
            JourneyWrite::RecordNode {
                id,
                profile_id,
                tab_id,
                url,
                title,
                committed_at,
                transition,
                source,
                retention_before,
                parent_id,
            } => {
                self.record_journey_node_with_retention_and_parent(
                    id,
                    profile_id,
                    tab_id,
                    url,
                    title,
                    *committed_at,
                    transition,
                    source.as_deref(),
                    *retention_before,
                    parent_id.as_deref(),
                )?;
                Ok(true)
            }
            JourneyWrite::SetCurrentById { tab_id, node_id } => {
                self.set_current_journey_node_by_id(tab_id, node_id)
            }
            JourneyWrite::SetCurrentForUrl { tab_id, url } => {
                self.set_current_journey_node_for_url(tab_id, url)
            }
        }
    }

    /// Clears profile-local history, optionally retaining pages newer than a
    /// timestamp. `origin` is an exact security-origin filter supplied by the
    /// caller after validation; it never accepts arbitrary SQL fragments.
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction fails.
    pub fn clear_history(
        &self,
        since: Option<i64>,
        origin: Option<&str>,
    ) -> Result<u64, StoreError> {
        if since.is_some_and(|timestamp| timestamp < 0) {
            return Err(StoreError::InvalidInput(
                "history clear timestamp must not be negative",
            ));
        }
        if origin.is_some_and(|origin| !valid_history_origin(origin)) {
            return Err(StoreError::InvalidInput(
                "history clear origin must be an exact HTTP(S) origin",
            ));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let mut deleted: u64 = 0;
        let mut statement = transaction.prepare(
            "SELECT id, normalized_url, last_visit FROM pages
             WHERE (?1 IS NULL OR last_visit < ?1)
               AND (?2 IS NULL OR normalized_url = ?2
                    OR normalized_url LIKE ?2 || '/%'
                    OR normalized_url LIKE ?2 || '?%'
                    OR normalized_url LIKE ?2 || '#%')",
        )?;
        let rows = statement.query_map(params![since, origin], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let page_ids = rows.collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        for (page_id, _) in page_ids {
            transaction.execute("DELETE FROM pages WHERE id = ?1", [page_id])?;
            deleted = deleted.saturating_add(1);
        }
        transaction.execute(
            "DELETE FROM journey_nodes
             WHERE (?1 IS NULL OR committed_at < ?1)
               AND (?2 IS NULL OR url = ?2
                    OR url LIKE ?2 || '/%'
                    OR url LIKE ?2 || '?%'
                    OR url LIKE ?2 || '#%')",
            params![since, origin],
        )?;
        transaction.commit()?;
        Ok(deleted)
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the SQLite query fails.
    pub fn quickmark(&self, name: &str) -> Result<Option<Quickmark>, StoreError> {
        self.connection
            .query_row(
                "SELECT name, url FROM quickmarks WHERE name = ?1",
                [name],
                |row| {
                    Ok(Quickmark {
                        name: row.get(0)?,
                        url: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// Lists profile-local history pages newest first, without exposing visit
    /// rows or any URL that failed the safe-history policy at write time.
    ///
    /// # Errors
    ///
    /// Returns an error if SQLite cannot execute the bounded query.
    pub fn history(&self, limit: usize) -> Result<Vec<HistoryRecord>, StoreError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT id, normalized_url, safe_title, visit_count, last_visit
             FROM pages ORDER BY last_visit DESC, id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], |row| {
            Ok(HistoryRecord {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                visit_count: row.get(3)?,
                last_visit: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Persists one safe core journey node and links it to the tab's previous
    /// current node. The profile database is already profile-local, but the
    /// profile and tab identities are retained to make exports and audits
    /// explicit. Retention is bounded to 50,000 nodes and 100,000 edges.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe or unbounded fields, an unknown transition,
    /// or a SQLite failure.
    pub fn record_journey_node(
        &self,
        id: &str,
        profile_id: &str,
        tab_id: &str,
        url: &str,
        title: &str,
        committed_at: i64,
        transition: &str,
        source: Option<&str>,
    ) -> Result<(), StoreError> {
        self.record_journey_node_with_retention(
            id,
            profile_id,
            tab_id,
            url,
            title,
            committed_at,
            transition,
            source,
            None,
        )
    }

    /// Persists one journey node while applying an optional profile retention
    /// cutoff in the same transaction as insertion and hard-cap pruning.
    pub fn record_journey_node_with_retention(
        &self,
        id: &str,
        profile_id: &str,
        tab_id: &str,
        url: &str,
        title: &str,
        committed_at: i64,
        transition: &str,
        source: Option<&str>,
        retention_before: Option<i64>,
    ) -> Result<(), StoreError> {
        self.record_journey_node_with_retention_and_parent(
            id,
            profile_id,
            tab_id,
            url,
            title,
            committed_at,
            transition,
            source,
            retention_before,
            None,
        )
    }

    /// Persists one journey node and optionally links it to an exact
    /// cross-tab parent node, such as a popup opener.
    pub fn record_journey_node_with_retention_and_parent(
        &self,
        id: &str,
        profile_id: &str,
        tab_id: &str,
        url: &str,
        title: &str,
        committed_at: i64,
        transition: &str,
        source: Option<&str>,
        retention_before: Option<i64>,
        parent_id: Option<&str>,
    ) -> Result<(), StoreError> {
        if !valid_stored_text(id, MAX_STORED_IDENTIFIER_BYTES)
            || !valid_stored_text(profile_id, MAX_STORED_IDENTIFIER_BYTES)
            || !valid_stored_text(tab_id, MAX_STORED_IDENTIFIER_BYTES)
            || !is_safe_history_url(url)
            || !bounded_stored_text(title, MAX_STORED_TITLE_BYTES)
            || !valid_journey_transition(transition)
            || source.is_some_and(|value| !bounded_stored_text(value, 128))
            || parent_id.is_some_and(|value| !valid_stored_text(value, MAX_STORED_IDENTIFIER_BYTES))
        {
            return Err(StoreError::InvalidInput(
                "journey records must contain safe bounded fields",
            ));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let previous: Option<String> = transaction
            .query_row(
                "SELECT node_id FROM journey_current WHERE tab_id = ?1",
                [tab_id],
                |row| row.get(0),
            )
            .optional()?;
        transaction.execute(
            "INSERT INTO journey_nodes
                (id, profile_id, tab_id, url, title, committed_at, transition, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                profile_id,
                tab_id,
                url,
                title,
                committed_at,
                transition,
                source
            ],
        )?;
        let relation = if previous.is_some() {
            previous
        } else if let Some(parent_id) = parent_id {
            transaction
                .query_row(
                    "SELECT id FROM journey_nodes WHERE id = ?1",
                    [parent_id],
                    |row| row.get(0),
                )
                .optional()?
        } else {
            None
        };
        if let Some(previous) = relation {
            transaction.execute(
                "INSERT INTO journey_edges (source_id, target_id, transition, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![previous, id, transition, committed_at],
            )?;
        }
        transaction.execute(
            "INSERT INTO journey_current (tab_id, node_id) VALUES (?1, ?2)
             ON CONFLICT(tab_id) DO UPDATE SET node_id = excluded.node_id",
            params![tab_id, id],
        )?;
        prune_journey(&transaction, retention_before)?;
        transaction.commit()?;
        Ok(())
    }

    /// Lists durable journey nodes newest first.
    pub fn journey_nodes(&self, limit: usize) -> Result<Vec<JourneyNodeRecord>, StoreError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT id, profile_id, tab_id, url, title, committed_at, transition, source
             FROM journey_nodes ORDER BY committed_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], |row| {
            Ok(JourneyNodeRecord {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                tab_id: row.get(2)?,
                url: row.get(3)?,
                title: row.get(4)?,
                committed_at: row.get(5)?,
                transition: row.get(6)?,
                source: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Searches the safe journey fields without loading the whole graph.
    ///
    /// The query is literal (SQL wildcard characters in the user's text do
    /// not become wildcards) and remains bounded for native search surfaces.
    pub fn journey_nodes_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<JourneyNodeRecord>, StoreError> {
        if query.is_empty()
            || query.len() > MAX_STORED_IDENTIFIER_BYTES
            || query.chars().any(char::is_control)
        {
            return Err(StoreError::InvalidInput(
                "journey search must be 1..256 bytes without control characters",
            ));
        }
        let escaped = query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped}%");
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT id, profile_id, tab_id, url, title, committed_at, transition, source
             FROM journey_nodes
             WHERE title LIKE ?1 ESCAPE '\\' COLLATE NOCASE
                OR url LIKE ?1 ESCAPE '\\' COLLATE NOCASE
                OR transition LIKE ?1 ESCAPE '\\' COLLATE NOCASE
                OR COALESCE(source, '') LIKE ?1 ESCAPE '\\' COLLATE NOCASE
             ORDER BY committed_at DESC, id DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![pattern, limit], |row| {
            Ok(JourneyNodeRecord {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                tab_id: row.get(2)?,
                url: row.get(3)?,
                title: row.get(4)?,
                committed_at: row.get(5)?,
                transition: row.get(6)?,
                source: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Lists the bounded one-hop neighborhood of a retained node.
    pub fn journey_neighbors(
        &self,
        node_id: &str,
        limit: usize,
    ) -> Result<Vec<JourneyNodeRecord>, StoreError> {
        if !valid_stored_text(node_id, MAX_STORED_IDENTIFIER_BYTES) {
            return Err(StoreError::InvalidInput("journey node ID is invalid"));
        }
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT DISTINCT n.id, n.profile_id, n.tab_id, n.url, n.title,
                    n.committed_at, n.transition, n.source
             FROM journey_nodes n
             JOIN journey_edges e ON (e.source_id = ?1 AND e.target_id = n.id)
                                  OR (e.target_id = ?1 AND e.source_id = n.id)
             ORDER BY n.committed_at DESC, n.id DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![node_id, limit], |row| {
            Ok(JourneyNodeRecord {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                tab_id: row.get(2)?,
                url: row.get(3)?,
                title: row.get(4)?,
                committed_at: row.get(5)?,
                transition: row.get(6)?,
                source: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Lists edges whose two endpoints are in a bounded native-view set.
    pub fn journey_edges_for_nodes(
        &self,
        node_ids: &[String],
        limit: usize,
    ) -> Result<Vec<JourneyEdgeRecord>, StoreError> {
        if node_ids.is_empty() {
            return Ok(Vec::new());
        }
        if node_ids.len() > 1_000
            || node_ids
                .iter()
                .any(|id| !valid_stored_text(id, MAX_STORED_IDENTIFIER_BYTES))
        {
            return Err(StoreError::InvalidInput(
                "journey node set is invalid or too large",
            ));
        }
        let placeholders = (1..=node_ids.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let second_placeholders = (node_ids.len() + 1..=node_ids.len() * 2)
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let bounded_limit = limit.min(1_000);
        let sql = format!(
            "SELECT source_id, target_id, transition, created_at
             FROM journey_edges
             WHERE source_id IN ({placeholders}) AND target_id IN ({second_placeholders})
             ORDER BY created_at ASC, rowid ASC LIMIT {bounded_limit}"
        );
        let values = node_ids
            .iter()
            .map(String::as_str)
            .chain(node_ids.iter().map(String::as_str))
            .collect::<Vec<_>>();
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values), |row| {
            Ok(JourneyEdgeRecord {
                source_id: row.get(0)?,
                target_id: row.get(1)?,
                transition: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Reads one durable journey node by UUID without constructing a fallback
    /// from caller-supplied URL-shaped input.
    pub fn journey_node(&self, id: &str) -> Result<Option<JourneyNodeRecord>, StoreError> {
        self.connection
            .query_row(
                "SELECT id, profile_id, tab_id, url, title, committed_at, transition, source
                 FROM journey_nodes WHERE id = ?1",
                [id],
                |row| {
                    Ok(JourneyNodeRecord {
                        id: row.get(0)?,
                        profile_id: row.get(1)?,
                        tab_id: row.get(2)?,
                        url: row.get(3)?,
                        title: row.get(4)?,
                        committed_at: row.get(5)?,
                        transition: row.get(6)?,
                        source: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// Lists durable journey edges oldest first, bounded for native views.
    pub fn journey_edges(&self, limit: usize) -> Result<Vec<JourneyEdgeRecord>, StoreError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT source_id, target_id, transition, created_at
             FROM journey_edges ORDER BY created_at ASC, rowid ASC LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], |row| {
            Ok(JourneyEdgeRecord {
                source_id: row.get(0)?,
                target_id: row.get(1)?,
                transition: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Returns the durable current node for one live tab key, if one exists.
    pub fn current_journey_node(&self, tab_id: &str) -> Result<Option<String>, StoreError> {
        self.connection
            .query_row(
                "SELECT node_id FROM journey_current WHERE tab_id = ?1",
                [tab_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// Moves a durable tab pointer to the newest retained node with the
    /// committed URL. Traversal never creates a new journey node.
    pub fn set_current_journey_node_for_url(
        &self,
        tab_id: &str,
        url: &str,
    ) -> Result<bool, StoreError> {
        if !valid_stored_text(tab_id, MAX_STORED_IDENTIFIER_BYTES) || !is_safe_history_url(url) {
            return Err(StoreError::InvalidInput(
                "journey traversal requires safe bounded fields",
            ));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let node_id: Option<String> = transaction
            .query_row(
                "SELECT id FROM journey_nodes
                 WHERE tab_id = ?1 AND url = ?2
                 ORDER BY committed_at DESC, id DESC LIMIT 1",
                params![tab_id, url],
                |row| row.get(0),
            )
            .optional()?;
        let Some(node_id) = node_id else {
            transaction.commit()?;
            return Ok(false);
        };
        transaction.execute(
            "INSERT INTO journey_current (tab_id, node_id) VALUES (?1, ?2)
             ON CONFLICT(tab_id) DO UPDATE SET node_id = excluded.node_id",
            params![tab_id, node_id],
        )?;
        transaction.commit()?;
        Ok(true)
    }

    /// Moves a durable tab pointer to an exact retained node identifier.
    pub fn set_current_journey_node_by_id(
        &self,
        tab_id: &str,
        node_id: &str,
    ) -> Result<bool, StoreError> {
        if !valid_stored_text(tab_id, MAX_STORED_IDENTIFIER_BYTES)
            || !valid_stored_text(node_id, MAX_STORED_IDENTIFIER_BYTES)
        {
            return Err(StoreError::InvalidInput(
                "journey traversal requires safe bounded identifiers",
            ));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let exists: Option<i64> = transaction
            .query_row(
                "SELECT 1 FROM journey_nodes WHERE id = ?1 AND tab_id = ?2",
                params![node_id, tab_id],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            transaction.commit()?;
            return Ok(false);
        }
        transaction.execute(
            "INSERT INTO journey_current (tab_id, node_id) VALUES (?1, ?2)
             ON CONFLICT(tab_id) DO UPDATE SET node_id = excluded.node_id",
            params![tab_id, node_id],
        )?;
        transaction.commit()?;
        Ok(true)
    }

    /// Clears all durable journey state for this profile.
    pub fn clear_journey(&self) -> Result<u64, StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        let deleted = transaction.execute("DELETE FROM journey_nodes", [])? as u64;
        transaction.commit()?;
        Ok(deleted)
    }

    /// Lists profile-local bookmarks newest first.
    ///
    /// # Errors
    ///
    /// Returns an error if SQLite cannot execute the bounded query.
    pub fn bookmarks(&self, limit: usize) -> Result<Vec<BookmarkRecord>, StoreError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT id, url, title, updated_at FROM bookmarks
             ORDER BY updated_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], |row| {
            Ok(BookmarkRecord {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Lists profile-local quickmarks newest first.
    ///
    /// # Errors
    ///
    /// Returns an error if SQLite cannot execute the bounded query.
    pub fn quickmarks(&self, limit: usize) -> Result<Vec<Quickmark>, StoreError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT name, url FROM quickmarks ORDER BY updated_at DESC, name ASC LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], |row| {
            Ok(Quickmark {
                name: row.get(0)?,
                url: row.get(1)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Records eligible command text. Ineligible/sensitive commands are a
    /// successful no-op and never reach SQLite.
    ///
    /// # Errors
    ///
    /// Returns an error for an eligible empty command or a database failure.
    pub fn record_command(
        &self,
        command: &str,
        timestamp: i64,
        eligible: bool,
    ) -> Result<bool, StoreError> {
        if !eligible {
            return Ok(false);
        }
        if command.is_empty() {
            return Err(StoreError::InvalidInput(
                "command history text cannot be empty",
            ));
        }
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO command_history (command, created_at) VALUES (?1, ?2)",
            params![command, timestamp],
        )?;
        transaction.execute(
            "DELETE FROM command_history
             WHERE id NOT IN (
                 SELECT id FROM command_history ORDER BY created_at DESC, id DESC LIMIT ?1
             )",
            [MAX_COMMAND_HISTORY_ROWS],
        )?;
        transaction.commit()?;
        Ok(true)
    }

    /// Creates or replaces the durable record for a normal-profile download.
    /// The engine request and file contents remain owned by Qt; this row is
    /// only the Rust metadata index.
    ///
    /// # Errors
    ///
    /// Returns an error for empty identifiers/URLs or a database failure.
    pub fn create_download(
        &self,
        id: &str,
        source_url: &str,
        destination: &str,
        state: DownloadState,
        created_at: i64,
    ) -> Result<(), StoreError> {
        if !valid_stored_text(id, MAX_STORED_IDENTIFIER_BYTES)
            || !valid_stored_text(source_url, 16 * 1024)
        {
            return Err(StoreError::InvalidInput(
                "download ID and source URL must be bounded nonempty text",
            ));
        }
        self.connection.execute(
            "INSERT INTO downloads (id, source_url, destination, state, bytes_received, created_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, NULL)
             ON CONFLICT(id) DO UPDATE SET source_url = excluded.source_url,
             destination = excluded.destination, state = excluded.state,
             bytes_received = 0, created_at = excluded.created_at, completed_at = NULL",
            params![id, source_url, destination, state.as_str(), created_at],
        )?;
        Ok(())
    }

    /// Updates a download's lifecycle and byte progress.
    ///
    /// # Errors
    ///
    /// Returns an error if the row is missing or SQLite fails.
    pub fn update_download(
        &self,
        id: &str,
        state: DownloadState,
        bytes_received: i64,
        completed_at: Option<i64>,
    ) -> Result<(), StoreError> {
        if bytes_received < 0 {
            return Err(StoreError::InvalidInput(
                "download byte count cannot be negative",
            ));
        }
        let changed = self.connection.execute(
            "UPDATE downloads SET state = ?2, bytes_received = ?3, completed_at = ?4 WHERE id = ?1",
            params![id, state.as_str(), bytes_received, completed_at],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidInput("download ID was not found"));
        }
        Ok(())
    }

    /// Applies a bounded group of download lifecycle updates atomically.
    ///
    /// # Errors
    ///
    /// Returns an error if the batch is empty, an update is invalid, a
    /// download is missing, or SQLite fails. A failed update rolls back the
    /// entire batch.
    pub fn update_download_batch(&self, updates: &[DownloadUpdate]) -> Result<(), StoreError> {
        if updates.is_empty() {
            return Err(StoreError::InvalidInput(
                "download update batch cannot be empty",
            ));
        }
        for update in updates {
            if !valid_stored_text(&update.id, MAX_STORED_IDENTIFIER_BYTES)
                || update.bytes_received < 0
            {
                return Err(StoreError::InvalidInput(
                    "download update ID or byte count is invalid",
                ));
            }
        }
        let transaction = self.connection.unchecked_transaction()?;
        for update in updates {
            let changed = transaction.execute(
                "UPDATE downloads SET state = ?2, bytes_received = ?3, completed_at = ?4 WHERE id = ?1",
                params![
                    update.id,
                    update.state.as_str(),
                    update.bytes_received,
                    update.completed_at
                ],
            )?;
            if changed == 0 {
                return Err(StoreError::InvalidInput("download ID was not found"));
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// Records the final user-selected destination before the engine starts
    /// writing bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the row is missing or SQLite fails.
    pub fn set_download_destination(&self, id: &str, destination: &str) -> Result<(), StoreError> {
        if destination.is_empty() {
            return Err(StoreError::InvalidInput(
                "download destination cannot be empty",
            ));
        }
        let changed = self.connection.execute(
            "UPDATE downloads SET destination = ?2 WHERE id = ?1",
            params![id, destination],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidInput("download ID was not found"));
        }
        Ok(())
    }

    /// Reads one durable download record.
    ///
    /// # Errors
    ///
    /// Returns an error if SQLite fails or a stored state is unknown.
    pub fn download(&self, id: &str) -> Result<Option<DownloadRecord>, StoreError> {
        self.connection
            .query_row(
                "SELECT id, source_url, destination, state, bytes_received, created_at, completed_at FROM downloads WHERE id = ?1",
                [id],
                download_from_row,
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// Lists durable downloads newest first.
    ///
    /// # Errors
    ///
    /// Returns an error if SQLite fails or a stored state is unknown.
    pub fn downloads(&self) -> Result<Vec<DownloadRecord>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, source_url, destination, state, bytes_received, created_at, completed_at
             FROM downloads ORDER BY created_at DESC, id DESC",
        )?;
        let rows = statement.query_map([], download_from_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    #[must_use]
    pub fn schema_version(&self) -> i64 {
        SCHEMA_VERSION
    }
}

/// Inspects a profile database without taking a write lock or changing it.
///
/// The result is intentionally suitable for diagnostics: it contains no
/// profile name, URL, or filesystem path.
#[must_use]
pub fn inspect_store(path: impl AsRef<Path>) -> StoreInspection {
    let path = path.as_ref();
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.len() == 0 => {
            return StoreInspection {
                status: "missing".into(),
                schema_version: None,
                integrity: "not-initialized".into(),
                reason: "the profile database file is empty".into(),
                recovery: "open the profile once or create a new named profile".into(),
            };
        }
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => {
            return unavailable_inspection("the profile database path is not a regular file");
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return StoreInspection {
                status: "missing".into(),
                schema_version: None,
                integrity: "not-created".into(),
                reason: "the profile database file does not exist".into(),
                recovery: "open the profile once or create a new named profile".into(),
            };
        }
        Err(_) => return unavailable_inspection("the profile database metadata is unavailable"),
    }
    match inspect_existing_database(path) {
        Ok(schema_version) => StoreInspection {
            status: "available".into(),
            schema_version,
            integrity: "ok".into(),
            reason: "read-only SQLite integrity check passed".into(),
            recovery: "normal profile storage is available".into(),
        },
        Err(StoreError::Corrupt(reason)) => StoreInspection {
            status: "corrupt".into(),
            schema_version: None,
            integrity: "failed".into(),
            reason,
            recovery: "preserve the original; inspect read-only or create a new named profile"
                .into(),
        },
        Err(StoreError::UnsupportedSchema(version)) => StoreInspection {
            status: "unsupported".into(),
            schema_version: Some(version),
            integrity: "ok".into(),
            reason: "the database schema is newer than this application".into(),
            recovery: "use a newer RustBrowser or create a new named profile".into(),
        },
        Err(error) => unavailable_inspection(&format!("read-only inspection failed: {error}")),
    }
}

fn unavailable_inspection(reason: &str) -> StoreInspection {
    StoreInspection {
        status: "unavailable".into(),
        schema_version: None,
        integrity: "not-checked".into(),
        reason: reason.into(),
        recovery: "preserve the original and resolve the storage error before retrying".into(),
    }
}

fn inspect_existing_database(path: &Path) -> Result<Option<i64>, StoreError> {
    let connection =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|error| {
            match &error {
                rusqlite::Error::SqliteFailure(code, _)
                    if matches!(
                        code.code,
                        rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase
                    ) =>
                {
                    StoreError::Corrupt(error.to_string())
                }
                _ => StoreError::from(error),
            }
        })?;
    let integrity: String = connection
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(|error| match &error {
            rusqlite::Error::SqliteFailure(code, _)
                if matches!(
                    code.code,
                    rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase
                ) =>
            {
                StoreError::Corrupt(error.to_string())
            }
            _ => StoreError::from(error),
        })?;
    if integrity != "ok" {
        return Err(StoreError::Corrupt(format!(
            "SQLite quick_check reported {integrity}"
        )));
    }
    let has_migrations: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let Some(_) = has_migrations else {
        return Ok(None);
    };
    let mut migrations =
        connection.prepare("SELECT version, checksum FROM schema_migrations ORDER BY version")?;
    let rows = migrations.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    for migration in rows {
        let (version, checksum) = migration?;
        if version == SCHEMA_VERSION && checksum != SCHEMA_MIGRATION_CHECKSUM {
            return Err(StoreError::Corrupt(format!(
                "migration checksum mismatch for version {version}"
            )));
        }
    }
    let version: Option<i64> =
        connection.query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })?;
    if version.is_some_and(|version| version > SCHEMA_VERSION) {
        return Err(StoreError::UnsupportedSchema(
            version.expect("checked above"),
        ));
    }
    Ok(version)
}

fn pre_migration_backup_path(path: &Path, version: i64) -> PathBuf {
    let filename = path
        .file_name()
        .map_or_else(|| "browser.sqlite".into(), |name| name.to_string_lossy());
    path.with_file_name(format!("{filename}.pre-migration-v{version}.sqlite"))
}

fn create_pre_migration_backup(path: &Path, version: i64) -> Result<(), StoreError> {
    let metadata =
        fs::metadata(path).map_err(|error| StoreError::MigrationBackup(error.to_string()))?;
    if metadata.len() > MAX_PRE_MIGRATION_BACKUP_BYTES {
        return Err(StoreError::MigrationBackup(
            "the source database exceeds the 512 MiB backup limit".into(),
        ));
    }
    let backup_path = pre_migration_backup_path(path, version);
    if fs::metadata(&backup_path).is_ok_and(|metadata| metadata.is_file()) {
        return Ok(());
    }
    let temporary_path = backup_path.with_file_name(format!(
        ".{}.tmp-{}",
        backup_path
            .file_name()
            .map_or_else(|| "migration-backup".into(), |name| name.to_string_lossy()),
        std::process::id()
    ));
    let _ = fs::remove_file(&temporary_path);
    let source = rusqlite::Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| StoreError::MigrationBackup(error.to_string()))?;
    source
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| StoreError::MigrationBackup(error.to_string()))?;
    source
        .backup("main", &temporary_path, None)
        .map_err(|error| {
            let _ = fs::remove_file(&temporary_path);
            StoreError::from(error)
        })?;
    let backup_file = fs::File::open(&temporary_path).map_err(|error| {
        let _ = fs::remove_file(&temporary_path);
        StoreError::MigrationBackup(error.to_string())
    })?;
    backup_file.sync_all().map_err(|error| {
        let _ = fs::remove_file(&temporary_path);
        StoreError::MigrationBackup(error.to_string())
    })?;
    drop(backup_file);
    fs::rename(&temporary_path, &backup_path).map_err(|error| {
        let _ = fs::remove_file(&temporary_path);
        StoreError::MigrationBackup(error.to_string())
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&backup_path, fs::Permissions::from_mode(0o600))
            .map_err(|error| StoreError::MigrationBackup(error.to_string()))?;
    }
    if let Some(parent) = backup_path.parent() {
        let directory = fs::File::open(parent)
            .map_err(|error| StoreError::MigrationBackup(error.to_string()))?;
        directory
            .sync_all()
            .map_err(|error| StoreError::MigrationBackup(error.to_string()))?;
    }
    Ok(())
}

fn download_from_row(row: &rusqlite::Row<'_>) -> Result<DownloadRecord, rusqlite::Error> {
    let state: String = row.get(3)?;
    Ok(DownloadRecord {
        id: row.get(0)?,
        source_url: row.get(1)?,
        destination: row.get(2)?,
        state: DownloadState::parse(&state).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                format!("unknown download state: {state}").into(),
            )
        })?,
        bytes_received: row.get(4)?,
        created_at: row.get(5)?,
        completed_at: row.get(6)?,
    })
}

fn prune_history(
    transaction: &rusqlite::Transaction<'_>,
    retention_before: i64,
) -> Result<(), rusqlite::Error> {
    transaction.execute(
        "DELETE FROM visits WHERE committed_at < ?1",
        [retention_before],
    )?;
    transaction.execute(
        "DELETE FROM pages WHERE id NOT IN (SELECT DISTINCT page_id FROM visits)",
        [],
    )?;
    Ok(())
}

fn prune_journey(
    transaction: &rusqlite::Transaction<'_>,
    retention_before: Option<i64>,
) -> Result<(), rusqlite::Error> {
    if let Some(retention_before) = retention_before {
        transaction.execute(
            "DELETE FROM journey_nodes
             WHERE committed_at < ?1
               AND id NOT IN (SELECT node_id FROM journey_current)",
            [retention_before],
        )?;
    }
    let node_count: i64 =
        transaction.query_row("SELECT COUNT(*) FROM journey_nodes", [], |row| row.get(0))?;
    let excess = node_count.saturating_sub(50_000);
    if excess > 0 {
        let mut statement = transaction.prepare(
            "SELECT id FROM journey_nodes
             WHERE id NOT IN (SELECT node_id FROM journey_current)
             ORDER BY committed_at ASC, rowid ASC LIMIT ?1",
        )?;
        let ids = statement
            .query_map([excess], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        for id in ids {
            transaction.execute("DELETE FROM journey_nodes WHERE id = ?1", [id])?;
        }
    }
    let edge_count: i64 =
        transaction.query_row("SELECT COUNT(*) FROM journey_edges", [], |row| row.get(0))?;
    let edge_excess = edge_count.saturating_sub(100_000);
    if edge_excess > 0 {
        transaction.execute(
            "DELETE FROM journey_edges WHERE rowid IN
             (SELECT rowid FROM journey_edges ORDER BY created_at ASC, rowid ASC LIMIT ?1)",
            [edge_excess],
        )?;
    }
    Ok(())
}

/// Applies the storage boundary's conservative history policy. This is
/// intentionally stricter than URL parsing: a URL with credentials, a
/// fragment, or a query key that commonly carries a secret is not persisted.
#[must_use]
pub fn is_safe_history_url(url: &str) -> bool {
    if url.is_empty() || url.len() > 16 * 1024 || url.chars().any(char::is_control) {
        return false;
    }
    if url.contains('#') {
        return false;
    }
    let Some((scheme, remainder)) = url.split_once("://") else {
        return url.starts_with("file:") || url.starts_with("about:");
    };
    if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") {
        return false;
    }
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    if remainder[..authority_end].contains('@') {
        return false;
    }
    let without_fragment = remainder
        .split_once('#')
        .map_or(remainder, |(head, _)| head);
    let Some((_, query)) = without_fragment.split_once('?') else {
        return true;
    };
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .all(|part| {
            let key = part.split('=').next().unwrap_or_default();
            let Some(key) = decode_query_key(key) else {
                return false;
            };
            !sensitive_history_query_key(&key)
        })
}

fn decode_query_key(key: &str) -> Option<String> {
    let bytes = key.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = bytes.get(index + 1).and_then(|value| hex_value(*value))?;
            let low = bytes.get(index + 2).and_then(|value| hex_value(*value))?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded)
        .ok()
        .map(|key| key.to_ascii_lowercase())
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn sensitive_history_query_key(key: &str) -> bool {
    matches!(
        key,
        "access-token"
            | "access_token"
            | "api-key"
            | "api_key"
            | "apikey"
            | "auth"
            | "authorization"
            | "bearer"
            | "client-secret"
            | "client_secret"
            | "code"
            | "credential"
            | "credentials"
            | "jwt"
            | "nonce"
            | "password"
            | "passwd"
            | "private-key"
            | "private_key"
            | "refresh-token"
            | "refresh_token"
            | "secret"
            | "session"
            | "sig"
            | "signature"
            | "token"
    )
}

fn migrate(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE IF NOT EXISTS schema_migrations (
             version INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS pages (
             id INTEGER PRIMARY KEY, normalized_url TEXT NOT NULL UNIQUE, safe_title TEXT NOT NULL,
             first_visit INTEGER NOT NULL, last_visit INTEGER NOT NULL, visit_count INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS visits (
             id INTEGER PRIMARY KEY, page_id INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
             committed_at INTEGER NOT NULL, transition TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS visits_time ON visits(committed_at);
         CREATE TABLE IF NOT EXISTS bookmarks (
             id TEXT PRIMARY KEY, url TEXT NOT NULL, title TEXT NOT NULL,
             created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS quickmarks (
             name TEXT PRIMARY KEY, url TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS command_history (
             id INTEGER PRIMARY KEY, command TEXT NOT NULL, created_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS permission_rules (
             origin TEXT NOT NULL, permission TEXT NOT NULL, decision TEXT NOT NULL,
             expires_at INTEGER, updated_at INTEGER NOT NULL, UNIQUE(origin, permission)
         );
         CREATE TABLE IF NOT EXISTS downloads (
             id TEXT PRIMARY KEY, source_url TEXT NOT NULL, destination TEXT NOT NULL,
             state TEXT NOT NULL, bytes_received INTEGER NOT NULL, created_at INTEGER NOT NULL, completed_at INTEGER
         );
         CREATE TABLE IF NOT EXISTS site_preferences (
             origin TEXT PRIMARY KEY, preferences_json TEXT NOT NULL, revision INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS journey_nodes (
             id TEXT PRIMARY KEY, profile_id TEXT NOT NULL, tab_id TEXT NOT NULL,
             url TEXT NOT NULL, title TEXT NOT NULL, committed_at INTEGER NOT NULL,
             transition TEXT NOT NULL, source TEXT
         );
         CREATE INDEX IF NOT EXISTS journey_nodes_time
             ON journey_nodes(committed_at, id);
         CREATE TABLE IF NOT EXISTS journey_edges (
             source_id TEXT NOT NULL REFERENCES journey_nodes(id) ON DELETE CASCADE,
             target_id TEXT NOT NULL REFERENCES journey_nodes(id) ON DELETE CASCADE,
             transition TEXT NOT NULL, created_at INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS journey_edges_time
             ON journey_edges(created_at);
         CREATE TABLE IF NOT EXISTS journey_current (
             tab_id TEXT PRIMARY KEY,
             node_id TEXT NOT NULL REFERENCES journey_nodes(id) ON DELETE CASCADE
         );
         INSERT OR IGNORE INTO schema_migrations(version, checksum, applied_at)
             VALUES (1, 'builtin-schema-1', strftime('%s','now'));
         COMMIT;",
    )
}

#[must_use]
pub fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rustbrowser-storage-{label}-{}-{}.sqlite",
            std::process::id(),
            unix_timestamp()
        ))
    }

    #[test]
    fn migration_configures_durable_profile_storage() {
        let path = temp_path("migration");
        let store = ProfileStore::open(&path, StoreMode::Normal).expect("open");
        assert_eq!(store.schema_version(), 1);
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
    fn older_schema_is_backed_up_before_migration() {
        let path = temp_path("migration-backup");
        let connection = Connection::open(&path).expect("create fixture");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, checksum TEXT NOT NULL, applied_at INTEGER NOT NULL);
                 INSERT INTO schema_migrations(version, checksum, applied_at) VALUES (0, 'legacy', 1);",
            )
            .expect("write legacy schema");
        drop(connection);

        let backup_path = pre_migration_backup_path(&path, 0);
        let store = ProfileStore::open(&path, StoreMode::Normal).expect("migrate legacy schema");
        assert_eq!(store.schema_version(), 1);
        let backup = Connection::open_with_flags(&backup_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("open migration backup");
        assert_eq!(
            backup
                .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| row
                    .get::<_, i64>(
                    0
                ))
                .expect("read backup schema"),
            0
        );
        drop(backup);
        drop(store);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&backup_path);
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
                 INSERT INTO schema_migrations(version, checksum, applied_at) VALUES (1, 'tampered', 1);",
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
}
