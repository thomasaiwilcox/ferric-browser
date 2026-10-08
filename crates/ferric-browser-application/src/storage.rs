use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicU8, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

use ferric_browser_storage::versioning::{
    VersioningObjectCommand, VersioningObjectCompletion, VersioningObjectFailure,
    VersioningObjectWorker, VersioningObjectWorkerError,
};
use ferric_browser_storage::{
    ProfileStoreWorker, StorageCommand, StorageCompletion, StorageFailureKind, StorageOperation,
    StorageWorkerError,
};

const RETIRE_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_PENDING_REQUESTS: usize = 512;
const RECOVERY_PENDING: u8 = 0;
const RECOVERY_READY: u8 = 1;
const RECOVERY_FAILED: u8 = 2;

struct ProfileRecovery {
    state_root: PathBuf,
    profile_uuid: Uuid,
    status: AtomicU8,
}

type RecoveryRegistry = BTreeMap<PathBuf, Weak<ProfileRecovery>>;
static PROFILE_RECOVERIES: OnceLock<Mutex<RecoveryRegistry>> = OnceLock::new();

fn shared_profile_recovery(
    database: &Path,
    state_root: &Path,
    profile_uuid: Uuid,
) -> Result<(Arc<ProfileRecovery>, bool), StorageWorkerError> {
    let database = fs::canonicalize(database).or_else(|_| {
        let parent = database.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "profile path has no parent",
            )
        })?;
        let name = database.file_name().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "profile path has no filename",
            )
        })?;
        fs::canonicalize(parent).map(|parent| parent.join(name))
    });
    let database = database.map_err(|_| {
        StorageWorkerError::Spawn("profile recovery identity is unavailable".to_owned())
    })?;
    let state_root = fs::canonicalize(state_root).map_err(|_| {
        StorageWorkerError::Spawn("profile recovery root is unavailable".to_owned())
    })?;
    let registry = PROFILE_RECOVERIES.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut registry = registry.lock().map_err(|_| {
        StorageWorkerError::Spawn("profile recovery registry is unavailable".to_owned())
    })?;
    registry.retain(|_, recovery| recovery.strong_count() != 0);
    if let Some(recovery) = registry.get(&database).and_then(Weak::upgrade) {
        if recovery.profile_uuid != profile_uuid || recovery.state_root != state_root {
            return Err(StorageWorkerError::InvalidBatch);
        }
        return Ok((recovery, false));
    }
    let recovery = Arc::new(ProfileRecovery {
        state_root,
        profile_uuid,
        status: AtomicU8::new(RECOVERY_PENDING),
    });
    registry.insert(database, Arc::downgrade(&recovery));
    Ok((recovery, true))
}

/// An application-owned identity for one admitted durable request.
///
/// The ticket lets presentation code associate a typed completion with its
/// own transient UI metadata without owning persistence scheduling.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StorageTicket(u64);

