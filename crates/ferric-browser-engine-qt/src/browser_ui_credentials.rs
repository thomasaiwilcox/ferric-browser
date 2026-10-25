//! Per-owner, non-authorizing projection sink for the private helper bridge.

use crate::bridge::qobject::BrowserUi;
use crate::credential_protocol::{
    HelperFrame, Operation, ProtocolError, QueueBudget, decode_frame,
};
use cxx_qt::{CxxQtThread, Threading};
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

enum Projection {
    Status(u8),
    Qr { width: u8, modules: Vec<u8> },
    Result,
    Error(u8),
}

#[derive(Default)]
struct SinkState {
    count: u32,
    terminals: u8,
    kind: u8,
    detail: u8,
    progress_count: u8,
    progress_kind: u8,
    progress_qr_len: u16,
    qr_modules: Option<Vec<u8>>,
    last_qr_len: u16,
}

impl SinkState {
    fn record(&mut self, projection: Projection, terminal: bool) {
        self.count = self.count.saturating_add(1);
        if terminal {
            self.terminals = self.terminals.saturating_add(1);
        }
        match projection {
            Projection::Status(status) => {
                self.kind = 1;
                self.detail = status;
                if !terminal {
                    self.note_progress(1, 0);
                }
                self.qr_modules = None;
            }
            Projection::Qr { width, modules } => {
                self.kind = 2;
                self.detail = width;
                self.last_qr_len = modules.len().min(u16::MAX as usize) as u16;
                if !terminal {
                    self.note_progress(2, modules.len().min(u16::MAX as usize) as u16);
                }
                self.qr_modules = Some(modules);
            }
            Projection::Result => {
                self.kind = 3;
                self.detail = 0;
                self.qr_modules = None;
            }
            Projection::Error(code) => {
                self.kind = 4;
                self.detail = code;
                self.qr_modules = None;
            }
        }
    }

    fn packed(&self) -> u64 {
        let qr_len = self
            .qr_modules
            .as_ref()
            .map_or(self.last_qr_len, |modules| {
                modules.len().min(u16::MAX as usize) as u16
            });
        (u64::from(self.count.min(0x00ff_ffff)) << 40)
            | (u64::from(self.terminals) << 32)
            | (u64::from(self.kind) << 28)
            | (u64::from(self.detail) << 20)
            | (u64::from(qr_len) << 4)
    }

    fn note_progress(&mut self, kind: u8, qr_len: u16) {
        self.progress_count = self.progress_count.saturating_add(1);
        self.progress_kind = kind;
        self.progress_qr_len = qr_len;
    }

    fn progress_packed(&self, accepted_in_window: usize) -> u64 {
        u64::from(self.progress_count)
            | (u64::from(self.progress_kind) << 8)
            | (u64::from(self.progress_qr_len) << 12)
            | (u64::from(accepted_in_window.min(15) as u8) << 28)
    }
}

#[derive(Default)]
struct ProgressState {
    generation: u64,
    queued: bool,
    latest: Option<Projection>,
    accepted: VecDeque<Instant>,
}

struct Reservation {
    budget: Arc<Mutex<QueueBudget>>,
    bytes: usize,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if let Ok(mut budget) = self.budget.lock() {
            budget.release(self.bytes);
        }
    }
}

pub(crate) struct CredentialEvents {
    ui: Pin<Box<CxxQtThread<BrowserUi>>>,
    request_id: String,
    operation: Operation,
    generation: Arc<AtomicU64>,
    active_token: Arc<AtomicU64>,
    terminal_queued: Arc<AtomicBool>,
    terminal_delivered: Arc<AtomicBool>,
    budget: Arc<Mutex<QueueBudget>>,
    progress: Arc<Mutex<ProgressState>>,
    sink: Arc<Mutex<SinkState>>,
}

