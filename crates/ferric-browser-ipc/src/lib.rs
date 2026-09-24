//! Bounded, local-only IPC primitives.
//!
//! This crate owns framing and protocol validation. It does not know about Qt
//! or browser state; application adapters translate validated requests into
//! the shared core command/action paths.

#![allow(unsafe_code)]

mod command_schema;

pub use command_schema::{command_argument_names, validate_command_argument_fields};

use std::{
    collections::{HashMap, VecDeque},
    fs::{self, File, OpenOptions},
    hash::{Hash, Hasher},
    io::{self, Read, Seek, SeekFrom, Write},
    os::unix::{
        fs::{FileTypeExt, OpenOptionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};

use fs2::FileExt;
use serde::{Deserialize, Serialize, de, de::Deserializer as _};
use serde_json::{Value, json};
use uuid::Uuid;

/// Clean-break local-control protocol. Version 1 clients are deliberately
/// rejected during the hello negotiation before any method is dispatched.
pub const PROTOCOL_MAJOR: u32 = 2;
pub const PROTOCOL_MINOR: u32 = 0;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_JSON_DEPTH: usize = 32;
pub const MAX_CACHED_REQUESTS: usize = 1024;
pub const CACHE_TTL: Duration = Duration::from_secs(60);
pub const MAX_PENDING_REQUESTS: usize = 64;
pub const MAX_IPC_CONNECTIONS: usize = 32;
pub const MAX_EVENT_SUBSCRIBERS: usize = 32;
pub const MAX_OUTSTANDING_REQUESTS: usize = 64;
pub const MAX_EVENT_QUEUE_MESSAGES: usize = 256;
pub const MAX_EVENT_QUEUE_BYTES: usize = 4 * 1024 * 1024;
const MAX_ID_BYTES: usize = 256;
const MAX_ERROR_CORRELATIONS: usize = 32;
const REDACTED: &str = "[redacted]";

static ERROR_CORRELATIONS: OnceLock<Mutex<VecDeque<(String, String)>>> = OnceLock::new();

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ProtocolError>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EventNotification {
    pub instance_id: String,
    pub sequence: u64,
    pub event_type: String,
    pub payload: Value,
}

/// A validated request waiting for execution on the application thread.
#[derive(Debug)]
pub struct PendingRequest {
    request: Request,
    stream: UnixStream,
    cache: Option<Arc<Mutex<RequestCache>>>,
    outstanding: Option<Arc<std::sync::atomic::AtomicUsize>>,
}

impl PendingRequest {
    #[must_use]
    pub fn new(request: Request, stream: UnixStream) -> Self {
        Self {
            request,
            stream,
            cache: None,
            outstanding: None,
        }
    }

    #[must_use]
    pub fn with_cache(
        request: Request,
        stream: UnixStream,
        cache: Arc<Mutex<RequestCache>>,
    ) -> Self {
        Self {
            request,
            stream,
            cache: Some(cache),
            outstanding: None,
        }
    }

    #[must_use]
    pub fn with_cache_and_outstanding(
        request: Request,
        stream: UnixStream,
        cache: Arc<Mutex<RequestCache>>,
        outstanding: Arc<std::sync::atomic::AtomicUsize>,
    ) -> Self {
        Self {
            request,
            stream,
            cache: Some(cache),
            outstanding: Some(outstanding),
        }
    }

    #[must_use]
    pub fn request(&self) -> &Request {
        &self.request
    }

    /// Clones the authenticated connection stream for a long-lived event
    /// writer while the request response continues on the original stream.
    ///
    /// # Errors
    ///
    /// Returns the underlying operating-system error when cloning fails.
    pub fn clone_stream(&self) -> io::Result<UnixStream> {
        self.stream.try_clone()
    }

    /// Writes the response to the request's authenticated local connection.
    ///
    /// # Errors
    ///
    /// Returns an error when the response is too large or the connection
    /// cannot be written.
    pub fn respond(mut self, response: &Response) -> Result<(), FrameError> {
        let payload = serialize_response(response)?;
        let result = write_frame(&mut self.stream, &payload);
        if let Some(cache) = self.cache.as_ref()
            && let Ok(mut cache) = cache.lock()
        {
            cache.insert(&self.request, response.clone(), Instant::now());
        }
        if let Some(outstanding) = self.outstanding.take() {
            outstanding.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
        }
        result
    }
}

impl Drop for PendingRequest {
    fn drop(&mut self) {
        if let Some(outstanding) = self.outstanding.take() {
            outstanding.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
        }
    }
}

static PENDING_REQUESTS: OnceLock<Mutex<VecDeque<PendingRequest>>> = OnceLock::new();

pub type PendingRequestWaker = Arc<dyn Fn() + Send + Sync + 'static>;

/// Process-local callback used to wake the GUI event loop when the IPC queue
/// transitions from empty to non-empty. The callback is replaceable because a
/// browser window can be recreated in-process; callers must tolerate a final
/// invocation racing with replacement.
static PENDING_REQUEST_WAKER: OnceLock<Mutex<Option<PendingRequestWaker>>> = OnceLock::new();

struct EventSubscriber {
    queue: Arc<EventQueue>,
    active: Arc<std::sync::atomic::AtomicBool>,
    event_types: Option<Vec<String>>,
    consecutive_overflows: usize,
    last_sequence: u64,
}

const MAX_CONSECUTIVE_EVENT_OVERFLOWS: usize = 2;

struct EventQueue {
    state: Mutex<EventQueueState>,
    wake: Condvar,
}

struct EventQueueState {
    messages: VecDeque<Vec<u8>>,
    bytes: usize,
    closed: bool,
}

impl EventQueue {
    fn new() -> Self {
        Self {
            state: Mutex::new(EventQueueState {
                messages: VecDeque::new(),
                bytes: 0,
                closed: false,
            }),
            wake: Condvar::new(),
        }
    }

    fn push(&self, payload: Vec<u8>) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if state.closed
            || state.messages.len() >= MAX_EVENT_QUEUE_MESSAGES
            || state
                .bytes
                .checked_add(payload.len())
                .is_none_or(|bytes| bytes > MAX_EVENT_QUEUE_BYTES)
        {
            return false;
        }
        state.bytes += payload.len();
        state.messages.push_back(payload);
        self.wake.notify_one();
        true
    }

    fn replace_with_gap(&self, gap: Vec<u8>, payload: Vec<u8>) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if state.closed
            || gap.len() > MAX_EVENT_QUEUE_BYTES
            || payload.len() > MAX_EVENT_QUEUE_BYTES
            || gap.len().saturating_add(payload.len()) > MAX_EVENT_QUEUE_BYTES
        {
            return false;
        }
        state.messages.clear();
        state.bytes = gap.len().saturating_add(payload.len());
        state.messages.push_back(gap);
        state.messages.push_back(payload);
        self.wake.notify_one();
        true
    }

    fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
            state.messages.clear();
            state.bytes = 0;
            self.wake.notify_all();
        }
    }

    fn take(&self, active: &std::sync::atomic::AtomicBool) -> Option<Vec<u8>> {
        let mut state = self.state.lock().ok()?;
        loop {
            if let Some(payload) = state.messages.pop_front() {
                state.bytes = state.bytes.saturating_sub(payload.len());
                return Some(payload);
            }
            if state.closed || !active.load(std::sync::atomic::Ordering::Acquire) {
                return None;
            }
            state = self.wake.wait(state).ok()?;
        }
    }
}

