//! Bounded, same-process handoff of explicitly addressed IPC requests.
//!
//! The instance owner alone consumes the authenticated socket queue. QML
//! synchronously calls the exact live secondary `BrowserUi` for each returned
//! window ID; neither the socket thread nor a QML-assembled JSON payload may
//! execute commands against a different window's application state.

use ferric_browser_core::{DispatchTarget, WindowId};
use ferric_browser_ipc::{PendingRequest, Request};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Mutex, OnceLock},
};

use super::{typed_ipc_action, typed_ipc_command};

struct RoutedRequest {
    owner_token: String,
    pending: PendingRequest,
}

static ROUTED_REQUESTS: OnceLock<Mutex<BTreeMap<WindowId, VecDeque<RoutedRequest>>>> =
    OnceLock::new();

fn routed_requests() -> &'static Mutex<BTreeMap<WindowId, VecDeque<RoutedRequest>>> {
    ROUTED_REQUESTS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub(super) fn explicit_window_target(request: &Request) -> Option<WindowId> {
    let selector = match request.method.as_str() {
        "command.execute" => typed_ipc_command(&request.params).ok()?.1.selector,
        "action.execute" => typed_ipc_action(&request.params).ok()?.1.selector,
        _ => return None,
    };
    match selector {
        DispatchTarget::Window(window) => Some(window),
        _ => None,
    }
}

pub(super) fn enqueue_routed(
    window: WindowId,
    owner_token: String,
    pending: PendingRequest,
) -> Result<(), PendingRequest> {
    let Ok(mut queues) = routed_requests().lock() else {
        return Err(pending);
    };
    let count = queues.values().map(VecDeque::len).sum::<usize>();
    if count >= ferric_browser_ipc::MAX_PENDING_REQUESTS {
        return Err(pending);
    }
    queues.entry(window).or_default().push_back(RoutedRequest {
        owner_token,
        pending,
    });
    Ok(())
}

pub(super) fn take_routed(window: WindowId, owner_token: &str) -> Vec<PendingRequest> {
    let Ok(mut queues) = routed_requests().lock() else {
        return Vec::new();
    };
    let Some(queue) = queues.get(&window) else {
        return Vec::new();
    };
    if queue
        .iter()
        .any(|request| request.owner_token != owner_token)
    {
        return Vec::new();
    }
    queues
        .remove(&window)
        .expect("checked routed queue")
        .into_iter()
        .map(|request| request.pending)
        .collect()
}

pub(super) fn reject_routed(window: WindowId) -> Vec<PendingRequest> {
    routed_requests()
        .lock()
        .ok()
        .and_then(|mut queues| queues.remove(&window))
        .map(|requests| {
            requests
                .into_iter()
                .map(|request| request.pending)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::net::UnixStream;

    #[test]
    fn only_typed_explicit_window_commands_are_routed() {
        let target = WindowId::from_display("windowid-919").expect("test window ID");
        let request = Request {
            id: "route-1".into(),
            method: "command.execute".into(),
            params: json!({
                "command": "help",
                "arguments": {},
                "context": {"window": target.to_string()}
            }),
        };
        assert_eq!(explicit_window_target(&request), Some(target));
        let mut active = request.clone();
        active.params["context"]["window"] = json!("active");
        assert_eq!(explicit_window_target(&active), None);
        let mut malformed = request.clone();
        malformed.params["context"]["window"] = json!("windowid-not-a-uuid");
        assert_eq!(explicit_window_target(&malformed), None);
        let mut query = request;
        query.method = "windows.query".into();
        assert_eq!(explicit_window_target(&query), None);
    }

    #[test]
    fn routed_request_requires_the_exact_live_owner_token() {
        let window = WindowId::from_display("windowid-920").expect("test window ID");
        let (_reader, writer) = UnixStream::pair().expect("disposable socket pair");
        let request = Request {
            id: "route-2".into(),
            method: "command.execute".into(),
            params: json!({}),
        };
        enqueue_routed(
            window,
            "exact-owner".into(),
            PendingRequest::new(request, writer),
        )
        .expect("bounded handoff");
        assert!(take_routed(window, "stale-owner").is_empty());
        let taken = take_routed(window, "exact-owner");
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].request().id, "route-2");
        assert!(reject_routed(window).is_empty());
    }
}
