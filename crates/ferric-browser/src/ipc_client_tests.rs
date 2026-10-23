//! Real Unix-socket regressions for the CLI's failure boundary.

#[allow(clippy::wildcard_imports)]
use super::*;
use crate::ipc_client::{
    finish_forward, forward_activate, forward_command, forward_focus, forward_open, forward_query,
    instance_request, ipc_roundtrip, print_diagnostics,
};
use ferric_browser_ipc::{ProtocolError, PublicError};
use std::{os::unix::net::UnixListener, path::PathBuf, thread, time::Instant};

struct Peer {
    base: PathBuf,
    paths: InstancePaths,
    worker: Option<thread::JoinHandle<()>>,
}

impl Peer {
    fn start(
        count: usize,
        mut reply: impl FnMut(Request) -> Option<Response> + Send + 'static,
    ) -> Self {
        let base = std::env::temp_dir().join(format!("fe-{}", Uuid::new_v4().simple()));
        let paths = existing_instance_paths(Some(&base), Some("error-test")).expect("test roots");
        fs::create_dir_all(&paths.directory).expect("socket directory");
        let listener = UnixListener::bind(&paths.socket).expect("test socket");
        listener.set_nonblocking(true).expect("nonblocking accept");
        let worker = thread::spawn(move || {
            for _ in 0..count {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "client did not connect");
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("accept: {error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .expect("read deadline");
                for _ in 0..2 {
                    let request: Request = serde_json::from_slice(
                        &read_frame(&mut stream)
                            .expect("request frame")
                            .expect("request"),
                    )
                    .expect("request JSON");
                    let Some(response) = reply(request) else {
                        break;
                    };
                    let rejected = response.error.is_some();
                    write_frame(
                        &mut stream,
                        &serde_json::to_vec(&response).expect("response JSON"),
                    )
                    .expect("response frame");
                    if rejected {
                        break;
                    }
                }
            }
        });
        Self {
            base,
            paths,
            worker: Some(worker),
        }
    }

