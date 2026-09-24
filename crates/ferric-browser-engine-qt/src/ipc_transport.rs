//! Authenticated local IPC transport.
//!
//! This module owns framing, handshake, connection limits, and request queue
//! admission. Qt-facing code consumes the queued requests separately.

use ferric_browser_ipc::{
    CacheLookup, EventNotification, HelloParams, HelloResult, PendingRequest, ProtocolError,
    Request, RequestCache, Response, current_uid, enqueue_request, parse_request, peer_uid,
    publish_event, read_frame, serialize_response, write_frame,
};
use serde_json::Value;
use std::{
    os::unix::net::{UnixListener, UnixStream},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

static IPC_INSTANCE_ID: OnceLock<String> = OnceLock::new();

#[must_use]
pub(crate) fn instance_id() -> Option<&'static str> {
    IPC_INSTANCE_ID.get().map(String::as_str)
}

pub(crate) fn publish_ipc_event(sequence: &mut u64, event_type: &str, payload: Value) {
    *sequence = sequence.saturating_add(1);
    if let Some(instance_id) = instance_id() {
        publish_event(&EventNotification {
            instance_id: instance_id.to_owned(),
            sequence: *sequence,
            event_type: event_type.into(),
            payload,
        });
    }
}

/// Starts the authenticated local IPC accept loop used by the Qt-thread
/// request queue.
pub fn spawn_ipc_server(listener: UnixListener, instance_id: String) {
    let _ = IPC_INSTANCE_ID.set(instance_id.clone());
    let connections = Arc::new(AtomicUsize::new(0));
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else {
                break;
            };
            let accepted = connections
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                    (count < ferric_browser_ipc::MAX_IPC_CONNECTIONS).then_some(count + 1)
                })
                .is_ok();
            if !accepted {
                continue;
            }
            let instance_id = instance_id.clone();
            let connection_counter = Arc::clone(&connections);
            let started = thread::Builder::new()
                .name("ferric-browser-ipc-client".into())
                .spawn(move || {
                    serve_connection(stream, &instance_id);
                    connection_counter.fetch_sub(1, Ordering::AcqRel);
                });
            if started.is_err() {
                connections.fetch_sub(1, Ordering::AcqRel);
            }
        }
    });
}

fn serve_connection(mut stream: UnixStream, instance_id: &str) {
    if peer_uid(&stream)
        .zip(current_uid())
        .is_some_and(|(peer, current)| peer != current)
    {
        return;
    }
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let Some(request) = read_request(&mut stream) else {
        return;
    };
    let (is_hello, response) = handshake_response(request, instance_id);
    if !write_response(&mut stream, &response) || !is_hello || response.error.is_some() {
        return;
    }
    // The timeout is a handshake deadline only; an authenticated subscription
    // may remain idle until either peer closes it.
    let _ = stream.set_read_timeout(None);
    let cache = Arc::new(Mutex::new(RequestCache::new()));
    let outstanding = Arc::new(AtomicUsize::new(0));
    while let Some(request) = read_request(&mut stream) {
        let lookup = cache.lock().map_or(CacheLookup::Miss, |mut cache| {
            cache.reserve(&request, Instant::now())
        });
        match lookup {
            CacheLookup::Hit(response) => {
                if !write_response(&mut stream, &response) {
                    return;
                }
                continue;
            }
            CacheLookup::IdReuse => {
                if !write_failure(
                    &mut stream,
                    request.id,
                    "E_ID_REUSE",
                    "request ID was reused with a different payload",
                ) {
                    return;
                }
                continue;
            }
            CacheLookup::InFlight => {
                if !write_failure(
                    &mut stream,
                    request.id,
                    "E_IN_FLIGHT",
                    "request with this ID is already being processed",
                ) {
                    return;
                }
                continue;
            }
            CacheLookup::Miss => {}
        }
        if outstanding
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < ferric_browser_ipc::MAX_OUTSTANDING_REQUESTS).then_some(count + 1)
            })
            .is_err()
        {
            let _ = write_failure(
                &mut stream,
                request.id,
                "E_BUSY",
                "connection outstanding-request limit reached",
            );
            continue;
        }
        let Ok(response_stream) = stream.try_clone() else {
            outstanding.fetch_sub(1, Ordering::AcqRel);
            return;
        };
        let pending = PendingRequest::with_cache_and_outstanding(
            request,
            response_stream,
            Arc::clone(&cache),
            Arc::clone(&outstanding),
        );
        if let Err(pending) = enqueue_request(pending) {
            let id = pending.request().id.clone();
            let _ = pending.respond(&Response::failure(
                id,
                ProtocolError {
                    code: "E_BUSY".into(),
                    message: "IPC request queue is full".into(),
                    details: None,
                },
            ));
        }
    }
}

fn write_failure(stream: &mut UnixStream, id: String, code: &str, message: &str) -> bool {
    write_response(
        stream,
        &Response::failure(
            id,
            ProtocolError {
                code: code.into(),
                message: message.into(),
                details: None,
            },
        ),
    )
}

fn write_response(stream: &mut UnixStream, response: &Response) -> bool {
    let Ok(payload) = serialize_response(response) else {
        return false;
    };
    write_frame(stream, &payload).is_ok()
}

fn read_request(stream: &mut UnixStream) -> Option<Request> {
    let payload = read_frame(stream).ok()??;
    parse_request(&payload).ok()
}

