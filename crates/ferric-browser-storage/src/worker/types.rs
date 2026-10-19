//! Commands, admission checks and typed completions.
use super::{
    BookmarkRecord, ClosedTabSnapshot, DownloadRecord, DownloadUpdate, HistoryRecord,
    JourneyNodeRecord, JourneyWrite, MAX_DOWNLOAD_BATCH, MAX_HISTORY_BATCH, MAX_LIBRARY_LIMIT,
    MAX_MARK_BATCH, MAX_PERMISSION_BATCH, MarkWrite, PathBuf, PermissionRule,
    PermissionRulesSnapshot, Quickmark, RestorePlanEntry, SessionSnapshot, Uuid, VisitInput,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileLibrarySnapshot {
    pub history: Vec<HistoryRecord>,
    pub command_history: Vec<String>,
    pub bookmarks: Vec<BookmarkRecord>,
    pub quickmarks: Vec<Quickmark>,
    pub downloads: Vec<DownloadRecord>,
    pub permissions: Vec<PermissionRule>,
    pub permissions_complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageWorkerError {
    Busy,
    QueueFull,
    Stopped,
    Spawn(String),
    InvalidLimit,
    InvalidBatch,
    InvalidPermissionBatch,
    InvalidPermissionOrigin,
    InvalidDownloadBatch,
    InvalidMarkBatch,
}

impl std::fmt::Display for StorageWorkerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => formatter.write_str("a profile metadata query is already pending"),
            Self::QueueFull => formatter.write_str("profile metadata worker queue is full"),
            Self::Stopped => formatter.write_str("profile metadata worker has stopped"),
            Self::Spawn(reason) => write!(
                formatter,
                "profile metadata worker could not start: {reason}"
            ),
            Self::InvalidLimit => formatter.write_str("profile metadata query limit is invalid"),
            Self::InvalidBatch => formatter.write_str("history write batch is invalid"),
            Self::InvalidPermissionBatch => {
                formatter.write_str("permission write batch is invalid")
            }
            Self::InvalidPermissionOrigin => formatter.write_str("permission origin is invalid"),
            Self::InvalidDownloadBatch => formatter.write_str("download write batch is invalid"),
            Self::InvalidMarkBatch => {
                formatter.write_str("bookmark/quickmark write batch is invalid")
            }
        }
    }
}

impl std::error::Error for StorageWorkerError {}

#[derive(Clone, Debug, PartialEq)]
pub enum StorageCommand {
    StartPage {
        now: i64,
        retention_before: i64,
        rank: bool,
    },
    StartPageWrite {
        mutation: crate::StartPageMutation,
        timestamp: i64,
    },
    Library {
        limit: usize,
    },
    SessionList {
        root: PathBuf,
        profile_id: Uuid,
    },
    SessionRestore {
        paths: Vec<PathBuf>,
    },
    SessionRecovery {
        root: PathBuf,
        profile_id: Uuid,
    },
    SessionCheckpointClear {
        root: PathBuf,
        profile_id: Uuid,
    },
    SessionSave {
        path: PathBuf,
        snapshot: SessionSnapshot,
    },
    SessionDelete {
        root: PathBuf,
        profile_id: Uuid,
        name: String,
    },
    HistoryBatch {
        visits: Vec<VisitInput>,
    },
    CommandRecord {
        command: String,
        timestamp: i64,
    },
    PermissionBatch {
        rules: Vec<PermissionRule>,
    },
    PermissionLookup {
        origin: String,
    },
    PermissionReset {
        origin: String,
        permissions: Vec<String>,
    },
    DownloadSave {
        records: Vec<DownloadRecord>,
    },
    DownloadBatch {
        updates: Vec<DownloadUpdate>,
    },
    DownloadDestination {
        id: String,
        destination: String,
    },
    DownloadCreate {
        id: String,
        source_url: String,
        destination: String,
        state: crate::DownloadState,
        created_at: i64,
    },
    JourneyWrite {
        write: JourneyWrite,
    },
    JourneyNode {
        id: String,
    },
    JourneyQuery {
        current_tab_id: Option<String>,
        search: Option<String>,
        expand: Option<String>,
    },
    JourneyExport,
    MarkBatch {
        writes: Vec<MarkWrite>,
    },
    HistoryClear {
        since: Option<i64>,
        origin: Option<String>,
    },
    Flush,
}

