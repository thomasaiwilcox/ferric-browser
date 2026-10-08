//! Profile-local durable metadata storage.
//!
//! `QtWebEngine` owns cookies, cache, service workers, and its native browsing
//! data. This crate owns only `Ferric Browser` metadata and refuses durable opening
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
mod store_catalog;
mod store_commands;
mod store_downloads;
mod store_journeys;
mod store_library;
pub mod versioning;
mod worker;

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, Weak},
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
pub use roots::{
    CURRENT_ROOT_SCHEMA_VERSION, ResetReport, RootSpec, StorageRoots, StorageRootsError,
};
pub use sessions::{
    ClosedTabSnapshot, RestoreMethod, RestorePlanEntry, RestoreSafety, SessionError,
    SessionSnapshot, SessionTab, SnapshotTabInput, SnapshotWindowInput,
    clear_current_session_checkpoints, current_session_paths, delete_named_session,
    list_named_sessions, load_session, named_session_path, save_session_atomic,
};
#[cfg(target_os = "linux")]
pub use versioning::{MediaFingerprintKey, MediaFingerprintKeyError};
pub use worker::JourneyQuerySnapshot;
pub use worker::{
    ProfileLibrarySnapshot, ProfileStoreWorker, SessionRestoreSnapshot, StorageCommand,
    StorageCompletion, StorageFailureKind, StorageOperation, StorageOperationError,
    StorageWorkerError, VersioningObjectPublishReceipt,
};

const SCHEMA_VERSION: i64 = versioning::schema::VERSION;
const BASE_SCHEMA_VERSION: i64 = 3;
const BASE_SCHEMA_CHECKSUM: &str = "builtin-schema-3";
const DEFAULT_HISTORY_RETENTION_SECONDS: i64 = 90 * 24 * 60 * 60;
const MAX_COMMAND_HISTORY_ROWS: i64 = 1_000;
const MAX_STORED_TITLE_BYTES: usize = 4 * 1024;
const MAX_STORED_IDENTIFIER_BYTES: usize = 256;
const PROFILE_WAL_AUTOCHECKPOINT_PAGES: i64 = 1_000;
const PROFILE_WAL_RETAINED_BYTES: i64 = 16 * 1024 * 1024;

type ProfileOpenLocks = BTreeMap<PathBuf, Weak<Mutex<()>>>;
static PROFILE_OPEN_LOCKS: OnceLock<Mutex<ProfileOpenLocks>> = OnceLock::new();

fn profile_open_lock(path: &Path) -> Result<Arc<Mutex<()>>, StoreError> {
    let identity = fs::canonicalize(path)
        .or_else(|_| {
            let parent = path.parent().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "profile path has no parent",
                )
            })?;
            let name = path.file_name().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "profile path has no filename",
                )
            })?;
            fs::canonicalize(parent).map(|parent| parent.join(name))
        })
        .map_err(|_| StoreError::InvalidInput("profile database identity is unavailable"))?;
    let locks = PROFILE_OPEN_LOCKS.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut locks = locks
        .lock()
        .map_err(|_| StoreError::InvalidInput("profile open registry is unavailable"))?;
    locks.retain(|_, lock| lock.strong_count() != 0);
    if let Some(lock) = locks.get(&identity).and_then(Weak::upgrade) {
        return Ok(lock);
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(identity, Arc::downgrade(&lock));
    Ok(lock)
}

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

