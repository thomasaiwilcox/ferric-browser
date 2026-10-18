use std::{
    collections::{BTreeMap, VecDeque},
    path::Path,
    thread,
    time::{Duration, Instant},
};

use ferric_browser_storage::{
    ProfileStoreWorker, StorageCommand, StorageCompletion, StorageOperation, StorageWorkerError,
};

use crate::{session_downloads::DownloadWork, session_permissions::durable::PermissionWork};

pub(crate) enum StorageObservation {
    Permission(PermissionWork),
    Download(DownloadWork),
}
impl StorageObservation {
    fn admitted(self) -> Self {
        match self {
            Self::Permission(work) => Self::Permission(work.admitted()),
            Self::Download(work) => Self::Download(work),
        }
    }
    fn finish(self, completion: &StorageCompletion) {
        match self {
            Self::Permission(work) => work.finish(completion),
            Self::Download(work) => work.finish(completion),
        }
    }
}

const RETIRE_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_PENDING_REQUESTS: usize = 512;

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
    command: StorageCommand,
}

pub(crate) struct StorageCoordinator {
    worker: Option<ProfileStoreWorker>,
    pending: VecDeque<ScheduledRequest>,
    active: BTreeMap<StorageOperation, StorageTicket>,
    effects: VecDeque<StorageEffect>,
    observations: BTreeMap<StorageTicket, StorageObservation>,
    next_ticket: u64,
}

impl Default for StorageCoordinator {
    fn default() -> Self {
        Self {
            worker: None,
            pending: VecDeque::new(),
            active: BTreeMap::new(),
            effects: VecDeque::new(),
            observations: BTreeMap::new(),
            next_ticket: 1,
        }
    }
}

impl StorageCoordinator {
    pub(crate) fn replace(&mut self, path: Option<&Path>) -> Result<(), StorageWorkerError> {
        self.replace_until(path, Instant::now() + RETIRE_TIMEOUT)
    }

    pub(crate) fn replace_until(
        &mut self,
        path: Option<&Path>,
        deadline: Instant,
    ) -> Result<(), StorageWorkerError> {
        let replacement = path.map(ProfileStoreWorker::spawn).transpose()?;
        self.retire(CancellationReason::ProfileReplaced, deadline);
        self.worker = replacement;
        Ok(())
    }

    pub(crate) fn close(&mut self) {
        self.close_until(Instant::now() + RETIRE_TIMEOUT);
    }

    pub(crate) fn close_until(&mut self, deadline: Instant) {
        self.retire(CancellationReason::ProfileClosed, deadline);
    }

    pub(crate) const fn is_open(&self) -> bool {
        self.worker.is_some()
    }

    pub(crate) fn submit(
        &mut self,
        command: StorageCommand,
        observation: Option<StorageObservation>,
    ) -> Result<StorageTicket, StorageWorkerError> {
        let ticket = self.next_ticket();
        let result = match self.submit_now(ticket, command.clone()) {
            Ok(()) => Ok(ticket),
            Err(StorageWorkerError::Busy | StorageWorkerError::QueueFull) => {
                if self.pending.len() >= MAX_PENDING_REQUESTS {
                    return Err(StorageWorkerError::QueueFull);
                }
                self.pending.push_back(ScheduledRequest { ticket, command });
                Ok(ticket)
            }
            Err(error) => Err(error),
        };
        if result.is_ok()
            && let Some(observation) = observation
        {
            self.observations.insert(ticket, observation.admitted());
        }
        result
    }

    fn observe(&mut self, ticket: StorageTicket, completion: &StorageCompletion) {
        if let Some(observation) = self.observations.remove(&ticket) {
            observation.finish(completion);
        }
    }

    pub(crate) fn poll(&mut self) -> Vec<StorageEffect> {
        self.collect_completions();
        self.submit_pending();
        self.effects.drain(..).collect()
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
                    let completion = StorageCompletion::failed(operation, error.to_string());
                    self.observe(request.ticket, &completion);
                    self.effects.push_back(StorageEffect {
                        ticket: request.ticket,
                        completion,
                    });
                }
            }
        }
    }

    pub(crate) fn collect_for_drain(&mut self) {
        self.collect_completions();
        self.submit_pending();
    }

    fn collect_completions(&mut self) {
        let completions = self
            .worker
            .as_mut()
            .map(ProfileStoreWorker::poll_events)
            .unwrap_or_default();
        for completion in completions {
            let operation = completion.operation();
            if let Some(ticket) = self.active.remove(&operation) {
                self.observe(ticket, &completion);
                self.effects.push_back(StorageEffect { ticket, completion });
            }
        }
    }

    fn retire(&mut self, reason: CancellationReason, deadline: Instant) {
        if self.worker.is_none() {
            self.cancel_outstanding(reason);
            return;
        }

        while (!self.pending.is_empty() || !self.active.is_empty()) && Instant::now() < deadline {
            self.collect_completions();
            self.submit_pending();
            if !self.pending.is_empty() || !self.active.is_empty() {
                thread::sleep(Duration::from_millis(2));
            }
        }

        if self.pending.is_empty()
            && self.active.is_empty()
            && let Some(worker) = self.worker.as_mut()
            && worker.request_flush().is_ok()
        {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let _ = worker.wait_flush(remaining);
        }

        self.worker = None;
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
        for effect in cancelled {
            self.observe(effect.ticket, &effect.completion);
            self.effects.push_back(effect);
        }
    }

    fn next_ticket(&mut self) -> StorageTicket {
        let ticket = StorageTicket(self.next_ticket);
        self.next_ticket = self.next_ticket.saturating_add(1).max(1);
        ticket
    }

    fn submit_now(
        &mut self,
        ticket: StorageTicket,
        command: StorageCommand,
    ) -> Result<(), StorageWorkerError> {
        let operation = command.operation();
        let worker = self.worker.as_mut().ok_or(StorageWorkerError::Stopped)?;
        worker.try_submit(command)?;
        self.active.insert(operation, ticket);
        Ok(())
    }
}
