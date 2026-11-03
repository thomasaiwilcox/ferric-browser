use super::{
    HyprlandEvent, HyprlandVersion, MAX_CLIENTS, MAX_EVENT_BATCH_BYTES, MAX_EVENT_LINE_BYTES,
    MAX_SNAPSHOT_BYTES, ObservationCache, ObservationError, SequencedEvent, parse_event_batch,
};

fn version() -> HyprlandVersion {
    HyprlandVersion {
        branch: Some("main".to_owned()),
        tag: None,
        commit: Some("fixture".to_owned()),
    }
}

fn one_client(class: &str) -> Vec<u8> {
    format!(
        "[{{\"address\":\"0xabc\",\"class\":\"{class}\",\"initialClass\":\"{class}\",\"pid\":7,\"workspace\":{{\"name\":\"2\"}},\"mapped\":true,\"focusHistoryID\":0,\"grouped\":[]}}]"
    )
    .into_bytes()
}

#[test]
fn event_batch_is_bounded_and_sequenced_in_wire_order() {
    let events = parse_event_batch(
        b"openwindow>>abc,2,workspace,org.example.App\nactivewindowv2>>abc\n",
        8,
    )
    .expect("valid event batch");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].sequence, 9);
    assert!(matches!(&events[0].event, HyprlandEvent::OpenWindow { .. }));
    assert_eq!(events[1].sequence, 10);
    assert!(matches!(
        &events[1].event,
        HyprlandEvent::ActiveWindow { .. }
    ));
    assert_eq!(
        parse_event_batch(b"closewindow>>not-hex!\n", 0),
        Err(ObservationError::InvalidIdentity)
    );
    assert_eq!(
        parse_event_batch(b"activewindowv2>>abc", 0),
        Err(ObservationError::Malformed)
    );
    let oversized_batch = vec![b'x'; MAX_EVENT_BATCH_BYTES + 1];
    assert_eq!(
        parse_event_batch(&oversized_batch, 0),
        Err(ObservationError::TooLarge)
    );
    let oversized_line = vec![b'x'; MAX_EVENT_LINE_BYTES + 1];
    let mut terminated_line = oversized_line;
    terminated_line.push(b'\n');
    assert_eq!(
        parse_event_batch(&terminated_line, 0),
        Err(ObservationError::TooLarge)
    );
}

#[test]
fn snapshot_identity_and_instance_epoch_prevent_stale_address_reuse() {
    let mut cache = ObservationCache::default();
    let first = cache
        .observe("instance-a", version(), &one_client("Ferric"), &[])
        .expect("first snapshot");
    let original_generation = first.clients[0].generation;
    let same = cache
        .observe("instance-a", version(), &one_client("Ferric"), &[])
        .expect("same instance and owner");
    assert_eq!(same.clients[0].generation, original_generation);

    let closed = vec![SequencedEvent {
        sequence: cache.sequence() + 1,
        event: HyprlandEvent::CloseWindow {
            address: "0xabc".to_owned(),
        },
    }];
    let reused = cache
        .observe("instance-a", version(), &one_client("Ferric"), &closed)
        .expect("same address after close");
    assert_ne!(reused.clients[0].generation, original_generation);

    let other_instance = cache
        .observe("instance-b", version(), &one_client("Ferric"), &[])
        .expect("new compositor instance");
    assert!(other_instance.instance_epoch > reused.instance_epoch);
    assert_ne!(
        other_instance.clients[0].generation,
        reused.clients[0].generation
    );

    let before_reconnect = other_instance.instance_epoch;
    cache.invalidate();
    assert!(cache.identities.is_empty());
    let after_reconnect = cache
        .observe("instance-b", version(), &one_client("Ferric"), &[])
        .expect("snapshot after event socket reconnect");
    assert!(after_reconnect.instance_epoch > before_reconnect);
    assert_ne!(
        after_reconnect.clients[0].generation,
        other_instance.clients[0].generation
    );
}

