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
    JourneyNodeRecord, JourneyWrite, MarkWrite, PermissionRule, PermissionRulesSnapshot,
    ProfileStore, Quickmark, RestorePlanEntry, SessionSnapshot, StoreMode, VisitInput,
    clear_current_session_checkpoints, current_session_paths, delete_named_session,
    list_named_sessions, load_session, save_session_atomic,
};
use uuid::Uuid;

const QUEUE_CAPACITY: usize = 1;
const COMPLETION_CAPACITY: usize = 32;
const MAX_LIBRARY_LIMIT: usize = 1_000;
const MAX_HISTORY_BATCH: usize = 128;
const MAX_PERMISSION_BATCH: usize = 16;
const MAX_DOWNLOAD_BATCH: usize = 32;
const MAX_MARK_BATCH: usize = 16;

mod types;
use types::WorkerMessage;
pub use types::{
    JourneyQuerySnapshot, ProfileLibrarySnapshot, SessionRestoreSnapshot, StorageCommand,
    StorageCompletion, StorageOperation, StorageOperationError, StorageWorkerError,
};
mod execution;
use execution::run_worker;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

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
        request.validate()?;
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

fn stringify_error<T>(result: Result<T, StorageOperationError>) -> Result<T, String> {
    result.map_err(|error| error.message)
}
