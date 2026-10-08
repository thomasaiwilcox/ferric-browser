//! Bounded file-only capture and archive-view work. This worker never opens
//! the profile database, so large byte I/O cannot occupy its metadata
//! connection.

use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    },
    thread::{self, JoinHandle},
};

use ferric_browser_core::OperationId;
use uuid::Uuid;

#[cfg(feature = "native-archive-fault-test")]
use std::sync::atomic::AtomicUsize;

use super::{
    ArchiveViewFile, CaptureStageTarget, InstalledObject, ObjectPathError, VersioningObjectStore,
};

const OBJECT_QUEUE_CAPACITY: usize = 1;
const OBJECT_COMPLETION_CAPACITY: usize = 4;

#[cfg(feature = "native-archive-fault-test")]
static REFUSE_NEXT_ARCHIVE_CLEANUP: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "native-archive-fault-test")]
static INJECTED_ARCHIVE_CLEANUP_REFUSALS: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "native-archive-fault-test")]
static REFUSE_NEXT_SIDECAR_INSTALL: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "native-archive-fault-test")]
static INJECTED_SIDECAR_INSTALL_REFUSALS: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "native-archive-fault-test")]
static PAUSE_NEXT_SIDECAR_INSTALL: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "native-archive-fault-test")]
static SIDECAR_INSTALL_PAUSED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "native-archive-fault-test")]
static RESUME_SIDECAR_INSTALL: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "native-archive-fault-test")]
static SIDECAR_PERMISSION_DENIALS: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "native-archive-fault-test")]
static PAUSE_NEXT_ARCHIVE_INSTALL: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "native-archive-fault-test")]
static ARCHIVE_INSTALL_PAUSED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "native-archive-fault-test")]
static RESUME_ARCHIVE_INSTALL: AtomicBool = AtomicBool::new(false);

/// Arms one worker-side cleanup refusal in a disposable native test process.
/// This API and its branch are absent from default builds.
#[cfg(feature = "native-archive-fault-test")]
pub fn arm_one_archive_cleanup_refusal() {
    INJECTED_ARCHIVE_CLEANUP_REFUSALS.store(0, Ordering::SeqCst);
    REFUSE_NEXT_ARCHIVE_CLEANUP.store(true, Ordering::SeqCst);
}

/// Reports whether the armed refusal actually crossed the object worker.
#[cfg(feature = "native-archive-fault-test")]
#[must_use]
pub fn injected_archive_cleanup_refusals() -> usize {
    INJECTED_ARCHIVE_CLEANUP_REFUSALS.load(Ordering::SeqCst)
}

/// Arms one sidecar-only installation refusal in a disposable native test.
/// The archive object install remains unaffected. Absent from default builds.
#[cfg(feature = "native-archive-fault-test")]
pub fn arm_one_sidecar_install_refusal() {
    INJECTED_SIDECAR_INSTALL_REFUSALS.store(0, Ordering::SeqCst);
    REFUSE_NEXT_SIDECAR_INSTALL.store(true, Ordering::SeqCst);
}

/// Reports whether the refusal actually crossed the sidecar object worker.
#[cfg(feature = "native-archive-fault-test")]
#[must_use]
pub fn injected_sidecar_install_refusals() -> usize {
    INJECTED_SIDECAR_INSTALL_REFUSALS.load(Ordering::SeqCst)
}

/// Pauses one sidecar job before filesystem access so a disposable native
/// test can change only its own staging directory permissions.
#[cfg(feature = "native-archive-fault-test")]
pub fn arm_one_sidecar_install_pause() {
    SIDECAR_PERMISSION_DENIALS.store(0, Ordering::SeqCst);
    SIDECAR_INSTALL_PAUSED.store(false, Ordering::SeqCst);
    RESUME_SIDECAR_INSTALL.store(false, Ordering::SeqCst);
    PAUSE_NEXT_SIDECAR_INSTALL.store(true, Ordering::SeqCst);
}

#[cfg(feature = "native-archive-fault-test")]
#[must_use]
pub fn sidecar_install_paused() -> bool {
    SIDECAR_INSTALL_PAUSED.load(Ordering::SeqCst)
}

#[cfg(feature = "native-archive-fault-test")]
pub fn resume_sidecar_install() {
    RESUME_SIDECAR_INSTALL.store(true, Ordering::SeqCst);
}

/// Counts only operating-system permission denials, not injected refusals.
#[cfg(feature = "native-archive-fault-test")]
#[must_use]
pub fn sidecar_permission_denials() -> usize {
    SIDECAR_PERMISSION_DENIALS.load(Ordering::SeqCst)
}

/// Pauses one archive-object copy after its first chunk. The hook is absent
/// from default builds and has a bounded self-cancelling deadline.
#[cfg(feature = "native-archive-fault-test")]
pub fn arm_one_archive_install_pause() {
    ARCHIVE_INSTALL_PAUSED.store(false, Ordering::SeqCst);
    RESUME_ARCHIVE_INSTALL.store(false, Ordering::SeqCst);
    PAUSE_NEXT_ARCHIVE_INSTALL.store(true, Ordering::SeqCst);
}

#[cfg(feature = "native-archive-fault-test")]
#[must_use]
pub fn archive_install_paused() -> bool {
    ARCHIVE_INSTALL_PAUSED.load(Ordering::SeqCst)
}

#[cfg(feature = "native-archive-fault-test")]
pub fn resume_archive_install() {
    RESUME_ARCHIVE_INSTALL.store(true, Ordering::SeqCst);
}