#[test]
fn malformed_snapshot_clears_previous_observation() {
    let mut cache = ObservationCache::default();
    let initial = cache
        .observe("instance-a", version(), &one_client("Ferric"), &[])
        .expect("initial snapshot");
    assert_eq!(initial.clients.len(), 1);
    assert_eq!(
        cache.observe("instance-a", version(), b"not-json", &[]),
        Err(ObservationError::Malformed)
    );
    assert!(cache.identities.is_empty());
    assert_eq!(cache.active_address, None);
}

#[test]
fn grouped_clients_are_canonical_and_snapshot_order_is_stable() {
    let mut cache = ObservationCache::default();
    let snapshot = br#"[
        {"address":"0xdef","class":"B","initialClass":"B","pid":8,"workspace":{"name":"2"},"mapped":true,"focusHistoryID":2,"grouped":["0xabc"]},
        {"address":"0xabc","class":"A","initialClass":"A","pid":7,"workspace":{"name":"1"},"mapped":true,"focusHistoryID":1,"grouped":["0xdef"]}
    ]"#;
    let state = cache
        .observe("instance-a", version(), snapshot, &[])
        .expect("valid grouped snapshot");
    assert_eq!(state.clients[0].address, "0xabc");
    assert_eq!(state.clients[1].address, "0xdef");
    assert_eq!(state.groups.len(), 1);
    assert_eq!(state.groups[0].members, vec!["0xabc", "0xdef"]);
}

#[test]
fn snapshot_size_and_client_count_are_bounded() {
    let mut cache = ObservationCache::default();
    let oversized_snapshot = vec![b' '; MAX_SNAPSHOT_BYTES + 1];
    assert_eq!(
        cache.observe("instance-a", version(), &oversized_snapshot, &[]),
        Err(ObservationError::TooLarge)
    );

    let one = String::from_utf8(one_client("Ferric")).expect("fixture is UTF-8");
    let row = one.trim_start_matches('[').trim_end_matches(']');
    let too_many = format!(
        "[{}]",
        std::iter::repeat_n(row, MAX_CLIENTS + 1)
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(
        cache.observe("instance-a", version(), too_many.as_bytes(), &[]),
        Err(ObservationError::TooLarge)
    );
    assert!(cache.identities.is_empty());
}

#[test]
fn socket_disconnect_and_reconnect_clear_stale_window_identity() {
    use std::io::Write;
    use std::os::unix::net::UnixListener;

    static NEXT_SOCKET: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let socket_path = std::env::temp_dir().join(format!(
        "ferric-w04-{}-{}.sock",
        std::process::id(),
        NEXT_SOCKET.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let listener = UnixListener::bind(&socket_path).expect("bind owned test socket");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept first observer");
        stream
            .write_all(b"activewindowv2>>abc\n")
            .expect("send complete event");
    });
    let mut session = super::ObservationSession::default();
    let first = session
        .cache
        .observe("instance-a", version(), &one_client("Ferric"), &[])
        .expect("initial cache");
    let pending_handle = super::ObservationCache::handle_for(&first, "0xabc")
        .expect("bind read-only pending observation handle");
    assert!(session.cache.is_current(&pending_handle));
    assert!(session.read_events(Some(&socket_path)).is_err());
    server.join().expect("server exits after closing socket");
    assert!(session.cache.identities.is_empty());
    assert!(session.cache.active_address.is_none());
    std::fs::remove_file(&socket_path).expect("remove owned socket path");

    let listener = UnixListener::bind(&socket_path).expect("rebind owned test socket");
    let server = std::thread::spawn(move || {
        let (_stream, _) = listener.accept().expect("accept reconnect");
        std::thread::sleep(std::time::Duration::from_millis(100));
    });
    assert_eq!(
        session
            .read_events(Some(&socket_path))
            .expect("reconnected stream"),
        Vec::new()
    );
    let after = session
        .cache
        .observe("instance-a", version(), &one_client("Ferric"), &[])
        .expect("snapshot after reconnect");
    assert!(after.instance_epoch > first.instance_epoch);
    assert_ne!(after.clients[0].generation, first.clients[0].generation);
    assert!(!session.cache.is_current(&pending_handle));
    server.join().expect("reconnect server exits");
    drop(session);
    std::fs::remove_file(&socket_path).expect("remove reconnected socket path");
}

#[test]
fn malformed_event_batch_invalidates_cached_state() {
    use std::io::Write;
    use std::os::unix::net::UnixListener;

    let socket_path =
        std::env::temp_dir().join(format!("ferric-w04-malformed-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path).expect("bind owned test socket");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept observer");
        stream
            .write_all(b"closewindow>>not-hex!\n")
            .expect("send malformed event");
        std::thread::sleep(std::time::Duration::from_millis(100));
    });
    let mut session = super::ObservationSession::default();
    session
        .cache
        .observe("instance-a", version(), &one_client("Ferric"), &[])
        .expect("initial cache");
    assert_eq!(
        session.read_events(Some(&socket_path)),
        Err(ObservationError::InvalidIdentity)
    );
    assert!(session.cache.identities.is_empty());
    assert!(session.cache.active_address.is_none());
    server.join().expect("malformed event server exits");
    drop(session);
    std::fs::remove_file(&socket_path).expect("remove owned socket path");
}