pub enum StoreError {
    PrivateNoDurableState,
    Corrupt(String),
    LegacySchema(i64),
    UnsupportedSchema(i64),
    DiskFull,
    CheckpointBusy,
    Sql(rusqlite::Error),
    InvalidInput(&'static str),
    UnsafeHistoryUrl,
}

impl std::fmt::Debug for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrivateNoDurableState => formatter.write_str("PrivateNoDurableState"),
            Self::Corrupt(_) => formatter
                .debug_tuple("Corrupt")
                .field(&"[redacted]")
                .finish(),
            Self::LegacySchema(version) => formatter
                .debug_tuple("LegacySchema")
                .field(version)
                .finish(),
            Self::UnsupportedSchema(version) => formatter
                .debug_tuple("UnsupportedSchema")
                .field(version)
                .finish(),
            Self::DiskFull => formatter.write_str("DiskFull"),
            Self::CheckpointBusy => formatter.write_str("CheckpointBusy"),
            Self::Sql(_) => formatter.write_str("Sql([redacted])"),
            Self::InvalidInput(message) => formatter
                .debug_tuple("InvalidInput")
                .field(message)
                .finish(),
            Self::UnsafeHistoryUrl => formatter.write_str("UnsafeHistoryUrl"),
        }
    }
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrivateNoDurableState => {
                formatter.write_str("private sessions cannot open durable metadata")
            }
            Self::Corrupt(_) => formatter.write_str(
                "profile database integrity check failed; original data was preserved; use read-only inspection or create a new named profile",
            ),
            Self::LegacySchema(version) => write!(
                formatter,
                "profile storage schema version {version} belongs to an earlier clean-break release; original data was preserved; reset only Ferric-owned data or create a new named profile"
            ),
            Self::UnsupportedSchema(version) => write!(
                formatter,
                "profile database schema version {version} is newer than this application; original data was preserved; use a newer Ferric Browser or create a new named profile"
            ),
            Self::DiskFull => formatter.write_str(
                "profile storage is full; no replacement was written; free disk space and retry once",
            ),
            Self::CheckpointBusy => formatter.write_str(
                "profile storage could not be checkpointed because another connection is active",
            ),
            Self::Sql(_) => formatter.write_str("profile metadata operation failed"),
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
    /// Opens or creates a normal profile database, upgrading schema 3 atomically.
    ///
    /// # Errors
    ///
    /// Returns an error if the mode is private, SQLite cannot be configured,
    /// or existing data belongs to an unsupported schema. Schemas before 3
    /// remain behind the earlier clean-break boundary.
    pub fn open(path: impl AsRef<Path>, mode: StoreMode) -> Result<Self, StoreError> {
        if mode == StoreMode::Private {
            return Err(StoreError::PrivateNoDurableState);
        }
        let path = path.as_ref().to_owned();
        // Multiple same-process owners may start against one new profile.
        // Serialize their read-only preflight, journal-mode setup, and schema
        // migration so an opening peer cannot observe a transient lock.
        let open_lock = profile_open_lock(&path)?;
        let _open_guard = open_lock
            .lock()
            .map_err(|_| StoreError::InvalidInput("profile open lock is unavailable"))?;
        let existing_version =
            if fs::metadata(&path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0) {
                inspect_existing_database(&path)?
            } else {
                None
            };
        if let Some(version) = existing_version {
            if version < BASE_SCHEMA_VERSION {
                return Err(StoreError::LegacySchema(version));
            }
            if version > SCHEMA_VERSION {
                return Err(StoreError::UnsupportedSchema(version));
            }
        }
        let connection = Connection::open(&path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL;",
        )?;
        connection.execute_batch(&format!(
            "PRAGMA wal_autocheckpoint={PROFILE_WAL_AUTOCHECKPOINT_PAGES};\
             PRAGMA journal_size_limit={PROFILE_WAL_RETAINED_BYTES};"
        ))?;
        migrate(&connection, existing_version)?;
        Ok(Self { path, connection })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Flushes committed metadata from SQLite's WAL and truncates the idle log.
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
        let mut statement = self.connection.prepare("PRAGMA wal_checkpoint(TRUNCATE)")?;
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
        Err(StoreError::Corrupt(_)) => StoreInspection {
            status: "corrupt".into(),
            schema_version: None,
            integrity: "failed".into(),
            reason: "profile database integrity check failed; original data was preserved".into(),
            recovery: "preserve the original; inspect read-only or create a new named profile"
                .into(),
        },
        Err(StoreError::UnsupportedSchema(version)) => StoreInspection {
            status: "unsupported".into(),
            schema_version: Some(version),
            integrity: "ok".into(),
            reason: "the database schema is newer than this application".into(),
            recovery: "use a newer Ferric Browser or create a new named profile".into(),
        },
        Err(_) => unavailable_inspection("read-only profile inspection failed"),
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
    let mut found_base = false;
    let mut found_current = false;
    for migration in rows {
        let (version, checksum) = migration?;
        found_base |= version == BASE_SCHEMA_VERSION;
        found_current |= version == SCHEMA_VERSION;
        let expected = match version {
            BASE_SCHEMA_VERSION => Some(BASE_SCHEMA_CHECKSUM),
            SCHEMA_VERSION => Some(versioning::schema::CHECKSUM),
            _ => None,
        };
        if expected.is_some_and(|expected| checksum != expected) {
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
    if found_current && !found_base {
        return Err(StoreError::Corrupt(
            "schema migration chain is incomplete".into(),
        ));
    }
    if found_current && !versioning::schema::is_present(&connection)? {
        return Err(StoreError::Corrupt(
            "versioning index tables are missing".into(),
        ));
    }
    Ok(version)
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

fn migrate(connection: &Connection, existing_version: Option<i64>) -> Result<(), rusqlite::Error> {
    if existing_version == Some(SCHEMA_VERSION) {
        return Ok(());
    }
    connection.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        // The read-only preflight can predate another owner's completed
        // migration. Read the authoritative version only after taking the
        // write lock, then adopt a valid schema instead of replaying CREATE.
        let has_migrations: bool = connection
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_migrations'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some();
        let current_version: Option<i64> = if has_migrations {
            connection.query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })?
        } else {
            None
        };
        match current_version {
            Some(SCHEMA_VERSION | BASE_SCHEMA_VERSION) => {
                let checksum: String = connection.query_row(
                    "SELECT checksum FROM schema_migrations WHERE version = ?1",
                    [BASE_SCHEMA_VERSION],
                    |row| row.get(0),
                )?;
                if checksum != BASE_SCHEMA_CHECKSUM {
                    return Err(rusqlite::Error::InvalidQuery);
                }
            }
            None if existing_version.is_none() => install_base_schema(connection)?,
            _ => return Err(rusqlite::Error::InvalidQuery),
        }
        if current_version == Some(SCHEMA_VERSION) {
            let checksum: String = connection.query_row(
                "SELECT checksum FROM schema_migrations WHERE version = ?1",
                [SCHEMA_VERSION],
                |row| row.get(0),
            )?;
            if checksum != versioning::schema::CHECKSUM
                || !versioning::schema::is_present(connection)?
            {
                return Err(rusqlite::Error::InvalidQuery);
            }
        } else {
            versioning::schema::install(connection)?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => connection.execute_batch("COMMIT"),
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

fn install_base_schema(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
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
             VALUES (3, 'builtin-schema-3', strftime('%s','now'));",
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
mod tests;