pub(super) enum WorkerMessage {
    Execute(Box<StorageCommand>),
    Shutdown,
}

/// Validated restore data returned by the metadata worker. Closed-tab
/// descriptors are carried separately so normal session loads can keep their
/// existing semantics while crash recovery can restore profile-local undo.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionRestoreSnapshot {
    pub entries: Vec<RestorePlanEntry>,
    pub closed_tabs: Vec<ClosedTabSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JourneyQuerySnapshot {
    pub nodes: Vec<JourneyNodeRecord>,
    pub edges: Vec<crate::JourneyEdgeRecord>,
}

/// One typed storage outcome emitted by the single profile metadata owner.
///
/// Application code consumes this stream instead of coupling to the worker's
/// internal response channel.
/// The durable operation that produced a completion or failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StorageOperation {
    StartPage,
    StartPageWrite,
    Library,
    SessionList,
    SessionRestore,
    SessionSave,
    SessionDelete,
    SessionCheckpointClear,
    HistoryBatch,
    CommandRecord,
    PermissionBatch,
    PermissionLookup,
    PermissionReset,
    DownloadSave,
    DownloadBatch,
    DownloadDestination,
    DownloadCreate,
    JourneyWrite,
    JourneyNode,
    JourneyQuery,
    JourneyExport,
    MarkWrite,
    HistoryClear,
    Flush,
}

impl StorageCommand {
    #[must_use]
    pub const fn operation(&self) -> StorageOperation {
        match self {
            Self::StartPage { .. } => StorageOperation::StartPage,
            Self::StartPageWrite { .. } => StorageOperation::StartPageWrite,
            Self::Library { .. } => StorageOperation::Library,
            Self::SessionList { .. } => StorageOperation::SessionList,
            Self::SessionRestore { .. } | Self::SessionRecovery { .. } => {
                StorageOperation::SessionRestore
            }
            Self::SessionSave { .. } => StorageOperation::SessionSave,
            Self::SessionDelete { .. } => StorageOperation::SessionDelete,
            Self::SessionCheckpointClear { .. } => StorageOperation::SessionCheckpointClear,
            Self::HistoryBatch { .. } => StorageOperation::HistoryBatch,
            Self::CommandRecord { .. } => StorageOperation::CommandRecord,
            Self::PermissionBatch { .. } => StorageOperation::PermissionBatch,
            Self::PermissionLookup { .. } => StorageOperation::PermissionLookup,
            Self::PermissionReset { .. } => StorageOperation::PermissionReset,
            Self::DownloadSave { .. } => StorageOperation::DownloadSave,
            Self::DownloadBatch { .. } => StorageOperation::DownloadBatch,
            Self::DownloadDestination { .. } => StorageOperation::DownloadDestination,
            Self::DownloadCreate { .. } => StorageOperation::DownloadCreate,
            Self::JourneyWrite { .. } => StorageOperation::JourneyWrite,
            Self::JourneyNode { .. } => StorageOperation::JourneyNode,
            Self::JourneyQuery { .. } => StorageOperation::JourneyQuery,
            Self::JourneyExport => StorageOperation::JourneyExport,
            Self::MarkBatch { .. } => StorageOperation::MarkWrite,
            Self::HistoryClear { .. } => StorageOperation::HistoryClear,
            Self::Flush => StorageOperation::Flush,
        }
    }

