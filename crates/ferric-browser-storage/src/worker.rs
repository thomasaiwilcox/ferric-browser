//! Bounded background ownership for read-heavy profile metadata queries.

#![allow(
    clippy::struct_excessive_bools,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    thread::{self, JoinHandle},
    time::Duration,
};

use super::{
    BookmarkRecord, ClosedTabSnapshot, DownloadRecord, DownloadUpdate, HistoryRecord,
    JourneyNodeRecord, JourneyWrite, MarkWrite, PermissionRule, ProfileStore, Quickmark,
    RestorePlanEntry, SessionSnapshot, StoreMode, VisitInput, clear_current_session_checkpoints,
    current_session_paths, delete_named_session, list_named_sessions, load_session,
    save_session_atomic,
};
use uuid::Uuid;

const QUEUE_CAPACITY: usize = 1;
const COMPLETION_CAPACITY: usize = 32;
const MAX_LIBRARY_LIMIT: usize = 1_000;
const MAX_HISTORY_BATCH: usize = 128;
const MAX_PERMISSION_BATCH: usize = 16;
const MAX_DOWNLOAD_BATCH: usize = 32;
const MAX_MARK_BATCH: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileLibrarySnapshot {
    pub history: Vec<HistoryRecord>,
    pub bookmarks: Vec<BookmarkRecord>,
    pub quickmarks: Vec<Quickmark>,
    pub downloads: Vec<DownloadRecord>,
    pub permissions: Vec<PermissionRule>,
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
    PermissionBatch {
        rules: Vec<PermissionRule>,
    },
    PermissionReset {
        origin: String,
        permissions: Vec<String>,
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
        state: super::DownloadState,
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

enum WorkerMessage {
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
    pub edges: Vec<super::JourneyEdgeRecord>,
}

/// One typed storage outcome emitted by the single profile metadata owner.
///
/// Application code consumes this stream instead of coupling to the worker's
/// internal response channel.
/// The durable operation that produced a completion or failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum StorageOperation {
    Library,
    SessionList,
    SessionRestore,
    SessionSave,
    SessionDelete,
    SessionCheckpointClear,
    HistoryBatch,
    PermissionBatch,
    PermissionReset,
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
            Self::Library { .. } => StorageOperation::Library,
            Self::SessionList { .. } => StorageOperation::SessionList,
            Self::SessionRestore { .. } | Self::SessionRecovery { .. } => {
                StorageOperation::SessionRestore
            }
            Self::SessionSave { .. } => StorageOperation::SessionSave,
            Self::SessionDelete { .. } => StorageOperation::SessionDelete,
            Self::SessionCheckpointClear { .. } => StorageOperation::SessionCheckpointClear,
            Self::HistoryBatch { .. } => StorageOperation::HistoryBatch,
            Self::PermissionBatch { .. } => StorageOperation::PermissionBatch,
            Self::PermissionReset { .. } => StorageOperation::PermissionReset,
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

    fn validate(&self) -> Result<(), StorageWorkerError> {
        match self {
            Self::Library { limit } if !(1..=MAX_LIBRARY_LIMIT).contains(limit) => {
                Err(StorageWorkerError::InvalidLimit)
            }
            Self::HistoryBatch { visits }
                if visits.is_empty() || visits.len() > MAX_HISTORY_BATCH =>
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
            Self::DownloadBatch { updates }
                if updates.is_empty() || updates.len() > MAX_DOWNLOAD_BATCH =>
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
    Library(Result<ProfileLibrarySnapshot, StorageOperationError>),
    SessionList(Result<Vec<String>, StorageOperationError>),
    SessionRestore(Result<SessionRestoreSnapshot, StorageOperationError>),
    SessionSave(Result<(), StorageOperationError>),
    SessionDelete(Result<(), StorageOperationError>),
    SessionCheckpointClear(Result<(), StorageOperationError>),
    HistoryBatch(Result<usize, StorageOperationError>),
    PermissionBatch(Result<usize, StorageOperationError>),
    PermissionReset(Result<bool, StorageOperationError>),
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
            Self::Library(_) => StorageOperation::Library,
            Self::SessionList(_) => StorageOperation::SessionList,
            Self::SessionRestore(_) => StorageOperation::SessionRestore,
            Self::SessionSave(_) => StorageOperation::SessionSave,
            Self::SessionDelete(_) => StorageOperation::SessionDelete,
            Self::SessionCheckpointClear(_) => StorageOperation::SessionCheckpointClear,
            Self::HistoryBatch(_) => StorageOperation::HistoryBatch,
            Self::PermissionBatch(_) => StorageOperation::PermissionBatch,
            Self::PermissionReset(_) => StorageOperation::PermissionReset,
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
            StorageOperation::Library => Self::Library(Err(error)),
            StorageOperation::SessionList => Self::SessionList(Err(error)),
            StorageOperation::SessionRestore => Self::SessionRestore(Err(error)),
            StorageOperation::SessionSave => Self::SessionSave(Err(error)),
            StorageOperation::SessionDelete => Self::SessionDelete(Err(error)),
            StorageOperation::SessionCheckpointClear => Self::SessionCheckpointClear(Err(error)),
            StorageOperation::HistoryBatch => Self::HistoryBatch(Err(error)),
            StorageOperation::PermissionBatch => Self::PermissionBatch(Err(error)),
            StorageOperation::PermissionReset => Self::PermissionReset(Err(error)),
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

/// Owns one SQLite connection on one bounded background thread.
///
/// The handle is intentionally single-flight: a caller must consume the
/// previous result before submitting another query. This prevents a fast UI
/// typing sequence from growing an unbounded database backlog.
pub struct ProfileStoreWorker {
    sender: Option<SyncSender<WorkerMessage>>,
    completion_receiver: Receiver<StorageCompletion>,
    ready: BTreeMap<StorageOperation, StorageCompletion>,
    pending: BTreeSet<StorageOperation>,
    disconnected: bool,
    join: Option<JoinHandle<()>>,
}

impl ProfileStoreWorker {
    /// Starts the worker with one bounded command channel and one typed
    /// completion channel. Database opening remains confined to the worker.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker thread cannot be spawned.
    pub fn spawn(path: impl AsRef<Path>) -> Result<Self, StorageWorkerError> {
        let path = path.as_ref().to_owned();
        let (sender, requests) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (completion_sender, completion_receiver) = mpsc::sync_channel(COMPLETION_CAPACITY);
        let join = thread::Builder::new()
            .name("ferric-browser-profile-metadata".into())
            .spawn(move || run_worker(path, requests, completion_sender))
            .map_err(|error| StorageWorkerError::Spawn(error.to_string()))?;
        Ok(Self {
            sender: Some(sender),
            completion_receiver,
            ready: BTreeMap::new(),
            pending: BTreeSet::new(),
            disconnected: false,
            join: Some(join),
        })
    }

    fn submit(
        &mut self,
        operation: StorageOperation,
        request: StorageCommand,
    ) -> Result<(), StorageWorkerError> {
        if self.pending.contains(&operation) {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(WorkerMessage::Execute(Box::new(request))) {
            Ok(()) => {
                self.pending.insert(operation);
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    /// Submits one validated command to the bounded worker.
    pub fn try_submit(&mut self, command: StorageCommand) -> Result<(), StorageWorkerError> {
        command.validate()?;
        self.submit(command.operation(), command)
    }

    fn record_completion(&mut self, completion: StorageCompletion) {
        let operation = completion.operation();
        self.pending.remove(&operation);
        self.ready.insert(operation, completion);
    }

    fn record_disconnect(&mut self) {
        if self.disconnected {
            return;
        }
        self.disconnected = true;
        self.sender = None;
        for operation in std::mem::take(&mut self.pending) {
            self.ready.insert(
                operation,
                StorageCompletion::failed(operation, "profile metadata worker disconnected"),
            );
        }
    }

    #[cfg(test)]
    fn poll_operation(&mut self, operation: StorageOperation) -> Option<StorageCompletion> {
        if let Some(completion) = self.ready.remove(&operation) {
            return Some(completion);
        }
        loop {
            match self.completion_receiver.try_recv() {
                Ok(completion) if completion.operation() == operation => {
                    self.pending.remove(&operation);
                    return Some(completion);
                }
                Ok(completion) => self.record_completion(completion),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    self.record_disconnect();
                    return self.ready.remove(&operation);
                }
            }
        }
    }

    fn wait_operation(
        &mut self,
        operation: StorageOperation,
        timeout: Duration,
    ) -> Option<StorageCompletion> {
        if !self.pending.contains(&operation) && !self.ready.contains_key(&operation) {
            return None;
        }
        if let Some(completion) = self.ready.remove(&operation) {
            return Some(completion);
        }
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return None;
            }
            match self.completion_receiver.recv_timeout(remaining) {
                Ok(completion) if completion.operation() == operation => {
                    self.pending.remove(&operation);
                    return Some(completion);
                }
                Ok(completion) => self.record_completion(completion),
                Err(mpsc::RecvTimeoutError::Timeout) => return None,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.record_disconnect();
                    return self.ready.remove(&operation);
                }
            }
        }
    }

    /// Drains all currently available typed completions exactly once.
    #[must_use]
    pub fn poll_events(&mut self) -> Vec<StorageCompletion> {
        let mut completions = std::mem::take(&mut self.ready)
            .into_values()
            .collect::<Vec<_>>();
        loop {
            match self.completion_receiver.try_recv() {
                Ok(completion) => {
                    self.pending.remove(&completion.operation());
                    completions.push(completion);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.record_disconnect();
                    completions.extend(std::mem::take(&mut self.ready).into_values());
                    break;
                }
            }
        }
        completions
    }
}

#[cfg(test)]
impl ProfileStoreWorker {
    pub fn request_library(&mut self, limit: usize) -> Result<(), StorageWorkerError> {
        if !(1..=MAX_LIBRARY_LIMIT).contains(&limit) {
            return Err(StorageWorkerError::InvalidLimit);
        }
        self.submit(StorageOperation::Library, StorageCommand::Library { limit })
    }

    pub fn poll_library(&mut self) -> Option<Result<ProfileLibrarySnapshot, String>> {
        match self.poll_operation(StorageOperation::Library)? {
            StorageCompletion::Library(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn request_session_list(
        &mut self,
        root: PathBuf,
        profile_id: Uuid,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::SessionList,
            StorageCommand::SessionList { root, profile_id },
        )
    }

    pub fn poll_session_list(&mut self) -> Option<Result<Vec<String>, String>> {
        match self.poll_operation(StorageOperation::SessionList)? {
            StorageCompletion::SessionList(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn session_list_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::SessionList)
    }

    pub fn request_session_restore(&mut self, path: PathBuf) -> Result<(), StorageWorkerError> {
        self.request_session_restore_paths(vec![path])
    }

    pub fn request_session_restore_paths(
        &mut self,
        paths: Vec<PathBuf>,
    ) -> Result<(), StorageWorkerError> {
        if paths.is_empty() {
            return Err(StorageWorkerError::QueueFull);
        }
        self.submit(
            StorageOperation::SessionRestore,
            StorageCommand::SessionRestore { paths },
        )
    }

    pub fn request_session_recovery(
        &mut self,
        root: PathBuf,
        profile_id: Uuid,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::SessionRestore,
            StorageCommand::SessionRecovery { root, profile_id },
        )
    }

    pub fn poll_session_restore(&mut self) -> Option<Result<SessionRestoreSnapshot, String>> {
        match self.poll_operation(StorageOperation::SessionRestore)? {
            StorageCompletion::SessionRestore(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn session_restore_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::SessionRestore)
    }

    pub fn request_session_save(
        &mut self,
        path: PathBuf,
        snapshot: SessionSnapshot,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::SessionSave,
            StorageCommand::SessionSave { path, snapshot },
        )
    }

    pub fn poll_session_save(&mut self) -> Option<Result<(), String>> {
        match self.poll_operation(StorageOperation::SessionSave)? {
            StorageCompletion::SessionSave(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn session_save_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::SessionSave)
    }

    pub fn request_session_delete(
        &mut self,
        root: PathBuf,
        profile_id: Uuid,
        name: String,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::SessionDelete,
            StorageCommand::SessionDelete {
                root,
                profile_id,
                name,
            },
        )
    }

    pub fn poll_session_delete(&mut self) -> Option<Result<(), String>> {
        match self.poll_operation(StorageOperation::SessionDelete)? {
            StorageCompletion::SessionDelete(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn request_session_checkpoint_clear(
        &mut self,
        root: PathBuf,
        profile_id: Uuid,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::SessionCheckpointClear,
            StorageCommand::SessionCheckpointClear { root, profile_id },
        )
    }

    pub fn poll_session_checkpoint_clear(&mut self) -> Option<Result<(), String>> {
        match self.poll_operation(StorageOperation::SessionCheckpointClear)? {
            StorageCompletion::SessionCheckpointClear(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn request_history_batch(
        &mut self,
        visits: Vec<VisitInput>,
    ) -> Result<(), StorageWorkerError> {
        if visits.is_empty() || visits.len() > MAX_HISTORY_BATCH {
            return Err(StorageWorkerError::InvalidBatch);
        }
        self.submit(
            StorageOperation::HistoryBatch,
            StorageCommand::HistoryBatch { visits },
        )
    }

    pub fn poll_history_batch(&mut self) -> Option<Result<usize, String>> {
        match self.poll_operation(StorageOperation::HistoryBatch)? {
            StorageCompletion::HistoryBatch(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn request_permission_batch(
        &mut self,
        rules: Vec<PermissionRule>,
    ) -> Result<(), StorageWorkerError> {
        if rules.is_empty() || rules.len() > MAX_PERMISSION_BATCH {
            return Err(StorageWorkerError::InvalidPermissionBatch);
        }
        self.submit(
            StorageOperation::PermissionBatch,
            StorageCommand::PermissionBatch { rules },
        )
    }

    pub fn poll_permission_batch(&mut self) -> Option<Result<usize, String>> {
        match self.poll_operation(StorageOperation::PermissionBatch)? {
            StorageCompletion::PermissionBatch(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn request_permission_reset(
        &mut self,
        origin: String,
        permissions: Vec<String>,
    ) -> Result<(), StorageWorkerError> {
        if origin.is_empty() || permissions.is_empty() || permissions.len() > 2 {
            return Err(StorageWorkerError::InvalidPermissionBatch);
        }
        self.submit(
            StorageOperation::PermissionReset,
            StorageCommand::PermissionReset {
                origin,
                permissions,
            },
        )
    }

    pub fn poll_permission_reset(&mut self) -> Option<Result<bool, String>> {
        match self.poll_operation(StorageOperation::PermissionReset)? {
            StorageCompletion::PermissionReset(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn request_download_batch(
        &mut self,
        updates: Vec<DownloadUpdate>,
    ) -> Result<(), StorageWorkerError> {
        if updates.is_empty() || updates.len() > MAX_DOWNLOAD_BATCH {
            return Err(StorageWorkerError::InvalidDownloadBatch);
        }
        self.submit(
            StorageOperation::DownloadBatch,
            StorageCommand::DownloadBatch { updates },
        )
    }

    pub fn poll_download_batch(&mut self) -> Option<Result<usize, String>> {
        match self.poll_operation(StorageOperation::DownloadBatch)? {
            StorageCompletion::DownloadBatch(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn request_download_destination(
        &mut self,
        id: String,
        destination: String,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::DownloadDestination,
            StorageCommand::DownloadDestination { id, destination },
        )
    }

    pub fn poll_download_destination(&mut self) -> Option<Result<(), String>> {
        match self.poll_operation(StorageOperation::DownloadDestination)? {
            StorageCompletion::DownloadDestination(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_download_destination(&mut self, timeout: Duration) -> Option<Result<(), String>> {
        match self.wait_operation(StorageOperation::DownloadDestination, timeout)? {
            StorageCompletion::DownloadDestination(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn download_destination_pending(&self) -> bool {
        self.pending
            .contains(&StorageOperation::DownloadDestination)
    }

    pub fn request_download_create(
        &mut self,
        id: String,
        source_url: String,
        destination: String,
        state: super::DownloadState,
        created_at: i64,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::DownloadCreate,
            StorageCommand::DownloadCreate {
                id,
                source_url,
                destination,
                state,
                created_at,
            },
        )
    }

    pub fn poll_download_create(&mut self) -> Option<Result<(), String>> {
        match self.poll_operation(StorageOperation::DownloadCreate)? {
            StorageCompletion::DownloadCreate(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_download_create(&mut self, timeout: Duration) -> Option<Result<(), String>> {
        match self.wait_operation(StorageOperation::DownloadCreate, timeout)? {
            StorageCompletion::DownloadCreate(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn download_create_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::DownloadCreate)
    }

    pub fn request_journey_write(&mut self, write: JourneyWrite) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::JourneyWrite,
            StorageCommand::JourneyWrite { write },
        )
    }

    pub fn poll_journey_write(&mut self) -> Option<Result<bool, String>> {
        match self.poll_operation(StorageOperation::JourneyWrite)? {
            StorageCompletion::JourneyWrite(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_journey_write(&mut self, timeout: Duration) -> Option<Result<bool, String>> {
        match self.wait_operation(StorageOperation::JourneyWrite, timeout)? {
            StorageCompletion::JourneyWrite(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn journey_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::JourneyWrite)
    }

    pub fn request_journey_node(&mut self, id: String) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::JourneyNode,
            StorageCommand::JourneyNode { id },
        )
    }

    pub fn poll_journey_node(&mut self) -> Option<Result<Option<JourneyNodeRecord>, String>> {
        match self.poll_operation(StorageOperation::JourneyNode)? {
            StorageCompletion::JourneyNode(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_journey_node(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<Option<JourneyNodeRecord>, String>> {
        match self.wait_operation(StorageOperation::JourneyNode, timeout)? {
            StorageCompletion::JourneyNode(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn journey_node_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::JourneyNode)
    }

    pub fn request_journey_query(
        &mut self,
        current_tab_id: Option<String>,
        search: Option<String>,
        expand: Option<String>,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::JourneyQuery,
            StorageCommand::JourneyQuery {
                current_tab_id,
                search,
                expand,
            },
        )
    }

    pub fn poll_journey_query(&mut self) -> Option<Result<JourneyQuerySnapshot, String>> {
        match self.poll_operation(StorageOperation::JourneyQuery)? {
            StorageCompletion::JourneyQuery(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_journey_query(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<JourneyQuerySnapshot, String>> {
        match self.wait_operation(StorageOperation::JourneyQuery, timeout)? {
            StorageCompletion::JourneyQuery(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn journey_query_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::JourneyQuery)
    }

    pub fn request_journey_export(&mut self) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::JourneyExport,
            StorageCommand::JourneyExport,
        )
    }

    pub fn poll_journey_export(&mut self) -> Option<Result<JourneyQuerySnapshot, String>> {
        match self.poll_operation(StorageOperation::JourneyExport)? {
            StorageCompletion::JourneyExport(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_journey_export(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<JourneyQuerySnapshot, String>> {
        match self.wait_operation(StorageOperation::JourneyExport, timeout)? {
            StorageCompletion::JourneyExport(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn journey_export_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::JourneyExport)
    }

    pub fn request_mark_write(&mut self, write: MarkWrite) -> Result<(), StorageWorkerError> {
        self.request_mark_writes(vec![write])
    }

    pub fn request_mark_writes(
        &mut self,
        writes: Vec<MarkWrite>,
    ) -> Result<(), StorageWorkerError> {
        if writes.is_empty() || writes.len() > MAX_MARK_BATCH {
            return Err(StorageWorkerError::InvalidMarkBatch);
        }
        self.submit(
            StorageOperation::MarkWrite,
            StorageCommand::MarkBatch { writes },
        )
    }

    pub fn poll_mark_write(&mut self) -> Option<Result<usize, String>> {
        match self.poll_operation(StorageOperation::MarkWrite)? {
            StorageCompletion::MarkWrite(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_mark_write(&mut self, timeout: Duration) -> Option<Result<usize, String>> {
        match self.wait_operation(StorageOperation::MarkWrite, timeout)? {
            StorageCompletion::MarkWrite(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn mark_write_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::MarkWrite)
    }

    pub fn request_history_clear(
        &mut self,
        since: Option<i64>,
        origin: Option<String>,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::HistoryClear,
            StorageCommand::HistoryClear { since, origin },
        )
    }

    pub fn poll_history_clear(&mut self) -> Option<Result<u64, String>> {
        match self.poll_operation(StorageOperation::HistoryClear)? {
            StorageCompletion::HistoryClear(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_history_clear(&mut self, timeout: Duration) -> Option<Result<u64, String>> {
        match self.wait_operation(StorageOperation::HistoryClear, timeout)? {
            StorageCompletion::HistoryClear(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn history_clear_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::HistoryClear)
    }
}

impl ProfileStoreWorker {
    pub fn request_flush(&mut self) -> Result<(), StorageWorkerError> {
        self.submit(StorageOperation::Flush, StorageCommand::Flush)
    }

    #[cfg(test)]
    pub fn poll_flush(&mut self) -> Option<Result<(), String>> {
        match self.poll_operation(StorageOperation::Flush)? {
            StorageCompletion::Flush(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_flush(&mut self, timeout: Duration) -> Option<Result<(), String>> {
        match self.wait_operation(StorageOperation::Flush, timeout)? {
            StorageCompletion::Flush(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }
}

#[cfg(test)]
impl ProfileStoreWorker {
    pub fn wait_download_batch(&mut self, timeout: Duration) -> Option<Result<usize, String>> {
        match self.wait_operation(StorageOperation::DownloadBatch, timeout)? {
            StorageCompletion::DownloadBatch(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_permission_batch(&mut self, timeout: Duration) -> Option<Result<usize, String>> {
        match self.wait_operation(StorageOperation::PermissionBatch, timeout)? {
            StorageCompletion::PermissionBatch(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_history_batch(&mut self, timeout: Duration) -> Option<Result<usize, String>> {
        match self.wait_operation(StorageOperation::HistoryBatch, timeout)? {
            StorageCompletion::HistoryBatch(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }
}

fn stringify_error<T>(result: Result<T, StorageOperationError>) -> Result<T, String> {
    result.map_err(|error| error.message)
}

impl Drop for ProfileStoreWorker {
    fn drop(&mut self) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.try_send(WorkerMessage::Shutdown);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn run_worker(
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
    Ok(ProfileLibrarySnapshot {
        history: store.history(limit).map_err(|error| error.to_string())?,
        bookmarks: store.bookmarks(limit).map_err(|error| error.to_string())?,
        quickmarks: store.quickmarks(limit).map_err(|error| error.to_string())?,
        downloads: store
            .downloads()
            .map_err(|error| error.to_string())?
            .into_iter()
            .take(limit)
            .collect(),
        permissions: store
            .permission_rules(None)
            .map_err(|error| error.to_string())?,
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
mod tests {
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
            let completion =
                worker
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
        assert!(store.permission_rules(None).expect("rules").is_empty());
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
        assert!(
            store
                .permission_rules(Some("https://example.test"))
                .expect("rules")
                .is_empty()
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
        assert!(store.history(10).expect("history").is_empty());
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
        assert!(snapshot.edges.is_empty());
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
}