static EVENT_SUBSCRIBERS: OnceLock<Mutex<Vec<EventSubscriber>>> = OnceLock::new();

/// Registers a bounded, server-pushed event stream on an authenticated local
/// connection.
///
/// # Errors
///
/// Returns an error if the writer thread cannot be started or the stream is
/// not usable.
pub fn subscribe_event_stream(
    stream: UnixStream,
    event_types: Option<Vec<String>>,
) -> Result<(), String> {
    let queue = Arc::new(EventQueue::new());
    let active = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let registry = EVENT_SUBSCRIBERS.get_or_init(|| Mutex::new(Vec::new()));
    let mut registry = registry
        .lock()
        .map_err(|_| "IPC event subscriber registry is poisoned".to_owned())?;
    registry.retain(|subscriber| subscriber.active.load(std::sync::atomic::Ordering::Acquire));
    if registry.len() >= MAX_EVENT_SUBSCRIBERS {
        return Err("IPC event subscriber limit reached".into());
    }
    let writer_active = Arc::clone(&active);
    let writer_queue = Arc::clone(&queue);
    thread::Builder::new()
        .name("ferric-browser-ipc-events".into())
        .spawn(move || {
            let mut stream = stream;
            while let Some(payload) = writer_queue.take(&writer_active) {
                if write_frame(&mut stream, &payload).is_err() {
                    writer_active.store(false, std::sync::atomic::Ordering::Release);
                    writer_queue.close();
                    break;
                }
            }
            writer_active.store(false, std::sync::atomic::Ordering::Release);
            writer_queue.close();
        })
        .map_err(|error| format!("could not start IPC event writer: {error}"))?;
    registry.push(EventSubscriber {
        queue,
        active,
        event_types,
        consecutive_overflows: 0,
        last_sequence: 0,
    });
    Ok(())
}

/// Publishes one sanitized notification to all active subscribers. The queue
/// is bounded by both message count and encoded bytes. On the first two
/// consecutive overflows, queued events are replaced by a gap marker and the
/// current event; a persistently slow subscriber is then disconnected.
pub fn publish_event(notification: &EventNotification) {
    let Ok(payload) = serde_json::to_vec(notification) else {
        return;
    };
    if payload.len() > MAX_FRAME_BYTES {
        return;
    }
    let Some(registry) = EVENT_SUBSCRIBERS.get() else {
        return;
    };
    let Ok(mut registry) = registry.lock() else {
        return;
    };
    registry.retain_mut(|subscriber| {
        if !subscriber.active.load(std::sync::atomic::Ordering::Acquire) {
            return false;
        }
        if let Some(event_types) = subscriber.event_types.as_ref()
            && !event_types
                .iter()
                .any(|event_type| event_type == &notification.event_type)
        {
            return true;
        }
        deliver_event_to_subscriber(subscriber, notification, &payload)
    });
}

fn deliver_event_to_subscriber(
    subscriber: &mut EventSubscriber,
    notification: &EventNotification,
    payload: &[u8],
) -> bool {
    if subscriber.queue.push(payload.to_vec()) {
        subscriber.consecutive_overflows = 0;
        subscriber.last_sequence = notification.sequence;
        return true;
    }
    if subscriber.consecutive_overflows >= MAX_CONSECUTIVE_EVENT_OVERFLOWS {
        subscriber
            .active
            .store(false, std::sync::atomic::Ordering::Release);
        subscriber.queue.close();
        return false;
    }

    let gap = EventNotification {
        instance_id: notification.instance_id.clone(),
        sequence: subscriber.last_sequence,
        event_type: "events.gap".into(),
        payload: json!({
            "reason": "subscriber_backpressure",
            "refresh_required": true,
            "watermark": notification.sequence
        }),
    };
    let Ok(gap) = serde_json::to_vec(&gap) else {
        subscriber
            .active
            .store(false, std::sync::atomic::Ordering::Release);
        subscriber.queue.close();
        return false;
    };
    if !subscriber.queue.replace_with_gap(gap, payload.to_vec()) {
        subscriber
            .active
            .store(false, std::sync::atomic::Ordering::Release);
        subscriber.queue.close();
        return false;
    }
    subscriber.consecutive_overflows += 1;
    subscriber.last_sequence = notification.sequence;
    true
}

/// Queues one request for the UI/application thread.
///
/// # Errors
///
/// Returns the request unchanged when the bounded queue is full or poisoned.
pub fn enqueue_request(request: PendingRequest) -> Result<(), PendingRequest> {
    let queue = PENDING_REQUESTS.get_or_init(|| Mutex::new(VecDeque::new()));
    let Ok(mut queue) = queue.lock() else {
        return Err(request);
    };
    if queue.len() >= MAX_PENDING_REQUESTS {
        return Err(request);
    }
    let should_wake = queue.is_empty();
    queue.push_back(request);
    drop(queue);
    if should_wake {
        wake_pending_request_consumer();
    }
    Ok(())
}

/// Installs the callback that wakes the owner of the pending-request queue.
/// If requests arrived before registration, the newly installed callback is
/// invoked immediately so startup cannot strand queued work.
pub fn install_pending_request_waker(waker: PendingRequestWaker) {
    let slot = PENDING_REQUEST_WAKER.get_or_init(|| Mutex::new(None));
    let registered = slot.lock().ok().and_then(|mut current| {
        *current = Some(waker);
        current.as_ref().cloned()
    });
    if has_pending_requests()
        && let Some(waker) = registered
    {
        waker();
    }
}

fn wake_pending_request_consumer() {
    let waker = PENDING_REQUEST_WAKER
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|waker| waker.clone());
    if let Some(waker) = waker {
        waker();
    }
}

/// Reports whether the GUI queue still contains work. This is primarily used
/// by event-loop adapters to close the enqueue/drain race without polling.
#[must_use]
pub fn has_pending_requests() -> bool {
    PENDING_REQUESTS
        .get()
        .and_then(|queue| queue.lock().ok())
        .is_some_and(|queue| !queue.is_empty())
}