impl StorageTicket {
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// A typed storage completion correlated to the intent that produced it.
#[derive(Debug)]
pub struct StorageEffect {
    pub ticket: StorageTicket,
    pub completion: StorageCompletion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CancellationReason {
    ProfileClosed,
    ProfileReplaced,
}

#[derive(Clone, Debug)]
struct ScheduledRequest {
    ticket: StorageTicket,
    command: ScheduledCommand,
}

#[derive(Clone, Debug)]
enum ScheduledCommand {
    Metadata(Box<StorageCommand>),
    VersioningObject(VersioningObjectCommand),
}

impl ScheduledCommand {
    const fn operation(&self) -> StorageOperation {
        match self {
            Self::Metadata(command) => command.operation(),
            Self::VersioningObject(
                VersioningObjectCommand::PrepareCapture { .. }
                | VersioningObjectCommand::PrepareWindowPage { .. },
            ) => StorageOperation::VersioningCaptureStage,
            Self::VersioningObject(
                VersioningObjectCommand::InstallCapture { .. }
                | VersioningObjectCommand::InstallWindowPage { .. }
                | VersioningObjectCommand::InstallSidecarBytes { .. },
            ) => StorageOperation::VersioningObjectInstall,
            Self::VersioningObject(VersioningObjectCommand::MaterializeArchiveView { .. }) => {
                StorageOperation::VersioningArchiveView
            }
            Self::VersioningObject(VersioningObjectCommand::RemoveArchiveView { .. }) => {
                StorageOperation::VersioningArchiveViewCleanup
            }
        }
    }
}

pub(crate) struct StorageCoordinator {
    worker: Option<ProfileStoreWorker>,
    object_worker: Option<VersioningObjectWorker>,
    pending: VecDeque<ScheduledRequest>,
    active: BTreeMap<StorageOperation, StorageTicket>,
    effects: VecDeque<StorageEffect>,
    next_ticket: u64,
    recovery_pending: bool,
    recovery_failed: bool,
    profile_recovery: Option<Arc<ProfileRecovery>>,
}

impl Default for StorageCoordinator {
    fn default() -> Self {
        Self {
            worker: None,
            object_worker: None,
            pending: VecDeque::new(),
            active: BTreeMap::new(),
            effects: VecDeque::new(),
            next_ticket: 1,
            recovery_pending: false,
            recovery_failed: false,
            profile_recovery: None,
        }
    }
}

impl Drop for StorageCoordinator {
    fn drop(&mut self) {
        if self.recovery_pending
            && let Some(recovery) = &self.profile_recovery
        {
            // A peer must not wait forever for the owner that initiated
            // recovery if that owner disappears before observing completion.
            recovery.status.store(RECOVERY_FAILED, Ordering::Release);
        }
    }
}

impl StorageCoordinator {
    pub(crate) fn replace(
        &mut self,
        path: Option<&Path>,
        versioning: Option<(&Path, Uuid)>,
    ) -> Result<(), StorageWorkerError> {
        // Recovery may move orphan files, so the prior worker must have
        // stopped before a replacement can inspect this profile.
        if versioning.is_some() {
            self.retire(CancellationReason::ProfileReplaced);
        }
        let mut object_replacement = versioning
            .map(|_| {
                VersioningObjectWorker::spawn().map_err(|_| {
                    StorageWorkerError::Spawn("versioning object worker could not start".to_owned())
                })
            })
            .transpose()?;
        let mut replacement = path.map(ProfileStoreWorker::spawn).transpose()?;
        let mut recovery_pending = false;
        let mut profile_recovery = None;
        if let (Some(worker), Some(database), Some((state_root, profile_uuid))) =
            (&mut replacement, path, versioning)
        {
            let (recovery, first_owner) =
                shared_profile_recovery(database, state_root, profile_uuid)?;
            if first_owner {
                if let Err(error) = worker.try_submit(StorageCommand::VersioningRecover {
                    profile_uuid,
                    state_root: state_root.to_owned(),
                }) {
                    recovery.status.store(RECOVERY_FAILED, Ordering::Release);
                    return Err(error);
                }
                recovery_pending = true;
            }
            profile_recovery = Some(recovery);
        }
        if versioning.is_none() {
            self.retire(CancellationReason::ProfileReplaced);
        }
        self.worker = replacement;
        self.object_worker = object_replacement.take();
        self.recovery_pending = recovery_pending;
        self.recovery_failed = false;
        self.profile_recovery = profile_recovery;
        Ok(())
    }

    pub(crate) fn close(&mut self) {
        self.retire(CancellationReason::ProfileClosed);
    }

    pub(crate) const fn is_open(&self) -> bool {
        self.worker.is_some()
    }

    /// Includes queued jobs until their worker completion is collected by `poll`.
    pub(crate) fn has_pending_versioning_object_work(&self) -> bool {
        self.active
            .keys()
            .any(|operation| is_versioning_object_operation(*operation))
            || self
                .pending
                .iter()
                .any(|request| matches!(&request.command, ScheduledCommand::VersioningObject(_)))
    }