#[derive(Clone, Eq, PartialEq)]
pub enum VersioningObjectCommand {
    PrepareCapture {
        operation: OperationId,
        profile_uuid: Uuid,
        state_root: PathBuf,
    },
    PrepareWindowPage {
        operation: OperationId,
        profile_uuid: Uuid,
        state_root: PathBuf,
        page_index: u16,
    },
    InstallCapture {
        operation: OperationId,
        profile_uuid: Uuid,
        state_root: PathBuf,
        max_bytes: u64,
        expected_size: Option<u64>,
        expected_sha256: Option<[u8; 32]>,
    },
    InstallWindowPage {
        operation: OperationId,
        profile_uuid: Uuid,
        state_root: PathBuf,
        page_index: u16,
        max_bytes: u64,
        expected_size: Option<u64>,
        expected_sha256: Option<[u8; 32]>,
    },
    InstallSidecarBytes {
        operation: OperationId,
        profile_uuid: Uuid,
        state_root: PathBuf,
        bytes: Vec<u8>,
    },
    MaterializeArchiveView {
        operation: OperationId,
        profile_uuid: Uuid,
        state_root: PathBuf,
        digest_hex: String,
        expected_bytes: u64,
    },
    RemoveArchiveView {
        operation: OperationId,
        profile_uuid: Uuid,
        state_root: PathBuf,
    },
}

impl fmt::Debug for VersioningObjectCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrepareCapture { operation, .. } => formatter
                .debug_struct("PrepareCapture")
                .field("operation", operation)
                .field("profile_uuid", &"[profile]")
                .field("state_root", &"[private state path]")
                .finish(),
            Self::PrepareWindowPage {
                operation,
                page_index,
                ..
            } => formatter
                .debug_struct("PrepareWindowPage")
                .field("operation", operation)
                .field("page_index", page_index)
                .field("profile_uuid", &"[profile]")
                .field("state_root", &"[private state path]")
                .finish(),
            Self::InstallCapture {
                operation,
                max_bytes,
                expected_size,
                ..
            } => formatter
                .debug_struct("InstallCapture")
                .field("operation", operation)
                .field("profile_uuid", &"[profile]")
                .field("state_root", &"[private state path]")
                .field("max_bytes", max_bytes)
                .field("expected_size", expected_size)
                .field("expected_sha256", &"[content digest]")
                .finish(),
            Self::InstallWindowPage {
                operation,
                page_index,
                max_bytes,
                expected_size,
                ..
            } => formatter
                .debug_struct("InstallWindowPage")
                .field("operation", operation)
                .field("page_index", page_index)
                .field("profile_uuid", &"[profile]")
                .field("state_root", &"[private state path]")
                .field("max_bytes", max_bytes)
                .field("expected_size", expected_size)
                .field("expected_sha256", &"[content digest]")
                .finish(),
            Self::InstallSidecarBytes {
                operation,
                profile_uuid: _,
                state_root: _,
                bytes,
            } => formatter
                .debug_struct("InstallSidecarBytes")
                .field("operation", operation)
                .field("profile_uuid", &"[profile]")
                .field("state_root", &"[private state path]")
                .field("byte_len", &bytes.len())
                .field("payload", &"[redacted]")
                .finish(),
            Self::MaterializeArchiveView {
                operation,
                expected_bytes,
                ..
            } => formatter
                .debug_struct("MaterializeArchiveView")
                .field("operation", operation)
                .field("profile_uuid", &"[profile]")
                .field("state_root", &"[private state path]")
                .field("digest_hex", &"[content digest]")
                .field("expected_bytes", expected_bytes)
                .finish(),
            Self::RemoveArchiveView { operation, .. } => formatter
                .debug_struct("RemoveArchiveView")
                .field("operation", operation)
                .field("profile_uuid", &"[profile]")
                .field("state_root", &"[private state path]")
                .finish(),
        }
    }
}

impl VersioningObjectCommand {
    #[must_use]
    pub const fn operation_id(&self) -> OperationId {
        match self {
            Self::PrepareCapture { operation, .. }
            | Self::PrepareWindowPage { operation, .. }
            | Self::InstallCapture { operation, .. }
            | Self::InstallWindowPage { operation, .. }
            | Self::InstallSidecarBytes { operation, .. }
            | Self::MaterializeArchiveView { operation, .. }
            | Self::RemoveArchiveView { operation, .. } => *operation,
        }
    }

    fn validate(&self) -> bool {
        match self {
            Self::PrepareCapture { .. } | Self::RemoveArchiveView { .. } => true,
            Self::PrepareWindowPage { page_index, .. } => {
                *page_index > 0
                    && usize::from(*page_index)
                        < ferric_browser_core::versioning::limits::MAX_PAGES_PER_WINDOW_CAPTURE
            }
            Self::InstallCapture {
                max_bytes,
                expected_size,
                ..
            } => *max_bytes > 0 && expected_size.is_none_or(|size| size > 0 && size <= *max_bytes),
            Self::InstallWindowPage {
                page_index,
                max_bytes,
                expected_size,
                ..
            } => {
                *page_index > 0
                    && usize::from(*page_index)
                        < ferric_browser_core::versioning::limits::MAX_PAGES_PER_WINDOW_CAPTURE
                    && *max_bytes > 0
                    && expected_size.is_none_or(|size| size > 0 && size <= *max_bytes)
            }
            Self::InstallSidecarBytes { bytes, .. } => {
                !bytes.is_empty()
                    && bytes.len() <= ferric_browser_core::versioning::MAX_PAGE_SIDECAR_BYTES
            }
            Self::MaterializeArchiveView {
                digest_hex,
                expected_bytes,
                ..
            } => {
                digest_hex.len() == 64
                    && digest_hex
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                    && *expected_bytes > 0
                    && *expected_bytes <= ferric_browser_core::versioning::limits::MAX_ARCHIVE_BYTES
            }
        }
    }
}

enum Message {
    Execute(VersioningObjectCommand, Arc<AtomicBool>),
    Shutdown,
}

#[cfg(test)]
struct InstallTestHook {
    operation: OperationId,
    reached: mpsc::Sender<()>,
    resume: mpsc::Receiver<()>,
}

#[cfg(test)]
static INSTALL_TEST_HOOK: std::sync::Mutex<Option<InstallTestHook>> = std::sync::Mutex::new(None);