/// Takes the oldest queued request for execution on the UI/application thread.
#[must_use]
pub fn take_pending_request() -> Option<PendingRequest> {
    PENDING_REQUESTS
        .get()
        .and_then(|queue| queue.lock().ok())
        .and_then(|mut queue| queue.pop_front())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProtocolError {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

/// A typed failure that is safe to expose through the CLI or local IPC.
///
/// The stable [`ErrorCode`] is selected where the failure occurs.  Adapters
/// must not infer it from a formatted diagnostic message: messages are for
/// people and diagnostics, while codes are the compatibility contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicError {
    code: ErrorCode,
    user_message: String,
    diagnostic_context: String,
}

impl PublicError {
    /// Creates a public failure with a stable code and safe user message.
    #[must_use]
    pub fn new(
        code: ErrorCode,
        user_message: impl Into<String>,
        diagnostic_context: impl Into<String>,
    ) -> Self {
        Self {
            code,
            user_message: user_message.into(),
            diagnostic_context: diagnostic_context.into(),
        }
    }

    /// Wraps an internal failure that is not safe to classify more precisely.
    #[must_use]
    pub fn engine(error: impl Into<String>) -> Self {
        let diagnostic_context = error.into();
        Self::new(
            ErrorCode::Engine,
            "The browser could not complete that request.",
            diagnostic_context,
        )
    }

    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        self.code
    }

    #[must_use]
    pub fn user_message(&self) -> &str {
        &self.user_message
    }

    #[must_use]
    pub fn diagnostic_context(&self) -> &str {
        &self.diagnostic_context
    }

    /// Converts this error into the public protocol envelope before response
    /// redaction and correlation metadata are applied.
    #[must_use]
    pub fn into_protocol_error(self) -> ProtocolError {
        ProtocolError {
            code: self.code.as_str().into(),
            message: self.user_message,
            details: (!self.diagnostic_context.is_empty())
                .then(|| json!({"diagnostic_context": self.diagnostic_context})),
        }
    }
}

impl std::fmt::Display for PublicError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.user_message)
    }
}

impl std::error::Error for PublicError {}

impl From<String> for PublicError {
    fn from(error: String) -> Self {
        Self::engine(error)
    }
}

impl From<&str> for PublicError {
    fn from(error: &str) -> Self {
        Self::engine(error)
    }
}

/// Stable public error vocabulary shared by the CLI and local IPC.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InvalidArgument,
    Config,
    NotFound,
    NoInstance,
    StaleTarget,
    Denied,
    ConfirmationRequired,
    Unsupported,
    Busy,
    Timeout,
    Protocol,
    Io,
    Storage,
    Engine,
    Cancelled,
}

impl ErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidArgument => "E_INVALID_ARGUMENT",
            Self::Config => "E_CONFIG",
            Self::NotFound => "E_NOT_FOUND",
            Self::NoInstance => "E_NO_INSTANCE",
            Self::StaleTarget => "E_STALE_TARGET",
            Self::Denied => "E_DENIED",
            Self::ConfirmationRequired => "E_CONFIRMATION_REQUIRED",
            Self::Unsupported => "E_UNSUPPORTED",
            Self::Busy => "E_BUSY",
            Self::Timeout => "E_TIMEOUT",
            Self::Protocol => "E_PROTOCOL",
            Self::Io => "E_IO",
            Self::Storage => "E_STORAGE",
            Self::Engine => "E_ENGINE",
            Self::Cancelled => "E_CANCELLED",
        }
    }

    #[must_use]
    pub const fn guidance(self) -> (&'static str, &'static str) {
        match self {
            Self::InvalidArgument => (
                "no browser state was changed",
                "correct the arguments and retry",
            ),
            Self::Config => (
                "the last good configuration remains active",
                "fix the configuration and reload or retry",
            ),
            Self::NotFound => (
                "existing state was left unchanged",
                "refresh the available state and retry",
            ),
            Self::NoInstance => (
                "no running browser instance was changed",
                "start or select a running instance",
            ),
            Self::StaleTarget => (
                "the stale target was not acted on",
                "refresh the target and retry",
            ),
            Self::Denied => (
                "the request was denied and no new access was granted",
                "request access or change the applicable policy",
            ),
            Self::ConfirmationRequired => (
                "the operation is still unchanged",
                "confirm the operation explicitly",
            ),
            Self::Unsupported => (
                "the unsupported capability was not invoked",
                "use diagnostics or another supported path",
            ),
            Self::Busy => (
                "the active operation remains unchanged",
                "wait for it to finish and retry",
            ),
            Self::Timeout => (
                "the timed-out operation was not committed",
                "check the target and retry",
            ),
            Self::Protocol => (
                "the request was not executed",
                "upgrade or reconnect the client",
            ),
            Self::Io => (
                "the affected data was left unchanged where possible",
                "check the path and permissions, then retry",
            ),
            Self::Storage => (
                "existing stored data was preserved where possible",
                "check storage health and available space",
            ),
            Self::Engine => (
                "other browser state remains active",
                "open diagnostics and retry the affected operation",
            ),
            Self::Cancelled => (
                "the operation was cancelled without a partial commit",
                "retry it when ready",
            ),
        }
    }
}

/// Maps legacy/internal wire codes to the stable public vocabulary.
#[must_use]
pub fn canonical_error_code(code: &str) -> ErrorCode {
    match code {
        "E_INVALID_ARGUMENT" | "E_INVALID_ARGUMENTS" | "E_INVALID_PARAMS" | "E_ID_REUSE" => {
            ErrorCode::InvalidArgument
        }
        "E_CONFIG" => ErrorCode::Config,
        "E_NOT_FOUND" => ErrorCode::NotFound,
        "E_NO_INSTANCE" => ErrorCode::NoInstance,
        "E_STALE_TARGET" => ErrorCode::StaleTarget,
        "E_DENIED" | "E_PERMISSION" => ErrorCode::Denied,
        "E_CONFIRMATION_REQUIRED" => ErrorCode::ConfirmationRequired,
        "E_UNSUPPORTED" => ErrorCode::Unsupported,
        "E_BUSY" | "E_IN_FLIGHT" => ErrorCode::Busy,
        "E_TIMEOUT" => ErrorCode::Timeout,
        "E_PROTOCOL" | "E_PROTOCOL_VERSION" => ErrorCode::Protocol,
        "E_IO" => ErrorCode::Io,
        "E_STORAGE" => ErrorCode::Storage,
        "E_CANCELLED" => ErrorCode::Cancelled,
        _ => ErrorCode::Engine,
    }
}

impl Response {
    #[must_use]
    pub fn success(id: impl Into<String>, result: Value) -> Self {
        Self {
            id: id.into(),
            result: Some(result),
            error: None,
        }
    }

