//! The worker may cancel an unstarted GUI send at its absolute deadline.
use std::sync::atomic::{AtomicU8, Ordering};

const QUEUED: u8 = 0;
const COMMITTING: u8 = 1;
const NOT_SENT: u8 = 2;
const CANCELLED: u8 = 3;

#[derive(Default)]
pub(crate) struct QueuedSend(AtomicU8);

impl QueuedSend {
    /// GUI-only, immediately after admission and before the single write.
    pub(crate) fn try_commit(&self) -> bool {
        self.0
            .compare_exchange(QUEUED, COMMITTING, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(crate) fn not_sent(&self) {
        self.0.store(NOT_SENT, Ordering::Release);
    }

    /// True is proof that no write started, including a cancelled late callback.
    pub(crate) fn cancel_or_prove_not_sent(&self) -> bool {
        matches!(
            self.0
                .compare_exchange(QUEUED, CANCELLED, Ordering::AcqRel, Ordering::Acquire),
            Ok(_) | Err(NOT_SENT | CANCELLED)
        )
    }
}