fn handshake_response(request: Request, instance_id: &str) -> (bool, Response) {
    let is_hello = request.method == "hello";
    let response = if is_hello {
        match serde_json::from_value::<HelloParams>(request.params) {
            Ok(params) if params.protocol_major == ferric_browser_ipc::PROTOCOL_MAJOR => {
                Response::success(
                    request.id,
                    serde_json::to_value(HelloResult {
                        protocol_major: ferric_browser_ipc::PROTOCOL_MAJOR,
                        protocol_minor: ferric_browser_ipc::PROTOCOL_MINOR,
                        instance_id: instance_id.to_owned(),
                        capabilities: vec![
                            "command.execute".into(),
                            "action.execute".into(),
                            "actions.query".into(),
                            "windows.query".into(),
                            "window.focus".into(),
                            "tabs.query".into(),
                            "profiles.query".into(),
                            "downloads.query".into(),
                            "permissions.query".into(),
                            "contexts.query".into(),
                            "switcher.query".into(),
                            "switcher.activate".into(),
                            "site.status".into(),
                            "blocking.status".into(),
                            "bindings.query".into(),
                            "bindings.explain".into(),
                            "operations.query".into(),
                            "operations.cancel".into(),
                            "config.get".into(),
                            "diagnostics.get".into(),
                            "events.subscribe".into(),
                        ],
                    })
                    .unwrap_or_else(|_| serde_json::json!({})),
                )
            }
            Ok(_) => Response::failure(
                request.id,
                ProtocolError {
                    code: "E_PROTOCOL_VERSION".into(),
                    message: "unsupported IPC protocol version".into(),
                    details: None,
                },
            ),
            Err(error) => Response::failure(
                request.id,
                ProtocolError {
                    code: "E_INVALID_PARAMS".into(),
                    message: error.to_string(),
                    details: None,
                },
            ),
        }
    } else {
        Response::failure(
            request.id,
            ProtocolError {
                code: "E_UNSUPPORTED".into(),
                message: "first IPC request must be hello".into(),
                details: None,
            },
        )
    };
    (is_hello, response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_ipc::{bind_listener, instance_paths, take_pending_request};
    use std::{fs, path::Path};
    use uuid::Uuid;

    #[test]
    fn server_queues_requests_for_the_qt_poller() {
        let root = std::env::temp_dir().join(format!("ferric-ipc-{}", Uuid::new_v4()));
        let paths = instance_paths(&root, Path::new("/tmp/ferric-browser"), "test");
        // AF_UNIX creation is denied by some restricted CI/sandbox runners.
        // Keep this as an explicit capability skip; native Wayland runs still
        // exercise the listener and the IPC unit suite covers framing and
        // request handling without requiring a socket bind.
        let listener = match bind_listener(&paths) {
            Ok(listener) => listener,
            Err(ferric_browser_ipc::InstanceError::Io { message, .. })
                if message.contains("Operation not permitted")
                    || message.contains("Permission denied")
                    || message.contains("Not supported") =>
            {
                eprintln!("skipping Qt IPC socket test: AF_UNIX creation is unavailable");
                let _ = fs::remove_dir_all(root);
                return;
            }
            Err(error) => panic!("listener: {error}"),
        };
        let socket = paths.socket.clone();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            serve_connection(stream, "instance-test");
        });
        let mut client = UnixStream::connect(socket).expect("client");
        let hello = serde_json::to_vec(&Request {
            id: "hello-1".into(),
            method: "hello".into(),
            params: serde_json::json!({
                "protocol_major": ferric_browser_ipc::PROTOCOL_MAJOR,
                "protocol_minor": ferric_browser_ipc::PROTOCOL_MINOR,
                "client": "test"
            }),
        })
        .expect("hello");
        write_frame(&mut client, &hello).expect("write hello");
        let _ = read_frame(&mut client)
            .expect("hello frame")
            .expect("hello response");
        let request = serde_json::to_vec(&Request {
            id: "query-1".into(),
            method: "tabs.query".into(),
            params: serde_json::json!({}),
        })
        .expect("query");
        write_frame(&mut client, &request).expect("write query");
        let deadline = Instant::now() + Duration::from_secs(2);
        let pending = loop {
            if let Some(pending) = take_pending_request() {
                break pending;
            }
            assert!(Instant::now() < deadline, "queued query");
            thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(pending.request().id, "query-1");
        pending
            .respond(&Response::success(
                "query-1",
                serde_json::json!({"tabs": []}),
            ))
            .expect("response");
        let response = read_frame(&mut client)
            .expect("response frame")
            .expect("response");
        let response: Response = serde_json::from_slice(&response).expect("response JSON");
        assert_eq!(response.id, "query-1");
        drop(client);
        server.join().expect("server");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hello_advertises_discoverable_query_and_activation_methods() {
        let request = Request {
            id: "hello-capabilities".into(),
            method: "hello".into(),
            params: serde_json::json!({
                "protocol_major": ferric_browser_ipc::PROTOCOL_MAJOR,
                "protocol_minor": ferric_browser_ipc::PROTOCOL_MINOR,
                "client": "test"
            }),
        };
        let (is_hello, response) = handshake_response(request, "instance-test");
        assert!(is_hello);
        let result = response.result.expect("hello result");
        let capabilities = result["capabilities"].as_array().expect("capability array");
        for method in [
            "window.focus",
            "permissions.query",
            "switcher.activate",
            "bindings.query",
            "bindings.explain",
            "blocking.status",
        ] {
            assert!(
                capabilities
                    .iter()
                    .any(|value| value.as_str() == Some(method)),
                "hello must advertise {method}"
            );
        }
        assert!(
            !capabilities
                .iter()
                .any(|value| value.as_str() == Some("blocklist.update"))
        );
    }
}