    #[must_use]
    pub fn failure(id: impl Into<String>, mut error: ProtocolError) -> Self {
        let code = canonical_error_code(&error.code);
        let (preserved, next_action) = code.guidance();
        let correlation_id = format!("err-{}", Uuid::new_v4());
        remember_error_correlation(&correlation_id, code.as_str());
        let details = match error.details.take() {
            Some(Value::Object(details)) => details,
            Some(other) => serde_json::Map::from_iter([("source".into(), other)]),
            None => serde_json::Map::new(),
        };
        let mut details = Value::Object(details);
        redact_value(&mut details);
        let Value::Object(mut details) = details else {
            unreachable!("error details are always converted to an object");
        };
        details.insert(
            "correlation_id".into(),
            Value::String(correlation_id.clone()),
        );
        details.insert("preserved".into(), Value::String(preserved.into()));
        details.insert("next_action".into(), Value::String(next_action.into()));
        error.code = code.as_str().into();
        error.message = format!(
            "{}. Preserved: {preserved}. Next action: {next_action}. [correlation {correlation_id}]",
            redact_text(error.message.trim_end_matches('.'))
        );
        error.details = Some(json!(details));
        Self {
            id: id.into(),
            result: None,
            error: Some(error),
        }
    }
}

fn remember_error_correlation(correlation_id: &str, code: &str) {
    let correlations = ERROR_CORRELATIONS.get_or_init(|| Mutex::new(VecDeque::new()));
    let Ok(mut correlations) = correlations.lock() else {
        return;
    };
    correlations.push_back((correlation_id.to_owned(), code.to_owned()));
    while correlations.len() > MAX_ERROR_CORRELATIONS {
        correlations.pop_front();
    }
}

/// Returns bounded, privacy-safe correlation records for the diagnostics
/// report. Only the opaque ID and stable public error code are retained.
#[must_use]
pub fn recent_error_correlations() -> Vec<Value> {
    ERROR_CORRELATIONS
        .get()
        .and_then(|correlations| correlations.lock().ok())
        .map(|correlations| {
            correlations
                .iter()
                .map(|(correlation_id, code)| {
                    json!({
                        "correlation_id": correlation_id,
                        "code": code
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn sensitive_key(key: &str) -> bool {
    let key = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    key.contains("password")
        || key.contains("passwd")
        || key == "pin"
        || key.contains("auth")
        || key.contains("cookie")
        || key.contains("bearer")
        || key.contains("token")
        || key.contains("secret")
        || key.contains("form")
        || key.contains("body")
        || key.contains("header")
}

fn redact_value(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if sensitive_key(key) {
                    *value = Value::String(REDACTED.into());
                } else {
                    redact_value(value);
                }
            }
        }
        Value::Array(values) => values.iter_mut().for_each(redact_value),
        Value::String(text) => *text = redact_text(text),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn redact_text(text: &str) -> String {
    let mut redacted = Vec::new();
    let mut redact_next = false;
    for token in text.split_whitespace() {
        if redact_next {
            redacted.push(REDACTED.to_owned());
            redact_next = false;
            continue;
        }
        let lower = token.to_ascii_lowercase();
        if lower == "bearer" || lower == "authorization:" {
            redacted.push(token.to_owned());
            redact_next = true;
            continue;
        }
        redacted.push(redact_token(token));
    }
    redacted.join(" ")
}

fn redact_token(token: &str) -> String {
    if looks_like_url(token) {
        let without_fragment = token.split('#').next().unwrap_or_default();
        let without_query = without_fragment.split('?').next().unwrap_or_default();
        let scheme_end = without_query
            .find("://")
            .expect("looks_like_url checked the scheme");
        let authority_start = scheme_end + 3;
        let authority_end = without_query[authority_start..]
            .find('/')
            .map_or(without_query.len(), |offset| authority_start + offset);
        let authority = &without_query[authority_start..authority_end];
        let authority = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        return format!(
            "{}{}{}",
            &without_query[..authority_start],
            authority,
            &without_query[authority_end..]
        );
    }
    if let Some((key, _)) = token.split_once('=')
        && sensitive_key(key.trim_matches(|character: char| !character.is_ascii_alphanumeric()))
    {
        return format!("{key}={REDACTED}");
    }
    token.to_owned()
}

fn looks_like_url(value: &str) -> bool {
    let Some(scheme_end) = value.find("://") else {
        return false;
    };
    scheme_end > 0
        && value[..scheme_end]
            .chars()
            .enumerate()
            .all(|(index, character)| {
                if index == 0 {
                    character.is_ascii_alphabetic()
                } else {
                    character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
                }
            })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HelloParams {
    pub protocol_major: u32,
    #[serde(default)]
    pub protocol_minor: u32,
    pub client: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HelloResult {
    pub protocol_major: u32,
    pub protocol_minor: u32,
    pub instance_id: String,
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FrameError {
    Io(String),
    ZeroLength,
    TooLarge(usize),
    InvalidUtf8,
    InvalidJson(String),
    TooDeep,
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(message) => write!(formatter, "IPC I/O error: {message}"),
            Self::ZeroLength => formatter.write_str("IPC frame length must be nonzero"),
            Self::TooLarge(length) => write!(formatter, "IPC frame is too large: {length} bytes"),
            Self::InvalidUtf8 => formatter.write_str("IPC frame is not UTF-8"),
            Self::InvalidJson(message) => write!(formatter, "IPC JSON is invalid: {message}"),
            Self::TooDeep => write!(formatter, "IPC JSON exceeds depth {MAX_JSON_DEPTH}"),
        }
    }
}

impl std::error::Error for FrameError {}

/// Reads one four-byte big-endian length-prefixed JSON frame.
///
/// # Errors
///
/// Returns an error for malformed lengths, oversized frames, invalid I/O, or
/// incomplete payloads.
pub fn read_frame(reader: &mut impl Read) -> Result<Option<Vec<u8>>, FrameError> {
    let mut length_bytes = [0_u8; 4];
    match reader.read_exact(&mut length_bytes) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(FrameError::Io(error.to_string())),
    }
    let length = u32::from_be_bytes(length_bytes) as usize;
    if length == 0 {
        return Err(FrameError::ZeroLength);
    }
    if length > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(length));
    }
    let mut payload = vec![0_u8; length];
    reader
        .read_exact(&mut payload)
        .map_err(|error| FrameError::Io(error.to_string()))?;
    Ok(Some(payload))
}

/// Writes one four-byte big-endian length-prefixed JSON frame.
///
/// # Errors
///
/// Returns an error for an empty or oversized payload or a failed write.
pub fn write_frame(writer: &mut impl Write, payload: &[u8]) -> Result<(), FrameError> {
    if payload.is_empty() {
        return Err(FrameError::ZeroLength);
    }
    if payload.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(payload.len()));
    }
    let length = u32::try_from(payload.len()).map_err(|_| FrameError::TooLarge(payload.len()))?;
    writer
        .write_all(&length.to_be_bytes())
        .and_then(|()| writer.write_all(payload))
        .and_then(|()| writer.flush())
        .map_err(|error| FrameError::Io(error.to_string()))
}

/// Parses a JSON request and enforces the bounded object/depth contract.
///
/// # Errors
///
/// Returns an error for invalid JSON, invalid identifiers, or excessive
/// nesting.
pub fn parse_request(payload: &[u8]) -> Result<Request, FrameError> {
    if payload.is_empty() {
        return Err(FrameError::ZeroLength);
    }
    if payload.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(payload.len()));
    }
    let mut deserializer = serde_json::Deserializer::from_slice(payload);
    let value = deserializer
        .deserialize_any(UniqueJsonVisitor)
        .map_err(|error| FrameError::InvalidJson(error.to_string()))?;
    deserializer
        .end()
        .map_err(|error| FrameError::InvalidJson(error.to_string()))?;
    if json_depth(&value) > MAX_JSON_DEPTH {
        return Err(FrameError::TooDeep);
    }
    let request: Request = serde_json::from_value(value)
        .map_err(|error| FrameError::InvalidJson(error.to_string()))?;
    validate_request(&request)?;
    Ok(request)
}