    fn finish(mut self) {
        self.worker
            .take()
            .expect("worker")
            .join()
            .expect("peer completed");
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn hello(request: &Request) -> Option<Response> {
    (request.method == "hello").then(|| {
        Response::success(
            &request.id,
            serde_json::json!({
                "protocol_major": PROTOCOL_MAJOR, "protocol_minor": PROTOCOL_MINOR,
                "instance_id": "error-test", "capabilities": []
            }),
        )
    })
}

fn failure(code: ErrorCode) -> ProtocolError {
    Response::failure(
        "template",
        PublicError::new(
            code,
            "Choose an available window.",
            "https://user:secret@example.test/?token=secret",
        )
        .into_protocol_error(),
    )
    .error
    .expect("failure")
}

#[test]
fn cli_remote_failures_survive_every_forwarding_path() {
    for operation in [
        "handshake",
        "focus",
        "open",
        "command",
        "query",
        "activate",
        "diagnostics",
    ] {
        let expected = failure(ErrorCode::InvalidArgument);
        let sent = expected.clone();
        let peer = Peer::start(1, move |request| {
            if operation != "handshake"
                && let Some(response) = hello(&request)
            {
                return Some(response);
            }
            Some(Response {
                id: request.id,
                result: None,
                error: Some(sent.clone()),
            })
        });
        let options = CliOptions::default();
        let error = match operation {
            "handshake" => instance_request(
                &peer.paths,
                &Request {
                    id: "test".into(),
                    method: "tabs.query".into(),
                    params: serde_json::json!({}),
                },
            )
            .map(|_| ()),
            "focus" => forward_focus(&peer.paths),
            "open" => forward_open(
                &peer.paths,
                "https://example.test",
                &[],
                None,
                false,
                &options,
            ),
            "command" => forward_command(&peer.paths, "reload", None, &options),
            "query" => forward_query(&peer.paths, "tabs.query", serde_json::json!({}), "json"),
            "activate" => forward_activate(&peer.paths, "tab", "test-tab"),
            "diagnostics" => {
                print_diagnostics("json", peer.base.to_str(), Some("error-test"), true)
            }
            _ => unreachable!(),
        }
        .expect_err(operation);
        assert_eq!(error.status(), 2, "{operation}");
        let protocol = error.into_protocol();
        assert_eq!(protocol, expected, "{operation}");
        let rendered = cli_error::structured(&protocol);
        assert_eq!(
            rendered["error"]["correlation_id"],
            expected.details.as_ref().unwrap()["correlation_id"]
        );
        assert!(!rendered.to_string().contains("secret"));
        assert!(!cli_error::text(&protocol).contains("correlation"));
        peer.finish();
    }
}

#[test]
fn cli_cleanup_does_not_replace_the_remote_failure() {
    let expected = failure(ErrorCode::Busy);
    // A non-temporary root deterministically refuses cleanup without deleting it.
    let roots = StorageRoots::resolve(RootSpec::Base(PathBuf::from("/unused-error-test"))).unwrap();
    let error = finish_forward(Err(CliError::Protocol(expected.clone())), Some(roots)).unwrap_err();
    assert_eq!(error.into_protocol(), expected);
}

#[test]
fn cli_partial_chains_report_admission_without_claiming_rollback() {
    for startup in [false, true] {
        let expected = failure(ErrorCode::Busy);
        let sent = expected.clone();
        let mut accepted = 0;
        let peer = Peer::start(2, move |request| {
            if let Some(response) = hello(&request) {
                return Some(response);
            }
            accepted += 1;
            Some(if accepted == 1 {
                Response::success(request.id, serde_json::json!({"status":"accepted"}))
            } else {
                Response {
                    id: request.id,
                    result: None,
                    error: Some(sent.clone()),
                }
            })
        });
        let options = CliOptions::default();
        let error = if startup {
            forward_open(
                &peer.paths,
                "https://first.test",
                &["https://second.test".into()],
                None,
                false,
                &options,
            )
        } else {
            forward_command(&peer.paths, "reload ;; reload", None, &options)
        }
        .unwrap_err();
        assert_eq!(error.status(), 6);
        let protocol = error.into_protocol();
        let details = protocol.details.unwrap();
        assert_eq!(details["accepted_requests"], 1);
        assert_eq!(
            details["correlation_id"],
            expected.details.unwrap()["correlation_id"]
        );
        assert!(
            details["preserved"]
                .as_str()
                .unwrap()
                .contains("earlier requests were accepted")
        );
        peer.finish();
    }
}

#[test]
fn cli_rejects_mismatched_or_ambiguous_responses() {
    for ambiguous in [false, true] {
        let peer = Peer::start(1, move |request| {
            if let Some(response) = hello(&request) {
                return Some(response);
            }
            Some(Response {
                id: if ambiguous {
                    request.id
                } else {
                    "wrong-id".into()
                },
                result: Some(serde_json::json!({})),
                error: ambiguous.then(|| failure(ErrorCode::Engine)),
            })
        });
        let error = forward_focus(&peer.paths).unwrap_err();
        assert_eq!(error.code(), ErrorCode::Protocol);
        let protocol = error.into_protocol();
        assert_eq!(
            protocol.details.unwrap()["preserved"],
            "request completion is unconfirmed"
        );
        peer.finish();
    }
}

#[test]
fn cli_live_diagnostic_failure_does_not_become_an_offline_success() {
    let peer = Peer::start(1, |request| hello(&request));
    let error =
        print_diagnostics("json", peer.base.to_str(), Some("error-test"), true).unwrap_err();
    assert_eq!(error.code(), ErrorCode::Io);
    peer.finish();
}

#[test]
fn cli_timeout_uses_io_kind_and_preserves_uncertain_completion() {
    let (mut client, _peer) = UnixStream::pair().expect("socket pair");
    client
        .set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    let error = ipc_roundtrip(
        &mut client,
        &Request {
            id: "timeout".into(),
            method: "command.execute".into(),
            params: serde_json::json!({}),
        },
    )
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::Timeout);
    assert_eq!(error.status(), 6);
    assert_eq!(
        error.into_protocol().details.unwrap()["preserved"],
        "request completion is unconfirmed"
    );
}

#[test]
fn cli_missing_instance_is_actionable_without_exposing_paths() {
    let base = PathBuf::from("/no-such-ferric-instance-error-test");
    let paths = instance_paths(&base, &base, "test");
    let error = instance_request(
        &paths,
        &Request {
            id: "missing".into(),
            method: "tabs.query".into(),
            params: serde_json::json!({}),
        },
    )
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::NoInstance);
    assert_eq!(error.status(), 3);
    assert!(!cli_error::text(&error.into_protocol()).contains("no-such"));
}