#[cfg(test)]
fn take_install_test_hook(operation: OperationId) -> Option<InstallTestHook> {
    let mut hook = INSTALL_TEST_HOOK.lock().expect("install test hook lock");
    if hook
        .as_ref()
        .is_some_and(|pending| pending.operation == operation)
    {
        hook.take()
    } else {
        None
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum WorkKind {
    Stage,
    Install,
    ArchiveMaterialize,
    ArchiveRemove,
}

impl VersioningObjectCommand {
    const fn kind(&self) -> WorkKind {
        match self {
            Self::PrepareCapture { .. } | Self::PrepareWindowPage { .. } => WorkKind::Stage,
            Self::InstallCapture { .. }
            | Self::InstallWindowPage { .. }
            | Self::InstallSidecarBytes { .. } => WorkKind::Install,
            Self::MaterializeArchiveView { .. } => WorkKind::ArchiveMaterialize,
            Self::RemoveArchiveView { .. } => WorkKind::ArchiveRemove,
        }
    }
}

/// A closed failure category from the file-only worker. Error values never
/// carry paths, content digests, page data, or raw filesystem diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VersioningObjectFailure {
    WorkerStopped,
    Cancelled,
    CaptureStagingFailed,
    ObjectInstallFailed,
    StorageFull,
    SidecarInstallFailed,
    ArchiveMaterializationFailed,
    ArchiveCleanupFailed,
}

impl fmt::Display for VersioningObjectFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WorkerStopped => "versioning object worker stopped",
            Self::Cancelled => "versioning object job was cancelled",
            Self::CaptureStagingFailed => "versioning capture staging failed",
            Self::ObjectInstallFailed => "versioning object installation failed",
            Self::StorageFull => "versioning object installation could not reserve storage",
            Self::SidecarInstallFailed => "versioning sidecar installation failed",
            Self::ArchiveMaterializationFailed => "versioning archive materialization failed",
            Self::ArchiveCleanupFailed => "versioning archive cleanup failed",
        })
    }
}

impl std::error::Error for VersioningObjectFailure {}

/// An exactly-correlated completion from the file-only worker.
pub enum VersioningObjectCompletion {
    CapturePrepared {
        operation: OperationId,
        result: Result<CaptureStageTarget, VersioningObjectFailure>,
    },
    ObjectInstalled {
        operation: OperationId,
        result: Result<InstalledObject, VersioningObjectFailure>,
    },
    ArchiveViewMaterialized {
        operation: OperationId,
        result: Result<ArchiveViewFile, VersioningObjectFailure>,
    },
    ArchiveViewRemoved {
        operation: OperationId,
        result: Result<(), VersioningObjectFailure>,
    },
}

impl fmt::Debug for VersioningObjectCompletion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, succeeded) = match self {
            Self::CapturePrepared { result, .. } => ("CapturePrepared", result.is_ok()),
            Self::ObjectInstalled { result, .. } => ("ObjectInstalled", result.is_ok()),
            Self::ArchiveViewMaterialized { result, .. } => {
                ("ArchiveViewMaterialized", result.is_ok())
            }
            Self::ArchiveViewRemoved { result, .. } => ("ArchiveViewRemoved", result.is_ok()),
        };
        formatter
            .debug_struct("VersioningObjectCompletion")
            .field("kind", &kind)
            .field("operation", &"[redacted]")
            .field("succeeded", &succeeded)
            .finish()
    }
}

impl VersioningObjectCompletion {
    #[must_use]
    pub const fn operation_id(&self) -> OperationId {
        match self {
            Self::CapturePrepared { operation, .. }
            | Self::ObjectInstalled { operation, .. }
            | Self::ArchiveViewMaterialized { operation, .. }
            | Self::ArchiveViewRemoved { operation, .. } => *operation,
        }
    }
}

/// Owns one bounded file worker and its exact cancellation tokens.
pub struct VersioningObjectWorker {
    sender: Option<SyncSender<Message>>,
    completion_receiver: Receiver<VersioningObjectCompletion>,
    pending: BTreeMap<OperationId, (WorkKind, Arc<AtomicBool>)>,
    disconnected: bool,
    join: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for VersioningObjectWorker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VersioningObjectWorker")
            .field("pending_jobs", &self.pending.len())
            .field("disconnected", &self.disconnected)
            .finish_non_exhaustive()
    }
}

impl VersioningObjectWorker {
    /// Starts a file-only worker with one queued job and bounded completions.
    ///
    /// # Errors
    /// Returns a generic error if the worker thread cannot be spawned.
    pub fn spawn() -> Result<Self, String> {
        let (sender, requests) = mpsc::sync_channel(OBJECT_QUEUE_CAPACITY);
        let (completion_sender, completion_receiver) =
            mpsc::sync_channel(OBJECT_COMPLETION_CAPACITY);
        let join = thread::Builder::new()
            .name("ferric-browser-versioning-objects".into())
            .spawn(move || run_worker(&requests, &completion_sender))
            .map_err(|_| "versioning object worker could not start".to_owned())?;
        Ok(Self {
            sender: Some(sender),
            completion_receiver,
            pending: BTreeMap::new(),
            disconnected: false,
            join: Some(join),
        })
    }