struct UniqueJsonVisitor;

impl<'de> de::Visitor<'de> for UniqueJsonVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("JSON number is not finite"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Value::String(value))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(UniqueJsonSeed)? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut values = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom("JSON contains a duplicate object key"));
            }
            values.insert(key, map.next_value_seed(UniqueJsonSeed)?);
        }
        Ok(Value::Object(values))
    }
}

struct UniqueJsonSeed;

impl<'de> de::DeserializeSeed<'de> for UniqueJsonSeed {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueJsonVisitor)
    }
}

/// Serializes a response into one bounded JSON payload.
///
/// # Errors
///
/// Returns an error when serialization fails or the encoded response exceeds
/// the frame limit.
pub fn serialize_response(response: &Response) -> Result<Vec<u8>, FrameError> {
    let payload =
        serde_json::to_vec(response).map_err(|error| FrameError::InvalidJson(error.to_string()))?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(payload.len()));
    }
    Ok(payload)
}

fn validate_request(request: &Request) -> Result<(), FrameError> {
    if request.id.is_empty()
        || request.id.len() > MAX_ID_BYTES
        || request.id.chars().any(char::is_control)
    {
        return Err(FrameError::InvalidJson(
            "request id is empty, too long, or contains a control character".into(),
        ));
    }
    if request.method.is_empty()
        || request.method.len() > MAX_ID_BYTES
        || request.method.chars().any(char::is_control)
    {
        return Err(FrameError::InvalidJson(
            "request method is empty, too long, or contains a control character".into(),
        ));
    }
    Ok(())
}

fn json_depth(value: &Value) -> usize {
    match value {
        Value::Array(values) => values
            .iter()
            .map(json_depth)
            .max()
            .unwrap_or(0)
            .saturating_add(1),
        Value::Object(values) => values
            .values()
            .map(json_depth)
            .max()
            .unwrap_or(0)
            .saturating_add(1),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 0,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheLookup {
    Miss,
    Hit(Response),
    IdReuse,
    InFlight,
}

#[derive(Debug)]
struct CacheEntry {
    fingerprint: u64,
    response: Response,
    expires: Instant,
}

/// Bounded per-connection response cache for safe request retransmission.
#[derive(Debug)]
pub struct RequestCache {
    entries: HashMap<String, CacheEntry>,
    pending: HashMap<String, (u64, Instant)>,
    order: VecDeque<String>,
}

impl Default for RequestCache {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            pending: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn lookup(&mut self, request: &Request, now: Instant) -> CacheLookup {
        self.prune(now);
        let Some(entry) = self.entries.get(&request.id) else {
            return self.pending.get(&request.id).map_or(
                CacheLookup::Miss,
                |(cached_fingerprint, _)| {
                    if *cached_fingerprint == fingerprint(request) {
                        CacheLookup::InFlight
                    } else {
                        CacheLookup::IdReuse
                    }
                },
            );
        };
        if entry.fingerprint == fingerprint(request) {
            CacheLookup::Hit(entry.response.clone())
        } else {
            CacheLookup::IdReuse
        }
    }

    pub fn reserve(&mut self, request: &Request, now: Instant) -> CacheLookup {
        let lookup = self.lookup(request, now);
        if matches!(lookup, CacheLookup::Miss) {
            self.pending
                .insert(request.id.clone(), (fingerprint(request), now + CACHE_TTL));
        }
        lookup
    }

    pub fn insert(&mut self, request: &Request, response: Response, now: Instant) {
        self.prune(now);
        self.pending.remove(&request.id);
        if self.entries.contains_key(&request.id) {
            self.order.retain(|id| id != &request.id);
        }
        self.entries.insert(
            request.id.clone(),
            CacheEntry {
                fingerprint: fingerprint(request),
                response,
                expires: now + CACHE_TTL,
            },
        );
        self.order.push_back(request.id.clone());
        while self.order.len() > MAX_CACHED_REQUESTS {
            if let Some(id) = self.order.pop_front() {
                self.entries.remove(&id);
            }
        }
    }

    fn prune(&mut self, now: Instant) {
        self.pending.retain(|_, (_, expires)| *expires > now);
        while let Some(id) = self.order.front() {
            let expired = self
                .entries
                .get(id)
                .is_none_or(|entry| entry.expires <= now);
            if !expired {
                break;
            }
            let id = self.order.pop_front().expect("front exists");
            self.entries.remove(&id);
        }
    }
}

fn fingerprint(request: &Request) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    request.method.hash(&mut hasher);
    request.params.to_string().hash(&mut hasher);
    hasher.finish()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstancePaths {
    pub directory: PathBuf,
    pub socket: PathBuf,
    pub lock: PathBuf,
}

/// Creates a stable, bounded instance path from a canonical base and identity.
pub fn instance_paths(
    runtime_root: impl AsRef<Path>,
    canonical_base: &Path,
    identity: &str,
) -> InstancePaths {
    let digest = stable_digest(&format!("{}\n{identity}", canonical_base.display()));
    let directory = runtime_root
        .as_ref()
        .join("instances")
        .join(format!("{digest:016x}"));
    InstancePaths {
        socket: directory.join("browser.sock"),
        lock: directory.join("instance.lock"),
        directory,
    }
}

fn stable_digest(value: &str) -> u64 {
    value.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        hash.wrapping_mul(0x0100_0000_01b3)
            .wrapping_add(u64::from(byte))
    })
}