    pub(super) fn validate(&self) -> Result<(), StorageWorkerError> {
        match self {
            Self::Library { limit } if !(1..=MAX_LIBRARY_LIMIT).contains(limit) => {
                Err(StorageWorkerError::InvalidLimit)
            }
            Self::PermissionLookup { origin }
                if crate::normalize_permission_origin(origin).is_err() =>
            {
                Err(StorageWorkerError::InvalidPermissionOrigin)
            }
            Self::HistoryBatch { visits }
                if visits.is_empty()
                    || visits.len() > MAX_HISTORY_BATCH
                    || visits.iter().any(|visit| {
                        crate::validate_visit_fields(&visit.url, &visit.title, &visit.transition)
                            .is_err()
                    }) =>
            {
                Err(StorageWorkerError::InvalidBatch)
            }
            Self::CommandRecord { command, .. }
                if command.is_empty() || command.len() > 64 * 1024 =>
            {
                Err(StorageWorkerError::InvalidBatch)
            }
            Self::PermissionBatch { rules }
                if rules.is_empty() || rules.len() > MAX_PERMISSION_BATCH =>
            {
                Err(StorageWorkerError::InvalidPermissionBatch)
            }
            Self::PermissionReset {
                origin,
                permissions,
            } if origin.is_empty() || permissions.is_empty() || permissions.len() > 2 => {
                Err(StorageWorkerError::InvalidPermissionBatch)
            }
            Self::DownloadSave { records }
                if crate::store_downloads::validate_download_records(records).is_err() =>
            {
                Err(StorageWorkerError::InvalidDownloadBatch)
            }
            Self::DownloadBatch { updates }
                if updates.is_empty()
                    || updates.len() > MAX_DOWNLOAD_BATCH
                    || updates.iter().any(|update| {
                        !crate::valid_stored_text(&update.id, crate::MAX_STORED_IDENTIFIER_BYTES)
                            || update.bytes_received < 0
                    }) =>
            {
                Err(StorageWorkerError::InvalidDownloadBatch)
            }
            Self::MarkBatch { writes } if writes.is_empty() || writes.len() > MAX_MARK_BATCH => {
                Err(StorageWorkerError::InvalidMarkBatch)
            }
            Self::SessionRestore { paths } if paths.is_empty() => {
                Err(StorageWorkerError::InvalidBatch)
            }
            _ => Ok(()),
        }
    }
}

/// A structured storage failure surfaced by the application completion stream.
///
/// The message remains suitable for diagnostics, while `operation` lets
/// callers choose recovery and presentation behavior without parsing prose.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageOperationError {
    pub operation: StorageOperation,
    pub message: String,
}

impl std::fmt::Display for StorageOperationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.operation, self.message)
    }
}

impl std::error::Error for StorageOperationError {}

#[derive(Debug)]
pub enum StorageCompletion {
    StartPage(Result<Box<crate::StartPageSnapshot>, StorageOperationError>),
    StartPageWrite(Result<(), StorageOperationError>),
    Library(Result<ProfileLibrarySnapshot, StorageOperationError>),
    SessionList(Result<Vec<String>, StorageOperationError>),
    SessionRestore(Result<SessionRestoreSnapshot, StorageOperationError>),
    SessionSave(Result<(), StorageOperationError>),
    SessionDelete(Result<(), StorageOperationError>),
    SessionCheckpointClear(Result<(), StorageOperationError>),
    HistoryBatch(Result<usize, StorageOperationError>),
    CommandRecord(Result<bool, StorageOperationError>),
    PermissionBatch(Result<usize, StorageOperationError>),
    PermissionLookup(Result<PermissionRulesSnapshot, StorageOperationError>),
    PermissionReset(Result<bool, StorageOperationError>),
    DownloadSave(Result<usize, StorageOperationError>),
    DownloadBatch(Result<usize, StorageOperationError>),
    DownloadDestination(Result<(), StorageOperationError>),
    DownloadCreate(Result<(), StorageOperationError>),
    JourneyWrite(Result<bool, StorageOperationError>),
    JourneyNode(Result<Option<JourneyNodeRecord>, StorageOperationError>),
    JourneyQuery(Result<JourneyQuerySnapshot, StorageOperationError>),
    JourneyExport(Result<JourneyQuerySnapshot, StorageOperationError>),
    MarkWrite(Result<usize, StorageOperationError>),
    HistoryClear(Result<u64, StorageOperationError>),
    Flush(Result<(), StorageOperationError>),
}