#[test]
fn observation_permission_split_is_enforced() {
    use super::ObservationCapability;

    let disabled =
        crate::hyprland::HyprlandAdapter::observation_test_adapter(false, false, None, None, None);
    let mut session = super::ObservationSession::default();
    let result = disabled.observe_events(&mut session);
    assert_eq!(result.capability, ObservationCapability::Disabled);
    assert!(result.state.is_none());

    let observe_only =
        crate::hyprland::HyprlandAdapter::observation_test_adapter(true, false, None, None, None);
    let result = observe_only.observe_events(&mut session);
    assert_eq!(result.capability, ObservationCapability::Unavailable);
    assert!(result.state.is_none());
}

#[test]
fn enabled_off_has_zero_socket_effects_even_with_live_endpoints() {
    use std::os::unix::net::UnixListener;

    let request_path = owned_socket_path();
    let event_path = owned_socket_path();
    let request_listener = UnixListener::bind(&request_path).expect("bind request fixture");
    let event_listener = UnixListener::bind(&event_path).expect("bind event fixture");
    request_listener
        .set_nonblocking(true)
        .expect("nonblocking request fixture");
    event_listener
        .set_nonblocking(true)
        .expect("nonblocking event fixture");

    let adapter = crate::hyprland::HyprlandAdapter::observation_test_adapter(
        false,
        false,
        Some(request_path.clone()),
        Some(event_path.clone()),
        None,
    );
    let mut session = super::ObservationSession::default();
    let result = adapter.observe_events(&mut session);
    assert_eq!(result.capability, super::ObservationCapability::Disabled);
    assert!(result.state.is_none());
    assert_eq!(
        adapter.route_workspace("2"),
        Err(crate::hyprland::HyprlandError::Disabled)
    );
    assert_eq!(
        adapter.move_active_window_to_workspace("2"),
        Err(crate::hyprland::HyprlandError::Disabled)
    );
    assert_eq!(
        request_listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "disabled observation must not connect to the request socket"
    );
    assert_eq!(
        event_listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "disabled observation must not connect to the event socket"
    );
    drop(request_listener);
    drop(event_listener);
    std::fs::remove_file(request_path).expect("remove owned request socket");
    std::fs::remove_file(event_path).expect("remove owned event socket");
}