#[derive(Debug)]
pub enum InstanceError {
    Io {
        path: PathBuf,
        message: String,
    },
    Busy {
        path: PathBuf,
        owner: Option<String>,
    },
}

impl std::fmt::Display for InstanceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => write!(formatter, "{}: {message}", path.display()),
            Self::Busy { path, owner } => write!(
                formatter,
                "instance is already owned ({}; owner {})",
                path.display(),
                owner.as_deref().unwrap_or("unknown")
            ),
        }
    }
}

impl std::error::Error for InstanceError {}

/// Holds the kernel-backed instance lock until dropped.
#[derive(Debug)]
pub struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    /// Acquires the instance ownership lock.
    ///
    /// # Errors
    ///
    /// Returns an error when the directory cannot be prepared or another
    /// process already owns the instance.
    pub fn acquire(paths: &InstancePaths) -> Result<Self, InstanceError> {
        fs::create_dir_all(&paths.directory)
            .map_err(|error| instance_io(&paths.directory, &error))?;
        set_mode(&paths.directory, 0o700).map_err(|error| instance_io(&paths.directory, &error))?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .custom_flags(libc::O_NOFOLLOW)
            .mode(0o600)
            .open(&paths.lock)
            .map_err(|error| instance_io(&paths.lock, &error))?;
        match file.try_lock_exclusive() {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Err(InstanceError::Busy {
                    path: paths.lock.clone(),
                    owner: fs::read_to_string(&paths.lock).ok(),
                });
            }
            Err(error) => return Err(instance_io(&paths.lock, &error)),
        }
        file.set_len(0)
            .and_then(|()| file.seek(SeekFrom::Start(0)).map(|_| ()))
            .map_err(|error| instance_io(&paths.lock, &error))?;
        writeln!(file, "pid={}", std::process::id())
            .and_then(|()| file.sync_all())
            .map_err(|error| instance_io(&paths.lock, &error))?;
        Ok(Self { _file: file })
    }
}

/// Binds an `AF_UNIX` listener after safely replacing a dead socket entry.
///
/// # Errors
///
/// Returns an error when a live owner exists, a socket symlink is present, or
/// the directory/socket cannot be prepared.
pub fn bind_listener(paths: &InstancePaths) -> Result<UnixListener, InstanceError> {
    fs::create_dir_all(&paths.directory).map_err(|error| instance_io(&paths.directory, &error))?;
    set_mode(&paths.directory, 0o700).map_err(|error| instance_io(&paths.directory, &error))?;
    if let Ok(metadata) = fs::symlink_metadata(&paths.socket) {
        if metadata.file_type().is_symlink() {
            return Err(InstanceError::Io {
                path: paths.socket.clone(),
                message: "refusing to replace a socket symlink".into(),
            });
        }
        if !metadata.file_type().is_socket() {
            return Err(InstanceError::Io {
                path: paths.socket.clone(),
                message: "refusing to replace a non-socket path".into(),
            });
        }
        if UnixStream::connect(&paths.socket).is_ok() {
            return Err(InstanceError::Busy {
                path: paths.socket.clone(),
                owner: None,
            });
        }
        fs::remove_file(&paths.socket).map_err(|error| instance_io(&paths.socket, &error))?;
    }
    let listener =
        UnixListener::bind(&paths.socket).map_err(|error| instance_io(&paths.socket, &error))?;
    set_mode(&paths.socket, 0o600).map_err(|error| instance_io(&paths.socket, &error))?;
    Ok(listener)
}

/// Returns the Linux peer UID for a connected Unix stream.
/// Other platforms return `None`; the production target is Linux/Wayland.
#[must_use]
pub fn peer_uid(stream: &UnixStream) -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        let mut credentials = std::mem::MaybeUninit::<libc::ucred>::uninit();
        let Ok(mut length) = libc::socklen_t::try_from(std::mem::size_of::<libc::ucred>()) else {
            return None;
        };
        // SAFETY: `credentials` points to writable storage of the advertised
        // size, and the stream owns a valid socket descriptor.
        let result = unsafe {
            libc::getsockopt(
                std::os::fd::AsRawFd::as_raw_fd(stream),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                credentials.as_mut_ptr().cast(),
                &raw mut length,
            )
        };
        if result == 0 && length as usize >= std::mem::size_of::<libc::ucred>() {
            // SAFETY: getsockopt initialized the structure on success.
            return Some(unsafe { credentials.assume_init() }.uid);
        }
    }
    None
}

/// Returns the effective UID used for local IPC peer checks on Linux.
#[must_use]
pub fn current_uid() -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: geteuid has no preconditions and only reads process state.
        Some(unsafe { libc::geteuid() })
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    Ok(())
}