impl CredentialEvents {
    fn new(
        ui: CxxQtThread<BrowserUi>,
        request_id: String,
        operation: Operation,
        transport_incarnation: u64,
    ) -> Self {
        let progress = ProgressState {
            generation: transport_incarnation,
            ..ProgressState::default()
        };
        Self {
            ui: Box::pin(ui),
            request_id,
            operation,
            generation: Arc::new(AtomicU64::new(transport_incarnation)),
            active_token: Arc::new(AtomicU64::new(0)),
            terminal_queued: Arc::new(AtomicBool::new(false)),
            terminal_delivered: Arc::new(AtomicBool::new(false)),
            budget: Arc::default(),
            progress: Arc::new(Mutex::new(progress)),
            sink: Arc::default(),
        }
    }

    pub(crate) fn process_started(&self, token: u64) {
        if token == 0 {
            return;
        }
        let generation = self
            .generation
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        self.active_token.store(token, Ordering::Release);
        self.terminal_queued.store(false, Ordering::Release);
        self.terminal_delivered.store(false, Ordering::Release);
        if let Ok(mut progress) = self.progress.lock() {
            *progress = ProgressState {
                generation,
                ..ProgressState::default()
            };
        }
    }

    pub(crate) fn process_frame(&self, token: u64, bytes: &[u8]) -> i32 {
        if token == 0 || self.active_token.load(Ordering::Acquire) != token {
            return 1;
        }
        let frame = match decode_frame(bytes, &self.request_id, self.operation) {
            Ok(frame) => frame,
            Err(
                ProtocolError::TooLarge
                | ProtocolError::QueueFull
                | ProtocolError::InvalidFrame
                | ProtocolError::InvalidRequest,
            ) => return 0,
        };
        let (projection, terminal) = match frame {
            HelperFrame::Status { status, .. } => (
                Projection::Status(if status == "submitted" { 1 } else { 2 }),
                false,
            ),
            HelperFrame::Qr { width, modules, .. } => (
                Projection::Qr {
                    width: width.min(u16::from(u8::MAX)) as u8,
                    modules,
                },
                false,
            ),
            HelperFrame::Result { body, .. } => {
                drop(body);
                (Projection::Result, true)
            }
            HelperFrame::Error { code, .. } => (Projection::Error(error_category(&code)), true),
        };
        if terminal {
            self.queue_terminal(token, bytes.len(), projection)
        } else {
            self.queue_progress(token, bytes.len(), projection)
        }
    }

    fn reserve(&self, bytes: usize) -> Option<Reservation> {
        let mut budget = self.budget.lock().ok()?;
        budget.reserve(bytes).ok()?;
        Some(Reservation {
            budget: Arc::clone(&self.budget),
            bytes,
        })
    }

