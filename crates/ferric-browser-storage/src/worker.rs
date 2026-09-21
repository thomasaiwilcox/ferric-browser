//! Bounded background ownership for read-heavy profile metadata queries.

#![allow(
    clippy::struct_excessive_bools,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use std::{
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

enum Request {
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
    Shutdown,
    Flush,
}

struct LibraryResponse {
    library: Result<ProfileLibrarySnapshot, String>,
}

struct SessionListResponse {
    sessions: Result<Vec<String>, String>,
}

struct SessionRestoreResponse {
    snapshot: Result<SessionRestoreSnapshot, String>,
}

/// Validated restore data returned by the metadata worker. Closed-tab
/// descriptors are carried separately so normal session loads can keep their
/// existing semantics while crash recovery can restore profile-local undo.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionRestoreSnapshot {
    pub entries: Vec<RestorePlanEntry>,
    pub closed_tabs: Vec<ClosedTabSnapshot>,
}

struct SessionSaveResponse {
    saved: Result<(), String>,
}

struct SessionDeleteResponse {
    deleted: Result<(), String>,
}

struct SessionCheckpointClearResponse {
    cleared: Result<(), String>,
}

struct HistoryResponse {
    committed: Result<usize, String>,
}

struct PermissionResponse {
    committed: Result<usize, String>,
}

struct PermissionResetResponse {
    removed: Result<bool, String>,
}

struct DownloadResponse {
    committed: Result<usize, String>,
}

struct DownloadDestinationResponse {
    updated: Result<(), String>,
}

struct DownloadCreateResponse {
    created: Result<(), String>,
}

struct JourneyResponse {
    applied: Result<bool, String>,
}