#[test]
fn cli_cleanup_failure_does_not_hide_that_a_request_was_sent() {
    let roots = StorageRoots::resolve(RootSpec::Base(PathBuf::from("/unused-error-test"))).unwrap();
    let error = finish_forward(Ok(()), Some(roots)).unwrap_err();
    assert_eq!(error.code(), ErrorCode::Io);
    let protocol = error.into_protocol();
    assert!(protocol.message.contains("request was accepted"));
    assert_eq!(
        protocol.details.unwrap()["preserved"],
        "request completion is unconfirmed"
    );
}

#[test]
fn cli_encodes_the_entire_chain_before_connecting() {
    let base = PathBuf::from("/no-such-ferric-chain-test");
    let paths = instance_paths(&base, &base, "test");
    let error = forward_command(
        &paths,
        "reload ;; settings unexpected",
        None,
        &CliOptions::default(),
    )
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidArgument);
    assert!(
        error
            .into_protocol()
            .message
            .contains("settings does not accept arguments")
    );
}

#[test]
fn cli_command_selectors_survive_cli_annotation() {
    let command = ferric_browser_core::ParsedCommand {
        name: "open".into(),
        arguments: vec![
            "--profile".into(),
            "work".into(),
            "--context".into(),
            "Research".into(),
            "https://example.test".into(),
        ],
    };
    let mut params = command_params(&command).unwrap();
    crate::ipc_client::add_cli_context(&mut params, &CliOptions::default(), None)
        .expect("CLI context");
    assert_eq!(params["context"]["profile"], "work");
    assert_eq!(params["context"]["context"], "Research");
    assert_eq!(params["context"]["source"], "cli");
}

#[test]
fn cli_selector_conflicts_fail_before_any_chain_request_is_sent() {
    let base = PathBuf::from("/no-such-ferric-selector-test");
    let paths = instance_paths(&base, &base, "test");
    let options = CliOptions {
        profile: Some("personal".into()),
        ..CliOptions::default()
    };
    let error = forward_command(
        &paths,
        "reload ;; open --profile work https://example.test",
        None,
        &options,
    )
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidArgument);
    assert!(error.into_protocol().message.contains("profile selectors"));
}

#[test]
fn cli_wire_categories_keep_their_public_exit_statuses() {
    for (code, status) in [
        (ErrorCode::InvalidArgument, 2),
        (ErrorCode::NotFound, 3),
        (ErrorCode::Denied, 4),
        (ErrorCode::Unsupported, 5),
        (ErrorCode::Busy, 6),
        (ErrorCode::Timeout, 6),
        (ErrorCode::Protocol, 7),
        (ErrorCode::Engine, 1),
    ] {
        let sent = failure(code);
        let peer = Peer::start(1, move |request| {
            hello(&request).or_else(|| {
                Some(Response {
                    id: request.id,
                    result: None,
                    error: Some(sent.clone()),
                })
            })
        });
        let error = forward_focus(&peer.paths).unwrap_err();
        assert_eq!(error.code(), code);
        assert_eq!(error.status(), status);
        peer.finish();
    }
}

