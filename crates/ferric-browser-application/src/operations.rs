//! Bounded operation ownership and irreversible terminal transitions.

use std::collections::{BTreeMap, VecDeque};

use ferric_browser_core::FailureKind;

pub const MAX_ACTIVE_OPERATIONS: usize = 64;
pub const MAX_TERMINAL_OPERATIONS: usize = 256;
pub const MAX_OPERATION_STDERR_BYTES: usize = 16 * 1024;
pub const MAX_RETAINED_STDERR_BYTES: usize = 1024 * 1024;
const MAX_DETAIL_BYTES: usize = 4096;
const MAX_PENDING_COMPLETIONS: usize = MAX_ACTIVE_OPERATIONS + MAX_TERMINAL_OPERATIONS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationState {
    Accepted,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl OperationState {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationFailure {
    pub kind: FailureKind,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationRecord {
    pub id: String,
    pub sequence: u64,
    pub state: OperationState,
    pub detail: String,
    pub failure: Option<OperationFailure>,
    pub stderr: Option<String>,
    pub cancellable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationUpdate {
    state: OperationState,
    detail: String,
    failure: Option<OperationFailure>,
}

impl OperationUpdate {
    #[must_use]
    pub const fn state(&self) -> OperationState {
        self.state
    }

    #[must_use]
    pub fn running(detail: impl Into<String>) -> Self {
        Self::new(OperationState::Running, detail.into(), None)
    }

    #[must_use]
    pub fn completed(detail: impl Into<String>) -> Self {
        Self::new(OperationState::Completed, detail.into(), None)
    }

    #[must_use]
    pub fn cancelled() -> Self {
        Self::new(OperationState::Cancelled, String::new(), None)
    }

    #[must_use]
    pub fn failed(kind: FailureKind, message: impl Into<String>) -> Self {
        let message = bounded(message.into(), MAX_DETAIL_BYTES);
        Self::new(
            OperationState::Failed,
            message.clone(),
            Some(OperationFailure { kind, message }),
        )
    }

    fn new(state: OperationState, detail: String, failure: Option<OperationFailure>) -> Self {
        Self {
            state,
            detail: bounded(detail, MAX_DETAIL_BYTES),
            failure,
        }
    }
}

/// Minimal terminal notification. Detailed and potentially private output stays
/// in the bounded owner catalog and is retrieved through an authorized query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationCompletion {
    pub id: String,
    pub sequence: u64,
    pub state: OperationState,
    pub error: Option<FailureKind>,
}

/// The catalog owns both admission and retained output. No caller can mutate
/// a record directly or recreate a forgotten operation with a late completion.
#[derive(Debug, Default)]
pub struct OperationCatalog {
    records: BTreeMap<String, OperationRecord>,
    terminal: VecDeque<String>,
    completions: VecDeque<OperationCompletion>,
    stderr_order: VecDeque<String>,
    active: usize,
    stderr_bytes: usize,
    revision: u64,
    sequence: u64,
}

impl OperationCatalog {
    /// Reserves capacity before any associated side effect begins.
    ///
    /// # Errors
    /// Rejects invalid/reused IDs and admission beyond the active bound.
    pub fn admit(&mut self, id: String) -> Result<(), FailureKind> {
        if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
            return Err(FailureKind::InvalidArgument);
        }
        if self.records.contains_key(&id) {
            return Err(FailureKind::InvalidArgument);
        }
        if self.active >= MAX_ACTIVE_OPERATIONS
            || self.active + self.completions.len() >= MAX_PENDING_COMPLETIONS
        {
            return Err(FailureKind::Busy);
        }
        self.sequence = self.sequence.checked_add(1).ok_or(FailureKind::Busy)?;
        self.records.insert(
            id.clone(),
            OperationRecord {
                id,
                sequence: self.sequence,
                state: OperationState::Accepted,
                detail: String::new(),
                failure: None,
                stderr: None,
                cancellable: true,
            },
        );
        self.active += 1;
        self.changed();
        Ok(())
    }

    /// Applies one transition; terminal records and unknown IDs reject replies.
    pub fn update(&mut self, id: &str, update: OperationUpdate) -> bool {
        let Some(record) = self.records.get_mut(id) else {
            return false;
        };
        if record.state.is_terminal() {
            return false;
        }
        record.state = update.state;
        record.detail = update.detail;
        record.failure = update.failure;
        if record.state.is_terminal() {
            record.cancellable = false;
            self.active -= 1;
            self.completions.push_back(OperationCompletion {
                id: record.id.clone(),
                sequence: record.sequence,
                state: record.state,
                error: record.failure.as_ref().map(|failure| failure.kind),
            });
            self.terminal.push_back(id.to_owned());
            while self.terminal.len() > MAX_TERMINAL_OPERATIONS {
                if let Some(oldest) = self.terminal.pop_front() {
                    self.remove(&oldest);
                }
            }
        }
        self.changed();
        true
    }

    /// Drains every terminal result exactly once, including owner retirement.
    /// Admission backpressure reserves room for active work to finish even if
    /// the notification consumer temporarily stops draining.
    pub fn drain_completions(&mut self) -> impl Iterator<Item = OperationCompletion> + '_ {
        self.completions.drain(..)
    }

    /// Makes an admitted mutation noncancellable before publication starts.
    pub fn begin_commit(&mut self, id: &str) -> bool {
        let Some(record) = self
            .records
            .get_mut(id)
            .filter(|record| !record.state.is_terminal())
        else {
            return false;
        };
        record.cancellable = false;
        self.changed();
        true
    }

    /// # Errors
    /// Reports missing, terminal, and publication-bound operations distinctly.
    pub fn cancel(&mut self, id: &str) -> Result<(), FailureKind> {
        let record = self.records.get(id).ok_or(FailureKind::NotFound)?;
        if record.state.is_terminal() {
            return Err(FailureKind::InvalidArgument);
        }
        if !record.cancellable {
            return Err(FailureKind::Unsupported);
        }
        self.update(id, OperationUpdate::cancelled());
        Ok(())
    }

    /// Accepts already redacted stderr and bounds bytes without breaking UTF-8.
    pub fn remember_stderr(&mut self, id: &str, stderr: String) {
        let Some(record) = self.records.get_mut(id) else {
            return;
        };
        if let Some(previous) = record.stderr.take() {
            self.stderr_bytes -= previous.len();
        }
        self.stderr_order.retain(|entry| entry != id);
        let stderr = bounded(stderr, MAX_OPERATION_STDERR_BYTES);
        self.stderr_bytes += stderr.len();
        record.stderr = Some(stderr);
        self.stderr_order.push_back(id.to_owned());
        while self.stderr_bytes > MAX_RETAINED_STDERR_BYTES {
            if let Some(oldest) = self.stderr_order.pop_front()
                && let Some(record) = self.records.get_mut(&oldest)
                && let Some(stderr) = record.stderr.take()
            {
                self.stderr_bytes -= stderr.len();
            }
        }
        self.changed();
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&OperationRecord> {
        self.records.get(id)
    }

    pub fn records(&self) -> impl Iterator<Item = &OperationRecord> {
        self.records.values()
    }

    #[must_use]
    pub const fn has_active(&self) -> bool {
        self.active != 0
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn retire(&mut self) {
        let active = self
            .records
            .values()
            .filter(|record| !record.state.is_terminal())
            .map(|record| record.id.clone())
            .collect::<Vec<_>>();
        for id in active {
            self.update(&id, OperationUpdate::cancelled());
        }
    }

    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    fn remove(&mut self, id: &str) {
        if let Some(record) = self.records.remove(id)
            && let Some(stderr) = record.stderr
        {
            self.stderr_bytes -= stderr.len();
        }
        self.stderr_order.retain(|entry| entry != id);
    }
}

fn bounded(mut text: String, limit: usize) -> String {
    let mut boundary = limit.min(text.len());
    while !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    text.truncate(boundary);
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_notifications_survive_retention_and_apply_bounded_backpressure() {
        let mut catalog = OperationCatalog::default();
        for index in 0..MAX_PENDING_COMPLETIONS {
            let id = format!("op-{index}");
            catalog.admit(id.clone()).unwrap();
            catalog.update(&id, OperationUpdate::completed("done"));
        }
        assert_eq!(catalog.admit("blocked".into()), Err(FailureKind::Busy));
        assert_eq!(catalog.records().count(), MAX_TERMINAL_OPERATIONS);
        assert_eq!(catalog.drain_completions().count(), MAX_PENDING_COMPLETIONS);
        assert_eq!(catalog.drain_completions().count(), 0);
        catalog.admit("retiring".into()).unwrap();
        catalog.retire();
        let completions: Vec<_> = catalog.drain_completions().collect();
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].id, "retiring");
        assert_eq!(completions[0].state, OperationState::Cancelled);
        assert!(!catalog.update("retiring", OperationUpdate::completed("late")));
        assert_eq!(catalog.drain_completions().count(), 0);
    }

    #[test]
    fn admission_precedes_work_and_never_evicts_active_operations() {
        let mut catalog = OperationCatalog::default();
        for i in 0..MAX_ACTIVE_OPERATIONS {
            catalog.admit(format!("op-{i}")).unwrap();
        }
        assert_eq!(catalog.admit("overflow".into()), Err(FailureKind::Busy));
        assert!(catalog.get("op-0").is_some());
        assert!(catalog.update("op-0", OperationUpdate::completed("done")));
        catalog.admit("next".into()).unwrap();
    }

    #[test]
    fn terminal_results_are_irreversible_and_idle_work_quiesces() {
        let mut catalog = OperationCatalog::default();
        catalog.admit("version".into()).unwrap();
        catalog.update("version", OperationUpdate::completed("version"));
        assert!(!catalog.has_active());
        assert!(!catalog.update("version", OperationUpdate::running("late")));
        assert!(!catalog.update("unknown", OperationUpdate::completed("late")));
        assert_eq!(catalog.cancel("version"), Err(FailureKind::InvalidArgument));
        assert_eq!(catalog.cancel("unknown"), Err(FailureKind::NotFound));
    }

    #[test]
    fn retention_uses_completion_order_and_forgets_stderr_together() {
        let mut catalog = OperationCatalog::default();
        catalog.admit("z-oldest".into()).unwrap();
        catalog.remember_stderr("z-oldest", "secret already redacted".into());
        catalog.update("z-oldest", OperationUpdate::completed("first"));
        for i in 0..MAX_TERMINAL_OPERATIONS {
            let id = format!("a-{i}");
            catalog.admit(id.clone()).unwrap();
            catalog.update(&id, OperationUpdate::completed("done"));
        }
        assert!(catalog.get("z-oldest").is_none());
        assert_eq!(catalog.records().count(), MAX_TERMINAL_OPERATIONS);
        assert_eq!(catalog.stderr_bytes, 0);
    }

    #[test]
    fn cancellation_cannot_cross_the_commit_boundary() {
        let mut catalog = OperationCatalog::default();
        catalog.admit("write".into()).unwrap();
        assert!(catalog.begin_commit("write"));
        assert_eq!(catalog.cancel("write"), Err(FailureKind::Unsupported));
        catalog.update("write", OperationUpdate::completed("durable"));
        assert!(!catalog.has_active());
    }

    #[test]
    fn output_is_byte_bounded_and_retirement_revokes_late_results() {
        let mut catalog = OperationCatalog::default();
        catalog.admit("script".into()).unwrap();
        catalog.remember_stderr("script", "é".repeat(MAX_OPERATION_STDERR_BYTES));
        assert_eq!(
            catalog
                .get("script")
                .unwrap()
                .stderr
                .as_ref()
                .unwrap()
                .len(),
            MAX_OPERATION_STDERR_BYTES
        );
        catalog.retire();
        assert_eq!(
            catalog.get("script").unwrap().state,
            OperationState::Cancelled
        );
        assert!(!catalog.update("script", OperationUpdate::completed("late")));
    }
}