struct JourneyNodeResponse {
    node: Result<Option<JourneyNodeRecord>, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JourneyQuerySnapshot {
    pub nodes: Vec<JourneyNodeRecord>,
    pub edges: Vec<super::JourneyEdgeRecord>,
}

struct JourneyQueryResponse {
    result: Result<JourneyQuerySnapshot, String>,
}

struct MarkResponse {
    committed: Result<usize, String>,
}

struct HistoryClearResponse {
    deleted: Result<u64, String>,
}

struct FlushResponse {
    flushed: Result<(), String>,
}

/// Owns one SQLite connection on one bounded background thread.
///
/// The handle is intentionally single-flight: a caller must consume the
/// previous result before submitting another query. This prevents a fast UI
/// typing sequence from growing an unbounded database backlog.
pub struct ProfileStoreWorker {
    sender: Option<SyncSender<Request>>,
    library_receiver: Receiver<LibraryResponse>,
    session_list_receiver: Receiver<SessionListResponse>,
    session_restore_receiver: Receiver<SessionRestoreResponse>,
    session_save_receiver: Receiver<SessionSaveResponse>,
    session_delete_receiver: Receiver<SessionDeleteResponse>,
    session_checkpoint_clear_receiver: Receiver<SessionCheckpointClearResponse>,
    history_receiver: Receiver<HistoryResponse>,
    permission_receiver: Receiver<PermissionResponse>,
    permission_reset_receiver: Receiver<PermissionResetResponse>,
    download_receiver: Receiver<DownloadResponse>,
    download_destination_receiver: Receiver<DownloadDestinationResponse>,
    download_create_receiver: Receiver<DownloadCreateResponse>,
    journey_receiver: Receiver<JourneyResponse>,
    journey_node_receiver: Receiver<JourneyNodeResponse>,
    journey_query_receiver: Receiver<JourneyQueryResponse>,
    journey_export_receiver: Receiver<JourneyQueryResponse>,
    mark_receiver: Receiver<MarkResponse>,
    history_clear_receiver: Receiver<HistoryClearResponse>,
    flush_receiver: Receiver<FlushResponse>,
    join: Option<JoinHandle<()>>,
    library_pending: bool,
    session_list_pending: bool,
    session_restore_pending: bool,
    session_save_pending: bool,
    session_delete_pending: bool,
    session_checkpoint_clear_pending: bool,
    history_pending: bool,
    permission_pending: bool,
    permission_reset_pending: bool,
    download_pending: bool,
    download_destination_pending: bool,
    download_create_pending: bool,
    journey_pending: bool,
    journey_node_pending: bool,
    journey_query_pending: bool,
    journey_export_pending: bool,
    mark_pending: bool,
    history_clear_pending: bool,
    flush_pending: bool,
}

impl ProfileStoreWorker {
    /// Starts the worker. Database opening occurs on the worker thread.
    ///
    /// # Errors
    ///
    /// Returns an error if the worker thread cannot be spawned.
    pub fn spawn(path: impl AsRef<Path>) -> Result<Self, StorageWorkerError> {
        let path = path.as_ref().to_owned();
        let (sender, requests) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (library_responses, library_receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (session_list_responses, session_list_receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (session_restore_responses, session_restore_receiver) =
            mpsc::sync_channel(QUEUE_CAPACITY);
        let (session_save_responses, session_save_receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (session_delete_responses, session_delete_receiver) =
            mpsc::sync_channel(QUEUE_CAPACITY);
        let (session_checkpoint_clear_responses, session_checkpoint_clear_receiver) =
            mpsc::sync_channel(QUEUE_CAPACITY);
        let (history_responses, history_receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (permission_responses, permission_receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (permission_reset_responses, permission_reset_receiver) =
            mpsc::sync_channel(QUEUE_CAPACITY);
        let (download_responses, download_receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (download_destination_responses, download_destination_receiver) =
            mpsc::sync_channel(QUEUE_CAPACITY);
        let (download_create_responses, download_create_receiver) =
            mpsc::sync_channel(QUEUE_CAPACITY);
        let (journey_responses, journey_receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (journey_node_responses, journey_node_receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (journey_query_responses, journey_query_receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (journey_export_responses, journey_export_receiver) =
            mpsc::sync_channel(QUEUE_CAPACITY);
        let (mark_responses, mark_receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (history_clear_responses, history_clear_receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let (flush_responses, flush_receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let join = thread::Builder::new()
            .name("ferric-browser-profile-metadata".into())
            .spawn(move || {
                run_worker(
                    path,
                    requests,
                    library_responses,
                    session_list_responses,
                    session_restore_responses,
                    session_save_responses,
                    session_delete_responses,
                    session_checkpoint_clear_responses,
                    history_responses,
                    permission_responses,
                    permission_reset_responses,
                    download_responses,
                    download_destination_responses,
                    download_create_responses,
                    journey_responses,
                    journey_node_responses,
                    journey_query_responses,
                    journey_export_responses,
                    mark_responses,
                    history_clear_responses,
                    flush_responses,
                );
            })
            .map_err(|error| StorageWorkerError::Spawn(error.to_string()))?;
        Ok(Self {
            sender: Some(sender),
            library_receiver,
            session_list_receiver,
            session_restore_receiver,
            session_save_receiver,
            session_delete_receiver,
            session_checkpoint_clear_receiver,
            history_receiver,
            permission_receiver,
            permission_reset_receiver,
            download_receiver,
            download_destination_receiver,
            download_create_receiver,
            journey_receiver,
            journey_node_receiver,
            journey_query_receiver,
            journey_export_receiver,
            mark_receiver,
            history_clear_receiver,
            flush_receiver,
            join: Some(join),
            library_pending: false,
            session_list_pending: false,
            session_restore_pending: false,
            session_save_pending: false,
            session_delete_pending: false,
            session_checkpoint_clear_pending: false,
            history_pending: false,
            permission_pending: false,
            permission_reset_pending: false,
            download_pending: false,
            download_destination_pending: false,
            download_create_pending: false,
            journey_pending: false,
            journey_node_pending: false,
            journey_query_pending: false,
            journey_export_pending: false,
            mark_pending: false,
            history_clear_pending: false,
            flush_pending: false,
        })
    }

    /// Queues one bounded library read if no prior request is outstanding.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the limit is invalid, another request is
    /// pending, the bounded queue is full, or the worker has stopped.
    pub fn request_library(&mut self, limit: usize) -> Result<(), StorageWorkerError> {
        if !(1..=MAX_LIBRARY_LIMIT).contains(&limit) {
            return Err(StorageWorkerError::InvalidLimit);
        }
        if self.library_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::Library { limit }) {
            Ok(()) => {
                self.library_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn poll_library(&mut self) -> Option<Result<ProfileLibrarySnapshot, String>> {
        match self.library_receiver.try_recv() {
            Ok(response) => {
                self.library_pending = false;
                Some(response.library)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.library_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    /// Queues a bounded session-name enumeration on the worker thread.
    ///
    /// Session files live beside, rather than inside, the metadata database;
    /// the worker still owns this filesystem read so switcher queries never
    /// scan the session directory on the Qt thread.
    pub fn request_session_list(
        &mut self,
        root: PathBuf,
        profile_id: Uuid,
    ) -> Result<(), StorageWorkerError> {
        if self.session_list_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::SessionList { root, profile_id }) {
            Ok(()) => {
                self.session_list_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn poll_session_list(&mut self) -> Option<Result<Vec<String>, String>> {
        match self.session_list_receiver.try_recv() {
            Ok(response) => {
                self.session_list_pending = false;
                Some(response.sessions)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.session_list_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn session_list_pending(&self) -> bool {
        self.session_list_pending
    }

    /// Queues validation and restore-plan construction for one session file.
    /// The returned plan contains only safe, bounded restore descriptors.
    pub fn request_session_restore(&mut self, path: PathBuf) -> Result<(), StorageWorkerError> {
        self.request_session_restore_paths(vec![path])
    }

    /// Queues validation and restore-plan construction for one or more
    /// checkpoint generations. The worker concatenates the validated plans in
    /// path order, preserving the recovery fallback semantics without making
    /// the GUI thread parse checkpoint files.
    pub fn request_session_restore_paths(
        &mut self,
        paths: Vec<PathBuf>,
    ) -> Result<(), StorageWorkerError> {
        if self.session_restore_pending {
            return Err(StorageWorkerError::Busy);
        }
        if paths.is_empty() {
            return Err(StorageWorkerError::QueueFull);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::SessionRestore { paths }) {
            Ok(()) => {
                self.session_restore_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn request_session_recovery(
        &mut self,
        root: PathBuf,
        profile_id: Uuid,
    ) -> Result<(), StorageWorkerError> {
        if self.session_restore_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::SessionRecovery { root, profile_id }) {
            Ok(()) => {
                self.session_restore_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn poll_session_restore(&mut self) -> Option<Result<SessionRestoreSnapshot, String>> {
        match self.session_restore_receiver.try_recv() {
            Ok(response) => {
                self.session_restore_pending = false;
                Some(response.snapshot)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.session_restore_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn session_restore_pending(&self) -> bool {
        self.session_restore_pending
    }

    /// Queues one atomic session snapshot write on the storage worker.
    pub fn request_session_save(
        &mut self,
        path: PathBuf,
        snapshot: SessionSnapshot,
    ) -> Result<(), StorageWorkerError> {
        if self.session_save_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::SessionSave { path, snapshot }) {
            Ok(()) => {
                self.session_save_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn poll_session_save(&mut self) -> Option<Result<(), String>> {
        match self.session_save_receiver.try_recv() {
            Ok(response) => {
                self.session_save_pending = false;
                Some(response.saved)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.session_save_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn session_save_pending(&self) -> bool {
        self.session_save_pending
    }

    /// Queues deletion of one validated named-session file on the worker.
    pub fn request_session_delete(
        &mut self,
        root: PathBuf,
        profile_id: Uuid,
        name: String,
    ) -> Result<(), StorageWorkerError> {
        if self.session_delete_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::SessionDelete {
            root,
            profile_id,
            name,
        }) {
            Ok(()) => {
                self.session_delete_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn poll_session_delete(&mut self) -> Option<Result<(), String>> {
        match self.session_delete_receiver.try_recv() {
            Ok(response) => {
                self.session_delete_pending = false;
                Some(response.deleted)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.session_delete_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn request_session_checkpoint_clear(
        &mut self,
        root: PathBuf,
        profile_id: Uuid,
    ) -> Result<(), StorageWorkerError> {
        if self.session_checkpoint_clear_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::SessionCheckpointClear { root, profile_id }) {
            Ok(()) => {
                self.session_checkpoint_clear_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn poll_session_checkpoint_clear(&mut self) -> Option<Result<(), String>> {
        match self.session_checkpoint_clear_receiver.try_recv() {
            Ok(response) => {
                self.session_checkpoint_clear_pending = false;
                Some(response.cleared)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.session_checkpoint_clear_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    /// Queues a bounded atomic history batch on the SQLite owner.
    ///
    /// # Errors
    ///
    /// Returns a typed error if the batch is empty/too large, another history
    /// batch is pending, the bounded queue is full, or the worker stopped.
    pub fn request_history_batch(
        &mut self,
        visits: Vec<VisitInput>,
    ) -> Result<(), StorageWorkerError> {
        if visits.is_empty() || visits.len() > MAX_HISTORY_BATCH {
            return Err(StorageWorkerError::InvalidBatch);
        }
        if self.history_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::HistoryBatch { visits }) {
            Ok(()) => {
                self.history_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn poll_history_batch(&mut self) -> Option<Result<usize, String>> {
        match self.history_receiver.try_recv() {
            Ok(response) => {
                self.history_pending = false;
                Some(response.committed)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.history_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    /// Queues a bounded atomic permission-rule batch on the SQLite owner.
    ///
    /// # Errors
    ///
    /// Returns a typed error if the batch is empty/too large, another
    /// permission batch is pending, the bounded queue is full, or the worker
    /// stopped.
    pub fn request_permission_batch(
        &mut self,
        rules: Vec<PermissionRule>,
    ) -> Result<(), StorageWorkerError> {
        if rules.is_empty() || rules.len() > MAX_PERMISSION_BATCH {
            return Err(StorageWorkerError::InvalidPermissionBatch);
        }
        if self.permission_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::PermissionBatch { rules }) {
            Ok(()) => {
                self.permission_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn poll_permission_batch(&mut self) -> Option<Result<usize, String>> {
        match self.permission_receiver.try_recv() {
            Ok(response) => {
                self.permission_pending = false;
                Some(response.committed)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.permission_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    /// Queues one bounded permission reset on the SQLite owner.
    pub fn request_permission_reset(
        &mut self,
        origin: String,
        permissions: Vec<String>,
    ) -> Result<(), StorageWorkerError> {
        if origin.is_empty() || permissions.is_empty() || permissions.len() > 2 {
            return Err(StorageWorkerError::InvalidPermissionBatch);
        }
        if self.permission_reset_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::PermissionReset {
            origin,
            permissions,
        }) {
            Ok(()) => {
                self.permission_reset_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn poll_permission_reset(&mut self) -> Option<Result<bool, String>> {
        match self.permission_reset_receiver.try_recv() {
            Ok(response) => {
                self.permission_reset_pending = false;
                Some(response.removed)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.permission_reset_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    /// Queues a bounded atomic download lifecycle batch on the SQLite owner.
    pub fn request_download_batch(
        &mut self,
        updates: Vec<DownloadUpdate>,
    ) -> Result<(), StorageWorkerError> {
        if updates.is_empty() || updates.len() > MAX_DOWNLOAD_BATCH {
            return Err(StorageWorkerError::InvalidDownloadBatch);
        }
        if self.download_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::DownloadBatch { updates }) {
            Ok(()) => {
                self.download_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn poll_download_batch(&mut self) -> Option<Result<usize, String>> {
        match self.download_receiver.try_recv() {
            Ok(response) => {
                self.download_pending = false;
                Some(response.committed)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.download_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    /// Queues one acknowledged download destination update on the SQLite
    /// owner.
    pub fn request_download_destination(
        &mut self,
        id: String,
        destination: String,
    ) -> Result<(), StorageWorkerError> {
        if self.download_destination_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::DownloadDestination { id, destination }) {
            Ok(()) => {
                self.download_destination_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn wait_download_destination(&mut self, timeout: Duration) -> Option<Result<(), String>> {
        if !self.download_destination_pending {
            return None;
        }
        match self.download_destination_receiver.recv_timeout(timeout) {
            Ok(response) => {
                self.download_destination_pending = false;
                Some(response.updated)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.download_destination_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn poll_download_destination(&mut self) -> Option<Result<(), String>> {
        match self.download_destination_receiver.try_recv() {
            Ok(response) => {
                self.download_destination_pending = false;
                Some(response.updated)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.download_destination_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn download_destination_pending(&self) -> bool {
        self.download_destination_pending
    }

    /// Queues one acknowledged download index creation on the SQLite owner.
    pub fn request_download_create(
        &mut self,
        id: String,
        source_url: String,
        destination: String,
        state: super::DownloadState,
        created_at: i64,
    ) -> Result<(), StorageWorkerError> {
        if self.download_create_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::DownloadCreate {
            id,
            source_url,
            destination,
            state,
            created_at,
        }) {
            Ok(()) => {
                self.download_create_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn wait_download_create(&mut self, timeout: Duration) -> Option<Result<(), String>> {
        if !self.download_create_pending {
            return None;
        }
        match self.download_create_receiver.recv_timeout(timeout) {
            Ok(response) => {
                self.download_create_pending = false;
                Some(response.created)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.download_create_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn poll_download_create(&mut self) -> Option<Result<(), String>> {
        match self.download_create_receiver.try_recv() {
            Ok(response) => {
                self.download_create_pending = false;
                Some(response.created)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.download_create_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn download_create_pending(&self) -> bool {
        self.download_create_pending
    }

    /// Queues one acknowledged journey mutation on the SQLite owner.
    pub fn request_journey_write(&mut self, write: JourneyWrite) -> Result<(), StorageWorkerError> {
        if self.journey_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::JourneyWrite { write }) {
            Ok(()) => {
                self.journey_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn wait_journey_write(&mut self, timeout: Duration) -> Option<Result<bool, String>> {
        if !self.journey_pending {
            return None;
        }
        match self.journey_receiver.recv_timeout(timeout) {
            Ok(response) => {
                self.journey_pending = false;
                Some(response.applied)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.journey_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn poll_journey_write(&mut self) -> Option<Result<bool, String>> {
        match self.journey_receiver.try_recv() {
            Ok(response) => {
                self.journey_pending = false;
                Some(response.applied)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.journey_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn journey_pending(&self) -> bool {
        self.journey_pending
    }

    /// Queues one bounded durable journey-node lookup on the SQLite owner.
    pub fn request_journey_node(&mut self, id: String) -> Result<(), StorageWorkerError> {
        if self.journey_node_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::JourneyNode { id }) {
            Ok(()) => {
                self.journey_node_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn poll_journey_node(&mut self) -> Option<Result<Option<JourneyNodeRecord>, String>> {
        match self.journey_node_receiver.try_recv() {
            Ok(response) => {
                self.journey_node_pending = false;
                Some(response.node)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.journey_node_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn wait_journey_node(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<Option<JourneyNodeRecord>, String>> {
        if !self.journey_node_pending {
            return None;
        }
        match self.journey_node_receiver.recv_timeout(timeout) {
            Ok(response) => {
                self.journey_node_pending = false;
                Some(response.node)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.journey_node_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn journey_node_pending(&self) -> bool {
        self.journey_node_pending
    }

    /// Queues one bounded durable journey list/search query on the SQLite
    /// owner. The query parameters are validated by the store methods.
    pub fn request_journey_query(
        &mut self,
        current_tab_id: Option<String>,
        search: Option<String>,
        expand: Option<String>,
    ) -> Result<(), StorageWorkerError> {
        if self.journey_query_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::JourneyQuery {
            current_tab_id,
            search,
            expand,
        }) {
            Ok(()) => {
                self.journey_query_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn poll_journey_query(&mut self) -> Option<Result<JourneyQuerySnapshot, String>> {
        match self.journey_query_receiver.try_recv() {
            Ok(response) => {
                self.journey_query_pending = false;
                Some(response.result)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.journey_query_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn wait_journey_query(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<JourneyQuerySnapshot, String>> {
        if !self.journey_query_pending {
            return None;
        }
        match self.journey_query_receiver.recv_timeout(timeout) {
            Ok(response) => {
                self.journey_query_pending = false;
                Some(response.result)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.journey_query_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn journey_query_pending(&self) -> bool {
        self.journey_query_pending
    }

    /// Queues a bounded full journey export on the SQLite owner.
    pub fn request_journey_export(&mut self) -> Result<(), StorageWorkerError> {
        if self.journey_export_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::JourneyExport) {
            Ok(()) => {
                self.journey_export_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn poll_journey_export(&mut self) -> Option<Result<JourneyQuerySnapshot, String>> {
        match self.journey_export_receiver.try_recv() {
            Ok(response) => {
                self.journey_export_pending = false;
                Some(response.result)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.journey_export_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn wait_journey_export(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<JourneyQuerySnapshot, String>> {
        if !self.journey_export_pending {
            return None;
        }
        match self.journey_export_receiver.recv_timeout(timeout) {
            Ok(response) => {
                self.journey_export_pending = false;
                Some(response.result)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.journey_export_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn journey_export_pending(&self) -> bool {
        self.journey_export_pending
    }

    /// Queues one acknowledged bookmark/quickmark mutation on the SQLite
    /// owner. The single-item bound preserves command-level result semantics.
    pub fn request_mark_write(&mut self, write: MarkWrite) -> Result<(), StorageWorkerError> {
        self.request_mark_writes(vec![write])
    }

    /// Queues a bounded atomic group of bookmark/quickmark mutations on the
    /// SQLite owner. The worker acknowledges the whole group only after one
    /// SQLite transaction commits successfully.
    pub fn request_mark_writes(
        &mut self,
        writes: Vec<MarkWrite>,
    ) -> Result<(), StorageWorkerError> {
        if writes.is_empty() || writes.len() > MAX_MARK_BATCH {
            return Err(StorageWorkerError::InvalidMarkBatch);
        }
        if self.mark_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::MarkBatch { writes }) {
            Ok(()) => {
                self.mark_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn wait_mark_write(&mut self, timeout: Duration) -> Option<Result<usize, String>> {
        if !self.mark_pending {
            return None;
        }
        match self.mark_receiver.recv_timeout(timeout) {
            Ok(response) => {
                self.mark_pending = false;
                Some(response.committed)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.mark_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn poll_mark_write(&mut self) -> Option<Result<usize, String>> {
        match self.mark_receiver.try_recv() {
            Ok(response) => {
                self.mark_pending = false;
                Some(response.committed)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.mark_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn mark_write_pending(&self) -> bool {
        self.mark_pending
    }

    /// Queues a confirmed history clear on the SQLite owner.
    pub fn request_history_clear(
        &mut self,
        since: Option<i64>,
        origin: Option<String>,
    ) -> Result<(), StorageWorkerError> {
        if self.history_clear_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::HistoryClear { since, origin }) {
            Ok(()) => {
                self.history_clear_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn wait_history_clear(&mut self, timeout: Duration) -> Option<Result<u64, String>> {
        if !self.history_clear_pending {
            return None;
        }
        match self.history_clear_receiver.recv_timeout(timeout) {
            Ok(response) => {
                self.history_clear_pending = false;
                Some(response.deleted)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.history_clear_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn poll_history_clear(&mut self) -> Option<Result<u64, String>> {
        match self.history_clear_receiver.try_recv() {
            Ok(response) => {
                self.history_clear_pending = false;
                Some(response.deleted)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.history_clear_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    pub fn history_clear_pending(&self) -> bool {
        self.history_clear_pending
    }

    /// Queues a final WAL checkpoint on the SQLite owner.
    pub fn request_flush(&mut self) -> Result<(), StorageWorkerError> {
        if self.flush_pending {
            return Err(StorageWorkerError::Busy);
        }
        let sender = self.sender.as_ref().ok_or(StorageWorkerError::Stopped)?;
        match sender.try_send(Request::Flush) {
            Ok(()) => {
                self.flush_pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(StorageWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(StorageWorkerError::Stopped),
        }
    }

    pub fn wait_flush(&mut self, timeout: Duration) -> Option<Result<(), String>> {
        if !self.flush_pending {
            return None;
        }
        match self.flush_receiver.recv_timeout(timeout) {
            Ok(response) => {
                self.flush_pending = false;
                Some(response.flushed)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.flush_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    /// Waits briefly for the outstanding download batch during shutdown.
    pub fn wait_download_batch(&mut self, timeout: Duration) -> Option<Result<usize, String>> {
        if !self.download_pending {
            return None;
        }
        match self.download_receiver.recv_timeout(timeout) {
            Ok(response) => {
                self.download_pending = false;
                Some(response.committed)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.download_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    /// Waits briefly for the outstanding permission batch to finish during
    /// bounded application shutdown.
    pub fn wait_permission_batch(&mut self, timeout: Duration) -> Option<Result<usize, String>> {
        if !self.permission_pending {
            return None;
        }
        match self.permission_receiver.recv_timeout(timeout) {
            Ok(response) => {
                self.permission_pending = false;
                Some(response.committed)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.permission_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }

    /// Waits briefly for the outstanding history batch to finish.
    ///
    /// This is reserved for the bounded application-shutdown path. Normal UI
    /// work uses [`Self::poll_history_batch`] so the Qt thread never blocks on
    /// SQLite.
    pub fn wait_history_batch(&mut self, timeout: Duration) -> Option<Result<usize, String>> {
        if !self.history_pending {
            return None;
        }
        match self.history_receiver.recv_timeout(timeout) {
            Ok(response) => {
                self.history_pending = false;
                Some(response.committed)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.history_pending = false;
                Some(Err("profile metadata worker disconnected".into()))
            }
        }
    }
}

impl Drop for ProfileStoreWorker {
    fn drop(&mut self) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.try_send(Request::Shutdown);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn run_worker(
    path: PathBuf,
    requests: Receiver<Request>,
    library_responses: SyncSender<LibraryResponse>,
    session_list_responses: SyncSender<SessionListResponse>,
    session_restore_responses: SyncSender<SessionRestoreResponse>,
    session_save_responses: SyncSender<SessionSaveResponse>,
    session_delete_responses: SyncSender<SessionDeleteResponse>,
    session_checkpoint_clear_responses: SyncSender<SessionCheckpointClearResponse>,
    history_responses: SyncSender<HistoryResponse>,
    permission_responses: SyncSender<PermissionResponse>,
    permission_reset_responses: SyncSender<PermissionResetResponse>,
    download_responses: SyncSender<DownloadResponse>,
    download_destination_responses: SyncSender<DownloadDestinationResponse>,
    download_create_responses: SyncSender<DownloadCreateResponse>,
    journey_responses: SyncSender<JourneyResponse>,
    journey_node_responses: SyncSender<JourneyNodeResponse>,
    journey_query_responses: SyncSender<JourneyQueryResponse>,
    journey_export_responses: SyncSender<JourneyQueryResponse>,
    mark_responses: SyncSender<MarkResponse>,
    history_clear_responses: SyncSender<HistoryClearResponse>,
    flush_responses: SyncSender<FlushResponse>,
) {
    let store = ProfileStore::open(path, StoreMode::Normal).map_err(|error| error.to_string());
    while let Ok(request) = requests.recv() {
        match request {
            Request::Library { limit } => {
                let library = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| load_library(store, limit),
                );
                if library_responses
                    .try_send(LibraryResponse { library })
                    .is_err()
                {
                    break;
                }
            }
            Request::SessionList { root, profile_id } => {
                let sessions =
                    list_named_sessions(&root, profile_id).map_err(|error| error.to_string());
                if session_list_responses
                    .try_send(SessionListResponse { sessions })
                    .is_err()
                {
                    break;
                }
            }
            Request::SessionRestore { paths } => {
                let snapshot = paths.into_iter().try_fold(
                    SessionRestoreSnapshot {
                        entries: Vec::new(),
                        closed_tabs: Vec::new(),
                    },
                    |mut restored, path| {
                        let snapshot = load_session(&path).map_err(|error| error.to_string())?;
                        let mut entries =
                            snapshot.restore_plan().map_err(|error| error.to_string())?;
                        restored.entries.append(&mut entries);
                        if restored.closed_tabs.is_empty() {
                            restored.closed_tabs = snapshot.closed_tabs;
                        }
                        Ok::<_, String>(restored)
                    },
                );
                if session_restore_responses
                    .try_send(SessionRestoreResponse { snapshot })
                    .is_err()
                {
                    break;
                }
            }
            Request::SessionRecovery { root, profile_id } => {
                let snapshot = current_session_paths(&root, profile_id)
                    .into_iter()
                    .try_fold(
                        SessionRestoreSnapshot {
                            entries: Vec::new(),
                            closed_tabs: Vec::new(),
                        },
                        |mut restored, path| {
                            let snapshot =
                                load_session(&path).map_err(|error| error.to_string())?;
                            let mut entries =
                                snapshot.restore_plan().map_err(|error| error.to_string())?;
                            restored.entries.append(&mut entries);
                            if restored.closed_tabs.is_empty() {
                                restored.closed_tabs = snapshot.closed_tabs;
                            }
                            Ok::<_, String>(restored)
                        },
                    );
                if session_restore_responses
                    .try_send(SessionRestoreResponse { snapshot })
                    .is_err()
                {
                    break;
                }
            }
            Request::SessionSave { path, snapshot } => {
                let saved =
                    save_session_atomic(&path, &snapshot).map_err(|error| error.to_string());
                if session_save_responses
                    .try_send(SessionSaveResponse { saved })
                    .is_err()
                {
                    break;
                }
            }
            Request::SessionCheckpointClear { root, profile_id } => {
                let cleared = clear_current_session_checkpoints(&root, profile_id)
                    .map_err(|error| error.to_string());
                if session_checkpoint_clear_responses
                    .try_send(SessionCheckpointClearResponse { cleared })
                    .is_err()
                {
                    break;
                }
            }
            Request::SessionDelete {
                root,
                profile_id,
                name,
            } => {
                let deleted = delete_named_session(&root, profile_id, &name)
                    .map_err(|error| error.to_string());
                if session_delete_responses
                    .try_send(SessionDeleteResponse { deleted })
                    .is_err()
                {
                    break;
                }
            }
            Request::HistoryBatch { visits } => {
                let committed = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .record_visit_batch(&visits)
                            .map(|()| visits.len())
                            .map_err(|error| error.to_string())
                    },
                );
                if history_responses
                    .try_send(HistoryResponse { committed })
                    .is_err()
                {
                    break;
                }
            }
            Request::PermissionBatch { rules } => {
                let committed = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .set_permission_rule_batch(&rules)
                            .map(|()| rules.len())
                            .map_err(|error| error.to_string())
                    },
                );
                if permission_responses
                    .try_send(PermissionResponse { committed })
                    .is_err()
                {
                    break;
                }
            }
            Request::PermissionReset {
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
                if permission_reset_responses
                    .try_send(PermissionResetResponse { removed })
                    .is_err()
                {
                    break;
                }
            }
            Request::DownloadBatch { updates } => {
                let committed = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .update_download_batch(&updates)
                            .map(|()| updates.len())
                            .map_err(|error| error.to_string())
                    },
                );
                if download_responses
                    .try_send(DownloadResponse { committed })
                    .is_err()
                {
                    break;
                }
            }
            Request::DownloadDestination { id, destination } => {
                let updated = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .set_download_destination(&id, &destination)
                            .map_err(|error| error.to_string())
                    },
                );
                if download_destination_responses
                    .try_send(DownloadDestinationResponse { updated })
                    .is_err()
                {
                    break;
                }
            }
            Request::DownloadCreate {
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
                if download_create_responses
                    .try_send(DownloadCreateResponse { created })
                    .is_err()
                {
                    break;
                }
            }
            Request::JourneyWrite { write } => {
                let applied = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .apply_journey_write(&write)
                            .map_err(|error| error.to_string())
                    },
                );
                if journey_responses
                    .try_send(JourneyResponse { applied })
                    .is_err()
                {
                    break;
                }
            }
            Request::JourneyNode { id } => {
                let node = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| store.journey_node(&id).map_err(|error| error.to_string()),
                );
                if journey_node_responses
                    .try_send(JourneyNodeResponse { node })
                    .is_err()
                {
                    break;
                }
            }
            Request::JourneyQuery {
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
                if journey_query_responses
                    .try_send(JourneyQueryResponse { result })
                    .is_err()
                {
                    break;
                }
            }
            Request::JourneyExport => {
                let result = store
                    .as_ref()
                    .map_or_else(|error| Err(error.clone()), load_journey_export);
                if journey_export_responses
                    .try_send(JourneyQueryResponse { result })
                    .is_err()
                {
                    break;
                }
            }
            Request::MarkBatch { writes } => {
                let committed = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .apply_mark_write_batch(&writes)
                            .map(|()| writes.len())
                            .map_err(|error| error.to_string())
                    },
                );
                if mark_responses.try_send(MarkResponse { committed }).is_err() {
                    break;
                }
            }
            Request::HistoryClear { since, origin } => {
                let deleted = store.as_ref().map_or_else(
                    |error| Err(error.clone()),
                    |store| {
                        store
                            .clear_history(since, origin.as_deref())
                            .map_err(|error| error.to_string())
                    },
                );
                if history_clear_responses
                    .try_send(HistoryClearResponse { deleted })
                    .is_err()
                {
                    break;
                }
            }
            Request::Flush => {
                let flushed = match store.as_ref() {
                    Ok(store) => store.flush().map_err(|error| error.to_string()),
                    Err(error) => Err(error.clone()),
                };
                if flush_responses.try_send(FlushResponse { flushed }).is_err() {
                    break;
                }
            }
            Request::Shutdown => break,
        }
    }
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