    /// Submits one staging, object, or isolated-view job without waiting for
    /// bounded page-archive I/O.
    ///
    /// # Errors
    /// Rejects malformed limits, duplicate operation IDs, a full queue or a
    /// stopped worker.
    pub fn try_submit(
        &mut self,
        command: VersioningObjectCommand,
    ) -> Result<(), VersioningObjectWorkerError> {
        if !command.validate() {
            return Err(VersioningObjectWorkerError::InvalidRequest);
        }
        let operation = command.operation_id();
        if self.pending.contains_key(&operation) {
            return Err(VersioningObjectWorkerError::DuplicateOperation);
        }
        let kind = command.kind();
        if self
            .pending
            .values()
            .any(|(pending_kind, _)| *pending_kind == kind)
        {
            return Err(VersioningObjectWorkerError::Busy);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let sender = self
            .sender
            .as_ref()
            .ok_or(VersioningObjectWorkerError::Stopped)?;
        match sender.try_send(Message::Execute(command, Arc::clone(&cancel))) {
            Ok(()) => {
                self.pending.insert(operation, (kind, cancel));
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(VersioningObjectWorkerError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(VersioningObjectWorkerError::Stopped),
        }
    }

    /// Requests cancellation for only the exact operation. Hash/copy checks
    /// observe the token between bounded chunks.
    pub fn cancel(&mut self, operation: OperationId) -> bool {
        self.pending.get(&operation).is_some_and(|(_, token)| {
            token.store(true, Ordering::Release);
            true
        })
    }

    /// Requests cancellation for all admitted jobs before orderly retirement.
    pub fn cancel_all(&mut self) {
        for (_, token) in self.pending.values() {
            token.store(true, Ordering::Release);
        }
    }

    /// Drains completed jobs; operation IDs remain attached to every result.
    pub fn poll_events(&mut self) -> Vec<VersioningObjectCompletion> {
        let mut completions = Vec::new();
        loop {
            match self.completion_receiver.try_recv() {
                Ok(completion) => {
                    self.pending.remove(&completion.operation_id());
                    completions.push(completion);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.disconnected = true;
                    self.sender = None;
                    completions.extend(std::mem::take(&mut self.pending).into_iter().map(
                        |(operation, (kind, _))| match kind {
                            WorkKind::Stage => VersioningObjectCompletion::CapturePrepared {
                                operation,
                                result: Err(VersioningObjectFailure::WorkerStopped),
                            },
                            WorkKind::Install => VersioningObjectCompletion::ObjectInstalled {
                                operation,
                                result: Err(VersioningObjectFailure::WorkerStopped),
                            },
                            WorkKind::ArchiveMaterialize => {
                                VersioningObjectCompletion::ArchiveViewMaterialized {
                                    operation,
                                    result: Err(VersioningObjectFailure::WorkerStopped),
                                }
                            }
                            WorkKind::ArchiveRemove => {
                                VersioningObjectCompletion::ArchiveViewRemoved {
                                    operation,
                                    result: Err(VersioningObjectFailure::WorkerStopped),
                                }
                            }
                        },
                    ));
                    break;
                }
            }
        }
        completions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VersioningObjectWorkerError {
    DuplicateOperation,
    Busy,
    QueueFull,
    Stopped,
    InvalidRequest,
}

impl Drop for VersioningObjectWorker {
    fn drop(&mut self) {
        self.cancel_all();
        if let Some(sender) = self.sender.take() {
            let _ = sender.try_send(Message::Shutdown);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn run_worker(requests: &Receiver<Message>, completions: &SyncSender<VersioningObjectCompletion>) {
    while let Ok(message) = requests.recv() {
        let Message::Execute(command, cancelled) = message else {
            break;
        };
        if completions
            .send(execute_command(command, &cancelled))
            .is_err()
        {
            break;
        }
    }
}

fn execute_command(
    command: VersioningObjectCommand,
    cancelled: &AtomicBool,
) -> VersioningObjectCompletion {
    match command {
        VersioningObjectCommand::PrepareCapture {
            operation,
            profile_uuid,
            state_root,
        } => prepare_capture(operation, profile_uuid, &state_root, cancelled),
        VersioningObjectCommand::PrepareWindowPage {
            operation,
            profile_uuid,
            state_root,
            page_index,
        } => prepare_window_page(
            operation,
            profile_uuid,
            &state_root,
            usize::from(page_index),
            cancelled,
        ),
        VersioningObjectCommand::InstallCapture {
            operation,
            profile_uuid,
            state_root,
            max_bytes,
            expected_size,
            expected_sha256,
        } => install_capture(
            operation,
            profile_uuid,
            &state_root,
            0,
            max_bytes,
            expected_size,
            expected_sha256,
            cancelled,
        ),
        VersioningObjectCommand::InstallWindowPage {
            operation,
            profile_uuid,
            state_root,
            page_index,
            max_bytes,
            expected_size,
            expected_sha256,
        } => install_capture(
            operation,
            profile_uuid,
            &state_root,
            usize::from(page_index),
            max_bytes,
            expected_size,
            expected_sha256,
            cancelled,
        ),
        VersioningObjectCommand::InstallSidecarBytes {
            operation,
            profile_uuid,
            state_root,
            bytes,
        } => install_sidecar(operation, profile_uuid, &state_root, &bytes, cancelled),
        VersioningObjectCommand::MaterializeArchiveView {
            operation,
            profile_uuid,
            state_root,
            digest_hex,
            expected_bytes,
        } => materialize_archive_view(
            operation,
            profile_uuid,
            &state_root,
            &digest_hex,
            expected_bytes,
            cancelled,
        ),
        VersioningObjectCommand::RemoveArchiveView {
            operation,
            profile_uuid,
            state_root,
        } => remove_archive_view(operation, profile_uuid, &state_root),
    }
}

fn prepare_capture(
    operation: OperationId,
    profile_uuid: Uuid,
    state_root: &Path,
    cancelled: &AtomicBool,
) -> VersioningObjectCompletion {
    let result = if cancelled.load(Ordering::Acquire) {
        Err(VersioningObjectFailure::Cancelled)
    } else {
        VersioningObjectStore::open(state_root, profile_uuid)
            .and_then(|store| store.begin(operation))
            .map(|stage| CaptureStageTarget {
                operation,
                path: stage.capture_path().to_owned(),
            })
            .map_err(|_| VersioningObjectFailure::CaptureStagingFailed)
    };
    VersioningObjectCompletion::CapturePrepared { operation, result }
}

fn prepare_window_page(
    operation: OperationId,
    profile_uuid: Uuid,
    state_root: &Path,
    page_index: usize,
    cancelled: &AtomicBool,
) -> VersioningObjectCompletion {
    let result = if cancelled.load(Ordering::Acquire) {
        Err(VersioningObjectFailure::Cancelled)
    } else {
        VersioningObjectStore::open(state_root, profile_uuid)
            .and_then(|store| store.prepare_window_page(operation, page_index))
            .map(|stage| CaptureStageTarget {
                operation,
                path: stage.capture_path().to_owned(),
            })
            .map_err(|_| VersioningObjectFailure::CaptureStagingFailed)
    };
    VersioningObjectCompletion::CapturePrepared { operation, result }
}

fn install_capture(
    operation: OperationId,
    profile_uuid: Uuid,
    state_root: &Path,
    page_index: usize,
    max_bytes: u64,
    expected_size: Option<u64>,
    expected_sha256: Option<[u8; 32]>,
    cancelled: &AtomicBool,
) -> VersioningObjectCompletion {
    #[cfg(test)]
    let mut test_hook = take_install_test_hook(operation);
    #[cfg(test)]
    let mut cancel_checks = 0;
    #[cfg(feature = "native-archive-fault-test")]
    let mut native_cancel_checks = 0;
    let result = VersioningObjectStore::open(state_root, profile_uuid)
        .and_then(|store| {
            store
                .open_staged_window_page(operation, page_index)
                .and_then(|stage| {
                    stage.install_object_cancellable(
                        max_bytes,
                        expected_size,
                        expected_sha256,
                        || {
                            #[cfg(feature = "native-archive-fault-test")]
                            {
                                native_cancel_checks += 1;
                                if native_cancel_checks == 2
                                    && PAUSE_NEXT_ARCHIVE_INSTALL.swap(false, Ordering::SeqCst)
                                {
                                    use std::time::{Duration, Instant};

                                    ARCHIVE_INSTALL_PAUSED.store(true, Ordering::SeqCst);
                                    let deadline = Instant::now() + Duration::from_secs(10);
                                    // Model an in-flight filesystem operation that shutdown
                                    // cannot interrupt halfway through. The test releases this
                                    // pause explicitly after asserting the UI remains responsive;
                                    // cancellation is observed at the next normal chunk boundary.
                                    while !RESUME_ARCHIVE_INSTALL.load(Ordering::SeqCst)
                                        && Instant::now() < deadline
                                    {
                                        thread::sleep(Duration::from_millis(1));
                                    }
                                    ARCHIVE_INSTALL_PAUSED.store(false, Ordering::SeqCst);
                                    if !RESUME_ARCHIVE_INSTALL.swap(false, Ordering::SeqCst) {
                                        return true;
                                    }
                                }
                            }
                            #[cfg(test)]
                            {
                                cancel_checks += 1;
                                // The first check precedes the first read. Pause on the
                                // second so this witness is inside a real blob scan.
                                if cancel_checks == 2 {
                                    if let Some(hook) = test_hook.take() {
                                        let _ = hook.reached.send(());
                                        if hook
                                            .resume
                                            .recv_timeout(std::time::Duration::from_secs(5))
                                            .is_err()
                                        {
                                            return true;
                                        }
                                    }
                                }
                            }
                            cancelled.load(Ordering::Acquire)
                        },
                    )
                })
        })
        .map(|object| InstalledObject {
            sha256: object.sha256(),
            bytes: object.bytes(),
            reused: object.reused(),
        })
        .map_err(|error| match error {
            ObjectPathError::Cancelled => VersioningObjectFailure::Cancelled,
            #[cfg(unix)]
            ObjectPathError::Io(ref error) if error.raw_os_error() == Some(libc::ENOSPC) => {
                VersioningObjectFailure::StorageFull
            }
            _ => VersioningObjectFailure::ObjectInstallFailed,
        });
    VersioningObjectCompletion::ObjectInstalled { operation, result }
}

fn install_sidecar(
    operation: OperationId,
    profile_uuid: Uuid,
    state_root: &Path,
    bytes: &[u8],
    cancelled: &AtomicBool,
) -> VersioningObjectCompletion {
    #[cfg(feature = "native-archive-fault-test")]
    if !cancelled.load(Ordering::Acquire)
        && REFUSE_NEXT_SIDECAR_INSTALL.swap(false, Ordering::SeqCst)
    {
        INJECTED_SIDECAR_INSTALL_REFUSALS.fetch_add(1, Ordering::SeqCst);
        return VersioningObjectCompletion::ObjectInstalled {
            operation,
            result: Err(VersioningObjectFailure::SidecarInstallFailed),
        };
    }
    #[cfg(feature = "native-archive-fault-test")]
    if PAUSE_NEXT_SIDECAR_INSTALL.swap(false, Ordering::SeqCst) {
        use std::time::{Duration, Instant};

        SIDECAR_INSTALL_PAUSED.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !RESUME_SIDECAR_INSTALL.load(Ordering::SeqCst)
            && !cancelled.load(Ordering::Acquire)
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(1));
        }
        SIDECAR_INSTALL_PAUSED.store(false, Ordering::SeqCst);
        if !RESUME_SIDECAR_INSTALL.swap(false, Ordering::SeqCst) {
            return VersioningObjectCompletion::ObjectInstalled {
                operation,
                result: Err(VersioningObjectFailure::SidecarInstallFailed),
            };
        }
    }
    let result = if cancelled.load(Ordering::Acquire) {
        Err(VersioningObjectFailure::Cancelled)
    } else {
        VersioningObjectStore::open(state_root, profile_uuid)
            .and_then(|store| {
                store.install_sidecar_bytes_cancellable(operation, bytes, || {
                    cancelled.load(Ordering::Acquire)
                })
            })
            .map(|object| InstalledObject {
                sha256: object.sha256(),
                bytes: object.bytes(),
                reused: object.reused(),
            })
            .map_err(|error| match error {
                ObjectPathError::Cancelled => VersioningObjectFailure::Cancelled,
                #[cfg(feature = "native-archive-fault-test")]
                ObjectPathError::Io(ref error)
                    if error.kind() == std::io::ErrorKind::PermissionDenied =>
                {
                    SIDECAR_PERMISSION_DENIALS.fetch_add(1, Ordering::SeqCst);
                    VersioningObjectFailure::SidecarInstallFailed
                }
                _ => VersioningObjectFailure::SidecarInstallFailed,
            })
    };
    VersioningObjectCompletion::ObjectInstalled { operation, result }
}

fn materialize_archive_view(
    operation: OperationId,
    profile_uuid: Uuid,
    state_root: &Path,
    digest_hex: &str,
    expected_bytes: u64,
    cancelled: &AtomicBool,
) -> VersioningObjectCompletion {
    let result = if cancelled.load(Ordering::Acquire) {
        Err(VersioningObjectFailure::Cancelled)
    } else {
        VersioningObjectStore::open(state_root, profile_uuid)
            .and_then(|store| {
                store.materialize_archive_view_cancellable(
                    operation,
                    digest_hex,
                    expected_bytes,
                    || cancelled.load(Ordering::Acquire),
                )
            })
            .map_err(|error| match error {
                ObjectPathError::Cancelled => VersioningObjectFailure::Cancelled,
                _ => VersioningObjectFailure::ArchiveMaterializationFailed,
            })
    };
    VersioningObjectCompletion::ArchiveViewMaterialized { operation, result }
}

fn remove_archive_view(
    operation: OperationId,
    profile_uuid: Uuid,
    state_root: &Path,
) -> VersioningObjectCompletion {
    #[cfg(feature = "native-archive-fault-test")]
    if REFUSE_NEXT_ARCHIVE_CLEANUP.swap(false, Ordering::SeqCst) {
        INJECTED_ARCHIVE_CLEANUP_REFUSALS.fetch_add(1, Ordering::SeqCst);
        return VersioningObjectCompletion::ArchiveViewRemoved {
            operation,
            result: Err(VersioningObjectFailure::ArchiveCleanupFailed),
        };
    }
    let result = VersioningObjectStore::open(state_root, profile_uuid)
        .and_then(|store| store.discard_archive_view(operation))
        .map_err(|_| VersioningObjectFailure::ArchiveCleanupFailed);
    VersioningObjectCompletion::ArchiveViewRemoved { operation, result }
}

#[cfg(all(test, unix, not(any(target_os = "redox", target_os = "espidf"))))]
mod tests {
    use super::super::sha256_hex;
    use super::*;

    #[test]
    fn object_worker_debug_redacts_operation_paths_and_content_digests() {
        let operation = OperationId::fresh();
        let sentinel = "private-object-worker-debug-sentinel";
        let digest = [0xA5; 32];
        let stage = CaptureStageTarget {
            operation,
            path: PathBuf::from(format!("/private/{sentinel}/stage")),
        };
        let archive = ArchiveViewFile {
            operation,
            path: PathBuf::from(format!("/private/{sentinel}/archive.mhtml")),
            bytes: 12,
        };
        let installed = InstalledObject {
            sha256: digest,
            bytes: 12,
            reused: false,
        };
        let completion = VersioningObjectCompletion::ObjectInstalled {
            operation,
            result: Ok(installed),
        };
        let projections = [
            format!("{stage:?}"),
            format!("{archive:?}"),
            format!("{installed:?}"),
            format!("{completion:?}"),
        ];
        let operation_text = operation.to_string();
        let digest_text = "a5".repeat(32);

        for projection in projections {
            assert!(!projection.contains(&operation_text));
            assert!(!projection.contains(sentinel));
            assert!(!projection.contains(&digest_text));
        }
    }
    use crate::{ProfileStore, ProfileStoreWorker, RootSpec, StorageRoots, StoreMode};
    use sha2::{Digest, Sha256};
    use std::{fs, thread, time::Duration};

    fn poll_for(
        worker: &mut VersioningObjectWorker,
        operation: OperationId,
    ) -> VersioningObjectCompletion {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(completion) = worker
                .poll_events()
                .into_iter()
                .find(|completion| completion.operation_id() == operation)
            {
                return completion;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "object worker timed out"
            );
            thread::yield_now();
        }
    }

    #[test]
    fn sidecar_install_command_debug_never_formats_payload_bytes() {
        let command = VersioningObjectCommand::InstallSidecarBytes {
            operation: OperationId::fresh(),
            profile_uuid: Uuid::new_v4(),
            state_root: PathBuf::from("/private/state"),
            bytes: b"sensitive-page-value".to_vec(),
        };
        let debug = format!("{command:?}");
        assert!(!debug.contains("sensitive-page-value"));
        assert!(debug.contains("[redacted]"));
    }

    #[test]
    fn file_worker_stages_and_installs_distinct_window_pages() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        roots.ensure().expect("private roots");
        let profile_uuid = Uuid::new_v4();
        let operation = OperationId::fresh();
        let mut worker = VersioningObjectWorker::spawn().expect("file worker");
        worker
            .try_submit(VersioningObjectCommand::PrepareCapture {
                operation,
                profile_uuid,
                state_root: roots.state.clone(),
            })
            .expect("first page stage accepted");
        let VersioningObjectCompletion::CapturePrepared {
            result: Ok(first), ..
        } = poll_for(&mut worker, operation)
        else {
            panic!("first stage required");
        };
        worker
            .try_submit(VersioningObjectCommand::PrepareWindowPage {
                operation,
                profile_uuid,
                state_root: roots.state.clone(),
                page_index: 1,
            })
            .expect("second page stage accepted");
        let VersioningObjectCompletion::CapturePrepared {
            result: Ok(second), ..
        } = poll_for(&mut worker, operation)
        else {
            panic!("second stage required");
        };
        assert_ne!(first.path(), second.path());
        fs::write(first.path(), b"first archive").expect("first native save");
        fs::write(second.path(), b"second archive").expect("second native save");
        worker
            .try_submit(VersioningObjectCommand::InstallCapture {
                operation,
                profile_uuid,
                state_root: roots.state.clone(),
                max_bytes: 64,
                expected_size: Some(13),
                expected_sha256: None,
            })
            .expect("first install accepted");
        let VersioningObjectCompletion::ObjectInstalled {
            result: Ok(first_object),
            ..
        } = poll_for(&mut worker, operation)
        else {
            panic!("first object required");
        };
        worker
            .try_submit(VersioningObjectCommand::InstallWindowPage {
                operation,
                profile_uuid,
                state_root: roots.state.clone(),
                page_index: 1,
                max_bytes: 64,
                expected_size: Some(14),
                expected_sha256: None,
            })
            .expect("second install accepted");
        let VersioningObjectCompletion::ObjectInstalled {
            result: Ok(second_object),
            ..
        } = poll_for(&mut worker, operation)
        else {
            panic!("second object required");
        };
        assert_ne!(first_object.sha256, second_object.sha256);
        roots.cleanup().expect("temporary roots cleanup");
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn file_worker_stages_and_installs_one_exact_synthetic_capture() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        roots.ensure().expect("private roots");
        let profile_uuid = Uuid::new_v4();
        let operation = OperationId::fresh();
        let mut worker = VersioningObjectWorker::spawn().expect("file worker");
        worker
            .try_submit(VersioningObjectCommand::PrepareCapture {
                operation,
                profile_uuid,
                state_root: roots.state.clone(),
            })
            .expect("stage accepted");
        let staged_completion = poll_for(&mut worker, operation);
        let VersioningObjectCompletion::CapturePrepared {
            result: Ok(stage), ..
        } = &staged_completion
        else {
            panic!("exact staging result expected");
        };
        let private_stage_path = stage.path().display().to_string();
        let staged_debug = format!("{staged_completion:?}");
        assert!(!staged_debug.contains(&private_stage_path));
        let VersioningObjectCompletion::CapturePrepared {
            operation: staged_operation,
            result: Ok(stage),
        } = staged_completion
        else {
            panic!("exact staging result expected");
        };
        assert_eq!(staged_operation, operation);
        assert_eq!(stage.operation(), operation);
        fs::write(stage.path(), b"synthetic archive bytes").expect("write fixture");
        let bytes = b"synthetic archive bytes";
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        worker
            .try_submit(VersioningObjectCommand::InstallCapture {
                operation,
                profile_uuid,
                state_root: roots.state.clone(),
                max_bytes: 1024,
                expected_size: Some(bytes.len() as u64),
                expected_sha256: Some(digest),
            })
            .expect("install accepted");
        let install_completion = poll_for(&mut worker, operation);
        let VersioningObjectCompletion::ObjectInstalled {
            result: Ok(object), ..
        } = &install_completion
        else {
            panic!("validated object result expected");
        };
        let object_digest_hex = sha256_hex(&object.sha256);
        let install_debug = format!("{install_completion:?}");
        assert!(!install_debug.contains(&private_stage_path));
        assert!(!install_debug.contains(&object_digest_hex));
        assert!(!install_debug.contains("synthetic archive bytes"));
        let VersioningObjectCompletion::ObjectInstalled {
            operation: installed_operation,
            result: Ok(object),
        } = install_completion
        else {
            panic!("validated object result expected");
        };
        assert_eq!(installed_operation, operation);
        assert_eq!(object.sha256, digest);
        assert_eq!(object.bytes, bytes.len() as u64);

        let view_operation = OperationId::fresh();
        worker
            .try_submit(VersioningObjectCommand::MaterializeArchiveView {
                operation: view_operation,
                profile_uuid,
                state_root: roots.state.clone(),
                digest_hex: sha256_hex(&object.sha256),
                expected_bytes: object.bytes,
            })
            .expect("bounded archive materialization accepted");
        let archive_view_completion = poll_for(&mut worker, view_operation);
        let VersioningObjectCompletion::ArchiveViewMaterialized {
            result: Ok(view), ..
        } = &archive_view_completion
        else {
            panic!("verified isolated-view file expected");
        };
        let private_view_path = view.path().display().to_string();
        let archive_view_debug = format!("{archive_view_completion:?}");
        assert!(!archive_view_debug.contains(&private_view_path));
        assert!(!archive_view_debug.contains(&object_digest_hex));
        let VersioningObjectCompletion::ArchiveViewMaterialized {
            operation: view_result_operation,
            result: Ok(view),
        } = archive_view_completion
        else {
            panic!("verified isolated-view file expected");
        };
        assert_eq!(view_result_operation, view_operation);
        assert_eq!(fs::read(view.path()).expect("view bytes"), bytes);
        worker
            .try_submit(VersioningObjectCommand::RemoveArchiveView {
                operation: view_operation,
                profile_uuid,
                state_root: roots.state.clone(),
            })
            .expect("exact view cleanup accepted");
        assert!(matches!(
            poll_for(&mut worker, view_operation),
            VersioningObjectCompletion::ArchiveViewRemoved {
                operation,
                result: Ok(()),
            } if operation == view_operation
        ));
        assert!(!view.path().exists());
        drop(worker);
        roots.cleanup().expect("cleanup temporary roots");
    }

    #[test]
    fn same_kind_work_is_bounded_and_correlated_to_its_operation() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        roots.ensure().expect("private roots");
        let profile_uuid = Uuid::new_v4();
        let first = OperationId::fresh();
        let second = OperationId::fresh();
        let mut worker = VersioningObjectWorker::spawn().expect("file worker");
        worker
            .try_submit(VersioningObjectCommand::PrepareCapture {
                operation: first,
                profile_uuid,
                state_root: roots.state.clone(),
            })
            .expect("first stage accepted");
        assert_eq!(
            worker.try_submit(VersioningObjectCommand::PrepareCapture {
                operation: second,
                profile_uuid,
                state_root: roots.state.clone(),
            }),
            Err(VersioningObjectWorkerError::Busy)
        );
        assert!(matches!(
            poll_for(&mut worker, first),
            VersioningObjectCompletion::CapturePrepared {
                operation,
                result: Ok(_),
            } if operation == first
        ));
        worker
            .try_submit(VersioningObjectCommand::PrepareCapture {
                operation: second,
                profile_uuid,
                state_root: roots.state.clone(),
            })
            .expect("second stage accepted after the first completed");
        assert!(matches!(
            poll_for(&mut worker, second),
            VersioningObjectCompletion::CapturePrepared {
                operation,
                result: Ok(_),
            } if operation == second
        ));
        drop(worker);
        roots.cleanup().expect("cleanup temporary roots");
    }

    #[test]
    fn metadata_query_completes_while_large_archive_scan_is_in_flight() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        roots.ensure().expect("private roots");
        let database = roots.data.join("metadata.sqlite");
        let store = ProfileStore::open(&database, StoreMode::Normal).expect("metadata database");
        store
            .record_visit("https://example.test/archive", "Archive", "navigate", 1)
            .expect("fixture visit");
        drop(store);
        let mut metadata = ProfileStoreWorker::spawn(&database).expect("metadata worker");

        let profile_uuid = Uuid::new_v4();
        let operation = OperationId::fresh();
        let object_store =
            VersioningObjectStore::open(&roots.state, profile_uuid).expect("private object store");
        let stage = object_store.begin(operation).expect("exact stage");
        let archive_bytes = vec![0x5a; 4 * 1024 * 1024];
        fs::write(stage.capture_path(), &archive_bytes).expect("bounded archive fixture");
        let (reached_tx, reached_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        *INSTALL_TEST_HOOK.lock().expect("install test hook lock") = Some(InstallTestHook {
            operation,
            reached: reached_tx,
            resume: resume_rx,
        });
        let mut objects = VersioningObjectWorker::spawn().expect("file worker");
        objects
            .try_submit(VersioningObjectCommand::InstallCapture {
                operation,
                profile_uuid,
                state_root: roots.state.clone(),
                max_bytes: 8 * 1024 * 1024,
                expected_size: Some(archive_bytes.len() as u64),
                expected_sha256: None,
            })
            .expect("bounded install admitted");
        reached_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("real archive scan reached its second chunk");

        metadata
            .request_library(10)
            .expect("independent metadata query");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let result = loop {
            if let Some(result) = metadata.poll_library() {
                break result;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "metadata query stalled behind blob I/O"
            );
            thread::yield_now();
        };
        assert_eq!(result.expect("metadata query").history.len(), 1);
        assert!(
            objects.poll_events().is_empty(),
            "blob worker is still paused mid-scan"
        );
        resume_tx.send(()).expect("release bounded archive scan");
        assert!(matches!(
            poll_for(&mut objects, operation),
            VersioningObjectCompletion::ObjectInstalled {
                operation: completed,
                result: Ok(_),
            } if completed == operation
        ));
        drop(metadata);
        drop(objects);
        roots.cleanup().expect("cleanup temporary roots");
    }

    #[test]
    fn worker_command_debug_output_redacts_paths_and_content_digest() {
        let command = VersioningObjectCommand::InstallCapture {
            operation: OperationId::fresh(),
            profile_uuid: Uuid::new_v4(),
            state_root: PathBuf::from("/private/state/root"),
            max_bytes: 1024,
            expected_size: Some(64),
            expected_sha256: Some([0xaa; 32]),
        };
        let debug = format!("{command:?}");
        let digest_debug = format!("{:?}", [0xaa; 32]);
        assert!(!debug.contains("/private/state/root"));
        assert!(!debug.contains(&digest_debug));
    }

    #[test]
    fn worker_completion_debug_redacts_operation_and_failure_details() {
        let sentinels = [
            "/private/profile/archive.mhtml",
            "synthetic-page-secret",
            "content-digest-sentinel",
        ];
        let completions = [
            (
                VersioningObjectCompletion::CapturePrepared {
                    operation: OperationId::fresh(),
                    result: Err(VersioningObjectFailure::WorkerStopped),
                },
                "CapturePrepared",
                "WorkerStopped",
            ),
            (
                VersioningObjectCompletion::ObjectInstalled {
                    operation: OperationId::fresh(),
                    result: Err(VersioningObjectFailure::Cancelled),
                },
                "ObjectInstalled",
                "Cancelled",
            ),
            (
                VersioningObjectCompletion::ArchiveViewMaterialized {
                    operation: OperationId::fresh(),
                    result: Err(VersioningObjectFailure::ArchiveMaterializationFailed),
                },
                "ArchiveViewMaterialized",
                "ArchiveMaterializationFailed",
            ),
            (
                VersioningObjectCompletion::ArchiveViewRemoved {
                    operation: OperationId::fresh(),
                    result: Err(VersioningObjectFailure::ArchiveCleanupFailed),
                },
                "ArchiveViewRemoved",
                "ArchiveCleanupFailed",
            ),
            (
                VersioningObjectCompletion::CapturePrepared {
                    operation: OperationId::fresh(),
                    result: Err(VersioningObjectFailure::CaptureStagingFailed),
                },
                "CapturePrepared",
                "CaptureStagingFailed",
            ),
            (
                VersioningObjectCompletion::ObjectInstalled {
                    operation: OperationId::fresh(),
                    result: Err(VersioningObjectFailure::ObjectInstallFailed),
                },
                "ObjectInstalled",
                "ObjectInstallFailed",
            ),
            (
                VersioningObjectCompletion::ObjectInstalled {
                    operation: OperationId::fresh(),
                    result: Err(VersioningObjectFailure::StorageFull),
                },
                "ObjectInstalled",
                "StorageFull",
            ),
            (
                VersioningObjectCompletion::ObjectInstalled {
                    operation: OperationId::fresh(),
                    result: Err(VersioningObjectFailure::SidecarInstallFailed),
                },
                "ObjectInstalled",
                "SidecarInstallFailed",
            ),
        ];
        for (completion, kind, failure_detail) in completions {
            let debug = format!("{completion:?}");
            for sentinel in &sentinels {
                assert!(!debug.contains(*sentinel));
            }
            assert!(debug.contains(kind));
            assert!(debug.contains("succeeded: false"));
            assert!(!debug.contains(failure_detail));
        }
    }

    #[test]
    fn worker_failure_display_uses_only_fixed_categories() {
        let sentinels = [
            "/private/profile/archive.mhtml",
            "synthetic-page-secret",
            "content-digest-sentinel",
        ];
        let failures = [
            (
                VersioningObjectFailure::WorkerStopped,
                "versioning object worker stopped",
            ),
            (
                VersioningObjectFailure::Cancelled,
                "versioning object job was cancelled",
            ),
            (
                VersioningObjectFailure::CaptureStagingFailed,
                "versioning capture staging failed",
            ),
            (
                VersioningObjectFailure::ObjectInstallFailed,
                "versioning object installation failed",
            ),
            (
                VersioningObjectFailure::StorageFull,
                "versioning object installation could not reserve storage",
            ),
            (
                VersioningObjectFailure::SidecarInstallFailed,
                "versioning sidecar installation failed",
            ),
            (
                VersioningObjectFailure::ArchiveMaterializationFailed,
                "versioning archive materialization failed",
            ),
            (
                VersioningObjectFailure::ArchiveCleanupFailed,
                "versioning archive cleanup failed",
            ),
        ];
        for (failure, expected) in failures {
            let rendered = failure.to_string();
            assert_eq!(rendered, expected);
            for sentinel in &sentinels {
                assert!(!rendered.contains(*sentinel));
            }
        }
    }
}