    pub(crate) fn submit(
        &mut self,
        command: StorageCommand,
    ) -> Result<StorageTicket, StorageWorkerError> {
        let ticket = self.next_ticket();
        match self.submit_now(
            ticket,
            ScheduledCommand::Metadata(Box::new(command.clone())),
        ) {
            Ok(()) => Ok(ticket),
            Err(StorageWorkerError::Busy | StorageWorkerError::QueueFull) => {
                if self.pending.len() >= MAX_PENDING_REQUESTS {
                    return Err(StorageWorkerError::QueueFull);
                }
                self.pending.push_back(ScheduledRequest {
                    ticket,
                    command: ScheduledCommand::Metadata(Box::new(command)),
                });
                Ok(ticket)
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn submit_versioning_object(
        &mut self,
        command: VersioningObjectCommand,
    ) -> Result<StorageTicket, StorageWorkerError> {
        let ticket = self.next_ticket();
        match self.submit_now(ticket, ScheduledCommand::VersioningObject(command.clone())) {
            Ok(()) => Ok(ticket),
            Err(StorageWorkerError::Busy | StorageWorkerError::QueueFull) => {
                if self.pending.len() >= MAX_PENDING_REQUESTS {
                    return Err(StorageWorkerError::QueueFull);
                }
                self.pending.push_back(ScheduledRequest {
                    ticket,
                    command: ScheduledCommand::VersioningObject(command),
                });
                Ok(ticket)
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn poll(&mut self) -> Vec<StorageEffect> {
        self.collect_completions();
        self.submit_pending();
        self.effects.drain(..).collect()
    }

    pub(crate) fn cancel_versioning_object(&mut self, operation: ferric_browser_core::OperationId) {
        let mut kept = VecDeque::with_capacity(self.pending.len());
        while let Some(request) = self.pending.pop_front() {
            let matches = matches!(
                &request.command,
                ScheduledCommand::VersioningObject(command)
                    if command.operation_id() == operation
            );
            if matches {
                self.effects.push_back(StorageEffect {
                    ticket: request.ticket,
                    completion: StorageCompletion::failed(
                        request.command.operation(),
                        "versioning object job was cancelled",
                    ),
                });
            } else {
                kept.push_back(request);
            }
        }
        self.pending = kept;
        if let Some(worker) = self.object_worker.as_mut() {
            worker.cancel(operation);
        }
    }

    fn submit_pending(&mut self) {
        while let Some(request) = self.pending.front().cloned() {
            let operation = request.command.operation();
            match self.submit_now(request.ticket, request.command) {
                Ok(()) => {
                    self.pending.pop_front();
                }
                Err(StorageWorkerError::Busy | StorageWorkerError::QueueFull) => break,
                Err(error) => {
                    self.pending.pop_front();
                    self.effects.push_back(StorageEffect {
                        ticket: request.ticket,
                        completion: StorageCompletion::failed(operation, error.to_string()),
                    });
                }
            }
        }
    }

    fn collect_completions(&mut self) {
        let mut completions = self
            .worker
            .as_mut()
            .map(ProfileStoreWorker::poll_events)
            .unwrap_or_default();
        if let Some(worker) = self.object_worker.as_mut() {
            completions.extend(
                worker
                    .poll_events()
                    .into_iter()
                    .map(object_completion_to_storage),
            );
        }
        for completion in completions {
            let operation = completion.operation();
            if operation == StorageOperation::VersioningRecover && self.recovery_pending {
                self.recovery_pending = false;
                self.recovery_failed =
                    !matches!(completion, StorageCompletion::VersioningRecover(Ok(_)));
                if let Some(recovery) = &self.profile_recovery {
                    recovery.status.store(
                        if self.recovery_failed {
                            RECOVERY_FAILED
                        } else {
                            RECOVERY_READY
                        },
                        Ordering::Release,
                    );
                }
                continue;
            }
            if let Some(ticket) = self.active.remove(&operation) {
                self.effects.push_back(StorageEffect { ticket, completion });
            }
        }
    }

    fn retire(&mut self, reason: CancellationReason) {
        if self.worker.is_none() {
            if let Some(mut worker) = self.object_worker.take() {
                worker.cancel_all();
            }
            self.recovery_pending = false;
            self.recovery_failed = false;
            self.profile_recovery = None;
            self.cancel_outstanding(reason);
            return;
        }

        let deadline = Instant::now() + RETIRE_TIMEOUT;
        if let Some(worker) = self.object_worker.as_mut() {
            worker.cancel_all();
        }
        while (!self.pending.is_empty() || !self.active.is_empty() || self.recovery_pending)
            && Instant::now() < deadline
        {
            self.collect_completions();
            self.submit_pending();
            if !self.pending.is_empty() || !self.active.is_empty() {
                thread::sleep(Duration::from_millis(2));
            }
        }

        if self.pending.is_empty()
            && self.active.is_empty()
            && !self.recovery_pending
            && let Some(worker) = self.worker.as_mut()
            && worker.request_flush().is_ok()
        {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let _ = worker.wait_flush(remaining);
        }

        self.worker = None;
        self.object_worker = None;
        if self.recovery_pending
            && let Some(recovery) = &self.profile_recovery
        {
            recovery.status.store(RECOVERY_FAILED, Ordering::Release);
        }
        self.recovery_pending = false;
        self.recovery_failed = false;
        self.profile_recovery = None;
        self.cancel_outstanding(reason);
    }

    fn cancel_outstanding(&mut self, reason: CancellationReason) {
        let mut cancelled = self
            .active
            .iter()
            .map(|(operation, ticket)| StorageEffect {
                ticket: *ticket,
                completion: StorageCompletion::failed(
                    *operation,
                    format!("storage request cancelled: {reason:?}"),
                ),
            })
            .chain(self.pending.iter().map(|request| StorageEffect {
                ticket: request.ticket,
                completion: StorageCompletion::failed(
                    request.command.operation(),
                    format!("storage request cancelled: {reason:?}"),
                ),
            }))
            .collect::<Vec<_>>();
        cancelled.sort_by_key(|effect| effect.ticket);
        self.active.clear();
        self.pending.clear();
        self.effects.extend(cancelled);
    }

    fn next_ticket(&mut self) -> StorageTicket {
        let ticket = StorageTicket(self.next_ticket);
        self.next_ticket = self.next_ticket.saturating_add(1).max(1);
        ticket
    }

    fn submit_now(
        &mut self,
        ticket: StorageTicket,
        command: ScheduledCommand,
    ) -> Result<(), StorageWorkerError> {
        if matches!(
            &command,
            ScheduledCommand::Metadata(command)
                if matches!(
                    command.as_ref(),
                    StorageCommand::VersioningBegin { .. }
                        | StorageCommand::VersioningDelete { .. }
                        | StorageCommand::VersioningCollect { .. }
                        | StorageCommand::VersioningStructuralPublish { .. }
                        | StorageCommand::VersioningObjectPublish { .. }
                )
        ) || matches!(&command, ScheduledCommand::VersioningObject(_))
        {
            if let Some(recovery) = &self.profile_recovery {
                match recovery.status.load(Ordering::Acquire) {
                    RECOVERY_PENDING => return Err(StorageWorkerError::Busy),
                    RECOVERY_READY => {}
                    _ => return Err(StorageWorkerError::RecoveryFailed),
                }
            } else {
                if self.recovery_pending {
                    return Err(StorageWorkerError::Busy);
                }
                if self.recovery_failed {
                    return Err(StorageWorkerError::RecoveryFailed);
                }
            }
        }
        if matches!(
            &command,
            ScheduledCommand::Metadata(command)
                if matches!(command.as_ref(), StorageCommand::VersioningRecover { .. })
        ) {
            return Err(StorageWorkerError::InvalidBatch);
        }
        let operation = command.operation();
        match command {
            ScheduledCommand::Metadata(command) => {
                self.worker
                    .as_mut()
                    .ok_or(StorageWorkerError::Stopped)?
                    .try_submit(*command)?;
            }
            ScheduledCommand::VersioningObject(command) => {
                self.object_worker
                    .as_mut()
                    .ok_or(StorageWorkerError::Stopped)?
                    .try_submit(command)
                    .map_err(map_object_worker_error)?;
            }
        }
        self.active.insert(operation, ticket);
        Ok(())
    }
}

const fn is_versioning_object_operation(operation: StorageOperation) -> bool {
    matches!(
        operation,
        StorageOperation::VersioningCaptureStage
            | StorageOperation::VersioningObjectInstall
            | StorageOperation::VersioningArchiveView
            | StorageOperation::VersioningArchiveViewCleanup
    )
}

fn map_object_worker_error(error: VersioningObjectWorkerError) -> StorageWorkerError {
    match error {
        VersioningObjectWorkerError::DuplicateOperation
        | VersioningObjectWorkerError::InvalidRequest => StorageWorkerError::InvalidBatch,
        VersioningObjectWorkerError::Busy => StorageWorkerError::Busy,
        VersioningObjectWorkerError::QueueFull => StorageWorkerError::QueueFull,
        VersioningObjectWorkerError::Stopped => StorageWorkerError::Stopped,
    }
}

fn object_completion_to_storage(completion: VersioningObjectCompletion) -> StorageCompletion {
    match completion {
        VersioningObjectCompletion::CapturePrepared { result, .. } => {
            StorageCompletion::VersioningCaptureStage(
                result.map_err(|_| object_worker_failure(StorageOperation::VersioningCaptureStage)),
            )
        }
        VersioningObjectCompletion::ObjectInstalled { result, .. } => {
            StorageCompletion::VersioningObjectInstall(result.map_err(|failure| {
                let mut error = object_worker_failure(StorageOperation::VersioningObjectInstall);
                if failure == VersioningObjectFailure::StorageFull {
                    error.kind = StorageFailureKind::StorageFull;
                }
                error
            }))
        }
        VersioningObjectCompletion::ArchiveViewMaterialized { result, .. } => {
            StorageCompletion::VersioningArchiveView(
                result.map_err(|_| object_worker_failure(StorageOperation::VersioningArchiveView)),
            )
        }
        VersioningObjectCompletion::ArchiveViewRemoved { result, .. } => {
            StorageCompletion::VersioningArchiveViewCleanup(
                result.map_err(|_| {
                    object_worker_failure(StorageOperation::VersioningArchiveViewCleanup)
                }),
            )
        }
    }
}

fn object_worker_failure(
    operation: StorageOperation,
) -> ferric_browser_storage::StorageOperationError {
    // INVARIANT: only closed, operation-scoped diagnostics cross into the
    // application; low-level filesystem failures may contain private paths.
    let message = match operation {
        StorageOperation::VersioningCaptureStage => "versioning capture staging failed",
        StorageOperation::VersioningObjectInstall => "versioning object installation failed",
        StorageOperation::VersioningArchiveView => "versioning archive materialization failed",
        StorageOperation::VersioningArchiveViewCleanup => "versioning archive cleanup failed",
        _ => "versioning object operation failed",
    };
    ferric_browser_storage::StorageOperationError {
        operation,
        kind: StorageFailureKind::Other,
        message: message.to_owned(),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use ferric_browser_core::OperationId;
    use ferric_browser_storage::versioning::VersioningObjectFailure;
    use ferric_browser_storage::{RootSpec, StorageRoots};
    use std::{fs, os::unix::fs::symlink};

    #[test]
    fn shutdown_query_tracks_admitted_and_queued_object_worker_jobs() {
        let mut coordinator = StorageCoordinator::default();
        assert!(!coordinator.has_pending_versioning_object_work());

        coordinator
            .active
            .insert(StorageOperation::VersioningObjectInstall, StorageTicket(1));
        assert!(coordinator.has_pending_versioning_object_work());
        coordinator.active.clear();

        coordinator.pending.push_back(ScheduledRequest {
            ticket: StorageTicket(2),
            command: ScheduledCommand::VersioningObject(VersioningObjectCommand::PrepareCapture {
                operation: OperationId::fresh(),
                profile_uuid: Uuid::new_v4(),
                state_root: PathBuf::from("unused"),
            }),
        });
        assert!(coordinator.has_pending_versioning_object_work());
        coordinator.pending.clear();

        coordinator.active.insert(
            StorageOperation::VersioningStructuralPublish,
            StorageTicket(3),
        );
        assert!(!coordinator.has_pending_versioning_object_work());
    }

    #[test]
    fn object_worker_failures_are_redacted_in_storage_debug() {
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
                StorageOperation::VersioningCaptureStage,
            ),
            (
                VersioningObjectCompletion::ObjectInstalled {
                    operation: OperationId::fresh(),
                    result: Err(VersioningObjectFailure::Cancelled),
                },
                StorageOperation::VersioningObjectInstall,
            ),
            (
                VersioningObjectCompletion::ArchiveViewMaterialized {
                    operation: OperationId::fresh(),
                    result: Err(VersioningObjectFailure::ArchiveMaterializationFailed),
                },
                StorageOperation::VersioningArchiveView,
            ),
            (
                VersioningObjectCompletion::ArchiveViewRemoved {
                    operation: OperationId::fresh(),
                    result: Err(VersioningObjectFailure::ArchiveCleanupFailed),
                },
                StorageOperation::VersioningArchiveViewCleanup,
            ),
            (
                VersioningObjectCompletion::CapturePrepared {
                    operation: OperationId::fresh(),
                    result: Err(VersioningObjectFailure::CaptureStagingFailed),
                },
                StorageOperation::VersioningCaptureStage,
            ),
            (
                VersioningObjectCompletion::ObjectInstalled {
                    operation: OperationId::fresh(),
                    result: Err(VersioningObjectFailure::ObjectInstallFailed),
                },
                StorageOperation::VersioningObjectInstall,
            ),
            (
                VersioningObjectCompletion::ObjectInstalled {
                    operation: OperationId::fresh(),
                    result: Err(VersioningObjectFailure::SidecarInstallFailed),
                },
                StorageOperation::VersioningObjectInstall,
            ),
        ];
        for (completion, expected_operation) in completions {
            let storage = object_completion_to_storage(completion);
            assert_eq!(storage.operation(), expected_operation);
            let debug = format!("{storage:?}");
            for sentinel in &sentinels {
                assert!(!debug.contains(*sentinel));
            }
            assert!(debug.contains(&format!("{expected_operation:?}")));
            assert!(!debug.contains("message"));
        }
        let StorageCompletion::VersioningObjectInstall(Err(full)) =
            object_completion_to_storage(VersioningObjectCompletion::ObjectInstalled {
                operation: OperationId::fresh(),
                result: Err(VersioningObjectFailure::StorageFull),
            })
        else {
            panic!("storage-full object completion must remain typed");
        };
        assert_eq!(full.operation, StorageOperation::VersioningObjectInstall);
        assert_eq!(full.kind, StorageFailureKind::StorageFull);
        assert_eq!(full.message, "versioning object installation failed");
    }

    #[test]
    fn unsafe_restart_recovery_disables_new_captures_without_taking_unrelated_bytes() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let database_directory = roots.data.join("profiles").join(profile.to_string());
        fs::create_dir_all(&database_directory).expect("profile data directory");
        let database = database_directory.join("browser.sqlite");
        let unrelated = roots.data.join("unrelated");
        fs::write(&unrelated, b"preserve").expect("unrelated bytes");
        symlink(&roots.data, roots.state.join("versioning")).expect("unsafe versioning root");
        let mut coordinator = StorageCoordinator::default();
        coordinator
            .replace(Some(&database), Some((&roots.state, profile)))
            .expect("worker started");
        let deadline = Instant::now() + Duration::from_secs(2);
        while coordinator.recovery_pending {
            coordinator.poll();
            assert!(Instant::now() < deadline, "recovery result timed out");
            thread::yield_now();
        }
        assert!(coordinator.recovery_failed);
        assert!(matches!(
            coordinator.submit(StorageCommand::VersioningBegin {
                operation: OperationId::fresh(),
                profile_uuid: profile,
                reserved_bytes: 0,
                state_root: None,
            }),
            Err(StorageWorkerError::RecoveryFailed)
        ));
        let mut peer = StorageCoordinator::default();
        peer.replace(Some(&database), Some((&roots.state, profile)))
            .expect("same-profile peer");
        assert!(matches!(
            peer.submit(StorageCommand::VersioningBegin {
                operation: OperationId::fresh(),
                profile_uuid: profile,
                reserved_bytes: 0,
                state_root: None,
            }),
            Err(StorageWorkerError::RecoveryFailed)
        ));
        assert_eq!(
            fs::read(&unrelated).expect("unrelated preserved"),
            b"preserve"
        );
        peer.close();
        coordinator.close();
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn first_capture_waits_for_successful_restart_recovery() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let database_directory = roots.data.join("profiles").join(profile.to_string());
        fs::create_dir_all(&database_directory).expect("profile data directory");
        let database = database_directory.join("browser.sqlite");
        let mut coordinator = StorageCoordinator::default();
        coordinator
            .replace(Some(&database), Some((&roots.state, profile)))
            .expect("worker started");
        let ticket = coordinator
            .submit(StorageCommand::VersioningBegin {
                operation: OperationId::fresh(),
                profile_uuid: profile,
                reserved_bytes: 0,
                state_root: None,
            })
            .expect("queued behind recovery");
        let deadline = Instant::now() + Duration::from_secs(2);
        let effect = loop {
            if let Some(effect) = coordinator.poll().into_iter().next() {
                break effect;
            }
            assert!(Instant::now() < deadline, "capture reservation timed out");
            thread::yield_now();
        };
        assert_eq!(effect.ticket, ticket);
        assert!(matches!(
            effect.completion,
            StorageCompletion::VersioningBegin(Ok(()))
        ));
        assert!(!coordinator.recovery_pending);
        assert!(!coordinator.recovery_failed);
        coordinator.close();
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn second_same_profile_owner_does_not_recover_a_live_capture() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let database_directory = roots.data.join("profiles").join(profile.to_string());
        fs::create_dir_all(&database_directory).expect("profile data directory");
        let database = database_directory.join("browser.sqlite");
        let mut first = StorageCoordinator::default();
        first
            .replace(Some(&database), Some((&roots.state, profile)))
            .expect("first owner");
        let deadline = Instant::now() + Duration::from_secs(2);
        while first.recovery_pending {
            first.poll();
            assert!(Instant::now() < deadline, "first recovery timed out");
            thread::yield_now();
        }
        assert!(!first.recovery_failed);
        let operation = OperationId::fresh();
        let begin = first
            .submit(StorageCommand::VersioningBegin {
                operation,
                profile_uuid: profile,
                reserved_bytes: 0,
                state_root: None,
            })
            .expect("first capture reservation");
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(effect) = first
                .poll()
                .into_iter()
                .find(|effect| effect.ticket == begin)
            {
                assert!(matches!(
                    effect.completion,
                    StorageCompletion::VersioningBegin(Ok(()))
                ));
                break;
            }
            assert!(Instant::now() < deadline, "reservation timed out");
            thread::yield_now();
        }

        let mut second = StorageCoordinator::default();
        second
            .replace(Some(&database), Some((&roots.state, profile)))
            .expect("second owner");
        let listing = second
            .submit(StorageCommand::VersioningList {
                profile_uuid: profile,
                limit: 1,
                offset: 0,
            })
            .expect("second owner listing");
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(effect) = second
                .poll()
                .into_iter()
                .find(|effect| effect.ticket == listing)
            {
                assert!(matches!(
                    effect.completion,
                    StorageCompletion::VersioningList(Ok(_))
                ));
                break;
            }
            assert!(Instant::now() < deadline, "second owner timed out");
            thread::yield_now();
        }
        let connection = rusqlite::Connection::open(&database).expect("inspect durable operation");
        let state: String = connection
            .query_row(
                "SELECT state FROM vb_operations WHERE id=?1",
                [operation.to_string()],
                |row| row.get(0),
            )
            .expect("first owner's operation");
        assert_eq!(
            state, "staging",
            "second owner must not run restart recovery"
        );
        drop(connection);
        second.close();
        first.close();
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn second_same_profile_owner_waits_for_first_recovery_result() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let database_directory = roots.data.join("profiles").join(profile.to_string());
        fs::create_dir_all(&database_directory).expect("profile data directory");
        let database = database_directory.join("browser.sqlite");
        let mut first = StorageCoordinator::default();
        first
            .replace(Some(&database), Some((&roots.state, profile)))
            .expect("first owner");
        let mut second = StorageCoordinator::default();
        second
            .replace(Some(&database), Some((&roots.state, profile)))
            .expect("second owner");
        let operation = OperationId::fresh();
        let ticket = second
            .submit(StorageCommand::VersioningBegin {
                operation,
                profile_uuid: profile,
                reserved_bytes: 0,
                state_root: None,
            })
            .expect("queued behind shared recovery");
        assert!(
            second.poll().is_empty(),
            "peer must not publish before recovery"
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        while first.recovery_pending {
            first.poll();
            assert!(Instant::now() < deadline, "first recovery timed out");
            thread::yield_now();
        }
        assert!(!first.recovery_failed);
        let effect = loop {
            if let Some(effect) = second
                .poll()
                .into_iter()
                .find(|effect| effect.ticket == ticket)
            {
                break effect;
            }
            assert!(Instant::now() < deadline, "peer reservation timed out");
            thread::yield_now();
        };
        assert!(matches!(
            effect.completion,
            StorageCompletion::VersioningBegin(Ok(()))
        ));
        second.close();
        first.close();
        roots.cleanup().expect("cleanup");
    }

    #[test]
    fn object_worker_and_metadata_queries_keep_independent_ticket_correlation() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        roots.ensure().expect("private roots");
        let profile = Uuid::new_v4();
        let database_directory = roots.data.join("profiles").join(profile.to_string());
        fs::create_dir_all(&database_directory).expect("profile data directory");
        let database = database_directory.join("browser.sqlite");
        let mut coordinator = StorageCoordinator::default();
        coordinator
            .replace(Some(&database), Some((&roots.state, profile)))
            .expect("workers started");

        let deadline = Instant::now() + Duration::from_secs(2);
        while coordinator.recovery_pending {
            coordinator.poll();
            assert!(Instant::now() < deadline, "recovery timed out");
            thread::yield_now();
        }

        let operation = OperationId::fresh();
        let stage_ticket = coordinator
            .submit_versioning_object(VersioningObjectCommand::PrepareCapture {
                operation,
                profile_uuid: profile,
                state_root: roots.state.clone(),
            })
            .expect("stage accepted");
        let deadline = Instant::now() + Duration::from_secs(2);
        let stage = loop {
            if let Some(effect) = coordinator
                .poll()
                .into_iter()
                .find(|effect| effect.ticket == stage_ticket)
            {
                let StorageCompletion::VersioningCaptureStage(Ok(stage)) = effect.completion else {
                    panic!("staging completion should match its ticket");
                };
                break stage;
            }
            assert!(Instant::now() < deadline, "staging timed out");
            thread::yield_now();
        };
        assert_eq!(stage.operation(), operation);

        let bytes = vec![b'a'; 8 * 1024 * 1024];
        fs::write(stage.path(), &bytes).expect("synthetic archive bytes");
        let install_ticket = coordinator
            .submit_versioning_object(VersioningObjectCommand::InstallCapture {
                operation,
                profile_uuid: profile,
                state_root: roots.state.clone(),
                max_bytes: bytes.len() as u64,
                expected_size: Some(bytes.len() as u64),
                expected_sha256: None,
            })
            .expect("install accepted");
        let library_ticket = coordinator
            .submit(StorageCommand::Library { limit: 1 })
            .expect("metadata query accepted during object I/O");

        let deadline = Instant::now() + Duration::from_secs(5);
        let (mut installed, mut library_loaded) = (false, false);
        while !(installed && library_loaded) {
            for effect in coordinator.poll() {
                if effect.ticket == install_ticket {
                    let StorageCompletion::VersioningObjectInstall(Ok(object)) = effect.completion
                    else {
                        panic!("object install completion should match its ticket");
                    };
                    assert_eq!(object.bytes, bytes.len() as u64);
                    installed = true;
                } else if effect.ticket == library_ticket {
                    assert!(matches!(
                        effect.completion,
                        StorageCompletion::Library(Ok(_))
                    ));
                    library_loaded = true;
                }
            }
            assert!(Instant::now() < deadline, "blob/query completion timed out");
            thread::yield_now();
        }
        coordinator.close();
        roots.cleanup().expect("cleanup temporary roots");
    }
}