fn instance_io(path: &Path, error: &io::Error) -> InstanceError {
    InstanceError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[cfg(target_os = "linux")]
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    fn request(id: &str, method: &str, params: Value) -> Request {
        Request {
            id: id.into(),
            method: method.into(),
            params,
        }
    }

    #[test]
    fn instance_lock_crash_test_child() {
        let Ok(root) = std::env::var("FERRIC_BROWSER_INSTANCE_LOCK_CHILD_ROOT") else {
            return;
        };
        let ready = PathBuf::from(
            std::env::var("FERRIC_BROWSER_INSTANCE_LOCK_CHILD_READY").expect("child ready path"),
        );
        let paths = instance_paths(root, Path::new("/tmp/ferric-browser"), "uid=1000");
        let _lock = InstanceLock::acquire(&paths).expect("child lock");
        fs::write(ready, b"ready").expect("signal ready");
        thread::sleep(Duration::from_secs(30));
    }

    #[test]
    fn frames_are_bounded_and_big_endian() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, b"{}").expect("write");
        assert_eq!(&bytes[..4], &[0, 0, 0, 2]);
        assert_eq!(
            read_frame(&mut Cursor::new(bytes)).expect("read"),
            Some(b"{}".to_vec())
        );
        assert_eq!(
            read_frame(&mut Cursor::new([0, 0, 0, 0])).unwrap_err(),
            FrameError::ZeroLength
        );
        assert_eq!(
            read_frame(&mut Cursor::new([0xff, 0xff, 0xff, 0xff])).unwrap_err(),
            FrameError::TooLarge(u32::MAX as usize)
        );
    }

    #[test]
    fn request_parsing_enforces_depth_and_identifiers() {
        let valid = br#"{"id":"one","method":"hello","params":{"a":1}}"#;
        assert_eq!(parse_request(valid).expect("request").method, "hello");
        let mut deep = String::from(r#"{"id":"one","method":"hello","params":"#);
        for _ in 0..=MAX_JSON_DEPTH {
            deep.push('[');
        }
        deep.push('0');
        for _ in 0..=MAX_JSON_DEPTH {
            deep.push(']');
        }
        deep.push('}');
        assert_eq!(
            parse_request(deep.as_bytes()).unwrap_err(),
            FrameError::TooDeep
        );
        let invalid = br#"{"id":"","method":"hello"}"#;
        assert!(matches!(
            parse_request(invalid),
            Err(FrameError::InvalidJson(_))
        ));
        assert!(matches!(
            parse_request(br#"{"id":"one","id":"two","method":"hello"}"#),
            Err(FrameError::InvalidJson(message)) if message.contains("duplicate object key")
        ));
        assert!(matches!(
            parse_request(
                br#"{"id":"one","method":"hello","params":{"nested":{"x":1,"x":2}}}"#
            ),
            Err(FrameError::InvalidJson(message)) if message.contains("duplicate object key")
        ));
    }

    #[test]
    fn cache_hits_same_request_and_rejects_id_reuse() {
        let now = Instant::now();
        let mut cache = RequestCache::new();
        let first = request("one", "tabs.query", serde_json::json!({"x": 1}));
        let response = Response::success("one", serde_json::json!({"ok": true}));
        assert_eq!(cache.lookup(&first, now), CacheLookup::Miss);
        assert_eq!(cache.reserve(&first, now), CacheLookup::Miss);
        assert_eq!(cache.lookup(&first, now), CacheLookup::InFlight);
        cache.insert(&first, response.clone(), now);
        assert_eq!(cache.lookup(&first, now), CacheLookup::Hit(response));
        let reused = request("one", "tabs.query", serde_json::json!({"x": 2}));
        assert_eq!(cache.lookup(&reused, now), CacheLookup::IdReuse);
        assert_eq!(cache.lookup(&first, now + CACHE_TTL), CacheLookup::Miss);
    }

    #[test]
    fn error_aliases_map_to_the_stable_public_vocabulary() {
        assert_eq!(
            canonical_error_code("E_INVALID_PARAMS"),
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            canonical_error_code("E_PROTOCOL_VERSION"),
            ErrorCode::Protocol
        );
        assert_eq!(canonical_error_code("E_PERMISSION"), ErrorCode::Denied);
        assert_eq!(canonical_error_code("E_IN_FLIGHT"), ErrorCode::Busy);
        assert_eq!(
            canonical_error_code("unknown-internal-code"),
            ErrorCode::Engine
        );
    }

    #[test]
    fn public_error_keeps_code_independent_of_diagnostic_wording() {
        let public = PublicError::new(
            ErrorCode::StaleTarget,
            "The selected tab is no longer available.",
            "tab target was removed while a command was queued",
        );
        let protocol = public.into_protocol_error();
        assert_eq!(protocol.code, "E_STALE_TARGET");
        assert_eq!(protocol.message, "The selected tab is no longer available.");
        assert_eq!(
            protocol.details.expect("diagnostic context")["diagnostic_context"],
            "tab target was removed while a command was queued"
        );
    }

    #[test]
    fn failure_response_contains_guidance_and_opaque_correlation() {
        let response = Response::failure(
            "request-1",
            ProtocolError {
                code: "E_INVALID_PARAMS".into(),
                message: "bad request".into(),
                details: None,
            },
        );
        let error = response.error.expect("error response");
        assert_eq!(error.code, "E_INVALID_ARGUMENT");
        assert!(
            error
                .message
                .contains("Preserved: no browser state was changed")
        );
        assert!(
            error
                .message
                .contains("Next action: correct the arguments and retry")
        );
        let details = error.details.expect("error details");
        assert_eq!(details["preserved"], "no browser state was changed");
        assert_eq!(details["next_action"], "correct the arguments and retry");
        assert!(
            details["correlation_id"]
                .as_str()
                .is_some_and(|value| value.starts_with("err-") && value.len() > 8)
        );
        let correlation_id = details["correlation_id"].as_str().expect("correlation ID");
        assert!(recent_error_correlations().iter().any(|record| {
            record["correlation_id"] == correlation_id && record["code"] == "E_INVALID_ARGUMENT"
        }));
    }

    #[test]
    fn failure_response_redacts_sensitive_messages_and_details() {
        let response = Response::failure(
            "request-2",
            ProtocolError {
                code: "E_ENGINE".into(),
                message:
                    "failed https://user:pass@example.test/path?token=secret#fragment Bearer abc123"
                        .into(),
                details: Some(json!({
                    "authorization": "Bearer abc123",
                    "cookie": "session=secret",
                    "form_input": "pin=1234",
                    "nested": ["https://example.test/?password=secret"]
                })),
            },
        );
        let error = response.error.expect("error response");
        assert!(!error.message.contains("pass@example"));
        assert!(!error.message.contains("token=secret"));
        assert!(!error.message.contains("fragment"));
        assert!(!error.message.contains("abc123"));
        let details = error.details.expect("error details");
        assert_eq!(details["authorization"], REDACTED);
        assert_eq!(details["cookie"], REDACTED);
        assert_eq!(details["form_input"], REDACTED);
        assert_eq!(details["nested"][0], "https://example.test/");
    }

    #[test]
    fn instance_paths_are_digest_based_and_lock_is_exclusive() {
        let root =
            std::env::temp_dir().join(format!("ferric-browser-ipc-{}", uuid::Uuid::new_v4()));
        let paths = instance_paths(&root, Path::new("/tmp/ferric-browser"), "uid=1000");
        assert!(paths.socket.to_string_lossy().contains("instances"));
        let lock = InstanceLock::acquire(&paths).expect("lock");
        assert!(matches!(
            InstanceLock::acquire(&paths),
            Err(InstanceError::Busy { .. })
        ));
        drop(lock);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn persistent_unlocked_instance_file_is_reclaimed() {
        let root =
            std::env::temp_dir().join(format!("ferric-browser-ipc-stale-{}", uuid::Uuid::new_v4()));
        let paths = instance_paths(&root, Path::new("/tmp/ferric-browser"), "uid=1000");
        fs::create_dir_all(&paths.directory).expect("directory");
        fs::write(&paths.lock, "pid=4294967295\n").expect("persistent lock file");

        let lock = InstanceLock::acquire(&paths).expect("unlocked file is reclaimable");
        assert!(paths.lock.is_file());
        drop(lock);
        assert!(paths.lock.is_file());
        let second = InstanceLock::acquire(&paths).expect("lock is released on drop");
        drop(second);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn instance_lock_reports_owner_metadata_while_busy() {
        let root =
            std::env::temp_dir().join(format!("ferric-browser-ipc-live-{}", uuid::Uuid::new_v4()));
        let paths = instance_paths(&root, Path::new("/tmp/ferric-browser"), "uid=1000");
        let lock = InstanceLock::acquire(&paths).expect("lock");

        let error = InstanceLock::acquire(&paths).expect_err("second owner is refused");
        let InstanceError::Busy { owner, .. } = error else {
            panic!("expected busy error");
        };
        assert_eq!(
            owner.as_deref(),
            Some(format!("pid={}\n", std::process::id()).as_str())
        );
        drop(lock);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn instance_lock_is_released_when_owner_is_killed() {
        let root =
            std::env::temp_dir().join(format!("ferric-browser-ipc-crash-{}", uuid::Uuid::new_v4()));
        let paths = instance_paths(&root, Path::new("/tmp/ferric-browser"), "uid=1000");
        let ready = root.join("child-ready");
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "tests::instance_lock_crash_test_child",
                "--nocapture",
            ])
            .env("FERRIC_BROWSER_INSTANCE_LOCK_CHILD_ROOT", &root)
            .env("FERRIC_BROWSER_INSTANCE_LOCK_CHILD_READY", &ready)
            .spawn()
            .expect("spawn lock owner");
        for _ in 0..200 {
            if ready.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(ready.exists(), "child did not acquire the instance lock");
        assert!(matches!(
            InstanceLock::acquire(&paths),
            Err(InstanceError::Busy { .. })
        ));
        child.kill().expect("kill lock owner");
        child.wait().expect("reap lock owner");
        let reacquired = InstanceLock::acquire(&paths).expect("lock after owner death");
        drop(reacquired);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn listener_does_not_replace_a_regular_socket_path() {
        let root =
            std::env::temp_dir().join(format!("ferric-browser-ipc-file-{}", uuid::Uuid::new_v4()));
        let paths = instance_paths(&root, Path::new("/tmp/ferric-browser"), "uid=1000");
        fs::create_dir_all(&paths.directory).expect("directory");
        File::create(&paths.socket).expect("sentinel");
        assert!(matches!(
            bind_listener(&paths),
            Err(InstanceError::Io { .. })
        ));
        assert!(paths.socket.is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unix_socket_has_private_mode_and_peer_uid() {
        let root = std::env::temp_dir().join(format!(
            "ferric-browser-ipc-socket-{}",
            uuid::Uuid::new_v4()
        ));
        let paths = instance_paths(&root, Path::new("/tmp/ferric-browser"), "uid=1000");
        // Restricted runners may allow regular files but deny AF_UNIX
        // creation.  Preserve the explicit native-socket qualification while
        // allowing the portable IPC tests to run in that environment.
        let listener = match bind_listener(&paths) {
            Ok(listener) => listener,
            Err(InstanceError::Io { message, .. })
                if message.contains("Operation not permitted")
                    || message.contains("Permission denied")
                    || message.contains("Not supported") =>
            {
                eprintln!("skipping peer-UID socket test: AF_UNIX creation is unavailable");
                let _ = fs::remove_dir_all(root);
                return;
            }
            Err(error) => panic!("listener: {error}"),
        };
        let client = UnixStream::connect(&paths.socket).expect("client");
        let (server, _) = listener.accept().expect("accept");
        // SAFETY: geteuid has no preconditions and only reads process state.
        let expected_uid = unsafe { libc::geteuid() };
        assert_eq!(peer_uid(&server), Some(expected_uid));
        let mode = fs::symlink_metadata(&paths.socket)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        drop(client);
        drop(server);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn event_queue_replaces_overflowed_messages_with_a_gap() {
        let queue = EventQueue::new();
        let active = std::sync::atomic::AtomicBool::new(true);
        for _ in 0..MAX_EVENT_QUEUE_MESSAGES {
            assert!(queue.push(vec![1]));
        }
        assert!(!queue.push(vec![2]));
        assert!(queue.replace_with_gap(vec![3], vec![4]));
        assert_eq!(queue.take(&active), Some(vec![3]));
        assert_eq!(queue.take(&active), Some(vec![4]));
    }

    #[test]
    fn persistent_event_backpressure_disconnects_after_two_gap_markers() {
        let queue = Arc::new(EventQueue::new());
        for _ in 0..MAX_EVENT_QUEUE_MESSAGES {
            assert!(queue.push(vec![1]));
        }
        let active = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let mut subscriber = EventSubscriber {
            queue,
            active: Arc::clone(&active),
            event_types: None,
            consecutive_overflows: 0,
            last_sequence: 7,
        };
        let notification = EventNotification {
            instance_id: "instance".into(),
            sequence: 8,
            event_type: "tab.changed".into(),
            payload: json!({"tab_id": "tab-1"}),
        };
        let payload = serde_json::to_vec(&notification).expect("notification serializes");

        assert!(deliver_event_to_subscriber(
            &mut subscriber,
            &notification,
            &payload
        ));
        assert_eq!(subscriber.consecutive_overflows, 1);
        assert!(active.load(std::sync::atomic::Ordering::Acquire));
        for _ in 0..MAX_EVENT_QUEUE_MESSAGES.saturating_sub(2) {
            assert!(subscriber.queue.push(vec![1]));
        }

        assert!(deliver_event_to_subscriber(
            &mut subscriber,
            &notification,
            &payload
        ));
        assert_eq!(subscriber.consecutive_overflows, 2);
        assert!(active.load(std::sync::atomic::Ordering::Acquire));
        for _ in 0..MAX_EVENT_QUEUE_MESSAGES.saturating_sub(2) {
            assert!(subscriber.queue.push(vec![1]));
        }

        assert!(!deliver_event_to_subscriber(
            &mut subscriber,
            &notification,
            &payload
        ));
        assert!(!active.load(std::sync::atomic::Ordering::Acquire));
    }

    #[test]
    fn event_subscriber_registry_is_bounded() {
        let mut peers = Vec::new();
        for _ in 0..MAX_EVENT_SUBSCRIBERS {
            let (server, client) = UnixStream::pair().expect("socket pair");
            subscribe_event_stream(server, None).expect("subscriber within limit");
            peers.push(client);
        }
        let (server, _client) = UnixStream::pair().expect("overflow socket pair");
        assert_eq!(
            subscribe_event_stream(server, None),
            Err("IPC event subscriber limit reached".into())
        );

        if let Some(registry) = EVENT_SUBSCRIBERS.get()
            && let Ok(mut registry) = registry.lock()
        {
            for subscriber in &*registry {
                subscriber
                    .active
                    .store(false, std::sync::atomic::Ordering::Release);
                subscriber.queue.close();
            }
            registry.clear();
        }
        drop(peers);
    }
}