#[test]
fn workspace_permission_off_keeps_observation_read_only() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::mpsc;

    let request_path = owned_socket_path();
    let event_path = owned_socket_path();
    let request_listener = UnixListener::bind(&request_path).expect("bind request fixture");
    let event_listener = UnixListener::bind(&event_path).expect("bind event fixture");
    let request_server_listener = request_listener
        .try_clone()
        .expect("clone request socket for server");
    let event_server_listener = event_listener
        .try_clone()
        .expect("clone event socket for server");
    let (command_tx, command_rx) = mpsc::channel();
    let request_server = std::thread::spawn(move || {
        for (expected, response) in [
            (
                "j/version",
                br#"{"branch":"main","tag":null,"commit":"test"}"#.as_slice(),
            ),
            ("j/clients", b"[]".as_slice()),
        ] {
            let (mut stream, _) = request_server_listener
                .accept()
                .expect("accept read-only request");
            let mut command = [0_u8; 64];
            let count = stream.read(&mut command).expect("read request command");
            let command = String::from_utf8_lossy(&command[..count]).into_owned();
            command_tx
                .send(command.clone())
                .expect("record request command");
            assert_eq!(command, expected);
            stream
                .write_all(response)
                .expect("write read-only response");
        }
    });
    let event_server = std::thread::spawn(move || {
        let (_stream, _) = event_server_listener
            .accept()
            .expect("accept event subscription");
        std::thread::sleep(std::time::Duration::from_millis(120));
    });

    let adapter = crate::hyprland::HyprlandAdapter::observation_test_adapter(
        true,
        false,
        Some(request_path.clone()),
        Some(event_path.clone()),
        Some("w04-test-instance"),
    );
    let mut session = super::ObservationSession::default();
    let result = adapter.observe_events(&mut session);
    assert_eq!(result.capability, super::ObservationCapability::Available);
    assert!(result.state.is_some());
    assert_eq!(
        adapter.route_workspace("2"),
        Err(crate::hyprland::HyprlandError::Disabled)
    );
    assert_eq!(
        adapter.move_active_window_to_workspace("2"),
        Err(crate::hyprland::HyprlandError::Disabled)
    );
    request_server
        .join()
        .expect("read-only request server exits");
    event_server.join().expect("event server exits");
    request_listener
        .set_nonblocking(true)
        .expect("nonblocking request side-effect probe");
    event_listener
        .set_nonblocking(true)
        .expect("nonblocking event side-effect probe");
    let commands = command_rx.try_iter().collect::<Vec<_>>();
    assert_eq!(commands, vec!["j/version", "j/clients"]);
    assert!(commands.iter().all(|command| {
        !command.contains("dispatch")
            && !command.contains("movewindow")
            && !command.contains("workspace")
    }));
    assert_eq!(
        request_listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "disabled route/move requests must not connect to the request socket"
    );
    assert_eq!(
        event_listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "disabled route/move requests must not connect to the event socket"
    );
    drop(session);
    std::fs::remove_file(request_path).expect("remove owned request socket");
    std::fs::remove_file(event_path).expect("remove owned event socket");
}

fn owned_socket_path() -> std::path::PathBuf {
    static NEXT_SOCKET: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    std::env::temp_dir().join(format!(
        "w4-{}-{}.sock",
        std::process::id(),
        NEXT_SOCKET.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

#[test]
fn duplicate_or_decreasing_event_sequence_clears_cached_authorization_state() {
    for (first_sequence, retry_sequence) in [(1, 1), (2, 1)] {
        let mut cache = ObservationCache::default();
        let initial = cache
            .observe("instance-a", version(), &one_client("Ferric"), &[])
            .expect("initial snapshot");
        let handle = ObservationCache::handle_for(&initial, "0xabc")
            .expect("bind read-only observation handle");
        let accepted = [SequencedEvent {
            sequence: first_sequence,
            event: HyprlandEvent::Ignored,
        }];
        cache
            .observe("instance-a", version(), &one_client("Ferric"), &accepted)
            .expect("first ordered event");
        let repeated_or_decreasing = [SequencedEvent {
            sequence: retry_sequence,
            event: HyprlandEvent::Ignored,
        }];
        assert_eq!(
            cache.observe(
                "instance-a",
                version(),
                &one_client("Ferric"),
                &repeated_or_decreasing
            ),
            Err(ObservationError::OutOfOrder)
        );
        assert!(cache.identities.is_empty());
        assert!(cache.active_address.is_none());
        assert!(!cache.is_current(&handle));
    }
}