impl StorageCompletion {
    /// Identifies the durable operation that produced this completion.
    #[must_use]
    pub const fn operation(&self) -> StorageOperation {
        match self {
            Self::StartPage(_) => StorageOperation::StartPage,
            Self::StartPageWrite(_) => StorageOperation::StartPageWrite,
            Self::Library(_) => StorageOperation::Library,
            Self::SessionList(_) => StorageOperation::SessionList,
            Self::SessionRestore(_) => StorageOperation::SessionRestore,
            Self::SessionSave(_) => StorageOperation::SessionSave,
            Self::SessionDelete(_) => StorageOperation::SessionDelete,
            Self::SessionCheckpointClear(_) => StorageOperation::SessionCheckpointClear,
            Self::HistoryBatch(_) => StorageOperation::HistoryBatch,
            Self::CommandRecord(_) => StorageOperation::CommandRecord,
            Self::PermissionBatch(_) => StorageOperation::PermissionBatch,
            Self::PermissionLookup(_) => StorageOperation::PermissionLookup,
            Self::PermissionReset(_) => StorageOperation::PermissionReset,
            Self::DownloadSave(_) => StorageOperation::DownloadSave,
            Self::DownloadBatch(_) => StorageOperation::DownloadBatch,
            Self::DownloadDestination(_) => StorageOperation::DownloadDestination,
            Self::DownloadCreate(_) => StorageOperation::DownloadCreate,
            Self::JourneyWrite(_) => StorageOperation::JourneyWrite,
            Self::JourneyNode(_) => StorageOperation::JourneyNode,
            Self::JourneyQuery(_) => StorageOperation::JourneyQuery,
            Self::JourneyExport(_) => StorageOperation::JourneyExport,
            Self::MarkWrite(_) => StorageOperation::MarkWrite,
            Self::HistoryClear(_) => StorageOperation::HistoryClear,
            Self::Flush(_) => StorageOperation::Flush,
        }
    }

    /// Builds the operation-specific failure used when an accepted request
    /// terminates before the worker can execute it.
    #[must_use]
    pub fn failed(operation: StorageOperation, message: impl Into<String>) -> Self {
        let error = StorageOperationError {
            operation,
            message: message.into(),
        };
        match operation {
            StorageOperation::StartPage => Self::StartPage(Err(error)),
            StorageOperation::StartPageWrite => Self::StartPageWrite(Err(error)),
            StorageOperation::Library => Self::Library(Err(error)),
            StorageOperation::SessionList => Self::SessionList(Err(error)),
            StorageOperation::SessionRestore => Self::SessionRestore(Err(error)),
            StorageOperation::SessionSave => Self::SessionSave(Err(error)),
            StorageOperation::SessionDelete => Self::SessionDelete(Err(error)),
            StorageOperation::SessionCheckpointClear => Self::SessionCheckpointClear(Err(error)),
            StorageOperation::HistoryBatch => Self::HistoryBatch(Err(error)),
            StorageOperation::CommandRecord => Self::CommandRecord(Err(error)),
            StorageOperation::PermissionBatch => Self::PermissionBatch(Err(error)),
            StorageOperation::PermissionLookup => Self::PermissionLookup(Err(error)),
            StorageOperation::PermissionReset => Self::PermissionReset(Err(error)),
            StorageOperation::DownloadSave => Self::DownloadSave(Err(error)),
            StorageOperation::DownloadBatch => Self::DownloadBatch(Err(error)),
            StorageOperation::DownloadDestination => Self::DownloadDestination(Err(error)),
            StorageOperation::DownloadCreate => Self::DownloadCreate(Err(error)),
            StorageOperation::JourneyWrite => Self::JourneyWrite(Err(error)),
            StorageOperation::JourneyNode => Self::JourneyNode(Err(error)),
            StorageOperation::JourneyQuery => Self::JourneyQuery(Err(error)),
            StorageOperation::JourneyExport => Self::JourneyExport(Err(error)),
            StorageOperation::MarkWrite => Self::MarkWrite(Err(error)),
            StorageOperation::HistoryClear => Self::HistoryClear(Err(error)),
            StorageOperation::Flush => Self::Flush(Err(error)),
        }
    }
}