    fn queue_terminal(&self, token: u64, bytes: usize, projection: Projection) -> i32 {
        if self.terminal_queued.load(Ordering::Acquire)
            || self.active_token.load(Ordering::Acquire) != token
        {
            return 0;
        }
        let Some(reservation) = self.reserve(bytes) else {
            return 0;
        };
        if self
            .terminal_queued
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            drop(reservation);
            return 0;
        }
        let generation = self.generation.load(Ordering::Acquire);
        let active = Arc::clone(&self.active_token);
        let current_generation = Arc::clone(&self.generation);
        let delivered = Arc::clone(&self.terminal_delivered);
        let sink = Arc::clone(&self.sink);
        let queued = Arc::clone(&self.terminal_queued);
        if self
            .ui
            .queue(move |_ui: Pin<&mut BrowserUi>| {
                let _reservation = reservation;
                if active.load(Ordering::Acquire) != token
                    || current_generation.load(Ordering::Acquire) != generation
                    || !queued.load(Ordering::Acquire)
                    || delivered.swap(true, Ordering::AcqRel)
                {
                    return;
                }
                if let Ok(mut sink) = sink.lock() {
                    sink.record(projection, true);
                }
                active.store(0, Ordering::Release);
                current_generation.fetch_add(1, Ordering::AcqRel);
                queued.store(false, Ordering::Release);
            })
            .is_err()
        {
            self.terminal_queued.store(false, Ordering::Release);
            return 0;
        }
        2
    }

    fn queue_progress(&self, token: u64, bytes: usize, projection: Projection) -> i32 {
        if bytes > crate::credential_protocol::MAX_PROGRESS_FRAME {
            return 0;
        }
        let generation = self.generation.load(Ordering::Acquire);
        {
            let Ok(mut progress) = self.progress.lock() else {
                return 0;
            };
            if progress.generation != generation {
                return 1;
            }
            let now = Instant::now();
            while progress
                .accepted
                .front()
                .is_some_and(|time| now.duration_since(*time).as_secs() >= 1)
            {
                progress.accepted.pop_front();
            }
            if progress.queued {
                progress.latest = Some(projection);
                return 1;
            }
            if progress.accepted.len() >= 10 {
                progress.latest = Some(projection);
                return 1;
            }
            progress.accepted.push_back(now);
            progress.latest = Some(projection);
            progress.queued = true;
        }
        // Reserve one maximum-sized progress slot: later coalesced QR updates
        // may replace a smaller status while this same closure is pending.
        let Some(reservation) = self.reserve(crate::credential_protocol::MAX_PROGRESS_FRAME) else {
            if let Ok(mut progress) = self.progress.lock() {
                if progress.generation == generation {
                    progress.queued = false;
                }
            }
            return 0;
        };
        let progress_state = Arc::clone(&self.progress);
        let active = Arc::clone(&self.active_token);
        let current_generation = Arc::clone(&self.generation);
        let sink = Arc::clone(&self.sink);
        let queued = self.ui.queue(move |_ui: Pin<&mut BrowserUi>| {
            let _reservation = reservation;
            let projection = if let Ok(mut state) = progress_state.lock() {
                if state.generation == generation {
                    state.queued = false;
                    state.latest.take()
                } else {
                    None
                }
            } else {
                None
            };
            if active.load(Ordering::Acquire) != token
                || current_generation.load(Ordering::Acquire) != generation
            {
                return;
            }
            if let (Some(projection), Ok(mut sink)) = (projection, sink.lock()) {
                sink.record(projection, false);
            }
        });
        if queued.is_err() {
            if let Ok(mut state) = self.progress.lock() {
                if state.generation == generation {
                    state.queued = false;
                }
            }
            return 0;
        }
        1
    }

    pub(crate) fn process_finished(&self, token: u64, _outcome: i32) {
        if self.active_token.load(Ordering::Acquire) != token {
            return;
        }
        if !self.terminal_queued.load(Ordering::Acquire) {
            self.process_invalidated(token);
        }
    }

    pub(crate) fn process_invalidated(&self, token: u64) {
        if token != 0 && self.active_token.load(Ordering::Acquire) != token {
            return;
        }
        self.active_token.store(0, Ordering::Release);
        self.terminal_queued.store(false, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        if let Ok(mut progress) = self.progress.lock() {
            *progress = ProgressState {
                generation: self.generation.load(Ordering::Acquire),
                ..ProgressState::default()
            };
        }
    }

    pub(crate) fn process_snapshot(&self) -> u64 {
        self.sink.lock().map_or(0, |sink| sink.packed())
    }

    pub(crate) fn process_progress_snapshot(&self) -> u64 {
        let accepted = self
            .progress
            .lock()
            .map_or(0, |progress| progress.accepted.len());
        self.sink
            .lock()
            .map_or(0, |sink| sink.progress_packed(accepted))
    }

    pub(crate) fn process_pending_bytes(&self) -> u64 {
        self.budget
            .lock()
            .map_or(0, |budget| budget.pending_bytes() as u64)
    }
}

fn error_category(code: &str) -> u8 {
    match code {
        "invalid_envelope" | "invalid_request" => 1,
        "invalid_origin" | "unsupported_options" => 2,
        "unavailable" | "transport_unavailable" => 3,
        "authenticator_failed" => 4,
        _ => 6,
    }
}

pub(crate) fn create_credential_events(
    ui: &BrowserUi,
    request_id: &str,
    is_create: bool,
    incarnation: u64,
) -> Box<CredentialEvents> {
    let operation = if is_create {
        Operation::Create
    } else {
        Operation::Get
    };
    Box::new(CredentialEvents::new(
        ui.qt_thread(),
        request_id.to_owned(),
        operation,
        incarnation,
    ))
}