#[test]
fn cli_legacy_peer_guidance_is_not_printed_twice() {
    let mut error = failure(ErrorCode::Busy);
    let reference = error.details.as_ref().unwrap()["correlation_id"]
        .as_str()
        .unwrap();
    error.message =
        format!("Busy. Preserved: active work. Next action: wait. [correlation {reference}]");
    let output = cli_error::text(&CliError::Protocol(error).into_protocol());
    assert_eq!(output.matches("Preserved:").count(), 1);
    assert!(!output.contains("State:"));
}

#[test]
fn cli_unexpected_failures_keep_an_accessible_diagnostic_reference() {
    let error = failure(ErrorCode::Engine);
    let reference = error.details.as_ref().unwrap()["correlation_id"]
        .as_str()
        .unwrap();
    let output = cli_error::text(&error);
    assert!(output.contains(reference));
    assert!(!output.contains("diagnostic_context"));
    assert!(!output.contains("secret"));
}

#[test]
fn cli_accepts_legacy_error_envelopes_without_metadata() {
    for old_details in [None, Some(serde_json::json!(["safe legacy detail"]))] {
        let sent = ProtocolError {
            code: "E_PERMISSION".into(),
            message: "Access denied.".into(),
            details: old_details.clone(),
        };
        let peer = Peer::start(1, move |request| {
            hello(&request).or_else(|| {
                Some(Response {
                    id: request.id,
                    result: None,
                    error: Some(sent.clone()),
                })
            })
        });
        let error = forward_focus(&peer.paths).unwrap_err();
        assert_eq!(error.status(), 4);
        let protocol = error.into_protocol();
        assert_eq!(protocol.code, "E_DENIED");
        assert_eq!(protocol.message, "Access denied.");
        let details = protocol.details.unwrap();
        assert!(
            details["correlation_id"]
                .as_str()
                .unwrap()
                .starts_with("err-")
        );
        if let Some(old_details) = old_details {
            assert_eq!(details["source"], old_details);
        }
        peer.finish();
    }
}

#[test]
fn cli_accepts_a_successful_null_result() {
    let peer = Peer::start(1, |request| {
        hello(&request).or_else(|| Some(Response::success(request.id, serde_json::Value::Null)))
    });
    let response = instance_request(
        &peer.paths,
        &Request {
            id: "null-result".into(),
            method: "tabs.query".into(),
            params: serde_json::json!({}),
        },
    )
    .expect("null is a valid successful result");
    assert_eq!(response.result, Some(serde_json::Value::Null));
    peer.finish();
}

#[test]
fn multiple_open_inputs_keep_profile_context_and_destination_on_every_request() {
    for target in [None, Some("current"), Some("tab"), Some("tab-bg")] {
        let mut count = 0;
        let peer = Peer::start(2, move |request| {
            if let Some(response) = hello(&request) {
                return Some(response);
            }
            let params = &request.params;
            assert_eq!(params["command"], "open");
            assert_eq!(params["arguments"]["external"], true);
            assert_eq!(params["context"]["profile"], "work");
            assert_eq!(params["context"]["context"], "project");
            assert_eq!(params["context"]["source"], "cli");
            let expected = if count == 0 {
                target
            } else {
                Some(target.unwrap_or("tab"))
            };
            assert_eq!(
                params["arguments"]
                    .get("target")
                    .and_then(serde_json::Value::as_str),
                expected
            );
            count += 1;
            Some(Response::success(
                request.id,
                serde_json::json!({"status":"accepted"}),
            ))
        });
        let options = CliOptions {
            profile: Some("work".into()),
            context: Some("project".into()),
            ..CliOptions::default()
        };
        forward_open(
            &peer.paths,
            "https://example.test/first",
            &["https://example.test/second".into()],
            target,
            false,
            &options,
        )
        .unwrap();
        peer.finish();
    }
}
