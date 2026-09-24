use super::*;
use crate::qobject::BrowserUi;
use ferric_browser_ipc::ErrorCode;
use input_validation::MAX_JSEVAL_SCRIPT_BYTES;
use process_output::redact_process_stderr_token;
use std::thread;

fn bootstrap_application(
    privacy: PrivacyKind,
    profile_label: &str,
) -> Option<(BrowserApplication, WindowId, TabId)> {
    BrowserApplication::bootstrap(Config::default(), privacy, profile_label).ok()
}

#[test]
fn interactive_open_options_preserve_typed_url_and_route() {
    let command = ParsedCommand {
        name: "open".into(),
        arguments: vec![
            "--target".into(),
            "window".into(),
            "--profile".into(),
            "work".into(),
            "--".into(),
            "https://example.test/a".into(),
        ],
    };
    let (parsed, route) = interactive_open_command(&command)
        .expect("interactive open options")
        .expect("open options should use typed routing");
    assert_eq!(parsed.arguments, ["https://example.test/a"]);
    assert_eq!(route.open_target, IpcOpenTarget::Window);
    assert_eq!(route.profile.as_deref(), Some("work"));
}

#[test]
fn modal_navigation_bindings_open_an_editable_command_line() {
    let command = |name: &str, arguments: &[&str]| ParsedCommand {
        name: name.into(),
        arguments: arguments.iter().map(|value| (*value).into()).collect(),
    };
    assert_eq!(
        modal_command_prefill(&command("open", &[]), "https://example.test/page"),
        Some("open ".into())
    );
    assert_eq!(
        modal_command_prefill(&command("open-current", &[]), "https://example.test/page"),
        Some("open https://example.test/page".into())
    );
    assert_eq!(
        modal_command_prefill(&command("tab-open", &[]), "https://example.test/page"),
        Some("tab-open ".into())
    );
    assert_eq!(
        modal_command_prefill(
            &command("open-current", &["--target", "tab"]),
            "https://example.test/page"
        ),
        Some("tab-open https://example.test/page".into())
    );
    assert_eq!(
        modal_command_prefill(&command("tab-clone", &[]), "https://example.test/page"),
        None
    );
    assert_eq!(
        modal_command_prefill(&command("quickmark-add", &[]), "about:blank"),
        Some("quickmark-add ".into())
    );
    assert_eq!(
        modal_command_prefill(&command("quickmark-open", &[]), "about:blank"),
        Some("quickmark-open ".into())
    );
    assert_eq!(
        modal_command_prefill(&command("bookmark-open", &[]), "about:blank"),
        Some("bookmark-open ".into())
    );
    assert_eq!(
        modal_command_prefill(&command("open", &["example.test"]), "about:blank"),
        None
    );
    assert_eq!(
        modal_command_prefill(&command("reload", &[]), "about:blank"),
        None
    );

    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("action.indexOf(\"command-prefill\\t\") === 0"));
    assert!(qml.contains("commandSurface.cursorPosition = commandSurface.commandText.length"));
}

#[test]
fn local_empty_binding_commands_use_the_full_executor() {
    let command = |name: &str, arguments: &[&str]| ParsedCommand {
        name: name.into(),
        arguments: arguments.iter().map(|value| (*value).into()).collect(),
    };
    for name in [
        "binding-list",
        "bookmark-add",
        "bookmark-list",
        "devtools",
        "fullscreen",
        "history",
        "print",
        "tab-clone",
        "tab-move",
        "tab-mute",
        "tab-pin",
        "tab-undo",
        "view-source",
        "window-close",
        "window-new",
    ] {
        assert!(
            binding_uses_full_command_executor(&command(name, &[])),
            "{name} must not be sent to the core-only dispatcher"
        );
    }
    assert!(binding_uses_full_command_executor(&command(
        "tab-open",
        &["about:blank"]
    )));
    assert!(!binding_uses_full_command_executor(&command("back", &[])));
    assert!(!binding_uses_full_command_executor(&command(
        "tab-next",
        &[]
    )));

    for (name, arguments, expected) in [
        ("tab-pin", vec![], vec!["tab-1", "toggle"]),
        ("tab-mute", vec!["off"], vec!["tab-1", "off"]),
        ("tab-move", vec!["right"], vec!["tab-1", "right"]),
    ] {
        let mut parsed = command(name, &arguments);
        normalize_active_tab_command(&mut parsed, Some("tab-1".into()))
            .expect("active-tab shorthand should normalize");
        assert_eq!(parsed.arguments, expected);
    }
    let mut explicit = command("tab-pin", &["tab-2", "on"]);
    normalize_active_tab_command(&mut explicit, Some("tab-1".into()))
        .expect("explicit target should remain valid");
    assert_eq!(explicit.arguments, vec!["tab-2", "on"]);
}

#[test]
fn interactive_clean_link_rejects_background_target() {
    let command = ParsedCommand {
        name: "open".into(),
        arguments: vec![
            "--target".into(),
            "tab-bg".into(),
            "--clean-link".into(),
            "https://example.test/?utm_source=demo".into(),
        ],
    };
    let error = interactive_open_command(&command)
        .expect_err("clean-link background open must be rejected");
    assert!(
        error
            .to_string()
            .contains("cannot be combined with --target tab-bg")
    );
}

#[test]
fn macro_register_gestures_are_native_and_timeout_bounded() {
    let source = include_str!("lib.rs");
    assert!(source.contains("matches!(key, \"q\" | \"@\")"));
    assert!(source.contains("Macro recording: waiting for register"));
    assert!(source.contains("Macro replay: waiting for register"));
    assert!(source.contains("Macro register prefix timed out"));
    assert!(source.contains("this.macro_key_prefix = None"));
    assert!(source.contains("this.macro_key_started_ms = None"));
}

#[test]
fn switcher_command_parsing_is_scoped_and_bounded() {
    let args = vec![
        "--scope".into(),
        "history".into(),
        "ferric".into(),
        "browser".into(),
    ];
    assert_eq!(
        parse_switcher_command(&args).expect("valid switcher command"),
        ("history".into(), "ferric browser".into())
    );
    assert!(parse_switcher_command(&["--scope".into(), "unknown".into()]).is_err());
    assert!(
        parse_switcher_command(&[
            "--scope".into(),
            "tabs".into(),
            "--scope".into(),
            "history".into()
        ])
        .is_err()
    );
    let oversized = vec!["x".repeat(4_097)];
    assert!(parse_switcher_command(&oversized).is_err());
}

#[test]
fn switcher_input_coalesces_refreshes_and_cancels_on_close() {
    let qml = [
        include_str!("../qml/Main.qml"),
        include_str!("../qml/components/FerricSwitcher.qml"),
    ]
    .concat();
    assert!(qml.contains("id: switcherRefreshTimer"));
    assert!(qml.contains("interval: 35"));
    assert!(qml.contains("switcherRefreshTimer.restart()"));
    assert!(qml.contains("switcherRefreshTimer.stop()"));
    assert!(qml.contains("id: switcherBatchTimer"));
    assert!(qml.contains("switcherBatchTimer.start()"));
    assert!(qml.contains("switcherBatchGeneration += 1"));
    assert!(qml.contains("switcherBatchQuery !== switcherSurface.query"));
    assert!(qml.contains("function switcherMaxResults()"));
    assert!(qml.contains("return browserUi.feature_switcher_max_results"));
    assert!(qml.contains("Search up to \" + browserWindow.switcherMaxResults()"));
    assert!(qml.contains("Number(right.rank || 0)"));
    assert!(qml.contains("String(left.id || \"\").localeCompare"));
    assert!(qml.contains("if (window.switcherVisible)"));
}

#[test]
fn switcher_context_boost_is_bounded_and_neutral_without_a_context() {
    assert_eq!(switcher_context_boost(None, Some("research")), 0);
    assert_eq!(switcher_context_boost(Some("research"), None), 0);
    assert_eq!(
        switcher_context_boost(Some("research"), Some("personal")),
        0
    );
    assert_eq!(
        switcher_context_boost(Some("research"), Some("research")),
        25
    );
}

#[test]
fn switcher_result_limit_uses_the_validated_setting_and_safe_fallback() {
    assert_eq!(
        switcher_max_results(&serde_json::json!({"switcher": {"max_results": 250}})),
        250
    );
    assert_eq!(
        switcher_max_results(&serde_json::json!({"switcher": {"max_results": 1}})),
        10
    );
    assert_eq!(
        switcher_max_results(&serde_json::json!({"switcher": {"max_results": 5000}})),
        1000
    );
    assert_eq!(switcher_max_results(&serde_json::json!({})), 100);
}

#[test]
fn switcher_page_selection_preserves_order_and_total() {
    let candidate = |rank: i64, kind: &str, recency: i64, id: &str| SwitcherCandidate {
        rank,
        kind: kind.to_owned(),
        recency,
        value: serde_json::json!({"id": id}),
    };
    let (total, page) = select_switcher_page(
        vec![
            candidate(80, "tab", 0, "d"),
            candidate(90, "history", 5, "c"),
            candidate(100, "tab", 0, "a"),
            candidate(90, "history", 10, "b"),
            candidate(70, "tab", 0, "e"),
        ],
        1,
        2,
    );
    assert_eq!(total, 5);
    let ids = page
        .iter()
        .filter_map(|value| value.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["b", "c"]);
}

#[test]
fn closed_tab_undo_limit_respects_configured_zero_and_spec_cap() {
    assert_eq!(
        configured_undo_limit(&serde_json::json!({"tabs": {"undo_limit": 0}})),
        0
    );
    assert_eq!(
        configured_undo_limit(&serde_json::json!({"tabs": {"undo_limit": 75}})),
        75
    );
    assert_eq!(
        configured_undo_limit(&serde_json::json!({"tabs": {"undo_limit": 1000}})),
        100
    );
    assert_eq!(configured_undo_limit(&serde_json::json!({})), 100);
}

#[test]
fn switcher_cache_requires_the_same_revisions_and_snapshot_inputs() {
    let rust = BrowserUiRust::default();
    let params = serde_json::json!({"query": "tabs", "scope": "tabs"});
    let cache = SwitcherQueryCache {
        params: params.clone(),
        state_revision: rust.state.as_ref().map_or(0, BrowserApplication::revision),
        storage_library_revision: rust.storage_library_revision,
        session_names: rust.session_names.clone(),
        contexts_json: rust.contexts_json.to_string(),
        config_fingerprint: serde_json::to_string(&rust.config).expect("config snapshot"),
        profile_name: rust.profile_name.clone(),
        result: serde_json::json!({"results": []}),
    };
    assert!(cache.matches(&rust, &params));

    let mut changed = rust;
    let tab = changed
        .state
        .as_ref()
        .and_then(|state| state.active_tab())
        .map(|tab| tab.id)
        .expect("active tab");
    changed
        .state
        .as_mut()
        .expect("bootstrap runtime")
        .dispatch_runtime(RuntimeInput::EngineFact(Event::SetTabMuted {
            tab,
            muted: true,
        }))
        .expect("revision-changing event");
    assert!(!cache.matches(&changed, &params));
}

#[test]
fn hint_payload_carries_unique_bounded_element_ids() {
    let payload = serde_json::json!({
        "candidates": [{
            "element_id": 1,
            "kind": "link",
            "frame_path": "0",
            "text": "Example",
            "href": "https://example.test/",
            "geometry": {"x": 1.0, "y": 2.0, "width": 10.0, "height": 10.0}
        }]
    });
    let candidates = parse_hint_payload(&payload.to_string()).expect("hint payload");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].element_id, 1);
    assert_eq!(candidates[0].frame_path, "0");
    assert!(hint_payload::valid_hint_frame_path("0.2.3"));
    assert!(!hint_payload::valid_hint_frame_path("1.0"));
    assert!(!hint_payload::valid_hint_frame_path("0.1.2.3.4.5.6.7.8.9"));

    let source = include_str!("lib.rs");
    let qml = include_str!("../qml/Main.qml");
    let script = include_str!("../qml/scripts/BrowserScripts.js");
    assert!(script.contains("__ferric_browserHintElements"));
    assert!(script.contains("element_id"));
    assert!(script.contains("frame.contentDocument"));
    assert!(qml.contains("function hintFocusScript"));
    assert!(script.contains("record.element.isConnected"));
    assert!(script.contains("record.framePath!==path"));
    assert!(source.contains("candidate.element_id == fresh.element_id"));

    let duplicate = serde_json::json!({
        "candidates": [payload["candidates"][0].clone(), payload["candidates"][0].clone()]
    });
    assert!(parse_hint_payload(&duplicate.to_string()).is_err());
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn macro_status_is_redacted_and_rust_formats_its_presentation_text() {
    let mut ui = BrowserUiRust::default();
    ui.recording_macro = Some((
        "a".into(),
        vec![ParsedCommand {
            name: "open".into(),
            arguments: vec!["https://private.example/path".into()],
        }],
    ));
    ui.macro_registers.insert(
        "b".into(),
        vec![ParsedCommand {
            name: "scroll".into(),
            arguments: vec!["--amount".into(), "3".into()],
        }],
    );

    let state = macro_state_value(&ui);
    assert_eq!(state["recording"]["register"], "a");
    assert_eq!(state["recording"]["command_count"], 1);
    assert_eq!(state["registers"][0]["register"], "b");
    assert_eq!(state["registers"][0]["command_count"], 1);
    assert_eq!(state["limits"]["recording_commands"], 1_000);
    assert_eq!(state["limits"]["expanded_commands"], 1_000);
    assert_eq!(state["limits"]["nested_depth"], 8);
    assert_eq!(state["persistence"], "memory-only");
    assert!(!state.to_string().contains("private.example"));

    assert_eq!(macro_status_text(&ui), " · recording macro @a (1)");
    ui.recording_macro = None;
    assert_eq!(macro_status_text(&ui), " · macros @b:1");

    let qml = include_str!("../qml/Main.qml");
    assert!(!qml.contains("function macroStatusText(raw)"));
    assert!(qml.contains("browserUi.macro_status_text"));
}

#[test]
fn shutdown_gate_only_blocks_mutating_ipc_methods() {
    assert!(is_mutating_ipc_method("command.execute"));
    assert!(is_mutating_ipc_method("action.execute"));
    assert!(is_mutating_ipc_method("switcher.activate"));
    assert!(is_mutating_ipc_method("window.focus"));
    assert!(!is_mutating_ipc_method("tabs.query"));
    assert!(!is_mutating_ipc_method("diagnostics.get"));
    assert!(!is_mutating_ipc_method("events.subscribe"));
}

#[test]
fn current_session_paths_prefer_window_snapshots_and_retain_legacy_fallback() {
    let root = std::env::temp_dir().join(format!(
        "ferric-browser-current-sessions-{}",
        Uuid::new_v4()
    ));
    let profile = Uuid::new_v4();
    let directory = root.join("sessions").join(profile.to_string());
    fs::create_dir_all(&directory).expect("create session directory");
    fs::write(directory.join("current.json"), b"legacy").expect("legacy fixture");
    fs::write(
        directory.join(format!("current-{}.json", Uuid::new_v4())),
        b"window",
    )
    .expect("window fixture");
    fs::write(directory.join("current-not-a-uuid.json"), b"ignored").expect("invalid fixture");
    let paths = ferric_browser_storage::current_session_paths(&root, profile);
    assert_eq!(paths.len(), 1);
    assert!(
        paths[0]
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("current-") && name.ends_with(".json"))
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn redirect_observation_is_distinct_from_fragment_changes_and_traversal() {
    assert_eq!(
        strip_url_fragment("https://example.test/page#one"),
        "https://example.test/page"
    );
    assert_eq!(
        strip_url_fragment("https://example.test/page"),
        "https://example.test/page"
    );
    assert_eq!(
        journey_transition_after_load(None, false, Some(1)),
        Some((JourneyEdgeKind::Redirect, "redirect".into()))
    );
    assert_eq!(
        journey_transition_after_load(None, false, Some(3)),
        Some((JourneyEdgeKind::Redirect, "redirect-chain:3".into()))
    );
    assert_eq!(journey_transition_after_load(None, true, Some(3)), None);
    assert_eq!(
        journey_transition_after_load(Some((JourneyEdgeKind::Hint, "hint".into())), false, Some(2),),
        Some((JourneyEdgeKind::Hint, "hint;redirect-chain:2".into()))
    );
}

#[test]
fn same_document_history_avoids_pending_load_duplicates() {
    assert!(recordable_same_document_change(
        false,
        Some("https://example.test/page"),
        "https://example.test/page#section"
    ));
    assert!(!recordable_same_document_change(
        true,
        Some("https://example.test/page"),
        "https://example.test/page#section"
    ));
    assert!(!recordable_same_document_change(
        false,
        Some("https://example.test/page#section"),
        "https://example.test/page#section"
    ));
    assert!(!recordable_same_document_change(
        false,
        None,
        "https://example.test/page#section"
    ));
}

#[test]
fn editor_process_termination_reaps_the_child() {
    let mut command = Command::new("/bin/sleep");
    command.arg("30").process_group(0);
    let child = command.spawn().expect("start editor fixture");
    let process = Arc::new(Mutex::new(Some(child)));
    terminate_editor_process(&process);
    assert!(process.lock().expect("editor process lock").is_none());
}

#[test]
fn private_editor_artifact_cleanup_removes_only_its_managed_directory() {
    let root = std::env::temp_dir().join(format!(
        "ferric-browser-private-editor-cleanup-{}",
        Uuid::new_v4()
    ));
    let directory = root.join("scratch");
    let path = directory.join("editor.txt");
    fs::create_dir_all(&directory).expect("create private editor directory");
    fs::write(&path, b"private text").expect("write private editor fixture");
    cleanup_editor_artifact(&path, true);
    assert!(!directory.exists());
    assert!(
        root.exists(),
        "cleanup must not remove an enclosing directory"
    );
    fs::remove_dir(root).expect("remove test enclosing directory");
}

#[test]
fn process_group_termination_stops_a_child_descendant() {
    let pid_path = std::env::temp_dir().join(format!(
        "ferric-browser-process-group-{}.pid",
        Uuid::new_v4()
    ));
    let script = format!("sleep 30 & printf '%s' $! > {}; exit 0", pid_path.display());
    let mut command = Command::new("/bin/sh");
    command.args(["-c", &script]).process_group(0);
    let mut child = command.spawn().expect("start process-group fixture");
    let descendant_pid = (0..100).find_map(|_| {
        thread::sleep(Duration::from_millis(10));
        let value = fs::read_to_string(&pid_path).ok()?;
        let pid = value.trim().parse::<libc::pid_t>().ok()?;
        Some(pid)
    });
    assert!(
        descendant_pid.is_some(),
        "fixture did not start a descendant"
    );

    terminate_child_process(&mut child);
    let descendant_pid = descendant_pid.expect("descendant pid checked");
    let descendant_gone = (0..50).any(|_| {
        let gone = unsafe { libc::kill(descendant_pid, 0) == -1 };
        if !gone {
            thread::sleep(Duration::from_millis(10));
        }
        gone
    });
    let _ = fs::remove_file(pid_path);
    assert!(
        descendant_gone,
        "process-group descendant survived cancellation"
    );
}

#[test]
fn clipboard_navigation_input_is_bounded_and_rejects_failures() {
    assert_eq!(
        validate_clipboard_navigation_input("  https://example.test/path  ").unwrap(),
        "https://example.test/path"
    );
    for invalid in [
        "",
        "   ",
        "https://example.test/path\nmore",
        "https://example.test\u{0000}more",
    ] {
        assert!(validate_clipboard_navigation_input(invalid).is_err());
    }
    assert!(validate_clipboard_navigation_input(&"x".repeat(64 * 1024 + 1)).is_err());
    assert!(validate_clipboard_navigation_input(&"x".repeat(64 * 1024)).is_ok());
}

#[test]
fn ipc_profile_create_preserves_ephemeral_flag_as_typed_data() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "profile-create",
        "arguments": {"name": "task", "ephemeral": true}
    }))
    .expect("typed ephemeral profile command");
    assert_eq!(command.arguments, ["task", "--ephemeral"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "profile-create",
            "arguments": {"name": "task", "ephemeral": "yes"}
        }))
        .is_err()
    );
}

#[test]
fn ipc_window_move_requires_bounded_id_and_workspace_arguments() {
    let (command, route, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.window.move",
        "arguments": {"id": "window-42", "workspace": "name:research"}
    }))
    .expect("typed window move action");
    assert_eq!(action_id, "browser.window.move");
    assert_eq!(command.name, "window-move");
    assert_eq!(command.arguments, vec!["window-42", "name:research"]);
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
    assert!(
        typed_ipc_action(&serde_json::json!({
            "action": "browser.window.move",
            "arguments": {"id": "window-42", "workspace": "name:research", "extra": "reject"}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_action(&serde_json::json!({
            "action": "browser.window.move",
            "arguments": {"id": "window-42", "workspace": "bad\nworkspace"}
        }))
        .is_err()
    );
}

#[test]
fn typed_actions_enforce_their_registry_argument_schema() {
    assert!(
        typed_ipc_action(&serde_json::json!({
            "action": "browser.url.copy",
            "arguments": {"source": "title"}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_action(&serde_json::json!({
            "action": "browser.link.open",
            "arguments": {"url": "https://example.test", "profile": "other"}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_action(&serde_json::json!({
            "action": "browser.window.move",
            "arguments": {"id": "window-42"}
        }))
        .is_err()
    );
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.window.move",
        "arguments": {"id": "window-42", "workspace": "name:research"}
    }))
    .expect("declared action arguments remain accepted");
    assert_eq!(action_id, "browser.window.move");
    assert_eq!(command.arguments, vec!["window-42", "name:research"]);
}

#[test]
fn typed_actions_enforce_declared_argument_types() {
    for arguments in [
        serde_json::json!({"private": "true"}),
        serde_json::json!({"private": 1}),
    ] {
        assert!(
            typed_ipc_action(&serde_json::json!({
                "action": "browser.window.new",
                "arguments": arguments
            }))
            .is_err()
        );
    }
    assert!(
        typed_ipc_action(&serde_json::json!({
            "action": "browser.tab.scroll",
            "arguments": {"count": "2"}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_action(&serde_json::json!({
            "action": "browser.command.execute",
            "arguments": {"arguments": []}
        }))
        .is_err()
    );

    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.window.new",
        "arguments": {"private": true}
    }))
    .expect("well-typed boolean action argument");
    assert_eq!(action_id, "browser.window.new");
    assert_eq!(command.arguments, vec!["--private"]);
}

#[test]
fn typed_actions_validate_url_arguments_at_the_action_boundary() {
    for url in ["javascript:alert(1)", "not a URL", "https://"] {
        assert!(
            typed_ipc_action(&serde_json::json!({
                "action": "browser.quickmark.add",
                "arguments": {"name": "unsafe", "url": url}
            }))
            .is_err()
        );
    }
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.quickmark.add",
        "arguments": {"name": "docs", "url": "https://docs.example.test/"}
    }))
    .expect("safe URL action argument");
    assert_eq!(action_id, "browser.quickmark.add");
    assert_eq!(
        command.arguments,
        vec!["docs", "https://docs.example.test/"]
    );
}

#[test]
fn ipc_commands_use_typed_arguments_without_shell_parsing() {
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "open",
        "arguments": {"input": "https://example.test/a?x=$(touch nope)"}
    }))
    .expect("typed command");
    assert_eq!(command.name, "open");
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
    assert!(!route.external_open);
    assert_eq!(
        command.arguments[0],
        "https://example.test/a?x=$(touch nope)"
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "open",
            "arguments": {"input": "https://example.test", "extra": "reject"}
        }))
        .is_err()
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "open-current",
        "arguments": {"target": "tab"}
    }))
    .expect("typed current-URL new-tab command");
    assert_eq!(command.arguments, vec!["--target", "tab"]);
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "tab-clone",
        "arguments": {}
    }))
    .expect("typed tab-clone command");
    assert_eq!(command.name, "tab-clone");
    assert!(command.arguments.is_empty());
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
    assert!(
        typed_ipc_command(&serde_json::json!({
        "command": "tab-clone",
        "arguments": {"unexpected": true}
        }))
        .is_err()
    );
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "tab-select",
        "arguments": {"selector": "2"}
    }))
    .expect("typed tab-select command");
    assert_eq!(command.name, "tab-select");
    assert_eq!(command.arguments, vec!["2"]);
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "tab-select",
            "arguments": {"selector": 2}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "tab-select",
            "arguments": {"selector": "2", "extra": true}
        }))
        .is_err()
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "tab-suspend",
        "arguments": {"id": "tab-42"}
    }))
    .expect("typed tab-suspend command");
    assert_eq!(command.name, "tab-suspend");
    assert_eq!(command.arguments, vec!["tab-42"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "tab-discard",
        "arguments": {"id": "tab-42"}
    }))
    .expect("typed tab-discard command");
    assert_eq!(command.name, "tab-discard");
    assert_eq!(command.arguments, vec!["tab-42"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "tab-resume",
        "arguments": {"id": "tab-42"}
    }))
    .expect("typed tab-resume command");
    assert_eq!(command.name, "tab-resume");
    assert_eq!(command.arguments, vec!["tab-42"]);
    for command_name in ["tab-pin", "tab-mute"] {
        let (command, _) = typed_ipc_command(&serde_json::json!({
            "command": command_name,
            "arguments": {"state": "toggle"}
        }))
        .expect("typed active-tab state command");
        assert_eq!(command.name, command_name);
        assert_eq!(command.arguments, vec!["toggle"]);
    }
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "tab-move",
        "arguments": {"direction": "right"}
    }))
    .expect("typed active-tab move command");
    assert_eq!(command.arguments, vec!["right"]);
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "window-close",
        "arguments": {}
    }))
    .expect("typed window-close command");
    assert_eq!(command.name, "window-close");
    assert!(command.arguments.is_empty());
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "window-close",
            "arguments": {"unexpected": true}
        }))
        .is_err()
    );
    for command_name in ["reopen-in-window", "tab-detach"] {
        let (command, route) = typed_ipc_command(&serde_json::json!({
            "command": command_name,
            "arguments": {}
        }))
        .expect("typed window movement command");
        assert_eq!(command.name, command_name);
        assert!(command.arguments.is_empty());
        assert_eq!(route.open_target, IpcOpenTarget::Tab);
        assert!(
            typed_ipc_command(&serde_json::json!({
                "command": command_name,
                "arguments": {"unexpected": true}
            }))
            .is_err()
        );
    }
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "zoom",
        "arguments": {"factor": "1.25"}
    }))
    .expect("typed zoom command");
    assert_eq!(command.name, "zoom");
    assert_eq!(command.arguments, vec!["1.25"]);
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "zoom",
            "arguments": {"factor": 1.25}
        }))
        .is_err()
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "search-next",
        "arguments": {"direction": "backward", "count": 3}
    }))
    .expect("typed search-next command");
    assert_eq!(command.name, "search-next");
    assert_eq!(command.arguments, vec!["--backward", "--count", "3"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "search-next",
            "arguments": {"direction": "sideways"}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "search-next",
            "arguments": {"count": 101}
        }))
        .is_err()
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "scroll",
        "arguments": {"direction": "down", "count": 3}
    }))
    .expect("typed scroll command");
    assert_eq!(command.name, "scroll");
    assert_eq!(command.arguments, vec!["down", "--count", "3"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "scroll-page",
        "arguments": {"direction": "up", "half": true, "count": 2}
    }))
    .expect("typed page scroll command");
    assert_eq!(command.name, "scroll-page");
    assert_eq!(command.arguments, vec!["up", "--half", "--count", "2"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "scroll-to",
        "arguments": {"edge": "bottom"}
    }))
    .expect("typed scroll-to command");
    assert_eq!(command.name, "scroll-to");
    assert_eq!(command.arguments, vec!["bottom"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "scroll-page",
            "arguments": {"direction": "left"}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "scroll-to",
            "arguments": {"edge": "middle"}
        }))
        .is_err()
    );
    let (_, route) = typed_ipc_command(&serde_json::json!({
        "command": "open",
        "arguments": {"input": "https://example.test", "external": true}
    }))
    .expect("external open source");
    assert!(route.external_open);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "open",
            "arguments": {"input": "https://example.test", "external": "yes"}
        }))
        .is_err()
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "set",
        "arguments": {
            "key": "content.zoom",
            "value": "1.25",
            "temporary": true
        }
    }))
    .expect("typed set command");
    assert_eq!(command.arguments, vec!["--temp", "content.zoom=1.25"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "set",
        "arguments": {
            "key": "input.entry_mode",
            "value": "insert",
            "pattern": "https://docs.example/*"
        }
    }))
    .expect("typed site set command");
    assert_eq!(
        command.arguments,
        vec![
            "--pattern",
            "https://docs.example/*",
            "input.entry_mode=insert"
        ]
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "unset",
        "arguments": {"key": "content.zoom"}
    }))
    .expect("typed unset command");
    assert_eq!(command.arguments, vec!["content.zoom"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "unset",
        "arguments": {
            "key": "input.entry_mode",
            "pattern": "https://docs.example/*",
            "temporary": true
        }
    }))
    .expect("typed site unset command");
    assert_eq!(
        command.arguments,
        vec![
            "--temp",
            "--pattern",
            "https://docs.example/*",
            "input.entry_mode"
        ]
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "bind",
        "arguments": {
            "mode": "normal",
            "keychain": "g,g",
            "command": "open https://example.test"
        }
    }))
    .expect("typed bind command");
    assert_eq!(
        command.arguments,
        vec!["--mode", "normal", "g,g", "open https://example.test"]
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "unbind",
        "arguments": {"mode": "normal", "keychain": "g,g"}
    }))
    .expect("typed unbind command");
    assert_eq!(command.arguments, vec!["--mode", "normal", "g,g"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "config-export",
        "arguments": {"path": "/tmp/reviewed-config.toml"}
    }))
    .expect("typed config export command");
    assert_eq!(command.arguments, vec!["/tmp/reviewed-config.toml"]);
    let (_, route) = typed_ipc_command(&serde_json::json!({
        "command": "open",
        "arguments": {"input": "https://example.test", "target": "tab-bg"},
        "context": {"window": "last-focused"}
    }))
    .expect("background route");
    assert_eq!(route.open_target, IpcOpenTarget::BackgroundTab);
    assert_eq!(route.selector, DispatchTarget::LastFocused);
    let (_, route) = typed_ipc_command(&serde_json::json!({
        "command": "open",
        "arguments": {"input": "https://example.test"},
        "context": {"window": "windowid-42"}
    }))
    .expect("stable window route");
    assert_eq!(
        route.selector,
        DispatchTarget::Window(WindowId::from_display("windowid-42").expect("window ID"))
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "open",
            "arguments": {"input": "https://example.test"},
            "context": {"window": "window-42"}
        }))
        .is_err()
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "open",
        "arguments": {
            "input": "https://example.test/?utm_source=demo",
            "clean_link": true
        }
    }))
    .expect("typed clean-link open");
    assert_eq!(
        command.arguments,
        vec!["--clean-link", "https://example.test/?utm_source=demo"]
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "open",
            "arguments": {"input": "https://example.test", "clean_link": "yes"}
        }))
        .is_err()
    );
    let error = typed_ipc_command(&serde_json::json!({
        "command": "open",
        "arguments": {
            "input": "https://example.test/?utm_source=demo",
            "target": "tab-bg",
            "clean_link": true
        }
    }))
    .expect_err("typed clean-link background open must be rejected");
    assert!(
        error
            .to_string()
            .contains("cannot be combined with --target tab-bg")
    );
}

#[test]
fn ipc_blocking_toggle_uses_typed_site_flag() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "blocking-toggle",
        "arguments": {"site": true}
    }))
    .expect("site toggle command");
    assert_eq!(command.arguments, ["--site"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "blocking-toggle",
        "arguments": {}
    }))
    .expect("global toggle command");
    assert!(command.arguments.is_empty());
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "blocking-toggle",
            "arguments": {"site": "yes"}
        }))
        .is_err()
    );
}

#[test]
fn blocking_site_host_accepts_safe_http_authorities() {
    assert_eq!(
        blocking_site_host("HTTPS://Example.test:443/path"),
        Some("example.test".into())
    );
    assert_eq!(blocking_site_host("http://[::1]:8080/"), Some("::1".into()));
    assert_eq!(
        blocking_site_host("https://example.test./"),
        Some("example.test".into())
    );
    assert!(blocking_site_host("about:blank").is_none());
    assert!(blocking_site_host("https://user@example.test/").is_none());
    assert!(blocking_site_host("https://example.test:bad/").is_none());
    assert!(blocking_site_host("https://example.test:80:90/").is_none());
}

#[test]
fn durable_site_doctor_override_uses_a_typed_toml_array() {
    assert_eq!(
        toml_string_array_literal(&["example.test".into(), "*.sub.test".into()]),
        "[\"example.test\", \"*.sub.test\"]"
    );
}

#[test]
fn site_doctor_experiment_deadline_is_bounded() {
    let now = Instant::now();
    assert_eq!(site_doctor_remaining_seconds(now, now), Some(30));
    assert_eq!(
        site_doctor_remaining_seconds(now, now + Duration::from_secs(29)),
        Some(1)
    );
    assert_eq!(
        site_doctor_remaining_seconds(now, now + Duration::from_secs(30)),
        None
    );
}

#[test]
fn ipc_print_pdf_command_uses_a_typed_path_field() {
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "print-pdf",
        "arguments": {"path": "/tmp/page.pdf"}
    }))
    .expect("typed print-pdf command");
    assert_eq!(command.name, "print-pdf");
    assert_eq!(command.arguments, vec!["/tmp/page.pdf"]);
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "print-pdf",
            "arguments": {"path": "relative.pdf"}
        }))
        .is_ok()
    );
}

#[test]
fn search_next_options_accept_flags_in_any_order_and_reject_duplicates() {
    assert_eq!(
        parse_search_next_options(&[], true).expect("retained direction"),
        (true, 1)
    );
    assert_eq!(
        parse_search_next_options(&["--backward".into(), "--count".into(), "4".into()], false)
            .expect("ordered search-next options"),
        (true, 4)
    );
    assert_eq!(
        parse_search_next_options(&["--count".into(), "4".into(), "--backward".into()], false)
            .expect("flags in either order"),
        (true, 4)
    );
    assert!(parse_search_next_options(&["--backward".into(), "--backward".into()], false).is_err());
    assert!(
        parse_search_next_options(
            &["--count".into(), "2".into(), "--count".into(), "3".into()],
            false
        )
        .is_err()
    );
}

#[test]
fn scroll_command_options_reject_duplicates_and_wrong_order() {
    assert_eq!(
        parse_scroll_options("scroll", &["--count".into(), "3".into()])
            .expect("single scroll count"),
        (false, 3)
    );
    assert_eq!(
        parse_scroll_options(
            "scroll-page",
            &["--half".into(), "--count".into(), "2".into()]
        )
        .expect("ordered page options"),
        (true, 2)
    );
    assert!(
        parse_scroll_options(
            "scroll",
            &["--count".into(), "1".into(), "--count".into(), "2".into()]
        )
        .is_err()
    );
    assert!(
        parse_scroll_options(
            "scroll-page",
            &["--count".into(), "2".into(), "--half".into()]
        )
        .is_err()
    );
    assert!(parse_scroll_options("scroll-page", &["--half".into(), "--half".into()]).is_err());
}

#[test]
fn ipc_save_page_command_uses_a_typed_path_field() {
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "save-page",
        "arguments": {"path": "/tmp/page.html"}
    }))
    .expect("typed save-page command");
    assert_eq!(command.name, "save-page");
    assert_eq!(command.arguments, vec!["/tmp/page.html"]);
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
}

#[test]
fn ipc_view_source_command_is_typed_without_arguments() {
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "view-source",
        "arguments": {}
    }))
    .expect("typed view-source command");
    assert_eq!(command.name, "view-source");
    assert!(command.arguments.is_empty());
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
}

#[test]
fn ipc_script_run_command_uses_a_typed_name_field() {
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "script-run",
        "arguments": {"name": "video"}
    }))
    .expect("typed script-run command");
    assert_eq!(command.name, "script-run");
    assert_eq!(command.arguments, vec!["video"]);
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
}

#[test]
fn ipc_jseval_command_uses_typed_world_and_script_fields() {
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "jseval",
        "arguments": {
            "world": "page",
            "script": "document.title"
        }
    }))
    .expect("typed jseval command");
    assert_eq!(command.name, "jseval");
    assert_eq!(command.arguments, vec!["--world", "page", "document.title"]);
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "jseval",
            "arguments": {"world": "main", "script": "1 + 1"}
        }))
        .is_err()
    );
}

#[test]
fn ipc_support_commands_use_typed_help_and_empty_arguments() {
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "help",
        "arguments": {"topic": "content.zoom"}
    }))
    .expect("typed help command");
    assert_eq!(command.name, "help");
    assert_eq!(command.arguments, vec!["content.zoom"]);
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
    for name in ["version", "diagnostics"] {
        let (command, _) = typed_ipc_command(&serde_json::json!({
            "command": name,
            "arguments": {}
        }))
        .expect("typed support command");
        assert_eq!(command.name, name);
        assert!(command.arguments.is_empty());
    }
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "help",
            "arguments": {"topic": "content.zoom", "extra": true}
        }))
        .is_err()
    );
}

#[test]
fn jseval_script_validation_allows_js_whitespace_but_bounds_controls_and_size() {
    assert!(validate_jseval_script("function f() {\n\treturn 1\n}").is_ok());
    assert!(validate_jseval_script("1\u{0007}").is_err());
    assert!(validate_jseval_script(&"x".repeat(MAX_JSEVAL_SCRIPT_BYTES + 1)).is_err());
}

#[test]
fn ipc_print_command_is_typed_without_arguments() {
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "print",
        "arguments": {}
    }))
    .expect("typed print command");
    assert_eq!(command.name, "print");
    assert!(command.arguments.is_empty());
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
}

#[test]
fn ipc_devtools_command_uses_typed_detach_flag() {
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "devtools",
        "arguments": {"detach": true}
    }))
    .expect("typed devtools command");
    assert_eq!(command.name, "devtools");
    assert_eq!(command.arguments, vec!["--detach"]);
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "devtools",
            "arguments": {"detach": "true"}
        }))
        .is_err()
    );
}

#[test]
fn ipc_get_command_uses_typed_key_url_and_explain_fields() {
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "get",
        "arguments": {
            "key": "content.zoom",
            "url": "https://example.test/docs",
            "explain": true
        }
    }))
    .expect("typed get command");
    assert_eq!(command.name, "get");
    assert_eq!(
        command.arguments,
        vec![
            "content.zoom",
            "--url",
            "https://example.test/docs",
            "--explain"
        ]
    );
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "get",
            "arguments": {"key": "content.zoom", "explain": "yes"}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "get",
            "arguments": {"key": "content.zoom", "unknown": true}
        }))
        .is_err()
    );
}

#[test]
fn ipc_configuration_lifecycle_commands_use_typed_paths_and_no_arguments() {
    for name in [
        "config-edit",
        "config-reload",
        "config-check",
        "theme-reload",
    ] {
        let (command, _) = typed_ipc_command(&serde_json::json!({
            "command": name,
            "arguments": {}
        }))
        .expect("typed configuration lifecycle command");
        assert_eq!(command.name, name);
        assert!(command.arguments.is_empty());
        assert!(
            typed_ipc_command(&serde_json::json!({
                "command": name,
                "arguments": {"unexpected": true}
            }))
            .is_err()
        );
    }
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "config-write-defaults",
        "arguments": {"path": "/tmp/ferric-browser-defaults.toml"}
    }))
    .expect("typed default configuration path");
    assert_eq!(command.arguments, vec!["/tmp/ferric-browser-defaults.toml"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "config-write-defaults",
            "arguments": {}
        }))
        .is_err()
    );
}

#[test]
fn configured_editor_argv_replaces_only_the_complete_file_argument() {
    let config = serde_json::json!({
        "tools": {"editor": ["foot", "-e", "nvim", "{file}"]}
    });
    let (executable, arguments) =
        configured_editor_argv(&config, Path::new("/tmp/ferric-browser-config.toml"))
            .expect("configured editor argv");
    assert_eq!(executable, "foot");
    assert_eq!(
        arguments,
        vec!["-e", "nvim", "/tmp/ferric-browser-config.toml"]
    );

    let invalid = serde_json::json!({"tools": {"editor": ["nvim", "--cmd={file}"]}});
    assert!(configured_editor_argv(&invalid, Path::new("/tmp/config")).is_err());

    let oversized = "x".repeat(MAX_UNTRUSTED_ARGUMENT_BYTES + 1);
    let invalid = serde_json::json!({"tools": {"editor": [oversized, "{file}"]}});
    assert!(configured_editor_argv(&invalid, Path::new("/tmp/config")).is_err());
}

#[test]
fn config_editor_captures_bounded_stderr() {
    let io_source = include_str!("userscript_io.rs");
    let process_source = include_str!("editor_process.rs");
    assert!(io_source.contains("userscript pipe read failed"));
    assert!(io_source.contains("userscript output exceeds"));
    assert!(process_source.contains("exit_status: Option<ExitStatus>"));
}

#[test]
fn external_editor_rejects_rich_contenteditable_controls() {
    let qml = [
        include_str!("../qml/Main.qml"),
        include_str!("../qml/scripts/BrowserScripts.js"),
    ]
    .concat();
    assert!(qml.contains("e.contentEditable==='plaintext-only'"));
    assert!(qml.contains("return {error:'focused control is not a supported plain-text editor'}"));
    assert!(qml.contains("else e.textContent=next"));
    assert!(!qml.contains("if(e.isContentEditable)return {ok:true"));
}

#[test]
fn primary_web_engine_profile_uses_selected_profile_namespace() {
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains(
        "storageName: window.temporaryProfile ? \"\" : \"ferric-browser-\" + window.profileName"
    ));
    assert!(
        !qml.contains("storageName: window.temporaryProfile ? \"\" : \"ferric-browser-default\"")
    );
    assert!(qml.contains(
            "storageName: secondaryWindow.windowTransientProfile ? \"\" : \"ferric-browser-\" + secondaryWindow.windowProfileName"
        ));
    assert!(qml.contains("window.storageBasePath + \"/webengine/\" + window.profileName"));
    assert!(qml.contains("secondaryWindow.windowStorageBasePath + \"/webengine/\""));
    assert!(qml.contains("property string windowStorageBasePath: window.storageBasePath"));
}

#[test]
fn startup_profile_overrides_reach_the_browser_ui() {
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("property string startupProfileOverridesJson: \"{}\""));
    assert!(qml.contains("browserUi.set_startup_configuration("));
    assert!(qml.contains("window.startupProfileOverridesJson"));
    assert!(!qml.contains("browserUi.profile_overrides_json ="));
}

#[test]
fn command_completion_popup_has_accessible_popup_semantics() {
    let qml = include_str!("../qml/Main.qml");
    let popup = include_str!("../qml/components/FerricCommandLine.qml");
    assert!(qml.contains("FerricCommandLine {"));
    assert!(popup.contains("Accessible.role: Accessible.PopupMenu"));
    assert!(popup.contains("Accessible.name: \"Command completion popup\""));
    assert!(popup.contains("Accessible.description:"));
    assert!(popup.contains("? commandSurface.browserWindow.selectionColor"));
    assert!(popup.contains("? commandSurface.browserWindow.selectionTextColor"));
    assert!(popup.contains("commandSurface.browserWindow.readableTextColor("));
    assert!(!popup.contains("browserUi."));
}

#[test]
fn ipc_permission_commands_use_typed_origin_and_permission_fields() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "permissions",
        "arguments": {"origin": "https://example.test"}
    }))
    .expect("typed permissions command");
    assert_eq!(command.arguments, vec!["https://example.test"]);

    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "permission-reset",
        "arguments": {
            "origin": "https://example.test",
            "permission": "notifications"
        }
    }))
    .expect("typed permission reset command");
    assert_eq!(
        command.arguments,
        vec!["https://example.test", "notifications"]
    );
}

#[test]
fn ipc_site_status_accepts_an_optional_stable_tab_target() {
    let (active, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-status",
        "arguments": {}
    }))
    .expect("active site status command");
    assert!(active.arguments.is_empty());

    let (targeted, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-status",
        "arguments": {"tab": "tab-42"}
    }))
    .expect("targeted site status command");
    assert_eq!(targeted.arguments, vec!["--tab", "tab-42"]);

    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "site-status",
            "arguments": {"tab": "bad\nid"}
        }))
        .is_err()
    );
}

#[test]
fn permission_session_key_is_scoped_to_profile_uuid_and_lifetime() {
    let first_profile = Uuid::from_u128(1);
    let second_profile = Uuid::from_u128(2);
    let first = permission_session_key(first_profile, "https://example.test", "camera");
    let same = permission_session_key(first_profile, "https://example.test", "camera");
    let other_profile = permission_session_key(second_profile, "https://example.test", "camera");

    assert_eq!(first, same);
    assert_ne!(first, other_profile);
    assert_eq!(first.lifetime, PermissionLifetimeKey::ProfileSession);
}

#[test]
fn ipc_site_doctor_commands_use_typed_experiment_fields() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-doctor",
        "arguments": {"experiment": "blocking-bypass"}
    }))
    .expect("typed Site Doctor command");
    assert_eq!(command.arguments, vec!["blocking-bypass"]);

    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-doctor",
        "arguments": {"experiment": "userscripts-off"}
    }))
    .expect("typed userscript experiment command");
    assert_eq!(command.arguments, vec!["userscripts-off"]);

    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-doctor",
        "arguments": {"experiment": "fresh-view"}
    }))
    .expect("typed fresh-view experiment command");
    assert_eq!(command.arguments, vec!["fresh-view"]);

    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-doctor",
        "arguments": {"experiment": "compiled-defaults"}
    }))
    .expect("typed compiled-default experiment command");
    assert_eq!(command.arguments, vec!["compiled-defaults"]);

    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-doctor-undo",
        "arguments": {"id": "site-experiment-123"}
    }))
    .expect("typed Site Doctor undo command");
    assert_eq!(command.arguments, vec!["site-experiment-123"]);

    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "site-data-clear",
        "arguments": {"origin": "https://example.test", "confirmed": true}
    }))
    .expect("typed site-data clear command");
    assert_eq!(command.arguments, vec!["https://example.test", "--confirm"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "site-doctor",
            "arguments": {"kind": "blocking-bypass"}
        }))
        .is_err()
    );
}

#[test]
fn page_focus_requires_a_recent_user_gesture_before_insert_mode() {
    let qml = include_str!("../qml/Main.qml");
    let script = include_str!("../qml/scripts/BrowserScripts.js");
    assert!(script.contains("userGestureUntil=performance.now()+1500"));
    assert!(script.contains("__ferric_browserAuthorizeExplicitFocus"));
    assert!(script.contains("user_activated:!!userActivated"));
    assert!(qml.contains("!!state.user_activated"));
    assert!(script.contains("publish(false);})();"));
}

#[test]
fn hint_collection_accepts_qt_variant_candidate_lists() {
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("value && value.candidates !== undefined"));
    assert!(!qml.contains("Array.isArray(value.candidates)"));
}

#[test]
fn site_ledger_component_renders_data_and_emits_bridge_intents() {
    let qml = include_str!("../qml/components/FerricSiteLedger.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("signal activeOriginDataClearRequested()"));
    assert!(qml.contains("signal siteDoctorExperimentRequested(string kind)"));
    assert!(qml.contains("signal sanitizedReportCopyRequested(bool includeHost)"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("browserUi:"));
    assert!(composition_root.contains("FerricSiteLedger {"));
    assert!(!composition_root.contains("id: legacySiteLedger"));
}

#[test]
fn diagnostics_component_is_intent_only_and_replaces_the_inline_surface() {
    let qml = include_str!("../qml/components/FerricDiagnostics.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("signal copyRequested()"));
    assert!(qml.contains("signal saveRequested()"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricDiagnostics {"));
    assert!(!composition_root.contains("id: diagnosticsSurface"));
}

#[test]
fn binding_help_component_renders_rows_and_emits_root_owned_intents() {
    let qml = include_str!("../qml/components/FerricBindingHelp.qml");
    let composition_root = include_str!("../qml/Main.qml");
    let projection = include_str!("binding_presentation.rs");
    assert!(qml.contains("signal searchChanged(string text)"));
    assert!(qml.contains("bindingHelp.browserWindow.bindingHelpRows"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricBindingHelp {"));
    assert!(!composition_root.contains("id: bindingHelpSurface"));
    assert!(composition_root.contains("browserUi.refresh_binding_help(window.bindingHelpSearch)"));
    assert!(composition_root.contains("browserUi.binding_help_row_kinds"));
    assert!(!composition_root.contains("bindings_json"));
    assert!(!composition_root.contains("bindingHelpData"));
    assert!(!projection.contains("ipc_bindings_query"));
    assert!(include_str!("lib.rs").contains("#[qproperty(QStringList, binding_help_row_kinds)]"));
}

#[test]
fn profile_delete_preview_component_only_emits_confirmation_intents() {
    let qml = include_str!("../qml/components/FerricProfileDeletePreview.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("signal confirmRequested()"));
    assert!(qml.contains("signal cancelRequested()"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricProfileDeletePreview {"));
    assert!(!composition_root.contains("id: profileDeletePreview"));
    assert!(composition_root.contains("browserUi.delete_profile(window.profileDeleteName, true)"));
}

#[test]
fn reopen_window_confirmation_component_only_emits_root_owned_intents() {
    let qml = include_str!("../qml/components/FerricReopenWindowConfirmation.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("signal confirmRequested()"));
    assert!(qml.contains("signal cancelRequested()"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricReopenWindowConfirmation {"));
    assert!(!composition_root.contains("id: reopenWindowConfirmation"));
    assert!(composition_root.contains("window.confirmReopenWindow()"));
    assert!(composition_root.contains("window.cancelReopenWindow()"));
}

#[test]
fn session_preview_component_renders_state_and_emits_root_owned_intents() {
    let qml = include_str!("../qml/components/FerricSessionPreview.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("signal closeRequested()"));
    assert!(qml.contains("signal loadRequested(bool append)"));
    assert!(qml.contains("sessionPreviewLoading"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricSessionPreview {"));
    assert!(!composition_root.contains("id: sessionPreview\n"));
    assert!(composition_root.contains("window.loadPreviewedSession()"));
}

#[test]
fn session_manager_component_renders_model_and_emits_root_owned_intents() {
    let qml = include_str!("../qml/components/FerricSessionManager.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("required property var sessionsModel"));
    assert!(qml.contains("signal saveRequested(string name)"));
    assert!(qml.contains("signal deleteRequested(string name, bool confirmed)"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricSessionManager {"));
    assert!(!composition_root.contains("id: sessionManager\n"));
    assert!(composition_root.contains("browserUi.save_named_session(name)"));
    assert!(composition_root.contains("browserUi.delete_named_session(name, true)"));
}

#[test]
fn link_preview_component_renders_preview_data_and_emits_root_owned_intents() {
    let qml = include_str!("../qml/components/FerricLinkPreview.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("required property var previewUi"));
    assert!(qml.contains("link_preview_original"));
    assert!(!qml.contains("previewData"));
    assert!(qml.contains("signal navigationConfirmed()"));
    assert!(qml.contains("preview.previewUi.link_preview_removed_parameters"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricLinkPreview {"));
    assert!(!composition_root.contains("id: linkPreview\n"));
    assert!(composition_root.contains("browserUi.confirm_link_navigation()"));
}

#[test]
fn library_preview_components_render_models_and_emit_root_owned_intents() {
    let journey = include_str!("../qml/components/FerricJourneyExportPreview.qml");
    let transfer = include_str!("../qml/components/FerricPrivateHistoryTransfer.qml");
    let manager = include_str!("../qml/components/FerricLibraryManager.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(journey.contains("signal chooseFileRequested()"));
    assert!(!journey.contains("fileDialogSurfaces."));
    assert!(!journey.contains("browserUi."));
    assert!(transfer.contains("required property var profilesModel"));
    assert!(transfer.contains("signal reopenRequested(string profileName)"));
    assert!(!transfer.contains("browserUi."));
    assert!(manager.contains("FerricJourneyExportPreview {"));
    assert!(manager.contains("FerricPrivateHistoryTransfer {"));
    assert!(!composition_root.contains("id: journeyExportPreviewSurface"));
    assert!(!composition_root.contains("id: privateHistoryTransferSurface"));
    assert!(composition_root.contains("fileDialogSurfaces.openJourneyExport()"));
}

#[test]
fn download_manager_component_renders_model_and_emits_root_owned_intents() {
    let qml = include_str!("../qml/components/FerricDownloadManager.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("required property var downloadsModel"));
    assert!(qml.contains("signal openRequested(string downloadId, bool reveal)"));
    assert!(qml.contains("signal actionRequested(string downloadId, string action)"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricDownloadManager {"));
    assert!(!composition_root.contains("id: downloadManager\n"));
    assert!(composition_root.contains("window.requestDownloadAction(downloadId, action)"));
}

#[test]
fn profile_manager_component_renders_model_and_emits_root_owned_intents() {
    let qml = include_str!("../qml/components/FerricProfileManager.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("required property var profilesModel"));
    assert!(qml.contains("signal createRequested(string name, string label)"));
    assert!(qml.contains("function clearCreateInputs()"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricProfileManager {"));
    assert!(!composition_root.contains("id: profileManager\n"));
    assert!(composition_root.contains("browserUi.create_profile(name, label)"));
    assert!(composition_root.contains("browserUi.rename_profile(name, label)"));
}

#[test]
fn rapid_hint_confirmation_component_emits_root_owned_intents() {
    let qml = include_str!("../qml/components/FerricRapidHintConfirmation.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("signal continueRequested()"));
    assert!(qml.contains("signal cancelRequested()"));
    assert!(!qml.contains("browserUi."));
    assert!(composition_root.contains("FerricRapidHintConfirmation {"));
    assert!(!composition_root.contains("id: rapidHintConfirmation"));
    assert!(composition_root.contains("browserUi.confirm_rapid_hint_tabs()"));
}

#[test]
fn hint_overlay_component_renders_candidates_and_emits_root_owned_intents() {
    let qml = include_str!("../qml/components/FerricHintOverlay.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("required property var hintResults"));
    assert!(qml.contains("signal activationRequested(string label)"));
    assert!(qml.contains("signal actionsRequested(string label)"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("activateHint("));
    assert!(composition_root.contains("FerricHintOverlay {"));
    assert!(composition_root.contains("window.activateHint(label)"));
    assert!(composition_root.contains("window.showHintActions(label)"));
}

#[test]
fn userscript_inventory_component_keeps_enable_rollback_at_the_root_boundary() {
    let qml = include_str!("../qml/components/FerricUserscriptInventory.qml");
    let settings = include_str!("../qml/components/FerricSettings.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("signal enabledRequested(string name, bool enabled)"));
    assert!(qml.contains("function setEnabled(name, enabled)"));
    assert!(!qml.contains("browserUi."));
    assert!(settings.contains("FerricUserscriptInventory {"));
    assert!(!composition_root.contains("id: userscriptList"));
    assert!(composition_root.contains("browserUi.set_userscript_enabled(name, enabled)"));
    assert!(settings.contains("userscriptInventory.setEnabled(name, enabled)"));
    assert!(composition_root.contains("settingsSurface.setUserscriptEnabled(name, !enabled)"));
}

#[test]
fn setting_rows_component_emits_intents_without_reaching_the_runtime_bridge() {
    let qml = include_str!("../qml/components/FerricSettingRows.qml");
    let settings = include_str!("../qml/components/FerricSettings.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("signal applyRequested(var row, var value)"));
    assert!(qml.contains("signal resetRequested(var row)"));
    assert!(qml.contains("settingRows.applyRequested(settingRow.rowData, checked)"));
    assert!(qml.contains("settingRows.resetRequested(settingRow.rowData)"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("window.applySetting"));
    assert!(settings.contains("FerricSettingRows {"));
    assert!(!composition_root.contains("id: settingsList"));
    assert!(composition_root.contains("window.applySetting(row, value)"));
    assert!(composition_root.contains("window.resetSetting(row)"));
}

#[test]
fn settings_component_composes_schema_rows_and_keeps_mutations_at_the_root() {
    let qml = include_str!("../qml/components/FerricSettings.qml");
    let rows = include_str!("../qml/components/FerricSettingRows.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("FerricUserscriptInventory {"));
    assert!(qml.contains("FerricSettingRows {"));
    assert!(qml.contains("signal userscriptEnabledRequested(string name, bool enabled)"));
    assert!(qml.contains("signal settingApplyRequested(var row, var value)"));
    assert!(qml.contains("function setUserscriptEnabled(name, enabled)"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("set_runtime_setting"));
    assert!(qml.contains("required property var settingsModel"));
    assert!(rows.contains("required property var settingsModel"));
    assert!(rows.contains("model: settingRows.settingsModel"));
    assert!(rows.contains("rowData.options"));
    assert!(composition_root.contains("FerricSettings {"));
    assert!(composition_root.contains("FerricSettingsModel { id: settingsModel }"));
    assert!(composition_root.contains("settingsModel.replaceRows("));
    assert!(!composition_root.contains("JSON.parse(browserUi.config_json)"));
    assert!(!composition_root.contains("function settingConfigValue("));
    assert!(composition_root.contains("browserUi.set_userscript_enabled(name, enabled)"));
    assert!(composition_root.contains("settingsSurface.setUserscriptEnabled(name, !enabled)"));
    assert!(composition_root.contains("window.applySetting(row, value)"));
}

#[test]
fn journey_graph_component_only_renders_root_owned_graph_models() {
    let qml = include_str!("../qml/components/FerricJourneyGraph.qml");
    let manager = include_str!("../qml/components/FerricLibraryManager.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("required property var graphNodes"));
    assert!(qml.contains("required property var graphEntries"));
    assert!(qml.contains("required property var lineData"));
    assert!(qml.contains("function requestPaint()"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("execute_command"));
    assert!(manager.contains("FerricJourneyGraph {"));
    assert!(manager.contains("graphNodes: libraryManager.graphNodes"));
    assert!(manager.contains("graphEntries: libraryManager.graphEntries"));
    assert!(composition_root.contains("libraryManager.requestGraphPaint()"));
}

#[test]
fn switcher_results_component_keeps_navigation_and_dispatch_at_the_root_boundary() {
    let qml = include_str!("../qml/components/FerricSwitcherResults.qml");
    let switcher = include_str!("../qml/components/FerricSwitcher.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("signal activationRequested(int index)"));
    assert!(qml.contains("signal actionRequested(int index, string action)"));
    assert!(qml.contains("function moveBy(delta)"));
    assert!(qml.contains("function moveToBeginning()"));
    assert!(qml.contains("function moveToEnd()"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("activateSwitcher("));
    assert!(switcher.contains("FerricSwitcherResults {"));
    assert!(switcher.contains("switcherResultsList.moveBy(8)"));
    assert!(composition_root.contains("window.activateSwitcher(index)"));
    assert!(composition_root.contains("window.activateSwitcherAction(index, action)"));
}

#[test]
fn switcher_component_exposes_focus_and_query_without_runtime_policy() {
    let qml = include_str!("../qml/components/FerricSwitcher.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("property alias query: switcherInput.text"));
    assert!(qml.contains("function focusInput()"));
    assert!(qml.contains("signal queryChanged(string query)"));
    assert!(qml.contains("signal activationRequested(int index)"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("execute_command"));
    assert!(composition_root.contains("FerricSwitcher {"));
    assert!(composition_root.contains("switcherSurface.query = query || \"\""));
    assert!(composition_root.contains("switcherSurface.focusInput()"));
}

#[test]
fn command_line_component_emits_editing_intents_without_executing_commands() {
    let qml = include_str!("../qml/components/FerricCommandLine.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("signal completionUpdateRequested(string text, int cursorPosition)"));
    assert!(qml.contains("signal submitted(string text)"));
    assert!(qml.contains("signal completionMoveRequested(int delta)"));
    assert!(qml.contains("function focusInput()"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("execute_command"));
    assert!(composition_root.contains("FerricCommandLine {"));
    assert!(composition_root.contains("browserUi.update_completion(text, cursorPosition)"));
    assert!(composition_root.contains("browserUi.execute_command(text)"));
    assert!(composition_root.contains("browserUi.completion_move(delta)"));
}

#[test]
fn search_bar_component_emits_intents_without_page_or_engine_policy() {
    let qml = include_str!("../qml/components/FerricSearchBar.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("signal searchChanged(string text)"));
    assert!(qml.contains("signal nextRequested(bool backward)"));
    assert!(qml.contains("function focusInput()"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("execute_ui_action"));
    assert!(composition_root.contains("FerricSearchBar {"));
    assert!(composition_root.contains("browserUi.search_changed(text)"));
    assert!(composition_root.contains("browserUi.search_next(browserUi.search_backward)"));
    assert!(composition_root.contains("browserUi.execute_ui_action("));
}

#[test]
fn journey_search_component_emits_queries_without_constructing_commands() {
    let qml = include_str!("../qml/components/FerricJourneySearch.qml");
    let manager = include_str!("../qml/components/FerricLibraryManager.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("signal searchRequested(string text)"));
    assert!(qml.contains("signal clearRequested()"));
    assert!(qml.contains("signal currentRequested()"));
    assert!(qml.contains("signal allRequested()"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("execute_command"));
    assert!(manager.contains("FerricJourneySearch {"));
    assert!(
        composition_root.contains("window.runJourneyQuery(window.journeySearchArgument(text))")
    );
    assert!(!composition_root.contains("journeySearchField.text ="));
}

#[test]
fn library_entries_component_preserves_row_state_and_emits_root_owned_operations() {
    let qml = include_str!("../qml/components/FerricLibraryEntries.qml");
    let manager = include_str!("../qml/components/FerricLibraryManager.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("signal openRequested(string entryKind, string entryId)"));
    assert!(qml.contains("signal editRequested(string entryKind, string entryId, string value)"));
    assert!(qml.contains("signal deleteRequested(string entryKind, string entryId)"));
    assert!(qml.contains("signal journeyReopenRequested(string nodeId, string target)"));
    assert!(qml.contains("signal journeyExpandRequested(string nodeId)"));
    assert!(qml.contains("function markEditSaved(entryKind, entryId)"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("execute_command"));
    assert!(manager.contains("FerricLibraryEntries {"));
    assert!(composition_root.contains("window.editLibraryEntry(entryKind, entryId, value)"));
    assert!(composition_root.contains("libraryManager.markEditSaved(entryKind, entryId)"));
    assert!(composition_root.contains("window.deleteLibraryEntry(entryKind, entryId)"));
    assert!(composition_root.contains("browserUi.execute_command(command)"));
}

#[test]
fn library_manager_composes_presentation_and_forwards_every_operation_to_the_root() {
    let qml = include_str!("../qml/components/FerricLibraryManager.qml");
    let composition_root = include_str!("../qml/Main.qml");
    assert!(qml.contains("FerricJourneySearch {"));
    assert!(qml.contains("FerricLibraryEntries {"));
    assert!(qml.contains("FerricJourneyGraph {"));
    assert!(
        qml.contains("signal entryEditRequested(string entryKind, string entryId, string value)")
    );
    assert!(qml.contains("signal journeyReopenRequested(string nodeId, string target)"));
    assert!(!qml.contains("browserUi."));
    assert!(!qml.contains("execute_command"));
    assert!(composition_root.contains("FerricLibraryManager {"));
    assert!(composition_root.contains("onJourneyReopenRequested: function(nodeId, target)"));
    assert!(
        composition_root
            .contains("onPrivateHistoryBookmarkRequested: function(profileName, profileLabel)")
    );
}

#[test]
fn all_hints_cover_qutebrowser_control_families_and_activate_controls() {
    let qml = include_str!("../qml/Main.qml");
    let script = include_str!("../qml/scripts/BrowserScripts.js");
    for selector in [
        "input:not([type='hidden'])",
        "summary",
        "[onclick]",
        "[role='checkbox']",
        "[role='menuitem']",
        "[aria-haspopup]",
        "[tabindex]:not([tabindex='-1'])",
    ] {
        assert!(
            script.contains(selector),
            "missing hint selector {selector}"
        );
    }
    assert!(qml.contains("function hintClickScript(elementId)"));
    assert!(qml.contains("result.action === \"click\""));
    assert!(script.contains("window.__ferric_browserHintElements=elements"));
    assert!(script.contains("record&&record.element"));
    assert!(script.contains("el.click();return true"));
}

#[test]
fn config_watcher_notifies_after_atomic_file_replacement_with_polling_fallback() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-config-watch-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    fs::create_dir_all(&directory).expect("watch directory");
    let path = directory.join("config.toml");
    fs::write(&path, "[ui]\nfont_size_pt = 10.0\n").expect("initial config");

    let mut watch = ConfigWatch::default();
    watch.set_sources(&path, std::slice::from_ref(&path));
    // Force the portable polling path even on Linux, where inotify is
    // normally available. This also covers environments where native
    // notification setup fails or a directory cannot be watched.
    #[cfg(target_os = "linux")]
    watch.stop_inotify();
    let temporary = directory.join(".config.toml.tmp");
    fs::write(&temporary, "[ui]\nfont_size_pt = 11.0\n").expect("replacement");
    fs::rename(&temporary, &path).expect("atomic replacement");

    let deadline = Instant::now() + Duration::from_secs(1);
    let mut notified = false;
    while !notified && Instant::now() < deadline {
        notified = watch.changed();
        if notified {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(notified, "inotify did not report the replacement");
    drop(watch);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&directory);
}

#[test]
fn config_reload_worker_loads_and_validates_off_thread() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-config-worker-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    fs::create_dir_all(&directory).expect("worker directory");
    let path = directory.join("config.toml");
    fs::write(&path, "schema_version = 3\n[ui]\nfont_size_pt = 11.0\n").expect("worker config");

    let mut worker = ConfigReloadWorker::spawn().expect("config worker");
    worker
        .request(path.clone(), "default".into())
        .expect("reload request");
    let result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let result = result
        .expect("reload response")
        .expect("valid configuration");
    assert_eq!(result.operation, ConfigReadOperation::Reload);
    assert!((result.loaded.config.ui.font_size_pt - 11.0).abs() < f64::EPSILON);
    assert!(result.profile_overrides.settings.is_empty());

    let contexts_path = directory.join("contexts.toml");
    fs::write(&contexts_path, "unknown = true\n").expect("invalid contexts document");
    worker
        .request(path.clone(), "default".into())
        .expect("reload with invalid contexts request");
    let invalid_contexts = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let invalid_contexts = match invalid_contexts.expect("invalid contexts response") {
        Err(error) => error,
        Ok(_) => panic!("invalid contexts must reject reload"),
    };
    assert!(invalid_contexts.contains("contexts configuration is invalid"));
    let _ = fs::remove_file(&contexts_path);

    worker
        .request_check(path.clone())
        .expect("configuration check request");
    let check = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let check = check
        .expect("check response")
        .expect("valid checked configuration");
    assert_eq!(check.operation, ConfigReadOperation::Check);
    assert_eq!(check.loaded.sources.len(), 1);

    drop(worker);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&directory);
}

#[test]
fn config_write_worker_creates_private_non_overwriting_file() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-config-writer-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    fs::create_dir_all(&directory).expect("writer directory");
    let path = directory.join("export.toml");
    let contents = b"[ui]\nfont_size_pt = 11.0\n";

    let mut worker = ConfigWriteWorker::spawn().expect("config writer");
    worker
        .request(path.clone(), contents.to_vec())
        .expect("write request");
    let result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let result = result
        .expect("write response")
        .expect("private configuration write");
    assert_eq!(result.path, path);
    assert_eq!(result.bytes, contents.len());
    assert_eq!(fs::read(&path).expect("written configuration"), contents);
    #[cfg(unix)]
    assert_eq!(
        std::os::unix::fs::PermissionsExt::mode(
            &fs::metadata(&path).expect("written metadata").permissions(),
        ) & 0o777,
        0o600
    );

    worker
        .request(path.clone(), b"replacement".to_vec())
        .expect("second write request");
    let second = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let collision = second.expect("collision response");
    assert!(collision.is_err(), "create-new must refuse overwrite");
    assert_eq!(fs::read(&path).expect("original configuration"), contents);

    drop(worker);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(&directory);
}

#[test]
fn userscript_manager_worker_reads_and_updates_off_thread() {
    let root = std::env::temp_dir().join(format!(
        "ferric-browser-userscript-worker-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    let directory = root.join("userscripts");
    fs::create_dir_all(&directory).expect("userscript worker directory");
    fs::write(
        directory.join("worker.toml"),
        r#"
schema_version = 1
name = "worker"
executable = "/bin/true"
enabled = true
matches = ["https://example.test/*"]
"#,
    )
    .expect("userscript worker manifest");

    let mut worker = UserscriptManagerWorker::spawn().expect("userscript manager");
    worker
        .request(UserscriptManagerRequest::Refresh { root: root.clone() })
        .expect("inventory request");
    let inventory = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    match inventory.expect("inventory response") {
        UserscriptManagerResult::Inventory(Ok(scripts)) => {
            assert_eq!(scripts.len(), 1);
            assert!(scripts[0].enabled);
        }
        other => panic!("unexpected inventory result: {other:?}"),
    }

    worker
        .request(UserscriptManagerRequest::SetEnabled {
            root: root.clone(),
            name: "worker".into(),
            enabled: false,
        })
        .expect("toggle request");
    let toggled = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    match toggled.expect("toggle response") {
        UserscriptManagerResult::SetEnabled {
            enabled: false,
            result: Ok(scripts),
        } => assert!(!scripts[0].enabled),
        other => panic!("unexpected toggle result: {other:?}"),
    }

    let source = root.join("installed.toml");
    fs::write(
        &source,
        r#"
schema_version = 1
name = "installed"
executable = "/bin/true"
"#,
    )
    .expect("source userscript manifest");
    worker
        .request(UserscriptManagerRequest::Install {
            root: root.clone(),
            source,
        })
        .expect("install request");
    let installed = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    match installed.expect("install response") {
        UserscriptManagerResult::Installed(Ok(scripts)) => {
            assert_eq!(scripts.len(), 2);
        }
        other => panic!("unexpected install result: {other:?}"),
    }

    worker
        .request(UserscriptManagerRequest::Remove {
            root: root.clone(),
            name: "installed".into(),
        })
        .expect("remove request");
    let removed = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    match removed.expect("remove response") {
        UserscriptManagerResult::Removed(Ok(scripts)) => assert_eq!(scripts.len(), 1),
        other => panic!("unexpected remove result: {other:?}"),
    }
    assert!(!root.join("userscripts/installed.toml").exists());

    drop(worker);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn editor_write_worker_creates_private_scratch_file_off_thread() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-editor-writer-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    let path = directory.join("editor").join("scratch.txt");
    let contents = b"editor text";
    let mut worker = EditorWriteWorker::spawn().expect("editor writer");
    worker
        .request(path.clone(), contents.to_vec())
        .expect("scratch request");
    let result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    assert_eq!(
        result.expect("scratch response").expect("scratch write"),
        path
    );
    assert_eq!(fs::read(&path).expect("scratch contents"), contents);
    #[cfg(unix)]
    assert_eq!(
        std::os::unix::fs::PermissionsExt::mode(
            &fs::metadata(&path).expect("scratch metadata").permissions(),
        ) & 0o777,
        0o600
    );
    drop(worker);
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir(path.parent().expect("scratch parent"));
    let _ = fs::remove_dir(&directory);
}

#[test]
fn profile_delete_worker_removes_data_and_registry_off_thread() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
    roots.ensure().expect("root directories");
    let mut registry = ProfileRegistry::open(&roots).expect("profile registry");
    let profile = registry
        .create("worker-delete", "Worker delete", ProfilePrivacy::Normal)
        .expect("profile record")
        .clone();
    let profile_data = roots.data.join("profiles").join(profile.id.to_string());
    fs::create_dir_all(&profile_data).expect("profile data");
    fs::write(profile_data.join("marker"), b"delete me").expect("profile marker");

    let mut worker = ProfileDeleteWorker::spawn().expect("profile delete worker");
    worker
        .request_create(
            roots.clone(),
            "worker-create".to_owned(),
            "Worker create".to_owned(),
        )
        .expect("create request");
    let create_result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    assert!(matches!(
        create_result
            .expect("create response")
            .expect("profile creation"),
        ProfileMutationResult::Created
    ));
    worker
        .request_rename(
            roots.clone(),
            "worker-create".to_owned(),
            "Worker renamed".to_owned(),
        )
        .expect("rename request");
    let rename_result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    assert!(matches!(
        rename_result
            .expect("rename response")
            .expect("profile rename"),
        ProfileMutationResult::Renamed
    ));
    assert_eq!(
        ProfileRegistry::open(&roots)
            .expect("reopen after rename")
            .profiles()
            .iter()
            .find(|record| record.name == "worker-create")
            .expect("created profile")
            .label,
        "Worker renamed"
    );
    worker
        .request_delete(roots.clone(), profile.name.clone(), None)
        .expect("delete request");
    let result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    assert_eq!(
        match result.expect("delete response").expect("profile deletion") {
            ProfileMutationResult::Deleted(outcome) => outcome.profile_name,
            _ => panic!("unexpected profile mutation response"),
        },
        profile.name
    );
    assert!(!profile_data.exists());
    assert!(
        !ProfileRegistry::open(&roots)
            .expect("reopen profile registry")
            .profiles()
            .iter()
            .any(|record| record.id == profile.id)
    );
    drop(worker);
    roots.cleanup().expect("temporary cleanup");
}

#[test]
fn profile_list_worker_reads_registry_off_thread() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
    roots.ensure().expect("root directories");
    let mut registry = ProfileRegistry::open(&roots).expect("profile registry");
    registry
        .create("worker-list", "Worker list", ProfilePrivacy::Normal)
        .expect("profile record");

    let mut worker = ProfileListWorker::spawn().expect("profile list worker");
    worker.request(roots.clone()).expect("list request");
    let result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let values = result.expect("list response").expect("profile list");
    assert!(values.contains("worker-list\tWorker list\t"));
    assert_eq!(
        profile_from_list_values(&values, "worker-list"),
        Some(("worker-list".into(), "Worker list".into()))
    );
    assert!(profile_from_list_values(&values, "missing").is_none());
    drop(worker);
    roots.cleanup().expect("temporary cleanup");
}

#[test]
fn private_history_is_bounded_deduplicated_and_origin_clearable() {
    let mut records = Vec::new();
    let mut next_id = -1;
    BrowserUi::upsert_private_history(
        &mut records,
        &mut next_id,
        "https://example.test/one",
        "One",
        10,
    );
    BrowserUi::upsert_private_history(
        &mut records,
        &mut next_id,
        "https://other.test/two",
        "Two",
        20,
    );
    BrowserUi::upsert_private_history(
        &mut records,
        &mut next_id,
        "https://example.test/one",
        "Updated",
        30,
    );
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].url, "https://example.test/one");
    assert_eq!(records[0].visit_count, 2);
    assert_eq!(records[0].title, "Updated");
    assert!(records.iter().all(|record| record.id < 0));

    let deleted =
        BrowserUi::clear_private_history_records(&mut records, None, Some("https://example.test"));
    assert_eq!(deleted, 1);
    assert_eq!(records[0].url, "https://other.test/two");
    assert_eq!(
        BrowserUi::clear_private_history_records(&mut records, None, None),
        1
    );
    assert!(records.is_empty());
}

#[test]
fn network_policy_worker_loads_cached_policy_off_thread() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
    let mut worker = NetworkPolicyWorker::spawn().expect("network policy worker");
    worker
        .request(
            Some(roots.clone()),
            serde_json::json!({
                "blocking": {
                    "enabled": true,
                    "network_filtering": true,
                    "lists": [],
                }
            }),
        )
        .expect("policy request");
    let result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let policy = result.expect("policy response").expect("policy load");
    assert!(policy.blocked_hosts.is_empty());
    assert!(policy.compile_failures.is_empty());
    drop(worker);
    roots.cleanup().expect("temporary cleanup");
}

#[test]
fn profile_preview_worker_reads_delete_metadata_off_thread() {
    let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
    roots.ensure().expect("root directories");
    let mut registry = ProfileRegistry::open(&roots).expect("profile registry");
    registry
        .create("worker-preview", "Worker preview", ProfilePrivacy::Normal)
        .expect("profile record");

    let mut worker = ProfilePreviewWorker::spawn().expect("profile preview worker");
    worker
        .request(roots.clone(), "worker-preview".into())
        .expect("preview request");
    let result = (0..100).find_map(|_| {
        let value = worker.poll();
        if value.is_none() {
            thread::sleep(Duration::from_millis(1));
        }
        value
    });
    let preview = result.expect("preview response").expect("profile preview");
    assert!(preview.contains("Profile: worker-preview"));
    assert!(preview.contains("QtWebEngine storage is not deleted"));
    drop(worker);
    roots.cleanup().expect("temporary cleanup");
}

#[test]
fn ipc_copy_commands_use_safe_typed_url_arguments() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "yank",
        "arguments": {"source": "url", "input": "https://example.test/?x=$(safe)"}
    }))
    .expect("typed URL copy");
    assert_eq!(
        command.arguments,
        vec!["url", "https://example.test/?x=$(safe)"]
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "yank",
        "arguments": {"source": "url", "clean": true}
    }))
    .expect("typed clean URL copy");
    assert_eq!(command.arguments, vec!["url", "--clean"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "yank",
            "arguments": {"source": "selection"}
        }))
        .is_ok()
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "yank",
        "arguments": {"source": "selection", "primary": true}
    }))
    .expect("typed primary selection copy");
    assert_eq!(command.arguments, vec!["selection", "--primary"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "yank",
        "arguments": {"source": "title", "primary": true}
    }))
    .expect("typed title copy");
    assert_eq!(command.arguments, vec!["title", "--primary"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "yank",
            "arguments": {"source": "title", "clean": true}
        }))
        .is_err()
    );
}

#[test]
fn ipc_caret_commands_use_typed_fields() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "mode-enter",
        "arguments": {"mode": "caret"}
    }))
    .expect("typed mode command");
    assert_eq!(command.arguments, vec!["caret"]);

    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "caret-move",
        "arguments": {"direction": "word-next", "count": 2}
    }))
    .expect("typed caret movement");
    assert_eq!(command.arguments, vec!["word-next", "--count", "2"]);

    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "caret-select",
        "arguments": {"state": "toggle"}
    }))
    .expect("typed caret selection");
    assert_eq!(command.arguments, vec!["toggle"]);
}

#[test]
fn ipc_profile_open_uses_a_typed_name_and_optional_input() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "profile-open",
        "arguments": {"name": "work", "input": "https://example.test"}
    }))
    .expect("typed profile-open command");
    assert_eq!(command.arguments, vec!["work", "https://example.test"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "profile-open",
        "arguments": {"name": "work"}
    }))
    .expect("typed profile-open without input");
    assert_eq!(command.arguments, vec!["work"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "profile-open",
            "arguments": {"name": "work", "input": "https://example.test", "extra": true}
        }))
        .is_err()
    );
}

#[test]
fn profile_delete_command_uses_the_existing_preview_boundary() {
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("action.indexOf(\"profile-delete\\t\") === 0"));
    assert!(qml.contains("window.showProfileDeletePreview(profileDeleteAction"));
    assert!(qml.contains("browserUi.delete_profile(window.profileDeleteName, true)"));
    assert!(!qml.contains("delete_profile(window.profileDeleteName, false)"));
}

#[test]
fn fullscreen_command_and_page_request_share_the_window_boundary() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "fullscreen",
        "arguments": {"state": "toggle"}
    }))
    .expect("typed fullscreen command");
    assert_eq!(command.arguments, vec!["toggle"]);
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("action.indexOf(\"fullscreen\\t\") === 0"));
    assert!(qml.contains("onFullScreenRequested: function(request)"));
    assert!(qml.contains("request.accept()"));
    assert!(qml.contains("window.showFullScreen()"));
    assert!(qml.contains("window.showNormal()"));
}

#[test]
fn window_new_command_uses_typed_profile_and_private_options() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "window-new",
        "arguments": {"profile": "work", "private": true}
    }))
    .expect("typed window-new command");
    assert_eq!(command.arguments, vec!["--profile", "work", "--private"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "window-new",
        "arguments": {"private": false}
    }))
    .expect("typed normal window-new command");
    assert!(command.arguments.is_empty());
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "window-new",
            "arguments": {"private": "yes"}
        }))
        .is_err()
    );
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("action.indexOf(\"new-window\\t\") === 0"));
    assert!(qml.contains("windowPrivateProfile: windowAction[1] === \"true\""));
    assert!(qml.contains("windowStartupUrl: windowAction.slice(3).join(\"\\t\")"));
}

#[test]
fn external_window_focus_reports_bounded_activation_outcome() {
    let source = include_str!("../src/lib.rs");
    let qml = include_str!("../qml/Main.qml");
    assert!(source.contains(
        "operation_states\n            .insert(operation_id.clone(), \"running\".into())"
    ));
    assert!(source.contains("fn complete_window_focus("));
    assert!(source.contains("\"activation\": outcome"));
    assert!(qml.contains("var focusOperationId = focusParts.length >= 3 ? focusParts[2] : \"\""));
    assert!(qml.contains("complete_window_focus(focusOperationId, \"activated\")"));
    assert!(qml.contains("complete_window_focus(operationId, \"unknown\")"));
    assert!(qml.contains("complete_window_focus(focusOperationId, \"stale\")"));
}

#[test]
fn tab_give_command_preserves_target_and_queues_live_transfer() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "tab-give",
        "arguments": {"window_id": "window-42"}
    }))
    .expect("typed tab-give command");
    assert_eq!(command.arguments, vec!["window-42"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "tab-move",
        "arguments": {"id": "tab-42", "context": "research"}
    }))
    .expect("typed context tab-move command");
    assert_eq!(command.arguments, vec!["tab-42", "--context", "research"]);
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.move",
        "arguments": {"id": "tab-42", "context": "research"}
    }))
    .expect("typed context tab-move action");
    assert_eq!(action_id, "browser.tab.move");
    assert_eq!(command.arguments, vec!["tab-42", "--context", "research"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "tab-move",
            "arguments": {"id": "tab-42", "context": "research", "extra": true}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "tab-move",
            "arguments": {"id": "tab-42", "context": "research", "direction": "left"}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "tab-give",
            "arguments": {"window_id": "window-42", "extra": true}
        }))
        .is_err()
    );
    let source = include_str!("../src/lib.rs");
    assert!(source.contains("tab-give requires a bounded target window ID"));
    assert!(source.contains("format!(\"tab-give\\t{window_id}\\t{operation_id}\")"));
    assert!(source.contains("format!(\"tab-detach\\t{operation_id}\")"));
    assert!(source.contains("fn complete_transfer_operation("));
    let qml = [
        include_str!("../qml/Main.qml"),
        include_str!("../qml/components/FerricContextMoveDialog.qml"),
    ]
    .concat();
    assert!(qml.contains("windowTransferView"));
    assert!(qml.contains("completeDetachedSource"));
    assert!(qml.contains("rollback_tab_transfer(adoptedTabId)"));
    assert!(qml.contains("prepareTransferFallback"));
    assert!(qml.contains("discardPreparedTransferFallback"));
    assert!(qml.contains("browserWindowEntryForTarget"));
    assert!(qml.contains("browserWindowEntryForContext"));
    assert!(qml.contains("showContextMovePicker"));
    assert!(qml.contains("chooseContextMove"));
    assert!(qml.contains("contextMoveChoices"));
    assert!(qml.contains("Move tab to context"));
    assert!(qml.contains("tab-move-context\\t"));
    assert!(qml.contains("attachTransferredView"));
    assert!(qml.contains("focusedBrowserWindowEntry"));
    assert!(qml.contains("detachActiveViewForTransfer"));
    assert!(qml.contains("openDetachedWindow(sourceUi, sourceHost, transferAction, view)"));
    assert!(qml.contains("windowTransferOperationId"));
    assert!(qml.contains("complete_transfer_operation"));
    assert!(qml.contains("fields[2].indexOf(\"op-\") === 0"));
    assert!(source.contains("format!(\"tab-move-context\\t{context}\\t{id}\\t{operation_id}\")"));
    assert!(qml.contains("fields[3].indexOf(\"op-\") === 0"));
    assert!(qml.contains("sourceUi.complete_transfer_operation(arguments[4], true)"));
    assert!(qml.contains("operationId))"));
    assert!(qml.contains("complete_tab_transfer() creates the reducer's mandatory"));
    assert!(!qml.contains("secondaryUi.new_tab()"));
    assert!(qml.contains("action.indexOf(\"window-focus\\t\") === 0"));
    assert!(source.contains("pending_engine_action = Some(format!(\"window-focus\\t{id}\"))"));
}

#[test]
fn tab_open_command_uses_typed_input_and_background_target() {
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "tab-open",
        "arguments": {"input": "Ferric Browser", "background": true}
    }))
    .expect("typed background tab-open command");
    assert_eq!(command.arguments, vec!["--background", "Ferric Browser"]);
    assert_eq!(route.open_target, IpcOpenTarget::BackgroundTab);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "tab-open",
            "arguments": {"input": "https://example.test", "background": "yes"}
        }))
        .is_err()
    );
}

#[test]
fn registered_command_execution_enters_the_runtime_once() {
    let adapter = include_str!("runtime_bridge.rs");
    let forbidden_planner = ["dispatch", "_events("].concat();
    assert!(adapter.contains("fn dispatch_runtime("));
    assert!(adapter.contains("RuntimeCommand::Dispatch(Box::new("));
    assert!(!adapter.contains(&forbidden_planner));
}

#[test]
fn search_command_uses_typed_query_direction_and_case() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "search",
        "arguments": {
            "query": "Ferric Browser",
            "backward": true,
            "case": "sensitive"
        }
    }))
    .expect("typed search command");
    assert_eq!(
        command.arguments,
        vec!["--backward", "--case", "sensitive", "Ferric Browser"]
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "search",
        "arguments": {"query": "example"}
    }))
    .expect("typed default search command");
    assert_eq!(command.arguments, vec!["example"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "search",
            "arguments": {"query": "example", "case": "unicode"}
        }))
        .is_err()
    );
}

#[test]
fn reload_command_uses_typed_bypass_cache() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "reload",
        "arguments": {"bypass_cache": true}
    }))
    .expect("typed bypass-cache reload command");
    assert_eq!(command.arguments, vec!["--bypass-cache"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "reload",
        "arguments": {"bypass_cache": false}
    }))
    .expect("typed ordinary reload command");
    assert!(command.arguments.is_empty());
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "reload",
            "arguments": {"bypass_cache": "yes"}
        }))
        .is_err()
    );
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("reloadAndBypassCache"));
}

#[test]
fn tab_close_command_uses_optional_typed_id_and_count() {
    assert!(binding_uses_full_command_executor(&ParsedCommand {
        name: "tab-close".into(),
        arguments: Vec::new(),
    }));
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "tab-close",
        "arguments": {"id": "tab-42"}
    }))
    .expect("typed targeted tab-close command");
    assert_eq!(command.arguments, vec!["--id", "tab-42"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "tab-close",
        "arguments": {"count": 3}
    }))
    .expect("typed bulk tab-close command");
    assert_eq!(command.arguments, vec!["--count", "3"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "tab-close",
            "arguments": {"id": "tab-42", "count": 2}
        }))
        .is_err()
    );
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("action.indexOf(\"tab-close-active\\t\") === 0"));
    assert!(qml.contains("function closeTabAtIndex(index)"));
    assert!(
        include_str!("lib.rs").contains(".finish_tab_close(index, target.tab, target.generation)")
    );
    assert!(!qml.contains("browserUi.view_closed_for(index)"));
    assert!(qml.contains("view.visible = false"));
    assert!(qml.contains("view.parent = null"));
    assert!(qml.contains("Qt.callLater(function()"));
}

#[test]
fn ipc_spawn_commands_use_a_typed_argv_vector() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "spawn",
        "arguments": {"argv": ["/usr/bin/printf", "{title}"]}
    }))
    .expect("typed spawn command");
    assert_eq!(command.arguments, vec!["/usr/bin/printf", "{title}"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "spawn",
            "arguments": {"argv": ["/usr/bin/printf", "\0"]}
        }))
        .is_err()
    );
}

#[test]
fn history_commands_use_typed_bounded_counts() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "back",
        "arguments": {"count": 4}
    }))
    .expect("typed counted back command");
    assert_eq!(command.arguments, vec!["--count", "4"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "forward",
        "arguments": {}
    }))
    .expect("typed default forward command");
    assert!(command.arguments.is_empty());
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "forward",
            "arguments": {"count": 101}
        }))
        .is_err()
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "tab-next",
        "arguments": {"count": 3}
    }))
    .expect("typed counted tab-next command");
    assert_eq!(command.arguments, vec!["--count", "3"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "tab-prev",
        "arguments": {}
    }))
    .expect("typed default tab-prev command");
    assert!(command.arguments.is_empty());
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "tab-prev",
            "arguments": {"count": 101}
        }))
        .is_err()
    );
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("function executeHistoryTraversal(view, backwards, requestedCount)"));
    assert!(qml.contains("History boundary reached"));
}

#[test]
fn download_destination_uses_encoded_file_urls() {
    let qml = include_str!("../qml/Main.qml");
    let dialogs = include_str!("../qml/components/FerricFileDialogSurfaces.qml");
    assert!(qml.contains("function fileUrlForPath(path)"));
    assert!(qml.contains("segments[i] = encodeURIComponent(segments[i])"));
    assert!(qml.contains(
            "fileDialogSurfaces.downloadCurrentFile = window.fileUrlForPath(directory + \"/\" + safeName)"
        ));
    assert!(qml.contains("FerricFileDialogSurfaces"));
    assert!(dialogs.contains("title: \"Choose download destination\""));
    assert!(dialogs.contains("onAccepted: root.downloadAccepted()"));
    assert!(qml.contains("function downloadStagingDirectory(ui, id)"));
    assert!(qml.contains("download.downloadDirectory = stagingDirectory"));
    assert!(qml.contains("browserUi.finalize_download(id)"));
    assert!(qml.contains("discard_download_staging(id)"));
    assert!(!qml.contains("downloadChooser.currentFile = \"file://\" + directory"));
}

#[test]
fn staged_download_finalization_is_exclusive_and_cross_filesystem_safe() {
    let session_id = Uuid::new_v4();
    let destination_root =
        std::env::temp_dir().join(format!("ferric-browser-download-test-{session_id}"));
    fs::create_dir_all(&destination_root).expect("download test destination");
    let destination = destination_root.join("report.txt");
    let staged = stage_download_path(None, session_id, &destination)
        .expect("create private staging directory");
    fs::write(staged.staging_path(), b"download body").expect("write staged body");
    finalize_staged_download(&staged).expect("finalize staged body");
    assert_eq!(
        fs::read(&destination).expect("read finalized body"),
        b"download body"
    );

    let second = stage_download_path(None, session_id, &destination)
        .expect("create second private staging directory");
    fs::write(second.staging_path(), b"replacement").expect("write second staged body");
    assert!(finalize_staged_download(&second).is_err());
    cleanup_staged_download(&second);
    let _ = fs::remove_dir_all(&destination_root);
    let _ = fs::remove_dir_all(
        std::env::temp_dir()
            .join("ferric-browser-download-staging")
            .join(session_id.to_string()),
    );
}

#[test]
fn binding_commands_use_typed_mode_and_keychain_arguments() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "binding-list",
        "arguments": {"mode": "normal"}
    }))
    .expect("typed binding list command");
    assert_eq!(command.arguments, vec!["--mode", "normal"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "binding-explain",
        "arguments": {"keychain": "gg", "mode": "normal"}
    }))
    .expect("typed binding explanation command");
    assert_eq!(command.arguments, vec!["gg", "--mode", "normal"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "binding-list",
            "arguments": {"mode": "invalid"}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "binding-explain",
            "arguments": {}
        }))
        .is_err()
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "learning-mode",
        "arguments": {"state": "toggle"}
    }))
    .expect("typed learning mode command");
    assert_eq!(command.arguments, vec!["toggle"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "learning-mode",
            "arguments": {"state": "invalid"}
        }))
        .is_err()
    );
}

#[test]
fn learning_mode_requests_are_per_window_state_transitions() {
    let command = ParsedCommand {
        name: "learning-mode".into(),
        arguments: vec!["toggle".into()],
    };
    assert_eq!(learning_mode_request(&command, false).unwrap(), Some(true));
    assert_eq!(learning_mode_request(&command, true).unwrap(), Some(false));
    assert_eq!(
        learning_mode_request(
            &ParsedCommand {
                name: "learning-mode".into(),
                arguments: Vec::new(),
            },
            false,
        )
        .unwrap(),
        None
    );
    assert!(
        learning_mode_request(
            &ParsedCommand {
                name: "learning-mode".into(),
                arguments: vec!["invalid".into()],
            },
            false,
        )
        .is_err()
    );
}

#[test]
fn ipc_spawn_commands_accept_one_userscript_name() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "spawn",
        "arguments": {"userscript": "video"}
    }))
    .expect("typed userscript command");
    assert_eq!(command.arguments, vec!["--userscript", "video"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "spawn",
            "arguments": {"argv": ["/bin/true"], "userscript": "video"}
        }))
        .is_err()
    );
}

#[test]
fn copy_actions_map_to_yank_and_preserve_clean_intent() {
    let (command, action_id) = parse_action_invocation(
        &parse_chain("action url clean-copy", ParseInput::Interactive)
            .expect("clean-copy action")
            .remove(0),
    )
    .expect("mapped clean-copy action");
    assert_eq!(action_id, "browser.url.clean-copy");
    assert_eq!(command.arguments, vec!["url", "--clean"]);

    let (command, action_id) = parse_action_invocation(
        &parse_chain(
            "action link copy https://example.test/?utm_source=demo",
            ParseInput::Interactive,
        )
        .expect("link copy action")
        .remove(0),
    )
    .expect("mapped link copy action");
    assert_eq!(action_id, "browser.link.copy");
    assert_eq!(
        command.arguments,
        vec!["url", "https://example.test/?utm_source=demo"]
    );

    let (command, action_id) = parse_action_invocation(
        &parse_chain("action selection copy", ParseInput::Interactive)
            .expect("selection copy action")
            .remove(0),
    )
    .expect("mapped selection copy action");
    assert_eq!(action_id, "browser.selection.copy");
    assert_eq!(command.arguments, vec!["selection"]);
}

#[test]
fn action_url_inputs_use_the_shared_untrusted_argument_bound() {
    let oversized = "x".repeat(MAX_UNTRUSTED_ARGUMENT_BYTES + 1);
    for arguments in [
        vec!["url".to_owned(), "open".to_owned(), oversized.clone()],
        vec!["link".to_owned(), "open".to_owned(), oversized.clone()],
        vec!["url".to_owned(), "clean-copy".to_owned(), oversized.clone()],
        vec!["link".to_owned(), "copy".to_owned(), oversized.clone()],
        vec!["link".to_owned(), "download".to_owned(), oversized.clone()],
        vec!["url".to_owned(), "clean".to_owned(), oversized.clone()],
        vec!["url".to_owned(), "explain".to_owned(), oversized.clone()],
        vec![
            "link".to_owned(),
            "send".to_owned(),
            "--to".to_owned(),
            oversized.clone(),
        ],
        vec![
            "url".to_owned(),
            "send".to_owned(),
            "--to".to_owned(),
            oversized.clone(),
        ],
        vec![
            "tab".to_owned(),
            "send".to_owned(),
            "--to".to_owned(),
            oversized.clone(),
        ],
        vec![
            "selection".to_owned(),
            "send".to_owned(),
            "--to".to_owned(),
            oversized.clone(),
        ],
    ] {
        assert!(
            parse_action_invocation(&ParsedCommand {
                name: "action".into(),
                arguments,
            })
            .is_err()
        );
    }

    let mapped = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec![
            "url".into(),
            "open".into(),
            "https://example.test/?utm_source=demo".into(),
        ],
    })
    .expect("bounded URL action");
    assert_eq!(
        mapped.0.arguments,
        vec!["https://example.test/?utm_source=demo".to_owned()]
    );
}

#[test]
fn tab_open_action_maps_foreground_and_background_inputs() {
    for (arguments, expected) in [
        (
            vec!["tab", "open", "https://example.test"],
            vec!["https://example.test".to_owned()],
        ),
        (
            vec!["tab", "open", "--background", "https://example.test"],
            vec!["--background".to_owned(), "https://example.test".to_owned()],
        ),
    ] {
        let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
            name: "action".into(),
            arguments: arguments.into_iter().map(str::to_owned).collect(),
        })
        .expect("tab-open action mapping");
        assert_eq!(action_id, "browser.tab.open");
        assert_eq!(mapped.name, "tab-open");
        assert_eq!(mapped.arguments, expected);
    }

    let (typed, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.open",
        "arguments": {"input": "https://example.test", "background": true}
    }))
    .expect("typed background tab-open action");
    assert_eq!(action_id, "browser.tab.open");
    assert_eq!(typed.name, "tab-open");
    assert_eq!(
        typed.arguments,
        vec!["--background", "https://example.test"]
    );
}

#[test]
fn window_new_action_maps_profile_and_private_options() {
    for (arguments, expected) in [
        (vec!["window", "new"], Vec::<String>::new()),
        (
            vec!["window", "new", "--profile", "work", "--private"],
            vec!["--profile".into(), "work".into(), "--private".into()],
        ),
        (
            vec!["window", "new", "--private", "--profile", "work"],
            vec!["--private".into(), "--profile".into(), "work".into()],
        ),
    ] {
        let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
            name: "action".into(),
            arguments: arguments.into_iter().map(str::to_owned).collect(),
        })
        .expect("window-new action mapping");
        assert_eq!(action_id, "browser.window.new");
        assert_eq!(mapped.name, "window-new");
        assert_eq!(mapped.arguments, expected);
    }

    let (typed, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.window.new",
        "arguments": {"profile": "work", "private": true}
    }))
    .expect("typed private window-new action");
    assert_eq!(action_id, "browser.window.new");
    assert_eq!(typed.name, "window-new");
    assert_eq!(typed.arguments, vec!["--profile", "work", "--private"]);
}

#[test]
fn tab_give_action_maps_the_validated_target_window() {
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec!["tab".into(), "give".into(), "window-2".into()],
    })
    .expect("tab-give action mapping");
    assert_eq!(action_id, "browser.tab.give");
    assert_eq!(mapped.name, "tab-give");
    assert_eq!(mapped.arguments, vec!["window-2"]);

    let (typed, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.give",
        "arguments": {"window_id": "window-2"}
    }))
    .expect("typed tab-give action");
    assert_eq!(action_id, "browser.tab.give");
    assert_eq!(typed.name, "tab-give");
    assert_eq!(typed.arguments, vec!["window-2"]);
}

#[test]
fn bookmark_and_quickmark_creation_actions_map_typed_arguments() {
    let (bookmark, bookmark_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec![
            "bookmark".into(),
            "add".into(),
            "--title".into(),
            "Reference".into(),
        ],
    })
    .expect("bookmark add action mapping");
    assert_eq!(bookmark_id, "browser.bookmark.add");
    assert_eq!(bookmark.name, "bookmark-add");
    assert_eq!(bookmark.arguments, vec!["--title", "Reference"]);

    let (quickmark, quickmark_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec![
            "quickmark".into(),
            "add".into(),
            "work".into(),
            "https://work.example".into(),
        ],
    })
    .expect("quickmark add action mapping");
    assert_eq!(quickmark_id, "browser.quickmark.add");
    assert_eq!(quickmark.name, "quickmark-add");
    assert_eq!(quickmark.arguments, vec!["work", "https://work.example"]);

    let (typed, _, typed_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.quickmark.add",
        "arguments": {"name": "work", "url": "https://work.example"}
    }))
    .expect("typed quickmark add action");
    assert_eq!(typed_id, "browser.quickmark.add");
    assert_eq!(typed.name, "quickmark-add");
    assert_eq!(typed.arguments, vec!["work", "https://work.example"]);
}

#[test]
fn session_management_actions_map_typed_arguments() {
    let (saved, saved_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec!["session".into(), "save".into(), "work".into()],
    })
    .expect("session save action mapping");
    assert_eq!(saved_id, "browser.session.save");
    assert_eq!(saved.name, "session-save");
    assert_eq!(saved.arguments, vec!["work"]);

    let (deleted, deleted_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec!["session".into(), "delete".into(), "work".into()],
    })
    .expect("session delete action mapping");
    assert_eq!(deleted_id, "browser.session.delete");
    assert_eq!(deleted.name, "session-delete");
    assert_eq!(deleted.arguments, vec!["work"]);

    let (typed, _, typed_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.session.save",
        "arguments": {"name": "work"}
    }))
    .expect("typed session save action");
    assert_eq!(typed_id, "browser.session.save");
    assert_eq!(typed.name, "session-save");
    assert_eq!(typed.arguments, vec!["work"]);
}

#[test]
fn library_listing_actions_map_without_arguments() {
    for (subject, verb, expected_id, expected_command) in [
        ("bookmark", "list", "browser.bookmark.list", "bookmark-list"),
        (
            "quickmark",
            "list",
            "browser.quickmark.list",
            "quickmark-list",
        ),
        ("session", "list", "browser.session.list", "session-list"),
    ] {
        let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
            name: "action".into(),
            arguments: vec![subject.into(), verb.into()],
        })
        .expect("library list action mapping");
        assert_eq!(action_id, expected_id);
        assert_eq!(mapped.name, expected_command);
        assert!(mapped.arguments.is_empty());
    }

    let (typed, _, typed_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.session.list",
        "arguments": {}
    }))
    .expect("typed session list action");
    assert_eq!(typed_id, "browser.session.list");
    assert_eq!(typed.name, "session-list");
    assert!(typed.arguments.is_empty());
}

#[test]
fn history_clear_action_preserves_bounded_filters_and_confirmation() {
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec![
            "history-entry".into(),
            "clear".into(),
            "--origin".into(),
            "https://example.test".into(),
            "--confirm".into(),
        ],
    })
    .expect("history clear action mapping");
    assert_eq!(action_id, "browser.history-entry.clear");
    assert_eq!(mapped.name, "history-clear");
    assert_eq!(
        mapped.arguments,
        vec!["--origin", "https://example.test", "--confirm"]
    );

    let (typed, _, typed_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.history-entry.clear",
        "arguments": {"since": 123, "confirmed": true}
    }))
    .expect("typed history clear action");
    assert_eq!(typed_id, "browser.history-entry.clear");
    assert_eq!(typed.name, "history-clear");
    assert_eq!(typed.arguments, vec!["--since", "123", "--confirm"]);
}

#[test]
fn ui_action_values_use_the_shared_typed_argument_boundary() {
    assert_eq!(
        ui_action_arguments("browser.url.copy", "").unwrap(),
        serde_json::json!({})
    );
    assert_eq!(
        ui_action_arguments("browser.url.open", "https://example.test").unwrap(),
        serde_json::json!({"input": "https://example.test"})
    );
    assert_eq!(
        ui_action_arguments("browser.link.send", "mpv\thttps://example.test").unwrap(),
        serde_json::json!({"target": "mpv", "url": "https://example.test"})
    );
    assert_eq!(
        ui_action_arguments("browser.selection.send", "mpv").unwrap(),
        serde_json::json!({"target": "mpv"})
    );
    assert_eq!(
        ui_action_arguments("browser.bookmark.add", "Reference").unwrap(),
        serde_json::json!({"title": "Reference"})
    );
    assert_eq!(
        ui_action_arguments("browser.bookmark.edit", "bookmark-1\tEdited").unwrap(),
        serde_json::json!({"id": "bookmark-1", "title": "Edited"})
    );
    assert_eq!(
        ui_action_arguments("browser.quickmark.add", "work\thttps://work.example").unwrap(),
        serde_json::json!({"name": "work", "url": "https://work.example"})
    );
    assert_eq!(
        ui_action_arguments("browser.tab.mute", "tab-1\toff").unwrap(),
        serde_json::json!({"id": "tab-1", "state": "off"})
    );
    assert_eq!(
        ui_action_arguments("browser.tab.focus", "tab-1").unwrap(),
        serde_json::json!({"id": "tab-1"})
    );
    assert!(ui_action_arguments("browser.tab.move", "tab-1").is_err());
    assert!(ui_action_arguments("browser.bookmark.edit", "tab-1\t").is_err());
}

#[test]
fn ipc_url_output_removes_credentials_auth_secrets_and_fragments() {
    assert_eq!(
        safe_ipc_url("https://user:password@example.test/path?keep=1&token=secret#private"),
        "https://example.test/path?keep=1"
    );
    for key in [
        "refresh_token",
        "client_secret",
        "credential",
        "jwt",
        "%61ccess_token",
        "access%5Ftoken",
    ] {
        let url = format!("https://example.test/path?keep=1&{key}=secret");
        assert_eq!(safe_ipc_url(&url), "https://example.test/path?keep=1");
    }
    assert_eq!(
        safe_ipc_url("https://example.test/path?access%ZZtoken=secret&keep=1"),
        "https://example.test/path?keep=1"
    );
    assert_eq!(
        redact_process_stderr_token("refresh_token=secret"),
        "refresh_token=[redacted]"
    );
}

#[test]
fn display_url_redacts_credentials_secrets_and_directional_controls() {
    assert_eq!(
        display_url("https://user:password@example.test/path?keep=1&token=secret#private"),
        "https://example.test\u{2068}/path?keep=1\u{2069}"
    );
    assert_eq!(
        display_url("https://example.test/a\u{202e}b\u{0007}"),
        "https://example.test\u{2068}/a[bidi]b�\u{2069}"
    );
    assert_eq!(
        display_url("https://раypal.example/path"),
        "https://xn--ypal-43d9g.example\u{2068}/path\u{2069}"
    );
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("function addressPresentation(value, maxCharacters)"));
    assert!(qml.contains("var origin = safe.slice(0, authorityEnd)"));
    assert!(qml.contains("var separator = tail.charAt(0) === \"/\" ? \"/…\" : \"…\""));
    assert!(qml.contains("Accessible.description: browserUi.display_url"));
    assert!(qml.contains("Accessible.description: secondaryUi.display_url"));
    assert!(qml.contains("addressEditing = activeFocus"));
    assert!(display_url("https://example.test/path").contains('\u{2068}'));
    assert!(display_url("https://example.test/path").contains('\u{2069}'));
}

#[test]
fn site_origin_is_normalized_and_never_keeps_path_or_credentials() {
    assert_eq!(
        safe_site_origin("HTTPS://user:password@Example.test:443/private?token=secret"),
        Some("https://example.test".into())
    );
    assert_eq!(safe_site_origin("file:///tmp/private"), None);
    assert_eq!(
        safe_site_origin("https://[::1]:443/path"),
        Some("https://[::1]".into())
    );
}

#[test]
fn qt_url_conversion_is_strict_before_rust_policy_canonicalization() {
    assert_eq!(
        canonical_engine_url(QString::from(
            "HTTPS://Example.TEST.:443/a%2Fb?q=%2F#frag%23",
        ))
        .expect("canonical Qt URL"),
        "https://example.test/a%2Fb?q=%2F#frag%23"
    );
    assert_eq!(
        canonical_engine_url(QString::from(
            "https://login.example.test/return?next=https%3A%2F%2Fapp.example.test%2F#state",
        ))
        .expect("redirect URL"),
        "https://login.example.test/return?next=https%3A%2F%2Fapp.example.test%2F#state"
    );
    assert!(canonical_engine_url(QString::from("https://example.test/a b")).is_err());
    assert!(canonical_engine_url(QString::from("javascript:alert(1)")).is_err());
    assert!(canonical_engine_url(QString::from("rb://settings")).is_err());
}

#[test]
fn page_titles_are_sanitized_and_bounded_before_storage() {
    let title = sanitize_untrusted_title(&format!(
        "line\n{}\u{202e}tail",
        "x".repeat(MAX_PAGE_TITLE_BYTES)
    ));
    assert!(title.len() <= MAX_PAGE_TITLE_BYTES);
    assert!(!title.chars().any(char::is_control));
    assert!(!title.contains('\u{202e}'));
}

#[test]
fn page_authority_boundary_has_no_web_channel_or_privileged_page_object() {
    let qml = include_str!("../qml/Main.qml");
    assert!(!qml.contains("WebChannel"));
    assert!(!qml.contains("contextProperty"));
    assert!(qml.contains("WebEngineScript.ApplicationWorld"));
    assert!(qml.contains("runJavaScript"));
    assert!(qml.contains("page_focus_observed_for"));
}

#[test]
fn external_navigation_has_a_native_confirmation_boundary() {
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("external_navigation_visible"));
    assert!(qml.contains("confirm_external_navigation"));
    assert!(qml.contains("cancel_external_navigation"));
    assert!(qml.contains("external-open\\t"));
    assert!(qml.contains("function externalUriAllowed(uri)"));
    assert!(qml.contains("External URI rejected by scheme policy"));
    assert!(qml.contains("return /^file:\\/\\/\\/[^/]/i.test(value)"));
    assert!(qml.contains("Qt.openUrlExternally"));
}

#[test]
fn navigation_failure_surface_keeps_safe_context_without_retry_action() {
    let qml = [
        include_str!("../qml/Main.qml"),
        include_str!("../qml/components/FerricNavigationFailure.qml"),
    ]
    .concat();
    assert!(qml.contains("navigation_failure_requested_url"));
    assert!(qml.contains("navigation_failure_url"));
    assert!(qml.contains("navigation_failure_kind"));
    assert!(qml.contains("navigation_failed_with_details"));
    assert!(qml.contains("Retry is intentionally not offered"));
}

#[test]
fn switcher_surface_exposes_selection_state_and_scaled_keyboard_navigation() {
    let qml = [
        include_str!("../qml/Main.qml"),
        include_str!("../qml/components/FerricSwitcher.qml"),
        include_str!("../qml/components/FerricSwitcherResults.qml"),
    ]
    .concat();
    assert!(qml.contains("Accessible.role: Accessible.List"));
    assert!(qml.contains("Accessible.role: Accessible.ListItem"));
    assert!(qml.contains("Accessible.selected: index === resultList.currentIndex"));
    assert!(qml.contains("Qt.Key_PageDown"));
    assert!(qml.contains("Qt.Key_PageUp"));
    assert!(qml.contains("browserWindow.chromeRowHeight"));
}

#[test]
fn accessibility_surface_reports_theme_contrast_and_reduced_motion() {
    let source = include_str!("lib.rs");
    let preferences = include_str!("chrome_preferences.rs");
    let qml = [
        include_str!("../qml/Main.qml"),
        include_str!("../qml/components/FerricDiagnostics.qml"),
    ]
    .concat();
    assert!(qml.contains("theme_background_color"));
    assert!(qml.contains("theme_selection_text_color"));
    assert!(qml.contains("theme_contrast_status"));
    assert!(qml.contains("theme_contrast_reason"));
    assert!(!qml.contains("theme_palette_json"));
    assert!(!qml.contains("theme_contrast_json"));
    assert!(qml.contains("themeContrastWarning"));
    assert!(qml.contains("renderedContrastReport"));
    assert!(qml.contains("renderedThemeContrastStatus"));
    assert!(qml.contains("system_reduced_motion_status"));
    assert!(qml.contains("system_reduced_motion_enabled"));
    assert!(qml.contains("system_font_scale_status"));
    assert!(qml.contains("system_font_scale"));
    assert!(qml.contains("browserUi.system_reduced_motion_status === \"available\""));
    assert!(source.contains("chrome_reduced_motion"));
    assert!(preferences.contains("struct ChromePreferences"));
    assert!(qml.contains("browserUi.chrome_reduced_motion === \"on\""));
    assert!(qml.contains("window.chromeFontFamily = browserUi.chrome_font_family"));
    assert!(qml.contains("Optional interface motion is disabled"));
}

#[test]
fn scalar_feature_preferences_cross_the_qml_boundary_without_config_parsing() {
    let source = include_str!("lib.rs");
    let preferences = include_str!("feature_preferences.rs");
    let qml = include_str!("../qml/Main.qml");
    assert!(source.contains("feature_switcher_max_results"));
    assert!(source.contains("update_feature_preferences"));
    assert!(preferences.contains("struct FeaturePreferences"));
    assert!(qml.contains("return browserUi.feature_switcher_max_results"));
    assert!(qml.contains("return browserUi.feature_downloads_ask_destination"));
    assert!(qml.contains("feature_desktop_notifications_enabled"));
    assert!(qml.contains("feature_desktop_media_keys_enabled"));
    assert!(qml.contains("feature_push_service_enabled"));
    assert!(qml.contains("feature_blocking_update_interval_hours"));
    assert!(qml.contains("feature_link_cleaning_update_source"));
    assert!(qml.contains("feature_blocking_list_ids"));
}

#[test]
fn normal_input_uses_logical_unmodified_text_and_preserves_unicode_fields() {
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("function logicalNormalKeyText(event)"));
    assert!(qml.contains("event.key === Qt.Key_D"));
    assert!(qml.contains("event.key === Qt.Key_U"));
    assert!(qml.contains("return \"Ctrl+d\""));
    assert!(qml.contains("return \"Ctrl+u\""));
    assert!(qml.contains("return \"Ctrl+p\""));
    assert!(qml.contains("return \"Ctrl+Space\""));
    assert!(qml.contains("return \"Ctrl+v\""));
    assert!(qml.contains("return \"Ctrl+Shift+t\""));
    assert!(qml.contains("return \"Alt+m\""));
    assert!(qml.contains("return \"Ctrl+Alt+p\""));
    assert!(qml.contains("return \"Ctrl+PgDown\""));
    assert!(qml.contains("return \"Ctrl+F5\""));
    assert!(qml.contains("return \"F11\""));
    assert!(qml.contains("browserUi.binding_overlay.length > 0"));
    assert!(qml.contains("event.modifiers & Qt.AltModifier"));
    assert!(qml.contains("event.modifiers & Qt.MetaModifier"));
    assert!(qml.contains("event.text || \"\""));
    assert!(qml.contains("ui.handle_key(logicalText)"));
    assert!(qml.contains("function handleBrowserKey(ui, host, event)"));
    assert!(qml.matches("BrowserKeyRouter {").count() >= 2);
    assert!(qml.contains("browserKeyRouter.acceptCurrentEvent()"));
    assert!(qml.contains("secondaryKeyRouter.acceptCurrentEvent()"));
    assert!(qml.contains("readonly property bool browserChromeInputActive:"));
    assert!(qml.contains("browserUi.external_navigation_visible"));
    assert!(qml.contains("window.pendingContextMenuRequest !== null"));
    assert!(qml.contains("browserUi.mode === \"insert\""));
    assert!(qml.contains("browserUi.mode === \"pass-through\""));
    assert!(qml.contains("? window.modeFocusReturnTarget : window.activeWebView()"));
    assert!(!qml.contains("globalKeyHandler"));
    assert!(!qml.contains("window.handleBrowserKey(viewUi, viewHost, event)"));
    assert!(qml.contains("Accessible.role: Accessible.EditableText"));
}

#[test]
fn browser_key_router_filters_before_webengine_and_requires_explicit_acceptance() {
    let header = include_str!("browser_key_router.h");
    let source = include_str!("browser_key_router.cpp");
    assert!(header.contains("QML_NAMED_ELEMENT(BrowserKeyRouter)"));
    assert!(header.contains("Q_PROPERTY(QWindow *targetWindow"));
    assert!(header.contains("Q_INVOKABLE void acceptCurrentEvent()"));
    assert!(source.contains("application->installEventFilter(this)"));
    assert!(source.contains("event->type() == QEvent::KeyPress"));
    assert!(source.contains("event->type() == QEvent::ShortcutOverride"));
    assert!(source.contains("QGuiApplication::focusWindow() != targetWindow_"));
    assert!(source.contains("emit keyPressed("));
    assert!(source.contains("return handled;"));
}

#[test]
fn scaling_uses_qt_logical_units_and_bounds_large_font_overlays() {
    let qml = include_str!("../qml/Main.qml");
    let downloads = include_str!("../qml/components/FerricDownloadManager.qml");
    assert!(qml.contains("Screen.devicePixelRatio"));
    assert!(qml.contains("Screen.logicalPixelDensity"));
    assert!(qml.contains("readonly property real chromeScale"));
    assert!(downloads.contains("Math.min(760 * browserWindow.chromeScale"));
    assert!(downloads.contains("parent.height - 32"));
    assert!(qml.contains("anchors.bottom: parent.bottom"));
    assert!(qml.contains("double-scale high-DPI"));
}

#[test]
fn activation_uses_qt_request_and_reports_unknown_compositor_outcomes() {
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("hostWindow.requestActivate()"));
    assert!(qml.contains("activationOutcomeTimer"));
    assert!(qml.contains("Window activation outcome unknown"));
    assert!(qml.contains("hostWindow.active"));
    assert!(!qml.contains("xdotool"));
    assert!(!qml.contains("wmctrl"));
}

#[test]
fn authentication_and_client_certificates_stay_in_native_engine_prompts() {
    let qml = [
        include_str!("../qml/Main.qml"),
        include_str!("../qml/components/FerricPageDialog.qml"),
        include_str!("../qml/components/FerricCertificatePrompts.qml"),
    ]
    .concat();
    assert!(qml.contains("request.proxyHost"));
    assert!(qml.contains("HTTP authentication"));
    assert!(qml.contains("Proxy authentication"));
    assert!(qml.contains("authentication ? \"authentication\" : \"page-dialog\""));
    assert!(qml.contains("\"dialogAccept\", [username, password]"));
    assert!(qml.contains("pageDialogPopup.passwordText = \"\""));
    assert!(qml.contains("onSelectClientCertificate"));
    assert!(qml.contains("selection.certificates"));
    assert!(qml.contains("\"client-certificate\", \"select\", [index]"));
    assert!(qml.contains("\"client-certificate\", \"selectNone\", []"));
    assert!(qml.contains("Private keys are never exposed here."));
    assert!(!qml.contains("console.log(username"));
    assert!(!qml.contains("console.log(password"));
}

#[test]
fn qml_qt_request_resolution_is_centralized_and_bounded() {
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("function resolveQtRequest(ui, request, kind, method, args)"));
    assert!(qml.contains("resolved.length > 256"));
    assert!(qml.contains("record_request_resolution"));
    assert!(!qml.contains("request.dialogReject()"));
    assert!(!qml.contains("request.dialogAccept("));
    assert!(!qml.contains("permissionRequest.deny()"));
}

#[test]
fn extracted_prompt_components_render_state_and_emit_intents_without_engine_policy() {
    let main = include_str!("../qml/Main.qml");
    let certificate = include_str!("../qml/components/FerricCertificatePrompts.qml");
    let webauth = include_str!("../qml/components/FerricWebAuthPrompt.qml");
    let context_menu = include_str!("../qml/components/FerricContextMenu.qml");
    let userscript_removal = include_str!("../qml/components/FerricUserscriptRemovalDialog.qml");

    assert!(main.contains("FerricCertificatePrompts"));
    assert!(main.contains("FerricWebAuthPrompt"));
    assert!(main.contains("FerricContextMenu"));
    assert!(main.contains("FerricUserscriptRemovalDialog"));
    assert!(certificate.contains("signal clientCertificateAccepted(int index)"));
    assert!(certificate.contains("signal certificateAccepted()"));
    assert!(webauth.contains("signal pinSubmitted()"));
    assert!(webauth.contains("signal retryRequested()"));
    assert!(context_menu.contains("signal itemActivated(var item)"));
    assert!(context_menu.contains("signal dismissed()"));
    assert!(userscript_removal.contains("signal confirmed()"));
    for component in [certificate, webauth, context_menu, userscript_removal] {
        assert!(!component.contains("browserUi."));
        assert!(!component.contains("resolveQtRequest"));
    }
    assert!(!userscript_removal.contains("remove_userscript"));
}

#[test]
fn account_flows_keep_tabs_and_popups_on_the_opener_profile() {
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("profile: viewProfile"));
    assert!(qml.contains("property var viewProfile: secondaryWindow.windowWebEngineProfile"));
    assert!(qml.contains("popupProfile: viewProfile"));
    assert!(qml.contains("viewProfile: secondaryWindow.windowWebEngineProfile"));
    assert!(qml.contains("popupPrivateProfile: viewTransientProfile"));
    assert!(qml.contains("viewTransientProfile: secondaryWindow.windowTransientProfile"));
    assert!(qml.contains("storageName: window.temporaryProfile ? \"\""));
    assert!(qml.contains("storageName: secondaryWindow.windowTransientProfile ? \"\""));
    assert!(!qml.contains("new WebEngineProfile"));
}

#[test]
fn context_routes_validate_before_assigning_window_membership() {
    let source = include_str!("lib.rs");
    let command_start = source
        .find("fn execute_ipc_command(")
        .expect("IPC command executor exists");
    let command_source = &source[command_start..];
    let assign = command_source
        .find("self.as_mut().assign_ipc_context(route)?;")
        .expect("context assignment exists");
    let validate = command_source
        .find("self.as_ref().validate_ipc_route(route)?;")
        .expect("route validation exists");
    assert!(validate < assign);
}

#[test]
fn context_routes_are_pre_navigation_and_browser_confirmed() {
    let source = [include_str!("lib.rs"), include_str!("tab_presentation.rs")].concat();
    let qml = [
        include_str!("../qml/Main.qml"),
        include_str!("../qml/components/FerricContextRouteDialog.qml"),
    ]
    .concat();
    assert!(source.contains("matching_context_routes"));
    assert!(source.contains("queue_context_route_for_input"));
    assert!(source.contains("execute_context_route_command"));
    assert!(source.contains("save_contexts_atomic"));
    assert!(source.contains("fn navigate_initial"));
    assert!(source.contains("NavigationSource::ExplicitUrl"));
    assert!(source.contains("fn accept_context_route"));
    assert!(source.contains("fn dismiss_context_route"));
    assert!(source.contains("fn sync_context_metadata"));
    assert!(source.contains("context_workspace"));
    assert!(qml.contains("context_route_id"));
    assert!(!qml.contains("context_route_json"));
    assert!(qml.contains("Context route confirmation"));
    assert!(qml.contains("contextStatus"));
    assert!(qml.contains("contextStatusColor"));
    assert!(qml.contains("function routeContextWorkspace(ui)"));
    assert!(qml.contains("entry.host.active !== true"));
    assert!(qml.contains("Workspace routing pending until the browser window is active"));
    assert!(qml.contains("pendingWorkspaceRouteHost"));
    assert!(qml.contains("onContext_workspaceChanged"));
    assert!(qml.contains(
            "Existing redirects, popups, forms, permissions, and authentication chains are never moved automatically."
        ));
    assert!(qml.contains("windowStartupContext"));
}

#[test]
fn context_move_choices_are_a_typed_qml_projection() {
    let source = include_str!("lib.rs");
    let qml = include_str!("../qml/Main.qml");

    assert!(source.contains("fn set_contexts_configuration"));
    assert!(source.contains("set_context_choice_names"));
    assert!(source.contains("set_context_choice_labels"));
    assert!(source.contains("set_context_choice_profiles"));
    assert!(qml.contains("context_choice_names"));
    assert!(qml.contains("set_contexts_configuration("));
    assert!(!qml.contains("JSON.parse(browserUi.contexts_json"));
}

#[test]
fn live_window_registry_decoder_requires_complete_typed_rows() {
    let ids = vec!["window-1".into()];
    let owners = vec!["owner-1".into()];
    let profiles = vec!["default".into()];
    let private = vec!["false".into()];
    let ephemeral = vec!["true".into()];
    let tabs = vec!["2".into()];

    let decoded =
        decode_live_window_registry(&ids, &owners, &profiles, &private, &ephemeral, &tabs)
            .expect("complete typed registry row");
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].id, "window-1");
    assert!(decoded[0].ephemeral);
    assert_eq!(decoded[0].tab_count, 2);

    assert!(
        decode_live_window_registry(
            &ids,
            &owners,
            &profiles,
            &["not-a-bool".into()],
            &ephemeral,
            &tabs,
        )
        .is_none()
    );
    assert!(
        decode_live_window_registry(
            &ids,
            &owners,
            &profiles,
            &private,
            &ephemeral,
            &["1000001".into()],
        )
        .is_none()
    );
    assert!(
        decode_live_window_registry(&ids, &owners, &profiles, &private, &ephemeral, &[],).is_none()
    );
}

#[test]
fn live_window_registry_never_uses_qml_json() {
    let source = include_str!("window_registry.rs");
    let qml = include_str!("../qml/Main.qml");

    assert!(source.contains("fn publish_live_window_registry"));
    assert!(source.contains("decode_live_window_registry"));
    assert!(qml.contains("publish_live_window_registry("));
    assert!(!source.contains("window_registry_json"));
    assert!(!qml.contains("window_registry_json"));
}

#[test]
fn qml_json_contract_allowlist_is_explicit() {
    let qml = include_str!("../qml/Main.qml");
    let allowlist = include_str!("../../../docs/architecture/qml-json-contracts.md");

    assert!(allowlist.contains("Opaque page-script request/result contracts"));
    assert!(allowlist.contains("Prohibited presentation payloads"));
    assert_eq!(qml.matches("JSON.parse(").count(), 10);
    assert_eq!(qml.matches("JSON.stringify(").count(), 20);
}

#[test]
fn qml_submits_runtime_intents_instead_of_writing_runtime_state() {
    let qml = include_str!("../qml/Main.qml");
    let bridge_references = [
        "browserUi.",
        "secondaryUi.",
        "viewUi.",
        "ownerWindow.browserUi.",
    ];

    for line in qml.lines().filter(|line| line.contains(" = ")) {
        if bridge_references
            .iter()
            .any(|reference| line.trim_start().starts_with(reference))
        {
            // Status narration is transient presentation output from native
            // callbacks. Every application decision must instead be sent
            // through a named bridge request or a registered command.
            assert!(
                line.contains(".status_text ="),
                "QML directly writes runtime state: {line}"
            );
        }
    }
}

#[test]
fn download_desktop_actions_cross_the_qml_boundary_as_a_typed_uri() {
    let source = include_str!("lib.rs");
    let qml = include_str!("../qml/Main.qml");

    assert!(source.contains("#[qproperty(QString, download_desktop_uri)]"));
    assert!(source.contains("fn resolve_download_desktop_uri("));
    assert!(source.contains(
        "fn download_desktop_action(self: Pin<&mut BrowserUi>, id: &QString, reveal: bool) -> bool;"
    ));
    assert!(qml.contains("if (!browserUi.download_desktop_action(id, reveal))"));
    assert!(qml.contains("window.openExternalUri(browserUi, browserUi.download_desktop_uri)"));
}

#[test]
fn download_activation_requests_cross_the_qml_boundary_as_typed_fields() {
    let source = include_str!("lib.rs");
    let qml = include_str!("../qml/Main.qml");

    assert!(source.contains("#[qproperty(QString, download_request_token)]"));
    assert!(source.contains("#[qproperty(QString, download_request_url)]"));
    assert!(source.contains("fn take_download_request(self: Pin<&mut BrowserUi>) -> bool;"));
    assert!(qml.contains("ui.download_request_url"));
    assert!(qml.contains("browserUi.download_request_token"));
    assert!(!qml.contains("JSON.parse(downloadRequest)"));
}

#[test]
fn caret_requests_cross_the_qml_boundary_as_typed_fields() {
    let source = include_str!("lib.rs");
    let qml = include_str!("../qml/Main.qml");

    assert!(source.contains("#[qproperty(QString, caret_request_token)]"));
    assert!(source.contains("#[qproperty(QString, caret_request_operation)]"));
    assert!(source.contains("fn take_caret_request(self: Pin<&mut BrowserUi>) -> bool;"));
    assert!(qml.contains("var caretToken = browserUi.caret_request_token"));
    assert!(qml.contains("window.caretScript(caretOperation, window.caretSelecting)"));
    assert!(!qml.contains("JSON.parse(caretRequest)"));
}

#[test]
fn editor_completion_records_cross_the_qml_boundary_as_typed_fields() {
    let source = include_str!("lib.rs");
    let qml = include_str!("../qml/Main.qml");

    assert!(source.contains("#[qproperty(QString, editor_completion_token)]"));
    assert!(source.contains("#[qproperty(QString, editor_completion_stderr)]"));
    assert!(source.contains("fn take_editor_completion(self: Pin<&mut BrowserUi>) -> bool;"));
    assert!(qml.contains("var editorToken = browserUi.editor_completion_token"));
    assert!(qml.contains("window.editorApplyScript(editorOriginal, editorUpdated)"));
    assert!(!qml.contains("JSON.parse(editorCompletion)"));
}

#[test]
fn jseval_requests_cross_the_qml_boundary_as_typed_fields() {
    let source = include_str!("lib.rs");
    let qml = include_str!("../qml/Main.qml");

    assert!(source.contains("#[qproperty(QString, jseval_tab_id)]"));
    assert!(source.contains("set_jseval_script"));
    assert!(qml.contains("if (action === \"jseval\")"));
    assert!(!qml.contains("evalPayload = JSON.parse("));
    assert!(!qml.contains("secondaryEvalPayload = JSON.parse("));
}

#[test]
fn blocking_host_lists_cross_the_qml_boundary_as_string_lists() {
    let source = include_str!("lib.rs");
    let qml = include_str!("../qml/Main.qml");

    for property in [
        "blocking_hosts",
        "blocking_exceptions",
        "blocking_bypass_sites",
        "blocking_security_deny_hosts",
    ] {
        assert!(source.contains(&format!("#[qproperty(QStringList, {property})]")));
        assert!(!qml.contains(&format!("JSON.parse(browserUi.{property})")));
        assert!(!qml.contains(&format!("JSON.parse(secondaryUi.{property})")));
    }
    assert!(qml.contains("blockedHosts: browserUi.blocking_hosts"));
    assert!(qml.contains("blockedHosts: secondaryUi.blocking_hosts"));
    assert!(!qml.contains("JSON.parse(browserUi.blocking_rule_"));
    assert!(!qml.contains("JSON.parse(secondaryUi.blocking_rule_"));
    assert!(qml.contains("blockedRuleHosts: browserUi.blocking_rule_hosts"));
    assert!(qml.contains("blockedRuleHosts: secondaryUi.blocking_rule_hosts"));
    for property in [
        "blocking_cosmetic_rule_hosts",
        "blocking_cosmetic_rule_selectors",
        "blocking_cosmetic_exception_hosts",
        "blocking_cosmetic_exception_selectors",
    ] {
        assert!(source.contains(&format!("#[qproperty(QStringList, {property})]")));
        assert!(qml.contains(property));
    }
    assert!(!qml.contains("JSON.parse(ui.blocking_cosmetic_"));
}

#[test]
fn userscript_inventory_crosses_the_qml_boundary_as_typed_columns() {
    let projection = include_str!("userscript_presentation.rs");
    let qml = include_str!("../qml/Main.qml");

    for property in [
        "userscript_names",
        "userscript_enabled_values",
        "userscript_page_world_values",
        "userscript_action_counts",
    ] {
        assert!(projection.contains(&format!("set_{property}")));
        assert!(qml.contains(&format!("browserUi.{property}")));
    }
    assert!(!qml.contains("JSON.parse(browserUi.userscript_inventory"));
    assert!(qml.contains("names.length !== enabled.length"));
}

#[test]
fn userscript_actions_cross_the_qml_boundary_as_typed_columns() {
    let projection = include_str!("userscript_presentation.rs");
    let qml = include_str!("../qml/Main.qml");

    assert!(projection.contains("fn select_userscript_action_subject"));
    for property in [
        "userscript_action_ids",
        "userscript_action_labels",
        "userscript_action_availability",
    ] {
        assert!(projection.contains(&format!("set_{property}")));
        assert!(qml.contains(property));
    }
    assert!(!qml.contains("JSON.parse(browserUi.userscript_actions"));
    assert!(!qml.contains("ui.userscript_actions(subject)"));
}

#[test]
fn page_userscript_metadata_crosses_the_qml_boundary_as_typed_columns() {
    let source = include_str!("lib.rs");
    let qml = include_str!("../qml/Main.qml");

    assert!(source.contains("fn select_matching_page_scripts"));
    for property in [
        "page_userscript_names",
        "page_userscript_sources",
        "page_userscript_run_at",
        "page_userscript_runs_on_sub_frames",
    ] {
        assert!(source.contains(&format!("set_{property}")));
        assert!(qml.contains(property));
    }
    assert!(!qml.contains("JSON.parse(ui.matching_page_scripts("));
    assert!(!qml.contains("ui.matching_page_scripts(url, privateProfile)"));
}

#[test]
fn site_rules_cross_the_qml_boundary_as_typed_properties() {
    let source = include_str!("lib.rs");
    let qml = include_str!("../qml/Main.qml");

    assert!(source.contains("fn select_site_rule_settings"));
    for property in [
        "site_rule_javascript_set",
        "site_rule_images_set",
        "site_rule_force_dark_set",
        "site_rule_autoplay_set",
        "site_rule_zoom_set",
    ] {
        assert!(source.contains(&format!("set_{property}")));
        assert!(qml.contains(property));
    }
    assert!(!qml.contains("JSON.parse(ui.site_rule_settings(url))"));
}

#[test]
fn external_actions_cross_the_qml_boundary_as_typed_columns() {
    let source = include_str!("lib.rs");
    let qml = include_str!("../qml/Main.qml");

    assert!(source.contains("fn select_external_action_subject"));
    for property in [
        "external_action_ids",
        "external_action_labels",
        "external_action_availability",
    ] {
        assert!(source.contains(&format!("set_{property}")));
        assert!(qml.contains(property));
    }
    assert!(!qml.contains("JSON.parse(raw || \"[]\")"));
    assert!(!qml.contains("ui.external_action_values(subject)"));
}

#[test]
fn journey_graph_edges_cross_the_qml_boundary_as_typed_columns() {
    let projection = include_str!("library_presentation.rs");
    let source = include_str!("lib.rs");
    let production_source = source
        .split("\n#[cfg(test)]\nmod tests")
        .next()
        .expect("production source precedes the test module");
    let qml = include_str!("../qml/Main.qml");

    for property in [
        "library_graph_edge_sources",
        "library_graph_edge_targets",
        "library_graph_edge_transitions",
    ] {
        assert!(projection.contains(&format!("set_{property}")));
        assert!(production_source.contains(&format!("#[qproperty(QStringList, {property})]")));
        assert!(qml.contains(&format!("browserUi.{property}")));
    }
    assert!(!production_source.contains("library_graph_values"));
    assert!(!qml.contains("JSON.parse(browserUi.library_graph_values)"));
}

#[test]
fn blocking_policy_projection_is_separate_from_the_qt_bootstrap() {
    let projection = include_str!("blocking_presentation.rs");
    let bootstrap = include_str!("lib.rs");
    let production_bootstrap = bootstrap
        .split("\n#[cfg(test)]\nmod tests")
        .next()
        .expect("production source precedes the test module");

    assert!(projection.contains("fn apply_network_policy_snapshot"));
    assert!(projection.contains("fn reload_blocking_policy"));
    assert!(projection.contains("fn toggle_blocking_site"));
    assert!(!production_bootstrap.contains("fn apply_network_policy_snapshot"));
}

#[test]
fn blocking_evidence_uses_bounded_native_rows_not_qml_json() {
    let qml = include_str!("../qml/Main.qml");
    let projection = include_str!("blocking_evidence.rs");
    let native_header = include_str!("request_interceptor.h");

    assert!(qml.contains("set_blocking_active_evidence("));
    assert!(qml.contains("publish_blocking_live_counts("));
    assert!(qml.contains("blockedRequestExplanationFields(activeHost)"));
    assert!(!qml.contains("blocking_active_explanation = JSON.stringify"));
    assert!(!qml.contains("blocking_active_decisions = JSON.stringify"));
    assert!(!qml.contains("browserUi.blocking_blocked_count ="));
    assert!(!qml.contains("browserUi.blocking_unknown_context_count ="));
    assert!(projection.contains("const EVIDENCE_FIELD_COUNT: usize = 13"));
    assert!(projection.contains("const MAX_DECISIONS: usize = 100"));
    assert!(native_header.contains("blockedRequestDecisionFields"));
}

#[test]
fn desktop_multiple_url_launches_open_separate_startup_tabs() {
    let qml = include_str!("../qml/Main.qml");
    let executable = include_str!("../../ferric-browser/src/main.rs");
    assert!(qml.contains("property var startupAdditionalUrls: []"));
    assert!(qml.contains("property bool startupBackground: false"));
    assert!(qml.contains("function openAdditionalStartupUrls()"));
    assert!(qml.contains("var urls = window.startupAdditionalUrls || []"));
    assert!(executable.contains("QString::from(\"startupAdditionalUrls\")"));
    assert!(executable.contains("collect::<QStringList>()"));
    assert!(!qml.contains("startupAdditionalUrlsJson"));
    assert!(qml.contains("browserUi.new_tab()"));
    assert!(qml.contains("browserUi.navigate_initial(url, \"external-open\", true)"));
    assert!(qml.contains("Qt.callLater(window.openAdditionalStartupUrls)"));
    assert!(qml.contains("if (window.startupBackground)"));
}

#[test]
fn tab_projection_changes_use_qt_property_setters() {
    let source = include_str!("tab_presentation.rs");
    let direct_tab_count_assignment = ["this.", "tab_count", " ="].concat();
    let direct_active_index_assignment = ["this.", "active_tab_index", " ="].concat();

    assert!(!source.contains(&direct_tab_count_assignment));
    assert!(!source.contains(&direct_active_index_assignment));
    assert!(source.contains("self.as_mut().set_tab_count(tab_count)"));
    assert!(source.contains("self.as_mut().set_active_tab_properties(active_index, tab)"));
}

#[test]
fn status_surfaces_report_bounded_navigation_and_activity_state() {
    let qml = include_str!("../qml/Main.qml");
    let status_bar = include_str!("../qml/components/FerricStatusBar.qml");
    let window_status_bar = include_str!("../qml/components/FerricWindowStatusBar.qml");
    assert!(qml.contains("function statusTransport(view)"));
    assert!(qml.contains("HTTPS transport (site trust separate)"));
    assert!(qml.contains("HTTP transport (not secure)"));
    assert!(qml.contains("function statusLoad(view)"));
    assert!(qml.contains("function statusMedia(view)"));
    assert!(qml.contains("function statusPermission(ui)"));
    assert!(qml.contains("function statusCapture(hostWindow)"));
    assert!(qml.contains("function statusDownloads(hostWindow)"));
    assert!(qml.contains("function statusTabBlocking(view)"));
    assert!(qml.contains("function statusDetails(ui, hostWindow, view"));
    assert!(qml.contains("browserUi, window, window.activeWebView()"));
    assert!(qml.contains("secondaryUi, secondaryWindow,"));
    assert!(qml.contains("secondaryWindow.activeView,"));
    assert!(qml.matches("window.statusDetails(").count() >= 3);
    assert!(qml.contains("siteDoctorBadge(ui)"));
    assert!(qml.contains("FerricStatusBar {"));
    assert!(status_bar.contains("required property string accessibleDetails"));
    assert!(status_bar.contains("function activityLabel()"));
    assert!(status_bar.contains("id: statusUrl"));
    assert!(!status_bar.contains("browserUi."));
    assert!(qml.contains("FerricWindowStatusBar {"));
    assert!(window_status_bar.contains("required property string accessibleDetails"));
    assert!(window_status_bar.contains("id: modeLabel"));
    assert!(!window_status_bar.contains("browserUi."));
}

#[test]
fn site_doctor_presentation_uses_typed_properties_not_qml_json() {
    let source = include_str!("lib.rs");
    let qml = include_str!("../qml/Main.qml");
    let production_source = source
        .split("\n#[cfg(test)]\nmod tests")
        .next()
        .expect("production source precedes the test module");

    assert!(production_source.contains("fn publish_site_experiment_presentation"));
    assert!(production_source.contains("set_site_experiment_active"));
    assert!(production_source.contains("set_site_experiment_remaining_seconds"));
    assert!(qml.contains("site_experiment_active"));
    assert!(qml.contains("site_experiment_kind"));
    assert!(!production_source.contains("site_experiment_json"));
    assert!(!qml.contains("site_experiment_json"));
}

#[test]
fn default_chrome_is_compact_modal_and_content_first() {
    let qml = include_str!("../qml/Main.qml");
    let status_bar = include_str!("../qml/components/FerricStatusBar.qml");
    let command_line = include_str!("../qml/components/FerricCommandLine.qml");
    let chrome_presentation = include_str!("../qml/scripts/ChromePresentation.js");
    assert!(qml.contains("height: window.tabPosition === \"top\" && window.tabStripVisible"));
    assert!(qml.contains("visible: window.tabStripVisible"));
    assert!(qml.contains("text: (tabIndex + 1) + \"  \""));
    assert!(status_bar.contains("text: statusBar.mode.toUpperCase()"));
    assert!(status_bar.contains("id: statusUrl"));
    assert!(command_line.contains("id: commandPrefix"));
    assert!(command_line.contains("text: \":\""));
    assert!(command_line.contains("background: Rectangle { color: \"transparent\" }"));
    assert!(
        qml.contains(
            "header: ToolBar {\n                height: 0\n                visible: false"
        )
    );
    assert!(qml.contains("palette.buttonText: window.primaryTextColor"));
    assert!(qml.contains("ChromePresentation.contrastReport"));
    assert!(qml.contains("function readableTextColor(candidate, background)"));
    assert!(qml.contains("window.contrastText(parent.color)"));
    assert!(chrome_presentation.contains("function colorChannels(value)"));
    assert!(chrome_presentation.contains("function contrastReport(colors)"));
}

#[test]
fn presentation_helper_resources_have_no_runtime_or_page_authority() {
    let helpers = [
        include_str!("../qml/scripts/ChromePresentation.js"),
        include_str!("../qml/scripts/SpellcheckPresentation.js"),
    ];
    for helper in helpers {
        for forbidden in [
            "browserUi",
            "execute_command",
            "execute_ui_action",
            "runJavaScript",
            "WebEngine",
            "XMLHttpRequest",
        ] {
            assert!(
                !helper.contains(forbidden),
                "presentation helper must not gain authority: {forbidden}"
            );
        }
    }
}

#[test]
fn context_entry_restores_safe_descriptors_lazily_and_focuses_live_members() {
    let source = include_str!("lib.rs");
    let qml = include_str!("../qml/Main.qml");
    assert!(source.contains("fn restore_context_membership"));
    assert!(source.contains("focused_member"));
    assert!(source.contains("restored_tab_descriptors"));
    assert!(source.contains("Runtime tab IDs are deliberately not reused"));
    assert!(source.contains("selected_effects = Some(effects)"));
    assert!(source.contains("context_entry_force_reuse"));
    assert!(source.contains("context-window\\tfalse\\t"));
    assert!(source.contains("let live_member_window = self"));
    assert!(source.contains("window-focus\\t{member_window}"));
    assert!(qml.contains("function applyRestorePayload(payload, hasModeLine)"));
    assert!(qml.contains("tabIndex === browserUi.active_tab_index"));
    assert!(qml.contains("fields.length > 4 && fields[4].length > 0"));
    assert!(qml.contains("function focusExistingContextWindow(commandText, sourceUi)"));
    assert!(qml.contains("windowStartupContextRestore: true"));
    assert!(qml.contains("secondaryUi.set_context_entry_reuse("));
    assert!(qml.contains("browserUi.set_context_entry_reuse(true)"));
    assert!(!qml.contains("context_entry_force_reuse ="));
}

#[test]
fn popup_windows_capture_opener_context_identity() {
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("property string popupContextName"));
    assert!(qml.contains("property string popupContextLabel"));
    assert!(qml.contains("property string popupJourneyToken"));
    assert!(qml.contains("take_popup_journey_token"));
    assert!(qml.contains("popup_navigation_started"));
    assert!(qml.contains("popup_navigation_committed"));
    assert!(qml.contains("release_popup_journey_token"));
    assert!(qml.contains("popupContextName: viewUi.context_name"));
    assert!(qml.contains("property string popupProfileName"));
    assert!(qml.contains("popupWindow.popupProfileName"));
    assert!(qml.contains("popupWindow.popupEphemeralProfile"));
    assert!(
        qml.contains("popupView,\n                              popupWindow.popupPrivateProfile")
    );
    assert!(qml.contains("property var viewUi: secondaryUi"));
    assert!(qml.contains("Ferric Browser popup · context"));
    assert!(qml.contains("popupPermissionUi: viewUi"));
}

#[test]
fn journey_export_requires_preview_and_explicit_local_destination() {
    let source = include_str!("lib.rs");
    let qml = include_str!("../qml/Main.qml");
    let manager = include_str!("../qml/components/FerricLibraryManager.qml");
    let dialogs = include_str!("../qml/components/FerricFileDialogSurfaces.qml");
    assert!(source.contains("fn journey_export_preview"));
    assert!(source.contains("memory-only"));
    assert!(source.contains("refuses to overwrite an existing file"));
    assert!(source.contains("atomic_write_private(path_ref"));
    assert!(qml.contains("journey_export_preview"));
    assert!(qml.contains("property bool journeyExportPreviewVisible"));
    assert!(qml.contains("property bool journeyExportAwaiting"));
    assert!(source.contains("journey_export_preview_text"));
    assert!(qml.contains("fileDialogSurfaces.openJourneyExport()"));
    assert!(dialogs.contains("title: \"Choose journey export destination\""));
    assert!(manager.contains("Export journey records"));
}

#[test]
fn diagnostics_export_requires_explicit_local_save_and_never_overwrites() {
    let source = include_str!("lib.rs");
    let qml = [
        include_str!("../qml/Main.qml"),
        include_str!("../qml/components/FerricDiagnostics.qml"),
    ]
    .concat();
    let dialogs = include_str!("../qml/components/FerricFileDialogSurfaces.qml");
    assert!(source.contains("fn export_diagnostics"));
    assert!(source.contains("Diagnostics export requires reviewing the visible preview first"));
    assert!(source.contains("diagnostics_preview_ready = false"));
    assert!(source.contains("diagnostics_preview_payload = Some(serialized.clone())"));
    assert!(source.contains("MAX_DIAGNOSTICS_EXPORT_BYTES"));
    assert!(source.contains("Diagnostics export requires an absolute local file path"));
    assert!(source.contains("Diagnostics export refuses to overwrite an existing file"));
    assert!(source.contains("atomic_write_private(path_ref, payload.as_bytes())"));
    assert!(qml.contains("fileDialogSurfaces.openDiagnosticsExport()"));
    assert!(dialogs.contains("title: \"Choose diagnostics export destination\""));
    assert!(qml.contains("Save diagnostics preview"));
    assert!(qml.contains("browserUi.export_diagnostics(path)"));
}

#[test]
fn required_desktop_portals_gate_file_selection_asynchronously() {
    let source = include_str!("lib.rs");
    let portals = include_str!("desktop_portals.rs");
    let qml = include_str!("../qml/Main.qml");
    assert!(source.contains("fn portal_capability_status"));
    assert!(source.contains("portal_capabilities"));
    assert!(portals.contains("struct PortalCapabilities"));
    assert!(source.contains("portal_probe_worker"));
    assert!(qml.contains("ui.desktop_portal_mode"));
    assert!(!qml.contains("config.desktop.portals"));
    assert!(qml.contains("probe_desktop_portals()"));
    assert!(qml.contains("browserUi.portal_capability_status(capability)"));
    assert!(!qml.contains("desktop_portal_status"));
    assert!(qml.contains("desktopPortalCapabilityStatus(requestUi, \"file_chooser\")"));
    assert!(qml.contains("pendingFileDialogWaitingForPortal"));
    assert!(qml.contains("pendingFileDialogWaitingForPortal = true"));
    assert!(qml.contains("pendingFileDialogPortalDeadlineMs"));
    assert!(qml.contains("Desktop portal check timed out; file selection cancelled"));
    assert!(qml.contains("pendingDesktopMediaPortalDeadlineMs"));
    assert!(qml.contains("popupFilePortalTimer"));
    assert!(qml.contains("function maybeOpenPendingFileDialog()"));
    assert!(qml.contains("popupWindow.maybeOpenPendingFileDialog()"));
    assert!(qml.contains("secondaryWindow.maybeOpenPendingFileDialog()"));
    assert!(qml.contains("secondaryWindow.pendingFileDialogWaitingForPortal = true"));
    assert!(qml.contains("secondaryWindow.pendingFileDialogPortalDeadlineMs"));
    assert!(qml.contains("Required desktop portal unavailable; file selection cancelled"));
    assert!(qml.contains("window.openPendingEngineFileDialog()"));
    assert!(qml.contains("desktopPortalCapabilityStatus(ui, \"screen_cast\")"));
    assert!(qml.contains("Required ScreenCast portal unavailable; screen sharing cancelled"));
    assert!(qml.contains("maybeOpenPendingDesktopMediaRequest"));
    assert!(qml.contains("desktopPortalCapabilityStatus(targetUi, \"open_uri\")"));
    assert!(qml.contains("pendingExternalUris"));
    assert!(qml.contains("processPendingExternalUris"));
    assert!(qml.contains("Desktop portal check timed out; external action cancelled"));
    assert!(qml.contains("Required OpenURI portal unavailable; external action cancelled"));
    assert!(qml.contains("function openExternalUri(ui, uri)"));
    assert!(qml.contains("desktopPortalCapabilityStatus(ui, \"notifications\")"));
    assert!(qml.contains("pendingPortalNotifications"));
    assert!(qml.contains("deferNotificationUntilPortal"));
    assert!(qml.contains("processPendingPortalNotifications"));
    assert!(qml.contains("closeWebNotificationsForOrigin"));
    assert!(qml.contains("pendingOrigin"));
    assert!(qml.contains("Desktop portal check timed out; notification cancelled"));
    assert!(qml.contains("Required Notification portal unavailable; notification cancelled"));
}

#[test]
fn security_deny_hosts_are_separate_from_adblock_bypasses() {
    let qml = include_str!("../qml/Main.qml");
    let interceptor = include_str!("request_interceptor.cpp");
    assert!(qml.contains("securityDenyHosts"));
    assert!(qml.contains("security_deny_hosts"));
    assert!(interceptor.contains("securityDenyHosts"));
    assert!(interceptor.contains("security deny rule"));
    assert!(include_str!("lib.rs").contains("blocking.security-deny-host"));
    assert!(include_str!("lib.rs").contains("security_deny_rules"));
    let config = serde_json::to_value(Config::default()).expect("default config serializes");
    assert_eq!(
        config["blocking"]["security_deny_hosts"],
        serde_json::json!([])
    );
}

#[test]
fn request_interceptor_uses_bounded_engine_context_without_ui_fallback() {
    let interceptor = include_str!("request_interceptor.cpp");
    let header = include_str!("request_interceptor.h");
    let qml = include_str!("../qml/Main.qml");
    assert!(header.contains("std::shared_ptr<const PolicySnapshot> policy_"));
    assert!(interceptor.contains("info.requestUrl()"));
    assert!(interceptor.contains("info.firstPartyUrl()"));
    assert!(interceptor.contains("info.initiator()"));
    assert!(interceptor.contains("info.resourceType()"));
    assert!(interceptor.contains("info.navigationType()"));
    assert!(interceptor.contains("NavigationTypeRedirect"));
    assert!(interceptor.contains("contextKnown"));
    assert!(interceptor.contains("blockedRuleHosts"));
    assert!(interceptor.contains("blockedRuleListIds"));
    assert!(interceptor.contains("exceptionRuleHosts"));
    assert!(interceptor.contains("exceptionRuleListIds"));
    assert!(interceptor.contains("ferric_browser_adblock_check"));
    assert!(interceptor.contains("adblockResourceType"));
    assert!(header.contains("adblockEngineHandle"));
    assert!(qml.contains("blocking_adblock_handle"));
    assert!(interceptor.contains("list_id"));
    assert!(interceptor.contains("exception_list_id"));
    assert!(qml.contains("blocking_rule_hosts"));
    assert!(qml.contains("blocking_rule_list_ids"));
    assert!(qml.contains("blocking_exception_rule_hosts"));
    assert!(qml.contains("blocking_exception_rule_list_ids"));
    assert!(interceptor.contains("normalized.size() > 253"));
    assert!(!interceptor.contains("activeWebView"));
    assert!(interceptor.contains("scheduleEvidenceChanged()"));
    assert!(interceptor.contains("evidenceSignalPending_.exchange"));
    assert!(interceptor.contains("QMetaObject::invokeMethod"));
    assert!(interceptor.contains("Qt::QueuedConnection"));
}

#[test]
fn renderer_termination_is_reduced_before_qml_recovery_surface() {
    let source = include_str!("renderer_lifecycle.rs");
    let qml = include_str!("../qml/Main.qml");
    assert!(source.contains("fn note_renderer_process_terminated"));
    assert!(source.contains("fn prepare_renderer_recovery"));
    assert!(source.contains("Event::RendererTerminated { target }"));
    assert!(qml.contains("ui.note_renderer_process_terminated(tabIndex)"));
    assert!(qml.contains("ui.prepare_renderer_recovery"));
    assert!(qml.contains("onRenderProcessTerminated"));
}

#[test]
fn blocking_status_counts_security_deny_rules_without_exposing_hosts() {
    let config = serde_json::json!({
        "blocking": {
            "security_deny_hosts": ["one.example", "*.two.example"]
        }
    });
    let serialized = serde_json::to_string(&config).expect("config serializes");
    assert_eq!(security_deny_rule_count(&serialized), 2);
    assert_eq!(security_deny_rule_count("{\"blocking\":{}}"), 0);
    assert_eq!(security_deny_rule_count("not json"), 0);
}

#[test]
fn background_link_navigation_captures_the_source_journey_node() {
    let source = include_str!("lib.rs");
    assert!(source.contains("journey_parent: Option<JourneyNodeId>"));
    assert!(source.contains("let journey_parent = source_tab"));
    assert!(source.contains("self.as_mut().mark_journey_parent(&effects, parent)"));
}

#[test]
fn ephemeral_profile_uses_off_the_record_memory_only_setup() {
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("property bool ephemeralProfile"));
    assert!(qml.contains("WebEngineProfilePrototype {"));
    assert!(qml.contains("storageName: window.temporaryProfile ? \"\""));
    assert!(qml.contains("persistentStoragePath: window.temporaryProfile"));
    assert!(qml.contains("cachePath: window.temporaryProfile"));
    assert!(qml.contains("window.browserProfile = browserProfilePrototype.instance()"));
    assert!(qml.contains("title: window.ephemeralProfile"));
    assert!(qml.contains("? \"Ferric Browser · \" + window.profileLabel"));
    assert!(qml.contains("window.temporaryProfile && !window.ephemeralProfile"));
    assert!(qml.contains("window.ephemeralProfile,"));
    let source = include_str!("lib.rs");
    assert!(source.contains("PrivacyKind::Ephemeral"));
    assert!(source.contains("active_profile_is_transient"));
    assert!(source.contains("release_transient_resources"));
    assert!(source.contains("pending_profile_configuration = None"));
    assert!(source.contains("request_profile_open(ProfileOpenRequest"));
    assert!(!source.contains("ProfileSetupWorker"));
    assert!(source.contains("network_policy_worker = None"));
    assert!(source.contains("set_blocking_rule_hosts(QStringList::default())"));
    assert!(source.contains("set_blocking_rule_list_ids(QStringList::default())"));
    assert!(source.contains("set_blocking_exception_rule_hosts(QStringList::default())"));
    assert!(source.contains("set_blocking_exception_rule_list_ids(QStringList::default())"));
    assert!(source.contains("Durable profile resources remain owned"));
    assert!(source.contains("this.session_permissions.clear()"));
    assert!(!source.contains("this.profile_lock ="));
    assert!(source.contains("cannot save durable sessions"));
    assert!(source.contains("cannot use durable contexts"));
    assert!(source.contains("private or ephemeral switcher activation is disabled"));
    assert!(source.contains("Created a fresh ephemeral profile window; it is not durable"));
}

#[test]
fn ephemeral_hint_target_creates_a_typed_new_window_action() {
    let source = include_str!("lib.rs");
    let qml = include_str!("../qml/Main.qml");
    assert!(source.contains("target == \"ephemeral\""));
    assert!(source.contains("ephemeral-window\\t{}"));
    assert!(qml.contains("function openEphemeralWindow(url, requestedToken)"));
    assert!(qml.contains("function ephemeralProfileForToken(token)"));
    assert!(qml.contains("property var ephemeralProfileOwners"));
    assert!(qml.contains("function retainEphemeralProfileOwner(profile, token)"));
    assert!(qml.contains("function releaseEphemeralProfileOwner(profile, token)"));
    assert!(qml.contains("Ephemeral profile owner token is already bound"));
    assert!(qml.contains("ephemeralInvocationToken"));
    assert!(qml.contains("windowSharedProfile: sharedProfile"));
    assert!(qml.contains("property var viewProfile: secondaryWindow.windowWebEngineProfile"));
    assert!(qml.contains("action.indexOf(\"ephemeral-window\\t\") === 0"));
    assert!(qml.contains("window.registerBrowserWindow("));
    assert!(qml.contains("windowEphemeralProfile: true"));
    assert!(qml.contains("result.action === \"ephemeral\""));
    assert!(qml.contains("onClosing: function(close)"));
    assert!(qml.contains("windowShutdownPromptVisible"));
    assert!(qml.contains("secondaryUi.request_shutdown()"));
    assert!(qml.contains("hasActiveShutdownRequestsFor"));
    assert!(qml.contains("windowSharedProfile || secondaryProfile"));
}

#[test]
fn tls_errors_are_blocked_by_default_and_only_allow_scoped_confirmation() {
    let qml = [
        include_str!("../qml/Main.qml"),
        include_str!("../qml/components/FerricCertificatePrompts.qml"),
    ]
    .concat();
    assert!(qml.contains("onCertificateError"));
    assert!(qml.contains("!error.overridable"));
    assert!(qml.contains("!error.isMainFrame"));
    assert!(qml.contains("ui, error, \"client-certificate\", \"rejectCertificate\", []"));
    assert!(qml.contains("ui, error, \"client-certificate\", \"acceptCertificate\", []"));
    assert!(qml.contains("Accept once"));
    assert!(qml.contains("for this request only"));
    assert!(qml.contains("googleCertificateHost"));
    assert!(!qml.contains("ignoreCertificateErrors"));
    assert!(!qml.contains("setIgnoreCertificateErrors"));
}

#[test]
fn webauthn_uses_engine_state_and_clears_pin_input() {
    let qml = [
        include_str!("../qml/Main.qml"),
        include_str!("../qml/components/FerricWebAuthPrompt.qml"),
    ]
    .concat();
    assert!(qml.contains("onWebAuthUxRequested"));
    assert!(qml.contains("WebEngineWebAuthUxRequest.SelectAccount"));
    assert!(qml.contains("WebEngineWebAuthUxRequest.CollectPin"));
    assert!(qml.contains("request.setSelectedAccount"));
    assert!(qml.contains("request.setPin(pin)"));
    assert!(qml.contains("webAuthPrompt.pin = \"\""));
    assert!(qml.contains("property alias pin: webAuthPinField.text"));
    assert!(qml.contains("ui, request, \"webauth\", \"cancel\", []"));
    assert!(qml.contains("request.retry()"));
    assert!(qml.contains("request.relyingPartyId"));
    assert!(!qml.contains("pinRequest.password"));
    assert!(!qml.contains("console.log(pin"));
}

#[test]
fn context_menus_use_engine_actions_and_bound_spellcheck_data() {
    let source = include_str!("lib.rs");
    let qml = [
        include_str!("../qml/Main.qml"),
        include_str!("../qml/components/FerricContextMenu.qml"),
        include_str!("../qml/components/FerricHintOverlay.qml"),
        include_str!("../qml/scripts/SpellcheckPresentation.js"),
    ]
    .concat();
    assert!(qml.contains("onContextMenuRequested"));
    assert!(qml.contains("request.accepted = true"));
    assert!(qml.contains("appendUserscriptActions"));
    assert!(qml.contains("ui.select_userscript_action_subject(subject)"));
    assert!(qml.contains("userscript_action_availability"));
    assert!(qml.contains("function showHintActions(label)"));
    assert!(qml.contains("hint-userscript-action"));
    assert!(qml.contains("select_hint_action"));
    assert!(qml.contains("Qt.RightButton"));
    assert!(source.contains("execute_registered_userscript_action_for_hint"));
    assert!(source.contains("if hint_activation"));
    assert!(qml.contains("view.triggerWebAction(webAction)"));
    assert!(qml.contains("replaceMisspelledWord"));
    assert!(qml.contains("spellCheckerSuggestions"));
    assert!(qml.contains("Download link"));
    assert!(qml.contains("browser.link.open"));
    assert!(qml.contains("browser.link.download"));
    assert!(qml.contains("browser.download.pause"));
    assert!(qml.contains("browser.download.resume"));
    assert!(qml.contains("browser.download.cancel"));
    assert!(qml.contains("browser.download.retry"));
    assert!(qml.contains("browser.selection.copy"));
    assert!(qml.contains("browser.selection.search"));
    assert!(qml.contains("browser.tab.pin"));
    assert!(qml.contains("browser.tab.mute"));
    assert!(qml.contains("browser.tab.undo"));
    assert!(qml.contains(":tab-clone"));
    assert!(qml.contains("function scrollScript"));
    assert!(qml.contains("function searchFindFlags(query, caseMode, backward)"));
    assert!(qml.contains("var characters = Array.from(text)"));
    assert!(qml.contains("character.toUpperCase()"));
    assert!(qml.contains("scroll-page\\t"));
    assert!(qml.contains("scroll-to\\t"));
    assert!(qml.contains("tab_id_for_index"));
    assert!(qml.contains("execute_ui_action"));
    assert!(qml.contains("Inspect element"));
    assert!(qml.contains("spellCheckLanguages"));
    assert!(qml.contains("function languageIsValid(language)"));
    assert!(qml.contains("i-klingon"));
    assert!(qml.contains("zh-min-nan"));
    assert!(qml.contains("(?:[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*|x(?:-[A-Za-z0-9]{1,8})+)"));
    assert!(qml.contains("function safeContextUrl(value)"));
    assert!(qml.contains("decodeURIComponent(key)"));
    assert!(qml.contains("private[_-]?key"));
    assert!(qml.contains("client[_-]?secret"));
    assert!(!qml.contains("Qt.openUrlExternally(link"));
}

#[test]
fn browser_owned_script_resource_has_a_versioned_bounded_contract() {
    let qml = include_str!("../qml/Main.qml");
    let script = include_str!("../qml/scripts/BrowserScripts.js");
    assert!(qml.contains("import \"scripts/BrowserScripts.js\" as BrowserScripts"));
    assert!(qml.contains("return BrowserScripts.scroll(kind, direction, half, count)"));
    assert!(qml.contains("BrowserScripts.scrollPosition()"));
    assert!(qml.contains("BrowserScripts.restoreScrollPosition(x, y)"));
    assert!(qml.contains("return BrowserScripts.selection()"));
    assert!(qml.contains("return BrowserScripts.editor()"));
    assert!(qml.contains("return BrowserScripts.editorApply(original, updated)"));
    assert!(qml.contains("return BrowserScripts.caret(operation, selecting)"));
    assert!(qml.contains("return BrowserScripts.downloadLink(url)"));
    assert!(qml.contains("return BrowserScripts.clearSiteData()"));
    assert!(qml.contains("return BrowserScripts.focusProbe()"));
    assert!(qml.contains("return BrowserScripts.focusObserverSource()"));
    assert!(qml.contains("return BrowserScripts.shutdownPageProbe()"));
    assert!(qml.contains("return BrowserScripts.hintCollector(linksOnly)"));
    assert!(qml.contains("return BrowserScripts.hintFresh(candidate)"));
    assert!(qml.contains("return BrowserScripts.hintFocus(elementId)"));
    assert!(qml.contains("return BrowserScripts.hintClick(elementId)"));
    assert!(qml.contains("BrowserScripts.pageUserscriptRun(scriptSource)"));
    assert!(qml.contains("BrowserScripts.pageUserscriptInstall(script.source)"));
    assert!(!qml.contains("function hintSelector(linksOnly)"));
    assert!(script.contains("var VERSION = \"4\""));
    assert!(script.contains("function pageUserscriptRun(source)"));
    assert!(script.contains("function pageUserscriptInstall(source)"));
    assert!(!qml.contains("var source = \"(function(){try{\" + scriptSource"));
    assert!(!qml.contains("installedScript.sourceCode = \"(function(){try{\" + script.source"));
    assert!(script.contains("function boundedCount(value)"));
    assert!(script.contains("Math.max(1, Math.min(9999"));
    assert!(script.contains("password fields are not copied"));
    assert!(script.contains("password fields are not editable externally"));
    assert!(script.contains("field changed while editor was open"));
    assert!(script.contains("invalid caret movement"));
    assert!(script.contains("a.rel='noreferrer'"));
    assert!(script.contains("service_workers:'unavailable'"));
    assert!(script.contains("window.__ferric_browserFocusState"));
    assert!(script.contains("elements.length > 128"));
    assert!(script.contains("function hintCollector(linksOnly)"));
    assert!(script.contains("function hintFresh(candidate)"));
    assert!(script.contains("function hintFocus(elementId)"));
    assert!(script.contains("function hintClick(elementId)"));
    assert!(script.contains("function focusObserverSource()"));
    assert!(script.contains("function formStateProbe()"));
    assert!(script.contains("function siteDataClearResult()"));
    assert!(script.contains("function scrollPosition()"));
    assert!(script.contains("function restoreScrollPosition(x, y)"));
    assert!(script.contains("function cosmeticFilter(css)"));
    assert!(script.contains("out.length>=5000"));
    assert!(script.contains("depth>8"));
}

#[test]
fn screen_capture_keeps_scoped_indicator_and_reload_stop_boundary() {
    let qml = include_str!("../qml/Main.qml");
    let indicator = include_str!("../qml/components/FerricCaptureIndicator.qml");
    let source = include_str!("lib.rs");
    assert!(qml.contains("onDesktopMediaRequested"));
    assert!(qml.contains("\"desktop-media\", \"selectScreen\""));
    assert!(qml.contains("\"desktop-media\", \"selectWindow\""));
    assert!(qml.contains("function recordCaptureSession"));
    assert!(qml.contains("browser-owned capture ledger"));
    assert!(qml.contains("activeCapture.status"));
    assert!(qml.contains("FerricCaptureIndicator"));
    assert!(indicator.contains("Capture indicator"));
    assert!(indicator.contains("browserWindow.captureSessions"));
    assert!(indicator.contains("indicator.stopRequested(modelData.id)"));
    assert!(qml.contains("function stopCaptureSession"));
    assert!(qml.contains("target.view.reload()"));
    assert!(qml.contains("clearCaptureSessionsForHost"));
    assert!(!qml.contains("PipeWire"));
    assert!(source.contains("if permission == \"screen-capture\""));
    assert!(source.contains("Screen-capture consent reset; active captures will be stopped"));
}

#[test]
fn site_ledger_exposes_bounded_userscript_metadata_without_sources() {
    let source = include_str!("lib.rs");
    assert!(source.contains("profile-userscript-manifests"));
    assert!(source.contains("matching_active_site"));
}

#[test]
fn site_data_clear_is_bound_to_its_originating_view() {
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("property var siteDataClearView"));
    assert!(qml.contains("var view = window.siteDataClearView"));
    assert!(qml.contains("clearSiteDataClearForView"));
    assert!(qml.contains("site-data clear cancelled by navigation"));
    assert!(qml.contains("window.siteDataClearUi = browserUi"));
}

#[test]
fn notifications_require_consent_and_push_is_explicitly_opt_in() {
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("onPresentNotification"));
    assert!(qml.contains("notification.show()"));
    assert!(qml.contains("notification.close()"));
    assert!(qml.contains("permission_decision(origin, \"notifications\")"));
    assert!(qml.contains("activeWebNotifications"));
    assert!(qml.contains("notification.closed.connect"));
    assert!(qml.contains("previous.close()"));
    assert!(qml.contains("function notificationProfileScope(ui, privateProfile)"));
    assert!(qml.contains("window.notificationProfileScope(ui, privateProfile)"));
    assert!(qml.contains("function closeWebNotificationsForOrigin(origin)"));
    assert!(qml.contains("permissionParts[2] === \"notifications\""));
    assert!(qml.contains("NotificationPresenter"));
    assert!(qml.contains("notificationPresenter.present(notification, profileScope, origin)"));
    assert!(qml.contains("onNotificationUnavailable"));
    assert!(qml.contains("property var notificationOwners"));
    assert!(qml.contains("function notificationOwnerFor(notification)"));
    assert!(qml.contains("window.notificationOwnerFor(notification)"));
    assert!(qml.contains("rememberNotificationOwner(notification, ui)"));
    assert!(qml.contains("function activeWebNotification(notification)"));
    assert!(qml.contains("window.activeWebNotification(notification)"));
    assert!(qml.contains("Desktop notification service unavailable; using Qt fallback"));
    assert!(qml.contains("focusNotificationOrigin(profileScope, origin)"));
    assert!(qml.contains("closeWebNotifications()"));
    assert!(qml.contains("ui.feature_push_service_enabled"));
    assert!(qml.contains("if (privateProfile)"));
    assert!(!qml.contains("Qt.openUrlExternally(notification"));
}

#[test]
fn permission_revocation_reloads_all_registered_matching_views() {
    let qml = include_str!("../qml/Main.qml");
    assert!(qml.contains("function permissionOriginForView(view)"));
    assert!(qml.contains("function reloadViewsForPermission(origin)"));
    assert!(qml.contains("entry.view"));
    assert!(qml.contains("window.removePermissionGroups"));
    assert!(qml.contains("candidates[k].view.reload()"));
    assert!(qml.contains("secondaryWindow, secondaryUi, secondaryWindow.activeView,"));
    assert!(qml.contains("Permission revoked; reloaded "));
}

#[test]
fn media_keys_and_mpris_use_one_engine_toggle_path() {
    let qml = include_str!("../qml/Main.qml");
    let mpris = include_str!("mpris_controller.cpp");
    let mpris_header = include_str!("mpris_controller.h");
    assert!(qml.contains("Qt.Key_MediaTogglePlayPause"));
    assert!(qml.contains("WebEngineView.ToggleMediaPlayPause"));
    assert!(qml.contains("function triggerMediaToggle(view)"));
    assert!(qml.contains("view.recentlyAudible !== true"));
    assert!(qml.contains("!/^https?:\\/\\//i.test(url)"));
    assert!(qml.contains("ui.feature_desktop_media_keys_enabled"));
    assert!(qml.contains("Media play/pause toggle sent to the page"));
    assert!(qml.contains("MprisController"));
    assert!(qml.contains("onMediaToggleRequested"));
    assert!(qml.contains("mprisController.update"));
    assert!(qml.contains("var url = view.url ? view.url.toString() : \"\""));
    assert!(qml.contains("browserUi.current_url"));
    assert!(qml.contains("function updateMprisForPrimaryView(view)"));
    assert!(qml.contains("window.updateMprisForPrimaryView(webView)"));
    assert!(qml.contains("onRecentlyAudibleChanged"));
    assert!(qml.contains("onAudioMutedChanged"));
    assert!(mpris.contains("org.mpris.MediaPlayer2.Player"));
    assert!(mpris.contains("PlayPause"));
    assert!(mpris.contains("org.mpris.MediaPlayer2.ferric-browser.instance"));
    assert!(mpris.contains("privateProfile"));
    assert!(mpris.contains("safeMetadataUrl"));
    assert!(mpris.contains("sensitiveQueryKey"));
    assert!(mpris.contains("access_token"));
    assert!(mpris.contains("refresh_token"));
    assert!(mpris.contains("client_secret"));
    assert!(mpris.contains("setFragment({})"));
    assert!(mpris_header.contains("QML_NAMED_ELEMENT(MprisController)"));
    assert!(!mpris_header.contains("void Stop()"));
    assert!(!qml.contains("Key_MediaNext"));
    assert!(!qml.contains("Key_MediaPrevious"));
}

#[test]
fn file_urls_encode_download_paths_without_leaking_raw_delimiters() {
    assert_eq!(
        path_to_file_url(Path::new("/home/tom/Downloads/a file#1.txt")),
        Ok("file:///home/tom/Downloads/a%20file%231.txt".into())
    );
    assert!(path_to_file_url(Path::new("relative/file.txt")).is_err());
    assert_eq!(
        configured_download_directory_path(&serde_json::json!({
            "downloads": {"directory": {"path": "/tmp/ferric-browser-downloads"}}
        })),
        PathBuf::from("/tmp/ferric-browser-downloads")
    );
    assert_eq!(
        configured_download_directory_path(&serde_json::json!({
            "downloads": {"directory": {"Path": "/tmp/ferric-browser-downloads"}}
        })),
        PathBuf::from("/tmp/ferric-browser-downloads")
    );
}

#[test]
fn xdg_user_dirs_resolve_downloads_without_shell_expansion() {
    let home = Path::new("/home/test-user");
    assert_eq!(
        parse_user_dirs_download("XDG_DOWNLOAD_DIR=\"$HOME/Downloads\"\n", home),
        Some(PathBuf::from("/home/test-user/Downloads"))
    );
    assert_eq!(
        parse_user_dirs_download("XDG_DOWNLOAD_DIR=\"$HOME/Work\\x20Downloads\"\n", home),
        Some(PathBuf::from("/home/test-user/Work Downloads"))
    );
    assert_eq!(
        parse_user_dirs_download("XDG_DOWNLOAD_DIR=\"relative/Downloads\"\n", home),
        None
    );
    assert_eq!(
        parse_user_dirs_download(
            "XDG_DOWNLOAD_DIR=\"$HOME/Downloads; touch /tmp/pwned\"\n",
            home
        ),
        Some(PathBuf::from("/home/test-user/Downloads; touch /tmp/pwned"))
    );
    assert!(parse_user_dirs_download("XDG_DOWNLOAD_DIR=\"$HOME/Down\\qloads\"\n", home).is_none());
}

#[test]
fn ipc_context_commands_use_explicit_typed_fields() {
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "context-create",
        "arguments": {
            "name": "work",
            "label": "Work",
            "profile": "default",
            "workspace": "3"
        }
    }))
    .expect("typed context command");
    assert_eq!(route.open_target, IpcOpenTarget::Tab);
    let (_, cli_route) = typed_ipc_command(&serde_json::json!({
        "command": "back",
        "context": {"source": "cli"}
    }))
    .expect("CLI source metadata");
    assert_eq!(cli_route.source, CommandSource::Cli);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "back",
            "context": {"source": "automation"}
        }))
        .is_err()
    );
    assert_eq!(
        command.arguments,
        vec![
            "work",
            "--label",
            "Work",
            "--profile",
            "default",
            "--workspace",
            "3"
        ]
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "context-create",
            "arguments": {"name": "work"}
        }))
        .is_err()
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "context-delete",
        "arguments": {"name": "work", "confirmed": true}
    }))
    .expect("typed delete command");
    assert_eq!(command.arguments, vec!["work", "--confirm"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "context-delete",
            "arguments": {"name": "work", "confirmed": "yes"}
        }))
        .is_err()
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "context-route",
        "arguments": {
            "action": "add",
            "pattern": "https://*.company.test/*",
            "context": "work"
        }
    }))
    .expect("typed route add command");
    assert_eq!(
        command.arguments,
        vec!["add", "https://*.company.test/*", "work"]
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "context-route",
        "arguments": {
            "action": "add",
            "pattern": "https://*.company.test/*",
            "context": "work",
            "priority": -4,
            "behavior": "suggest",
            "entry_points": ["explicit-open", "typed-initial-url"]
        }
    }))
    .expect("typed configured route add command");
    assert_eq!(
        command.arguments,
        vec![
            "add",
            "https://*.company.test/*",
            "work",
            "--priority",
            "-4",
            "--behavior",
            "suggest",
            "--entry-point",
            "explicit-open",
            "--entry-point",
            "typed-initial-url"
        ]
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "context-route",
            "arguments": {
                "action": "add",
                "pattern": "https://*.company.test/*",
                "context": "work",
                "entry_points": ["address-bar"]
            }
        }))
        .is_err()
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "context-route",
        "arguments": {"action": "remove", "id": "company-work"}
    }))
    .expect("typed route remove command");
    assert_eq!(command.arguments, vec!["remove", "company-work"]);
}

#[test]
fn typed_history_clear_preserves_bounded_filters() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "history-clear",
        "arguments": {
            "since": 1_757_894_400,
            "origin": "HTTPS://Example.Test:443",
            "confirmed": true
        }
    }))
    .expect("typed history clear");
    assert_eq!(
        command.arguments,
        vec![
            "--since",
            "1757894400",
            "--origin",
            "https://example.test",
            "--confirm"
        ]
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "history-clear",
            "arguments": {"since": -1, "confirmed": true}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "history-clear",
            "arguments": {"origin": "https://example.test/path", "confirmed": true}
        }))
        .is_err()
    );
}

#[test]
fn typed_paste_open_preserves_target_and_primary_channel() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "paste-open",
        "arguments": {"target": "tab", "primary": true}
    }))
    .expect("typed paste-open command");
    assert_eq!(command.arguments, vec!["--target", "tab", "--primary"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "paste-open",
            "arguments": {"target": "window"}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "paste-open",
            "arguments": {"primary": "yes"}
        }))
        .is_err()
    );
}

#[test]
fn typed_tab_move_context_allows_active_or_indexed_targets() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "tab-move",
        "arguments": {"context": "work"}
    }))
    .expect("active context move command");
    assert_eq!(command.arguments, vec!["--context", "work"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "tab-move",
        "arguments": {"id": "2", "context": "work"}
    }))
    .expect("indexed context move command");
    assert_eq!(command.arguments, vec!["2", "--context", "work"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "tab-move",
            "arguments": {"context": "work", "direction": "left"}
        }))
        .is_err()
    );
}

#[test]
fn ipc_open_context_routes_preserve_new_window_context_metadata() {
    let (command, route) = typed_ipc_command(&serde_json::json!({
        "command": "open",
        "arguments": {
            "input": "https://work.example.test/",
            "target": "window"
        },
        "context": {"context": "work"}
    }))
    .expect("typed context-routed window open");
    assert_eq!(command.arguments, vec!["https://work.example.test/"]);
    assert_eq!(route.open_target, IpcOpenTarget::Window);
    assert_eq!(route.context.as_deref(), Some("work"));
    assert!(include_str!("../qml/Main.qml").contains("windowStartupContext: windowAction[3]"));
    assert!(include_str!("lib.rs").contains("private-window cannot use a durable context"));
}

#[test]
fn ipc_library_commands_use_typed_fields() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "quickmark-add",
        "arguments": {"name": "work", "url": "https://work.example.test/"}
    }))
    .expect("typed quickmark command");
    assert_eq!(
        command.arguments,
        vec!["work", "https://work.example.test/"]
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "bookmark-add",
        "arguments": {"title": "Example"}
    }))
    .expect("typed bookmark command");
    assert_eq!(command.arguments, vec!["--title", "Example"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "history-clear",
        "arguments": {"confirmed": true}
    }))
    .expect("typed history command");
    assert_eq!(command.arguments, vec!["--confirm"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "journey",
        "arguments": {"current": true}
    }))
    .expect("typed journey command");
    assert_eq!(command.arguments, vec!["--current"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "journey",
        "arguments": {"search": "example.test"}
    }))
    .expect("typed journey search command");
    assert_eq!(command.arguments, vec!["--search", "example.test"]);
    for search in ["", "bad\nsearch", &"x".repeat(257)] {
        assert!(
            typed_ipc_command(&serde_json::json!({
                "command": "journey",
                "arguments": {"search": search}
            }))
            .is_err()
        );
    }
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "journey",
        "arguments": {"expand": "123e4567-e89b-12d3-a456-426614174000"}
    }))
    .expect("typed journey expansion command");
    assert_eq!(
        command.arguments,
        vec!["--expand", "123e4567-e89b-12d3-a456-426614174000"]
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "journey-reopen",
        "arguments": {
            "node": "123e4567-e89b-12d3-a456-426614174000",
            "target": "tab"
        }
    }))
    .expect("typed journey reopen command");
    assert_eq!(
        command.arguments,
        vec!["123e4567-e89b-12d3-a456-426614174000", "--target", "tab"]
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "journey",
            "arguments": {"expand": "not-a-uuid"}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "journey-reopen",
            "arguments": {"node": "not-a-uuid"}
        }))
        .is_err()
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "journey-reopen",
        "arguments": {
            "node": "123e4567-e89b-12d3-a456-426614174000",
            "target": "window"
        }
    }))
    .expect("typed window journey reopen command");
    assert_eq!(command.arguments.last().map(String::as_str), Some("window"));
}

#[test]
fn ipc_link_commands_use_one_optional_typed_url() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "url-explain",
        "arguments": {"url": "https://example.test/?utm_source=demo&keep=1"}
    }))
    .expect("typed URL explanation command");
    assert_eq!(
        command.arguments,
        vec!["https://example.test/?utm_source=demo&keep=1"]
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "url-clean"
    }))
    .expect("current URL cleaning command");
    assert!(command.arguments.is_empty());
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "url-clean",
            "arguments": {"url": "https://example.test", "extra": "reject"}
        }))
        .is_err()
    );
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.url.explain",
        "arguments": {"url": "https://example.test/?utm_source=demo"}
    }))
    .expect("typed namespaced action");
    assert_eq!(action_id, "browser.url.explain");
    assert_eq!(command.name, "url-explain");
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.link.open",
        "arguments": {"url": "https://example.test/link"}
    }))
    .expect("typed link action");
    assert_eq!(action_id, "browser.link.open");
    assert_eq!(command.name, "open");
    assert_eq!(command.arguments, vec!["https://example.test/link"]);
    let (command, route, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.link.open",
        "arguments": {
            "url": "https://example.test/link",
            "target": "tab-bg"
        }
    }))
    .expect("typed background link action");
    assert_eq!(action_id, "browser.link.open");
    assert_eq!(command.name, "open");
    assert_eq!(command.arguments, vec!["https://example.test/link"]);
    assert_eq!(route.open_target, IpcOpenTarget::BackgroundTab);
    for name in [
        "download-open",
        "download-show",
        "download-cancel",
        "download-retry",
    ] {
        let (command, _) = typed_ipc_command(&serde_json::json!({
            "command": name,
            "arguments": {"id": "download-1"}
        }))
        .expect("typed download management command");
        assert_eq!(command.arguments, vec!["download-1"]);
    }
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "session-load",
        "arguments": {"name": "work", "append": true}
    }))
    .expect("typed session load command");
    assert_eq!(command.arguments, vec!["--append", "work"]);
}

#[test]
fn ipc_hint_commands_use_a_bounded_kind_selector() {
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "hint",
        "arguments": {"kind": "links"}
    }))
    .expect("typed hint command");
    assert_eq!(command.arguments, vec!["links"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "hint",
            "arguments": {"kind": "scripts"}
        }))
        .is_err()
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "hint",
        "arguments": {"rapid": true, "target": "yank", "kind": "links"}
    }))
    .expect("typed rapid hint command");
    assert_eq!(
        command.arguments,
        vec!["--rapid", "--target", "yank", "links"]
    );
    for target in ["tab", "window"] {
        let (command, _) = typed_ipc_command(&serde_json::json!({
            "command": "hint",
            "arguments": {"target": target, "kind": "links"}
        }))
        .expect("typed foreground hint target");
        assert_eq!(command.arguments, vec!["--target", target, "links"]);
        assert!(
            typed_ipc_command(&serde_json::json!({
                "command": "hint",
                "arguments": {"target": target, "rapid": true}
            }))
            .is_err()
        );
    }
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "hint",
        "arguments": {"target": "clean-yank", "rapid": true}
    }))
    .expect("typed clean rapid hint command");
    assert_eq!(command.arguments, vec!["--rapid", "--target", "clean-yank"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "hint",
        "arguments": {"target": "download", "rapid": true, "kind": "links"}
    }))
    .expect("typed rapid download hint command");
    assert_eq!(
        command.arguments,
        vec!["--rapid", "--target", "download", "links"]
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "hint",
        "arguments": {"target": "clean-yank"}
    }))
    .expect("typed one-shot clean-yank hint command");
    assert_eq!(command.arguments, vec!["--target", "clean-yank"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "hint",
        "arguments": {"target": "userscript", "script": "video", "kind": "links"}
    }))
    .expect("typed userscript hint command");
    assert_eq!(
        command.arguments,
        vec!["--target", "userscript", "--script", "video", "links"]
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "hint",
        "arguments": {"target": "external:mpv", "kind": "links"}
    }))
    .expect("typed external hint command");
    assert_eq!(command.arguments, vec!["--target", "external:mpv", "links"]);
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "hint",
        "arguments": {
            "rapid": true,
            "target": "userscript",
            "script": "video",
            "kind": "links"
        }
    }))
    .expect("typed rapid userscript hint command");
    assert_eq!(
        command.arguments,
        vec![
            "--rapid",
            "--target",
            "userscript",
            "--script",
            "video",
            "links"
        ]
    );
    let (command, _) = typed_ipc_command(&serde_json::json!({
        "command": "hint",
        "arguments": {"target": "tab-bg"}
    }))
    .expect("typed one-shot background-tab hint command");
    assert_eq!(command.arguments, vec!["--target", "tab-bg"]);
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "hint",
            "arguments": {"rapid": true, "target": "external:mpv"}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_command(&serde_json::json!({
            "command": "hint",
            "arguments": {"target": "external:Bad Name"}
        }))
        .is_err()
    );
    for action in ["clean-yank", "tab-bg", "userscript", "download"] {
        assert!(rapid_hint_keeps_mode(true, Some(action)));
    }
    assert!(rapid_hint_keeps_mode(true, Some("yank")));
    for action in ["navigate", "ephemeral"] {
        assert!(!rapid_hint_keeps_mode(true, Some(action)));
    }
    assert!(!rapid_hint_keeps_mode(false, Some("tab-bg")));
}

#[test]
fn action_command_maps_subject_and_verb_to_a_registered_executor() {
    for (verb, command_name) in [
        ("back", "back"),
        ("forward", "forward"),
        ("reload", "reload"),
        ("stop", "stop"),
    ] {
        let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
            name: "action".into(),
            arguments: vec!["tab".into(), verb.into()],
        })
        .expect("navigation action mapping");
        assert_eq!(action_id, format!("browser.tab.{verb}"));
        assert_eq!(mapped.name, command_name);
        assert!(mapped.arguments.is_empty());
        let (typed, _, typed_id) = typed_ipc_action(&serde_json::json!({
            "action": format!("browser.tab.{verb}"),
            "arguments": {}
        }))
        .expect("typed navigation action");
        assert_eq!(typed_id, format!("browser.tab.{verb}"));
        assert_eq!(typed.name, command_name);
        assert!(typed.arguments.is_empty());
    }
    for (verb, command_name) in [("back", "back"), ("forward", "forward")] {
        let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
            name: "action".into(),
            arguments: vec!["tab".into(), verb.into(), "2".into()],
        })
        .expect("counted history action mapping");
        assert_eq!(action_id, format!("browser.tab.{verb}"));
        assert_eq!(mapped.name, command_name);
        assert_eq!(mapped.arguments, vec!["--count", "2"]);
        let (typed, _, typed_id) = typed_ipc_action(&serde_json::json!({
            "action": format!("browser.tab.{verb}"),
            "arguments": {"count": 2}
        }))
        .expect("typed counted history action");
        assert_eq!(typed_id, format!("browser.tab.{verb}"));
        assert_eq!(typed.name, command_name);
        assert_eq!(typed.arguments, vec!["--count", "2"]);
    }
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec!["tab".into(), "reload".into(), "--bypass-cache".into()],
    })
    .expect("reload bypass action mapping");
    assert_eq!(action_id, "browser.tab.reload");
    assert_eq!(mapped.name, "reload");
    assert_eq!(mapped.arguments, vec!["--bypass-cache"]);
    let (typed, _, typed_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.reload",
        "arguments": {"bypass_cache": true}
    }))
    .expect("typed reload bypass action");
    assert_eq!(typed_id, "browser.tab.reload");
    assert_eq!(typed.name, "reload");
    assert_eq!(typed.arguments, vec!["--bypass-cache"]);
    for (verb, command_name, input, expected) in [
        (
            "scroll",
            "scroll",
            vec!["down", "3"],
            vec!["down", "--count", "3"],
        ),
        (
            "scroll-page",
            "scroll-page",
            vec!["up", "--half", "--count", "2"],
            vec!["up", "--half", "--count", "2"],
        ),
        ("scroll-to", "scroll-to", vec!["bottom"], vec!["bottom"]),
    ] {
        let mut action_arguments = vec!["tab".into(), verb.into()];
        action_arguments.extend(input.iter().map(|value| (*value).to_owned()));
        let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
            name: "action".into(),
            arguments: action_arguments,
        })
        .expect("scroll action mapping");
        assert_eq!(action_id, format!("browser.tab.{verb}"));
        assert_eq!(mapped.name, command_name);
        assert_eq!(mapped.arguments, expected);
    }
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.scroll-page",
        "arguments": {"direction": "down", "half": true, "count": 2}
    }))
    .expect("typed page scroll action");
    assert_eq!(action_id, "browser.tab.scroll-page");
    assert_eq!(command.name, "scroll-page");
    assert_eq!(command.arguments, vec!["down", "--half", "--count", "2"]);
    for (verb, command_name) in [("next", "tab-next"), ("previous", "tab-prev")] {
        let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
            name: "action".into(),
            arguments: vec!["tab".into(), verb.into(), "2".into()],
        })
        .expect("tab traversal action mapping");
        assert_eq!(action_id, format!("browser.tab.{verb}"));
        assert_eq!(mapped.name, command_name);
        assert_eq!(mapped.arguments, vec!["--count", "2"]);
        let (typed, _, typed_id) = typed_ipc_action(&serde_json::json!({
            "action": format!("browser.tab.{verb}"),
            "arguments": {"count": 2}
        }))
        .expect("typed tab traversal action");
        assert_eq!(typed_id, format!("browser.tab.{verb}"));
        assert_eq!(typed.name, command_name);
        assert_eq!(typed.arguments, vec!["--count", "2"]);
    }
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec!["tab".into(), "clone".into()],
    })
    .expect("tab clone action mapping");
    assert_eq!(action_id, "browser.tab.clone");
    assert_eq!(mapped.name, "tab-clone");
    assert!(mapped.arguments.is_empty());
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.window.fullscreen",
        "arguments": {"state": "on"}
    }))
    .expect("typed fullscreen action");
    assert_eq!(action_id, "browser.window.fullscreen");
    assert_eq!(command.name, "fullscreen");
    assert_eq!(command.arguments, vec!["on"]);
    let (mapped, mapped_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec![
            "bookmark".into(),
            "edit".into(),
            "bookmark-1".into(),
            "--title".into(),
            "Edited title".into(),
        ],
    })
    .expect("bookmark edit action mapping");
    assert_eq!(mapped_id, "browser.bookmark.edit");
    assert_eq!(mapped.name, "bookmark-edit");
    assert_eq!(
        mapped.arguments,
        vec!["bookmark-1", "--title", "Edited title"]
    );
    let (typed, _, typed_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.bookmark.edit",
        "arguments": {"id": "bookmark-1", "title": "Edited title"},
    }))
    .expect("typed bookmark edit action");
    assert_eq!(typed_id, "browser.bookmark.edit");
    assert_eq!(typed.name, "bookmark-edit");
    assert_eq!(
        typed.arguments,
        vec!["bookmark-1", "--title", "Edited title"]
    );
    let (mapped, mapped_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec![
            "quickmark".into(),
            "edit".into(),
            "work".into(),
            "https://edited.example/".into(),
        ],
    })
    .expect("quickmark edit action mapping");
    assert_eq!(mapped_id, "browser.quickmark.edit");
    assert_eq!(mapped.name, "quickmark-edit");
    assert_eq!(mapped.arguments, vec!["work", "https://edited.example/"]);
    let (typed, _, typed_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.quickmark.edit",
        "arguments": {"name": "work", "url": "https://edited.example/"},
    }))
    .expect("typed quickmark edit action");
    assert_eq!(typed_id, "browser.quickmark.edit");
    assert_eq!(typed.name, "quickmark-edit");
    assert_eq!(typed.arguments, vec!["work", "https://edited.example/"]);
    let command = ParsedCommand {
        name: "action".into(),
        arguments: vec!["url".into(), "open".into(), "https://example.test".into()],
    };
    let (mapped, action_id) = parse_action_invocation(&command).expect("action mapping");
    assert_eq!(action_id, "browser.url.open");
    assert_eq!(mapped.name, "open");
    assert_eq!(mapped.arguments, vec!["https://example.test"]);
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec![
            "link".into(),
            "open".into(),
            "--target".into(),
            "tab-bg".into(),
            "https://example.test/docs".into(),
        ],
    })
    .expect("link background action mapping");
    assert_eq!(action_id, "browser.link.open");
    assert_eq!(mapped.name, "open");
    assert_eq!(
        mapped.arguments,
        vec!["--target", "tab-bg", "https://example.test/docs"]
    );
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec![
            "url".into(),
            "open".into(),
            "--target".into(),
            "tab-bg".into(),
            "https://example.test".into(),
        ],
    })
    .expect("URL background action mapping");
    assert_eq!(action_id, "browser.url.open");
    assert_eq!(
        mapped.arguments,
        vec!["--target", "tab-bg", "https://example.test"]
    );
    assert!(
        parse_action_invocation(&ParsedCommand {
            name: "action".into(),
            arguments: vec![
                "url".into(),
                "open".into(),
                "--target".into(),
                "not-a-target".into(),
                "https://example.test".into(),
            ],
        })
        .is_err()
    );
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec!["link".into(), "send".into(), "--to".into(), "mpv".into()],
    })
    .expect("external action mapping");
    assert_eq!(action_id, "browser.link.send");
    assert_eq!(mapped.name, "send");
    assert_eq!(mapped.arguments, vec!["mpv"]);
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.link.send",
        "arguments": {"target": "mpv", "url": "https://example.test/docs"}
    }))
    .expect("typed external action");
    assert_eq!(action_id, "browser.link.send");
    assert_eq!(command.name, "send");
    assert_eq!(command.arguments, vec!["mpv", "https://example.test/docs"]);
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.url.send",
        "arguments": {"target": "mpv"}
    }))
    .expect("typed URL target action");
    assert_eq!(action_id, "browser.url.send");
    assert_eq!(command.name, "send");
    assert_eq!(command.arguments, vec!["mpv", "--url"]);
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.send",
        "arguments": {"target": "mpv"}
    }))
    .expect("typed tab target action");
    assert_eq!(action_id, "browser.tab.send");
    assert_eq!(command.name, "send");
    assert_eq!(command.arguments, vec!["mpv", "--tab"]);
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.selection.send",
        "arguments": {"target": "mpv"}
    }))
    .expect("typed selection target action");
    assert_eq!(action_id, "browser.selection.send");
    assert_eq!(command.name, "send");
    assert_eq!(command.arguments, vec!["mpv", "--selection"]);
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "external.mpv.url.send",
        "arguments": {"url": "https://example.test/current"}
    }))
    .expect("typed configured URL action");
    assert_eq!(action_id, "external.mpv.url.send");
    assert_eq!(command.name, "send");
    assert_eq!(
        command.arguments,
        vec!["mpv", "--url", "https://example.test/current"]
    );
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "external.mpv.link.send",
        "arguments": {"target": "mpv", "url": "https://example.test/link"}
    }))
    .expect("typed configured link action");
    assert_eq!(action_id, "external.mpv.link.send");
    assert_eq!(command.arguments, vec!["mpv", "https://example.test/link"]);
    assert!(
        typed_ipc_action(&serde_json::json!({
            "action": "external.mpv.link.send",
            "arguments": {}
        }))
        .is_err()
    );
    assert!(
        typed_ipc_action(&serde_json::json!({
            "action": "external.mpv.tab.send",
            "arguments": {"url": "https://example.test/not-allowed"}
        }))
        .is_err()
    );
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec![
            "selection".into(),
            "search".into(),
            "--engine".into(),
            "ddg".into(),
        ],
    })
    .expect("selection search action mapping");
    assert_eq!(action_id, "browser.selection.search");
    assert_eq!(mapped.name, "selection-search");
    assert_eq!(mapped.arguments, vec!["ddg"]);
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.selection.search",
        "arguments": {"engine": "ddg"}
    }))
    .expect("typed selection search action");
    assert_eq!(action_id, "browser.selection.search");
    assert_eq!(command.name, "selection-search");
    assert_eq!(command.arguments, vec!["ddg"]);
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec![
            "link".into(),
            "download".into(),
            "https://example.test/file.zip".into(),
        ],
    })
    .expect("download action mapping");
    assert_eq!(action_id, "browser.link.download");
    assert_eq!(mapped.name, "download");
    assert_eq!(mapped.arguments, vec!["https://example.test/file.zip"]);
    for (verb, command_name, action_id) in [
        ("open", "download-open", "browser.download.open"),
        ("show", "download-show", "browser.download.show"),
        ("cancel", "download-cancel", "browser.download.cancel"),
        ("pause", "download-pause", "browser.download.pause"),
        ("resume", "download-resume", "browser.download.resume"),
        ("retry", "download-retry", "browser.download.retry"),
    ] {
        let (mapped, mapped_id) = parse_action_invocation(&ParsedCommand {
            name: "action".into(),
            arguments: vec!["download".into(), verb.into(), "download-1".into()],
        })
        .expect("download action mapping");
        assert_eq!(mapped_id, action_id);
        assert_eq!(mapped.name, command_name);
        assert_eq!(mapped.arguments, vec!["download-1"]);
    }
    for (verb, command_name, expected_arguments) in [
        ("focus", "tab-focus", vec!["tabid-42".to_owned()]),
        ("select", "tab-select", vec!["tabid-42".to_owned()]),
        ("close", "tab-close", vec!["tabid-42".to_owned()]),
        ("suspend", "tab-suspend", vec!["tabid-42".to_owned()]),
        ("discard", "tab-discard", vec!["tabid-42".to_owned()]),
        ("resume", "tab-resume", vec!["tabid-42".to_owned()]),
        (
            "move",
            "tab-move",
            vec!["tabid-42".to_owned(), "left".to_owned()],
        ),
        ("mute", "tab-mute", vec!["tabid-42".to_owned()]),
    ] {
        let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
            name: "action".into(),
            arguments: {
                let mut arguments = vec!["tab".into(), verb.into(), "tabid-42".into()];
                if verb == "move" {
                    arguments.push("left".into());
                }
                arguments
            },
        })
        .expect("tab action mapping");
        assert_eq!(action_id, format!("browser.tab.{verb}"));
        assert_eq!(mapped.name, command_name);
        assert_eq!(mapped.arguments, expected_arguments);
    }
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec!["tab".into(), "pin".into(), "tabid-42".into(), "on".into()],
    })
    .expect("tab pin action mapping");
    assert_eq!(action_id, "browser.tab.pin");
    assert_eq!(mapped.name, "tab-pin");
    assert_eq!(mapped.arguments, vec!["tabid-42", "on"]);
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec!["tab".into(), "mute".into(), "tabid-42".into(), "on".into()],
    })
    .expect("tab mute action mapping");
    assert_eq!(action_id, "browser.tab.mute");
    assert_eq!(mapped.name, "tab-mute");
    assert_eq!(mapped.arguments, vec!["tabid-42", "on"]);
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.pin",
        "arguments": {"id": "tabid-42", "state": "off"}
    }))
    .expect("typed tab pin action");
    assert_eq!(action_id, "browser.tab.pin");
    assert_eq!(command.name, "tab-pin");
    assert_eq!(command.arguments, vec!["tabid-42", "off"]);
    assert!(
        typed_ipc_action(&serde_json::json!({
            "action": "browser.tab.pin",
            "arguments": {"id": "tabid-42", "state": "invalid"}
        }))
        .is_err()
    );
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.mute",
        "arguments": {"id": "tabid-42", "state": "off"}
    }))
    .expect("typed tab mute action");
    assert_eq!(action_id, "browser.tab.mute");
    assert_eq!(command.name, "tab-mute");
    assert_eq!(command.arguments, vec!["tabid-42", "off"]);
    assert!(
        typed_ipc_action(&serde_json::json!({
            "action": "browser.tab.mute",
            "arguments": {"id": "tabid-42", "state": "invalid"}
        }))
        .is_err()
    );
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.undo",
        "arguments": {}
    }))
    .expect("typed tab undo action");
    assert_eq!(action_id, "browser.tab.undo");
    assert_eq!(command.name, "tab-undo");
    assert!(command.arguments.is_empty());
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.move",
        "arguments": {"id": "tabid-42", "direction": "right"}
    }))
    .expect("typed tab action");
    assert_eq!(action_id, "browser.tab.move");
    assert_eq!(command.name, "tab-move");
    assert_eq!(command.arguments, vec!["tabid-42", "right"]);
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.select",
        "arguments": {"selector": "2"}
    }))
    .expect("typed tab select action");
    assert_eq!(action_id, "browser.tab.select");
    assert_eq!(command.name, "tab-select");
    assert_eq!(command.arguments, vec!["2"]);
    assert!(
        typed_ipc_action(&serde_json::json!({
            "action": "browser.tab.select",
            "arguments": {"selector": 2}
        }))
        .is_err()
    );
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.discard",
        "arguments": {"id": "tabid-42"}
    }))
    .expect("typed tab discard action");
    assert_eq!(action_id, "browser.tab.discard");
    assert_eq!(command.name, "tab-discard");
    assert_eq!(command.arguments, vec!["tabid-42"]);
    for (verb, command_name) in [
        ("reopen-window", "reopen-in-window"),
        ("detach", "tab-detach"),
    ] {
        let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
            name: "action".into(),
            arguments: vec!["tab".into(), verb.into()],
        })
        .expect("tab window action mapping");
        assert_eq!(action_id, format!("browser.tab.{verb}"));
        assert_eq!(mapped.name, command_name);
        assert!(mapped.arguments.is_empty());
    }
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.reopen-window",
        "arguments": {}
    }))
    .expect("typed tab reopen action");
    assert_eq!(action_id, "browser.tab.reopen-window");
    assert_eq!(command.name, "reopen-in-window");
    assert!(command.arguments.is_empty());
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.detach",
        "arguments": {}
    }))
    .expect("typed tab detach action");
    assert_eq!(action_id, "browser.tab.detach");
    assert_eq!(command.name, "tab-detach");
    assert!(command.arguments.is_empty());
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec!["tab".into(), "zoom".into(), "1.25".into()],
    })
    .expect("tab zoom action mapping");
    assert_eq!(action_id, "browser.tab.zoom");
    assert_eq!(mapped.name, "zoom");
    assert_eq!(mapped.arguments, vec!["1.25"]);
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.zoom",
        "arguments": {"factor": "in"}
    }))
    .expect("typed tab zoom action");
    assert_eq!(action_id, "browser.tab.zoom");
    assert_eq!(command.name, "zoom");
    assert_eq!(command.arguments, vec!["in"]);
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec![
            "tab".into(),
            "search-next".into(),
            "backward".into(),
            "3".into(),
        ],
    })
    .expect("tab search-next action mapping");
    assert_eq!(action_id, "browser.tab.search-next");
    assert_eq!(mapped.name, "search-next");
    assert_eq!(mapped.arguments, vec!["--backward", "--count", "3"]);
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.tab.search-next",
        "arguments": {"direction": "forward", "count": 2}
    }))
    .expect("typed tab search-next action");
    assert_eq!(action_id, "browser.tab.search-next");
    assert_eq!(command.name, "search-next");
    assert_eq!(command.arguments, vec!["--count", "2"]);
    let qml = [
        include_str!("../qml/Main.qml"),
        include_str!("../qml/components/FerricSettings.qml"),
        include_str!("../qml/components/FerricJourneySearch.qml"),
        include_str!("../qml/components/FerricSettingRows.qml"),
        include_str!("../qml/components/FerricLibraryManager.qml"),
        include_str!("../qml/components/FerricLibraryEntries.qml"),
        include_str!("../qml/components/FerricCommandLine.qml"),
        include_str!("../qml/scripts/SpellcheckPresentation.js"),
    ]
    .concat();
    let reopen_confirmation = include_str!("../qml/components/FerricReopenWindowConfirmation.qml");
    let downloads = include_str!("../qml/components/FerricDownloadManager.qml");
    let source = include_str!("lib.rs");
    let hint_policy = include_str!("hint_policy.rs");
    assert!(qml.contains("browser.tab.move"));
    assert!(qml.contains("browser.tab.select"));
    assert!(qml.contains("audioMuted: !!viewModel.muted"));
    assert!(qml.contains("action.indexOf(\"tab-mute\\t\") === 0"));
    assert!(qml.contains("tabs.setProperty(muteIndex, \"muted\", muted)"));
    assert!(
        source.contains(".pending_engine_action = Some(format!(\"tab-mute\\t{tab}\\t{muted}\"))")
    );
    assert!(source.contains("\"muted\": muted"));
    assert!(!source.contains("\"muted\": !muted"));
    assert!(qml.contains("window.executePendingEngineAction()"));
    assert!(qml.contains("tabId + \"\\tleft\""));
    assert!(qml.contains("tabId + \"\\tright\""));
    assert!(qml.contains("browser.tab.reopen-window"));
    assert!(qml.contains("browser.tab.suspend"));
    assert!(qml.contains("browser.tab.discard"));
    assert!(qml.contains("browser.tab.resume"));
    assert!(qml.contains("close_popup_tab"));
    assert!(qml.contains("WebEngineView.LifecycleState.Frozen"));
    assert!(qml.contains("WebEngineView.LifecycleState.Discarded"));
    assert!(qml.contains("recommendedState"));
    assert!(qml.contains("tabSuspensionBlockReason"));
    assert!(qml.contains("tabDiscardBlockReason"));
    assert!(qml.contains("discarded page reload requested"));
    assert!(qml.contains("reopen-window\\t"));
    assert!(qml.contains("reopen-window-confirm\\t"));
    assert!(qml.contains("showReopenWindowConfirmation"));
    assert!(qml.contains("confirmReopenWindow"));
    assert!(reopen_confirmation.contains("Reopen tab in a same-profile window?"));
    assert!(source.contains("reopen-window-confirm\\t{source_url}"));
    assert!(qml.contains("browser.tab.zoom"));
    assert!(qml.contains("zoom\\t"));
    assert!(qml.contains("function refreshEffectiveSiteSettings()"));
    assert!(qml.contains("Navigation-scoped content settings are snapshotted"));
    assert!(!qml.contains("var configGeneration = viewUi.config_json"));
    assert!(qml.contains("browser.tab.search-next"));
    assert!(qml.contains("function openLibraryEntry(entryKind, entryId)"));
    assert!(qml.contains("function deleteLibraryEntry(entryKind, entryId)"));
    assert!(qml.contains("function editLibraryEntry(entryKind, entryId, value)"));
    assert!(qml.contains("bookmark-edit"));
    assert!(qml.contains("quickmark-edit"));
    assert!(qml.contains("function changeLibraryPage(delta)"));
    assert!(qml.contains("function runJourneyCurrentQuery()"));
    assert!(qml.contains("Show current journey node only"));
    assert!(qml.contains("window.runJourneyQuery(\"--current\")"));
    assert!(qml.contains("Choose reopen target for journey node"));
    assert!(qml.contains("--target \" + target"));
    assert!(qml.contains("Previous library page"));
    assert!(qml.contains("Next library page"));
    assert!(qml.contains("Confirm delete"));
    assert!(qml.contains("entryKind === \"bookmark\""));
    assert!(qml.contains("window-close-request"));
    assert!(qml.contains("beginApplicationShutdown"));
    assert!(qml.contains("applicationShutdownForcePromptVisible"));
    assert!(qml.contains("registerPopupWindow"));
    assert!(qml.contains("begin_shutdown_gate"));
    assert!(qml.contains("force_quit"));
    assert!(qml.contains("flush_durable_state"));
    assert!(qml.contains("poll_storage_library"));
    assert!(qml.contains("storageLibraryPollTimer"));
    assert!(qml.contains("request_session_preview"));
    assert!(qml.contains("request_named_session_load"));
    assert!(qml.contains("sessionPreviewPollTimer"));
    assert!(qml.contains("Loading validated session descriptors"));
    assert!(source.contains("request_session_preview(&name)"));
    assert!(source.contains("session_restore_load_append = Some(append)"));
    assert!(qml.contains("update_completion(commandSurface.commandText"));
    assert!(qml.contains("window.downloadManagerVisible"));
    assert!(qml.contains("clear_current_session_checkpoints"));
    assert!(qml.contains("windowShutdownStoragePromptVisible"));
    assert!(qml.contains("cancelShutdownRequestsFor"));
    assert!(qml.contains("shutdownPageProbeScript"));
    assert!(qml.contains("shutdownPageProbeTimer"));
    assert!(qml.contains("shutdownPagePromptVisible"));
    assert!(qml.contains("Close anyway despite page state"));
    assert!(qml.contains("release_transient_resources()"));
    assert!(qml.contains("find-next\\t"));
    assert!(qml.contains("WebEngineDownloadRequest.MimeHtmlSaveFormat"));
    assert!(qml.contains("prepare_save_page"));
    assert!(qml.contains("take_save_page_path"));
    assert!(qml.contains("id: secondaryDownloadChooser"));
    assert!(qml.contains("secondaryProfile.acceptPendingDownload()"));
    assert!(qml.contains("secondaryProfile.cancelPendingDownload()"));
    assert!(qml.contains("secondaryWindow.pendingDownloadRequests"));
    assert!(qml.contains("secondaryDownloadChooser.open()"));
    assert!(qml.contains("secondaryWindow.pendingDownloadRequests = ({})"));
    assert!(qml.contains("download.view"));
    assert!(qml.contains("function downloadOwnerFor(download)"));
    assert!(qml.contains("id: popupDownloadChooser"));
    assert!(qml.contains("owner.handleDownloadRequested(download)"));
    assert!(qml.contains("popupWindow.cancelPendingDownload()"));
    assert!(qml.contains("download.totalBytes"));
    assert!(qml.contains("download.interruptReasonString"));
    assert!(qml.contains("function downloadMetrics(id, download)"));
    assert!(downloads.contains("Failure reason: "));
    assert!(qml.contains("function formatDownloadRate(bytesPerSecond)"));
    assert!(qml.contains("id: downloadMetricsTimer"));
    assert!(qml.contains("function spellcheckDictionaryStatus(ui)"));
    assert!(qml.contains("spellcheck_dictionaries"));
    assert!(
        source.contains("fn spellcheck_dictionaries(self: Pin<&mut BrowserUi>) -> QStringList")
    );
    assert!(!qml.contains("var payload = ui && ui.spellcheck_dictionaries"));
    assert!(qml.contains("no download was attempted"));
    assert!(qml.contains("feature_spellcheck_languages"));
    assert!(qml.contains("rowData.type === \"languages\""));
    assert!(qml.contains("view-source:"));
    assert!(qml.contains("Viewing page source"));
    assert!(qml.contains("action === \"jseval\""));
    assert!(source.contains("set_jseval_tab_id"));
    assert!(qml.contains("WebEngineScript.MainWorld"));
    assert!(source.contains("validate_jseval_script"));
    assert!(qml.contains("prepare_print_job"));
    assert!(qml.contains("finish_print_job"));
    assert!(qml.contains("printToPdf(function(success)"));
    assert!(qml.contains("devToolsView: window.devToolsVisible"));
    assert!(qml.contains("function toggleDevTools(detach)"));
    assert!(qml.contains("inspectedView: window.activeWebView()"));
    assert!(qml.contains("id: secondaryDevToolsLoader"));
    assert!(qml.contains("active: secondaryWindow.devToolsVisible"));
    assert!(qml.contains("devToolsView: secondaryWindow.devToolsVisible"));
    assert!(qml.contains("secondaryWindow.devToolsWindow"));
    assert!(qml.contains("action === \"show-binding-help\""));
    assert!(qml.contains("action === \"show-diagnostics\""));
    assert!(qml.contains("action.indexOf(\"show-settings\\t\") === 0"));
    assert!(qml.contains("function showSettings(search)"));
    assert!(qml.contains("function installUserscriptManifest()"));
    assert!(qml.contains("Install userscript manifest"));
    assert!(source.contains("fn install_userscript_manifest"));
    assert!(qml.contains("function refreshEngineUpdateNotice()"));
    assert!(
        qml.contains("QtWebEngine build is blocked; update it with the system package manager")
    );
    assert!(qml.contains("engine_update_policy"));
    assert!(qml.contains("result.action === \"clean-yank\""));
    assert!(qml.contains("Clean-copy link"));
    assert!(qml.contains("\"browser.link.clean-copy\""));
    assert!(qml.contains("ui.take_clipboard_request()"));
    assert!(
        hint_policy.contains(
            "Some(\"yank\" | \"clean-yank\" | \"tab-bg\" | \"userscript\" | \"download\")"
        )
    );
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec!["window".into(), "focus".into(), "window-42".into()],
    })
    .expect("window action mapping");
    assert_eq!(action_id, "browser.window.focus");
    assert_eq!(mapped.name, "window-focus");
    assert_eq!(mapped.arguments, vec!["window-42"]);
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.window.focus",
        "arguments": {"id": "window-42"}
    }))
    .expect("typed window action");
    assert_eq!(action_id, "browser.window.focus");
    assert_eq!(command.name, "window-focus");
    assert_eq!(command.arguments, vec!["window-42"]);
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec!["window".into(), "close".into()],
    })
    .expect("window close action mapping");
    assert_eq!(action_id, "browser.window.close");
    assert_eq!(mapped.name, "window-close");
    assert!(mapped.arguments.is_empty());
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.window.close",
        "arguments": {}
    }))
    .expect("typed window close action");
    assert_eq!(action_id, "browser.window.close");
    assert_eq!(command.name, "window-close");
    assert!(command.arguments.is_empty());
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec![
            "session".into(),
            "load".into(),
            "--append".into(),
            "work".into(),
        ],
    })
    .expect("session action mapping");
    assert_eq!(action_id, "browser.session.load");
    assert_eq!(mapped.name, "session-load");
    assert_eq!(mapped.arguments, vec!["--append", "work"]);
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.session.load",
        "arguments": {"name": "work", "append": true}
    }))
    .expect("typed session action");
    assert_eq!(action_id, "browser.session.load");
    assert_eq!(command.name, "session-load");
    assert_eq!(command.arguments, vec!["--append", "work"]);
    for (subject, verb, id, command_name) in [
        ("history-entry", "open", "history-42", "history-open"),
        ("bookmark", "open", "bookmark-42", "bookmark-open"),
        ("bookmark", "delete", "bookmark-42", "bookmark-delete"),
        ("quickmark", "open", "work", "quickmark-open"),
        ("quickmark", "delete", "work", "quickmark-delete"),
    ] {
        let (mapped, mapped_id) = parse_action_invocation(&ParsedCommand {
            name: "action".into(),
            arguments: vec![subject.into(), verb.into(), id.into()],
        })
        .expect("stored entry action mapping");
        assert_eq!(mapped_id, format!("browser.{subject}.{verb}"));
        assert_eq!(mapped.name, command_name);
        assert_eq!(mapped.arguments, vec![id.to_owned()]);
    }
    for (verb, command_name) in [("help", "command-help"), ("execute", "command-execute")] {
        let (mapped, mapped_id) = parse_action_invocation(&ParsedCommand {
            name: "action".into(),
            arguments: vec!["command".into(), verb.into(), "command-42".into()],
        })
        .expect("command action mapping");
        assert_eq!(mapped_id, format!("browser.command.{verb}"));
        assert_eq!(mapped.name, command_name);
        assert_eq!(mapped.arguments, vec!["command-42"]);
    }
    let open_id = CommandRegistry::default_v1()
        .definitions()
        .iter()
        .find(|definition| definition.name == "open")
        .expect("open command definition")
        .action
        .to_string();
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.command.execute",
        "arguments": {
            "id": open_id.clone(),
            "arguments": {"input": "https://example.test"}
        }
    }))
    .expect("typed argument-bearing command action");
    assert_eq!(action_id, "browser.command.execute");
    assert_eq!(command.name, "command-execute");
    assert_eq!(command.arguments.len(), 2);
    assert_eq!(command.arguments[0], open_id);
    assert_eq!(
        serde_json::from_str::<Value>(&command.arguments[1]).expect("encoded arguments"),
        serde_json::json!({"input": "https://example.test"})
    );
    assert!(
        typed_ipc_action(&serde_json::json!({
            "action": "browser.command.execute",
            "arguments": {"id": open_id, "arguments": "not-an-object"}
        }))
        .is_err()
    );
    let (mapped, action_id) = parse_action_invocation(&ParsedCommand {
        name: "action".into(),
        arguments: vec!["context".into(), "enter".into(), "research".into()],
    })
    .expect("context action mapping");
    assert_eq!(action_id, "browser.context.enter");
    assert_eq!(mapped.name, "context-enter");
    assert_eq!(mapped.arguments, vec!["research"]);
    let (command, _, action_id) = typed_ipc_action(&serde_json::json!({
        "action": "browser.context.save",
        "arguments": {"name": "research"}
    }))
    .expect("typed context action");
    assert_eq!(action_id, "browser.context.save");
    assert_eq!(command.name, "context-save");
    assert_eq!(command.arguments, vec!["research"]);
    assert!(
        parse_action_invocation(&ParsedCommand {
            name: "action".into(),
            arguments: vec!["tab".into(), "undo".into(), "unexpected".into()],
        })
        .is_err()
    );
}

#[test]
fn configured_selection_search_uses_https_data_only_templates() {
    let config = serde_json::to_value(Config::default()).expect("default config");
    let (engine, url) = configured_search_url(&config, Some("ddg"), "$() `quoted` ;; ")
        .expect("configured search URL");
    assert_eq!(engine, "ddg");
    assert_eq!(
        url,
        "https://duckduckgo.com/?q=%24%28%29%20%60quoted%60%20%3B%3B%20"
    );
    assert!(configured_search_url(&config, Some("missing"), "query").is_err());
}

#[test]
fn navigation_uses_configured_search_engines_for_keywords_and_fallback() {
    let mut config = Config::default();
    config.navigation.default_search = "g".into();
    config
        .search_engines
        .insert("g".into(), "https://search.example/?q={query}".into());
    let value = serde_json::to_value(config).expect("navigation config");
    let context = navigation_context_from_config_and_quickmarks(
        &value,
        &[Quickmark {
            name: "docs".into(),
            url: "https://docs.example/".into(),
        }],
        false,
    );
    assert_eq!(
        resolve_input("docs", &context)
            .expect("configured quickmark")
            .url
            .as_str(),
        "https://docs.example/"
    );
    assert_eq!(
        resolve_input("g ferric browser", &context)
            .expect("configured keyword")
            .url
            .as_str(),
        "https://search.example/?q=ferric%20browser"
    );
    assert_eq!(
        resolve_input("ferric browser", &context)
            .expect("configured fallback")
            .url
            .as_str(),
        "https://search.example/?q=ferric%20browser"
    );
    let trusted = navigation_context_from_config_and_quickmarks(&value, &[], true);
    assert_eq!(
        resolve_input("/tmp/report.html", &trusted)
            .expect("trusted CLI local path")
            .source,
        NavigationSource::LocalPath
    );
    let untrusted = navigation_context_from_config_and_quickmarks(&value, &[], false);
    assert_eq!(
        resolve_input("/tmp/report.html", &untrusted)
            .expect("untrusted local-looking input")
            .source,
        NavigationSource::Search
    );
}

#[test]
fn configured_target_discovery_preserves_declared_subjects() {
    let target = ActionTargetConfig {
        subject_types: vec!["link".into(), "selection".into()],
        executable: "tool".into(),
        argv: vec!["{url}".into()],
        detach: true,
        allow_private: false,
    };
    let values = configured_action_target_values("tool", &target, false);
    assert_eq!(values.len(), 2);
    assert_eq!(values[0]["id"], "external.tool.link.send");
    assert_eq!(values[0]["subject"], "link");
    assert_eq!(values[0]["arguments"][1]["name"], "url");
    assert_eq!(values[0]["availability"]["state"], "available");
    assert_eq!(values[0]["completion_provider"], "url");
    assert_eq!(values[0]["availability_predicate"], "captured-link");
    assert_eq!(values[0]["availability"]["predicate"], "captured-link");
    assert_eq!(
        values[0]["availability"]["requires_subject_revalidation"],
        true
    );
    assert_eq!(values[1]["id"], "external.tool.selection.send");
    assert_eq!(values[1]["completion_provider"], "text");
    assert_eq!(values[1]["availability_predicate"], "live-selection");
    assert_eq!(values[1]["subject"], "selection");
    assert_eq!(values[1]["examples"][0], "action selection send --to tool");
    let private_values = configured_action_target_values("tool", &target, true);
    assert_eq!(private_values[0]["availability"]["state"], "unavailable");
    assert_eq!(
        private_values[0]["availability"]["reason"],
        "private-profile"
    );
}

#[test]
fn configured_target_availability_is_subject_specific() {
    let mut config = Config::default();
    config.action_targets.insert(
        "selection-tool".into(),
        ActionTargetConfig {
            subject_types: vec!["selection".into()],
            executable: "tool".into(),
            argv: vec!["{selection}".into()],
            detach: false,
            allow_private: false,
        },
    );
    let config = serde_json::to_value(config).expect("action target config");
    assert!(
        configured_action_target_supports_subject(&config, ActionSubject::Selection)
            .expect("selection target availability")
    );
    assert!(
        !configured_action_target_supports_subject(&config, ActionSubject::Link)
            .expect("link target availability")
    );
}

#[test]
fn configured_target_discovery_exposes_url_and_tab_subjects() {
    let target = ActionTargetConfig {
        subject_types: vec!["url".into(), "tab".into()],
        executable: "tool".into(),
        argv: vec!["{url}".into(), "{title}".into()],
        detach: false,
        allow_private: true,
    };
    let values = configured_action_target_values("tool", &target, false);
    assert_eq!(values.len(), 2);
    assert_eq!(values[0]["id"], "external.tool.url.send");
    assert_eq!(values[0]["arguments"][1]["name"], "url");
    assert_eq!(values[1]["id"], "external.tool.tab.send");
    assert_eq!(values[1]["arguments"].as_array().unwrap().len(), 1);
    assert_eq!(
        values[0]["required_capabilities"],
        serde_json::json!(["configured-action-target"])
    );
}

#[test]
fn userscript_discovery_exposes_shared_action_metadata() {
    let action = userscript::RegisteredAction {
        script: "annotate".into(),
        id: "userscript.annotate.link".into(),
        subject: "link".into(),
        verb: "annotate".into(),
        label: "Annotate link".into(),
        required_fields: vec!["url".into()],
        allow_private: false,
    };
    let value = userscript_action_value(&action, true);
    assert_eq!(value["completion_provider"], "url");
    assert_eq!(value["availability_predicate"], "captured-link");
    assert_eq!(value["availability"]["predicate"], "captured-link");
    assert_eq!(
        value["required_capabilities"],
        serde_json::json!(["installed-userscript"])
    );
}

#[test]
fn userscript_discovery_matches_storage_capability_to_subject() {
    let action = userscript::RegisteredAction {
        script: "library".into(),
        id: "userscript.library.open-history".into(),
        subject: "history-entry".into(),
        verb: "open".into(),
        label: "Open history entry".into(),
        required_fields: vec!["history_id".into()],
        allow_private: false,
    };
    assert!(!userscript_action_is_available(&action, true, false, false));
    assert!(userscript_action_is_available(&action, true, true, false));
    assert!(!userscript_action_is_available(&action, true, true, true));
}

#[test]
fn switcher_external_actions_share_bounded_available_target_policy() {
    let config = serde_json::json!({
        "action_targets": {
            "player": {
                "subject_types": ["url", "link", "selection", "tab"],
                "executable": "player",
                "argv": ["{url}"],
                "detach": false,
                "allow_private": false
            }
        }
    });
    let values = configured_switcher_action_values(&config, false).expect("valid targets");
    assert_eq!(
        values
            .iter()
            .filter_map(|value| value.get("subject").and_then(Value::as_str))
            .collect::<Vec<_>>(),
        vec!["url", "tab"]
    );
    assert!(values.iter().all(|value| {
        value["availability"]["state"] == "available" && value["target"] == "player"
    }));
    assert!(
        configured_switcher_action_values(&config, true)
            .expect("valid private target")
            .is_empty()
    );
}

#[test]
fn external_action_ids_are_strictly_bounded_and_typed() {
    assert_eq!(
        parse_external_action_id("external.mpv.link.send"),
        Some(("mpv".into(), "link".into()))
    );
    assert_eq!(
        parse_external_action_id("external.player.tab.send"),
        Some(("player".into(), "tab".into()))
    );
    for value in [
        "external.MPV.link.send",
        "external.mpv.link.open",
        "external.mpv.command.send",
        "external.mpv.link.send.extra",
    ] {
        assert_eq!(parse_external_action_id(value), None);
    }
}

#[test]
fn action_failures_have_structured_redacted_details() {
    assert_eq!(
        action_failure_category(ErrorCode::InvalidArgument),
        "invalid-subject-or-parameters"
    );
    assert_eq!(
        action_failure_category(ErrorCode::StaleTarget),
        "stale-target"
    );
    assert_eq!(
        action_failure_category(ErrorCode::Unsupported),
        "missing-capability"
    );
    assert_eq!(
        action_failure_category(ErrorCode::Denied),
        "denied-source-or-privacy"
    );
    let response = ipc_action_failure(
        "req-1",
        "browser.tab.close",
        "op-1",
        PublicError::new(
            ErrorCode::StaleTarget,
            "The captured tab is no longer available.",
            "tab id tab-1 no longer belongs to the selected window",
        ),
    );
    let error = response.error.expect("structured action error");
    assert_eq!(error.code, "E_STALE_TARGET");
    let details = error.details.expect("action error details");
    assert_eq!(details["action_id"], "browser.tab.close");
    assert_eq!(details["operation_id"], "op-1");
    assert_eq!(details["category"], "stale-target");
    assert!(details.get("url").is_none());
}

#[test]
fn action_audit_event_is_structured_and_contains_only_bounded_ledger_fields() {
    let record = action_audit_record("browser.tab.reload", "op-7", "failed", Some("stale-target"));
    assert_eq!(
        record,
        serde_json::json!({
            "action_id": "browser.tab.reload",
            "operation_id": "op-7",
            "outcome": "failed",
            "category": "stale-target"
        })
    );
    assert_eq!(record.as_object().expect("object").len(), 4);
    assert!(!record.to_string().contains("url"));
    assert!(!record.to_string().contains("subject"));
}

#[test]
fn ipc_command_context_captures_route_and_privacy_metadata() {
    let (application, window, tab) =
        bootstrap_application(PrivacyKind::Normal, "default").expect("default application");
    let route = IpcRoute {
        selector: DispatchTarget::Active,
        open_target: IpcOpenTarget::Tab,
        profile: Some("default".into()),
        context: Some("research".into()),
        external_open: false,
        source: CommandSource::Ipc,
    };
    let context = ipc_command_context(Some(&application), &route, "op-1", 3);
    assert_eq!(context["source"], "ipc");
    assert_eq!(context["operation_id"], "op-1");
    assert_eq!(context["count"], 3);
    assert_eq!(context["window_id"], window.to_string());
    assert_eq!(context["tab_id"], tab.to_string());
    assert_eq!(context["privacy"], "normal");
    assert_eq!(context["profile"], "default");
    assert_eq!(context["requested_profile"], "default");
    assert_eq!(context["requested_context"], "research");
    assert_eq!(context["target"], "tab");
    let invocation = ipc_command_invocation(
        &application,
        ParsedCommand {
            name: "back".into(),
            arguments: vec!["--count".into(), "3".into()],
        },
        DispatchTarget::Active,
        CommandSource::Ipc,
        Some("op-1"),
    );
    assert_eq!(invocation.context.source, CommandSource::Ipc);
    assert_eq!(invocation.context.count, 3);
    assert_eq!(invocation.context.operation_id.as_deref(), Some("op-1"));
    assert_eq!(invocation.context.window, Some(window));
    assert_eq!(invocation.context.tab, Some(tab));
    assert_eq!(
        ipc_command_count(&ParsedCommand {
            name: "scroll".into(),
            arguments: vec!["--count".into(), "7".into()],
        }),
        7
    );
    assert_eq!(
        ipc_command_count(&ParsedCommand {
            name: "scroll".into(),
            arguments: vec!["--count".into(), "not-a-count".into()],
        }),
        1
    );
}

#[test]
fn ipc_command_failure_preserves_context_without_sensitive_details() {
    let context = serde_json::json!({
        "source": "ipc",
        "operation_id": "op-1",
        "count": 1,
        "privacy": "normal",
    });
    let response = ipc_command_failure_with_context(
        "req-1",
        PublicError::new(
            ErrorCode::StaleTarget,
            "The captured tab is no longer available.",
            "captured tab id tab-1 was removed before execution",
        ),
        context,
    );
    let error = response.error.expect("structured command error");
    assert_eq!(error.code, "E_STALE_TARGET");
    let details = error.details.expect("command error details");
    assert_eq!(details["command_context"]["operation_id"], "op-1");
    assert_eq!(details["command_context"]["privacy"], "normal");
    assert!(details.get("url").is_none());
    assert!(details.get("title").is_none());
}

#[test]
fn action_capabilities_are_declared_and_subjects_revalidate() {
    let registry = ActionRegistry::default_v1();
    assert_eq!(
        registry
            .resolve("browser.bookmark.open")
            .expect("bookmark action")
            .required_capabilities(),
        &["durable-profile-storage"]
    );
    assert!(
        registry
            .resolve("browser.url.open")
            .expect("URL action")
            .required_capabilities()
            .is_empty()
    );
    assert_eq!(
        registry
            .resolve("browser.tab.detach")
            .expect("detach action")
            .required_capabilities(),
        &["live-window-reparent"]
    );

    let actions = action_list_value(Some("bookmark")).expect("bookmark action list");
    let bookmark = actions
        .as_array()
        .and_then(|actions| actions.first())
        .expect("bookmark action row");
    assert_eq!(
        bookmark["required_capabilities"],
        serde_json::json!(["durable-profile-storage"])
    );
    assert_eq!(bookmark["availability"]["predicate"], "stored-bookmark");
    assert_eq!(
        bookmark["availability"]["requires_subject_revalidation"],
        serde_json::json!(true)
    );
    let tab_actions = action_list_value(Some("tab")).expect("tab action list");
    let reload = tab_actions
        .as_array()
        .expect("tab actions array")
        .iter()
        .find(|action| action["id"] == "browser.tab.reload")
        .expect("reload action");
    assert_eq!(reload["arguments"][0]["name"], "bypass_cache");
    assert_eq!(reload["arguments"][0]["kind"], "boolean");
    assert!(
        reload["sources"]
            .as_array()
            .is_some_and(|sources| { sources.iter().any(|source| source == "ipc") })
    );
    assert_eq!(reload["sensitive"], false);
}

#[test]
fn userscript_action_arguments_follow_subject_schema() {
    assert_eq!(userscript_action_argument_name("link"), Some("url"));
    assert_eq!(
        userscript_action_argument_name("download"),
        Some("download_id")
    );
    assert_eq!(
        userscript_action_argument_name("history-entry"),
        Some("history_id")
    );
    assert_eq!(
        userscript_action_argument_name("selection"),
        Some("selection")
    );
    assert_eq!(userscript_action_argument_name("tab"), Some("tab_id"));
    assert_eq!(userscript_action_argument_name("window"), Some("window_id"));
    assert_eq!(
        userscript_action_argument_name("context"),
        Some("context_name")
    );
    assert_eq!(userscript_action_argument_name("url"), None);
    let hint_only = userscript::RegisteredAction {
        script: "hint".into(),
        id: "userscript.hint.send".into(),
        subject: "link".into(),
        verb: "send".into(),
        label: "Send hinted link".into(),
        required_fields: vec!["hint_url".into()],
        allow_private: false,
    };
    assert!(userscript_action_is_hint_only(&hint_only));
    let regular_link = userscript::RegisteredAction {
        required_fields: vec!["url".into()],
        ..hint_only
    };
    assert!(!userscript_action_is_hint_only(&regular_link));
}

#[test]
fn userscript_live_subjects_capture_stable_targets() {
    let (mut application, window, tab) = bootstrap_application(PrivacyKind::Normal, "default")
        .expect("bootstrap userscript target state");
    let target = application.capture_target(tab).expect("current target");
    assert_eq!(
        userscript_subject_target(&application, target, "tab", &tab.to_string()),
        Ok(target)
    );
    assert_eq!(
        userscript_subject_target(&application, target, "window", &window.to_string()),
        Ok(target)
    );
    application
        .dispatch_runtime(RuntimeInput::EngineFact(Event::SetWindowContext {
            window,
            context: Some("research".into()),
        }))
        .expect("window context");
    assert_eq!(
        userscript_subject_target(&application, target, "context", "research"),
        Ok(target)
    );
    assert!(userscript_subject_target(&application, target, "tab", "tabid-999").is_err());
    assert!(userscript_subject_target(&application, target, "context", "missing").is_err());
}

#[test]
fn every_builtin_action_example_maps_through_typed_parser() {
    let registry = ActionRegistry::default_v1();
    for definition in registry.definitions() {
        assert!(
            !definition.examples.is_empty(),
            "action {} has no example",
            definition.id
        );
        for example in &definition.examples {
            let commands = parse_chain(example, ParseInput::Interactive).unwrap_or_else(|error| {
                panic!("{} example does not parse: {error}", definition.id)
            });
            assert_eq!(
                commands.len(),
                1,
                "{} example must be one command",
                definition.id
            );
            let (mapped, action_id) = parse_action_invocation(&commands[0])
                .unwrap_or_else(|error| panic!("{} example does not map: {error}", definition.id));
            assert_eq!(action_id, definition.id);
            assert_eq!(mapped.name, definition.command);
        }
    }
}

#[test]
fn live_document_capability_requires_a_healthy_current_tab() {
    let (mut application, _, tab) =
        bootstrap_application(PrivacyKind::Normal, "default").expect("application bootstrap");

    assert!(live_document_available(Some(&application), Some(tab)));
    let target = application.capture_target(tab).expect("target");
    application
        .dispatch_runtime(RuntimeInput::EngineFact(Event::RendererTerminated {
            target,
        }))
        .expect("renderer termination");
    assert!(!live_document_available(Some(&application), Some(tab)));
    assert!(!live_document_available(Some(&application), None));
}

#[test]
fn captured_target_validation_is_not_limited_to_the_focused_tab() {
    let (mut application, window, background_tab) =
        bootstrap_application(PrivacyKind::Normal, "default").expect("application bootstrap");
    application
        .dispatch_runtime(RuntimeInput::EngineFact(Event::OpenTab { window }))
        .expect("second tab");
    let focused_tab = application.windows()[&window]
        .active_tab
        .expect("focused tab");
    assert_ne!(background_tab, focused_tab);

    let background_target = application.capture_target(background_tab).expect("target");
    assert!(captured_target_is_current(
        Some(&application),
        background_target
    ));

    application
        .dispatch_runtime(RuntimeInput::EngineFact(Event::StartNavigation {
            target: background_target,
            url: ValidatedUrl::parse("https://example.test/background").expect("URL"),
        }))
        .expect("background navigation");
    assert!(!captured_target_is_current(
        Some(&application),
        background_target
    ));
}

#[test]
fn switcher_actions_have_safe_defaults_and_kind_allowlists() {
    let source = [
        include_str!("lib.rs"),
        include_str!("action_catalog.rs"),
        include_str!("switcher_policy.rs"),
    ]
    .concat();
    let qml = [
        include_str!("../qml/Main.qml"),
        include_str!("../qml/components/FerricContextMenu.qml"),
        include_str!("../qml/components/FerricSwitcherResults.qml"),
    ]
    .concat();
    assert!(source.contains("browser.context.enter"));
    assert!(source.contains("browser.window.focus"));
    assert!(source.contains("browser.bookmark.delete"));
    assert!(source.contains("browser.session.load"));
    assert!(source.contains("browser.command.execute"));
    assert_eq!(switcher_default_action("tab"), Some("focus"));
    assert_eq!(switcher_default_action("history"), Some("open"));
    assert_eq!(switcher_default_action("session"), Some("load-preview"));
    assert_eq!(switcher_default_action("download"), Some("show"));
    assert_eq!(switcher_default_action("action"), Some("execute"));
    assert_eq!(switcher_default_action("unknown"), None);
    assert_eq!(parse_switcher_generation(""), Ok(None));
    assert_eq!(parse_switcher_generation("7"), Ok(Some(7)));
    assert!(parse_switcher_generation("stale").is_err());
    assert_eq!(validate_switcher_generation(None, None), Ok(()));
    assert_eq!(validate_switcher_generation(Some(7), Some(7)), Ok(()));
    assert_eq!(
        validate_switcher_generation(Some(7), Some(8)),
        Err("Switcher tab target is stale; refresh the results")
    );
    assert_eq!(
        validate_switcher_generation(Some(7), None),
        Err("Switcher target generation is only valid for tabs")
    );

    assert!(switcher_action_allowed("history", "open"));
    assert!(switcher_action_allowed("session", "load"));
    assert!(switcher_action_allowed("command", "help"));
    assert!(switcher_action_allowed("download", "open"));
    assert!(switcher_action_allowed("action", "execute"));
    assert!(!switcher_action_allowed("history", "delete"));
    assert!(!switcher_action_allowed("context", "open"));
    assert!(!switcher_action_allowed("command", "shell"));
    assert!(source.contains("\"actions\""));
    assert!(source.contains("definition.sources.contains(&ActionSource::Switcher)"));
    assert!(source.contains("configured_action_target_values(&name, &target, private_profile)"));
    assert!(source.contains("configured_switcher_action_values"));
    assert!(source.contains("validate_action_availability(id, CommandSource::Switcher)"));
    assert!(source.contains("no-live-tab-to-transfer"));
    assert!(source.contains("Configured switcher action is stale or unavailable"));
    assert!(
        source.contains("validate_action_availability(&definition.id, CommandSource::Switcher)")
    );
    assert!(source.contains("route.source = CommandSource::Switcher"));
    assert!(source.contains("switcher.activate generation"));
    assert!(source.contains("Switcher tab target is stale; refresh the results"));
    assert!(qml.contains("resultData.actions || []"));
    assert!(qml.contains("switcherResults.actionRequested(resultRow.resultIndex, modelData)"));
    assert!(qml.contains("String(result.generation)"));
    assert!(qml.contains("window.switcherOwner(result)"));
    assert!(qml.contains("property string switcherScope: \"all\""));
    assert!(qml.contains("window.switcherScope)"));
    assert!(qml.contains("entry.ui.switcher_result_kinds"));
    assert!(qml.contains("entry.ui.switcher_result_actions"));
    assert!(!qml.contains("JSON.parse(raw).results"));
    assert!(qml.contains("function setSwitcherScope(scope)"));
    assert!(qml.contains("\"actions\", \"history\""));
    assert!(qml.contains("Accessible.role: Accessible.PageTab"));
}

#[test]
fn operation_events_publish_only_bounded_status_categories() {
    assert_eq!(operation_status_kind("running"), "completed");
    assert_eq!(operation_status_kind("sent to editor: op-1"), "completed");
    assert_eq!(operation_status_kind("failed (timeout)"), "failed");
    assert_eq!(operation_status_kind("cancelled"), "cancelled");
}

#[test]
fn terminal_operation_states_cannot_be_cancelled_again() {
    for status in [
        "completed",
        "completed: message",
        "failed (child)",
        "cancelled",
        "detached",
    ] {
        assert!(operation_is_terminal(status), "{status}");
    }
    for status in ["running", "selection pending", "accepted"] {
        assert!(!operation_is_terminal(status), "{status}");
    }
}

#[test]
fn userscript_stderr_is_bounded_redacted_and_explicit() {
    let stderr =
        sanitize_process_stderr(b"password=secret https://example.test/?token=hidden\nmessage\x01");
    assert!(stderr.contains("password=[redacted]"));
    assert!(!stderr.contains("secret"));
    assert!(!stderr.contains("token=hidden"));
    assert!(stderr.contains("message"));

    let mut rust = BrowserUiRust::default();
    rust.operation_states
        .insert("op-stderr".into(), "failed (child)".into());
    rust.operation_stderr
        .insert("op-stderr".into(), stderr.clone());
    let hidden =
        operations_query_value(&rust, &serde_json::json!({})).expect("default operation query");
    assert!(hidden["operations"][0].get("stderr").is_none());
    let visible = operations_query_value(
        &rust,
        &serde_json::json!({
            "operation_id": "op-stderr",
            "include_stderr": true
        }),
    )
    .expect("explicit stderr operation query");
    assert_eq!(visible["operations"][0]["stderr"], stderr);
    assert!(operations_query_value(&rust, &serde_json::json!({"include_stderr": "yes"})).is_err());
}

#[test]
fn ipc_query_boundaries_reject_unknown_and_mistyped_fields() {
    let tabs = serde_json::json!({"include_private": "yes"});
    let tabs_object = query_object(
        &tabs,
        "tabs.query",
        &["include_private", "active_only", "window"],
    )
    .expect("tabs object");
    assert!(query_bool_param(tabs_object, "tabs.query", "include_private", false).is_err());
    assert!(query_object(&serde_json::json!({"unexpected": true}), "tabs.query", &[]).is_err());

    let windows = serde_json::json!({"window": 1});
    let windows_object =
        query_object(&windows, "windows.query", &["window"]).expect("windows object");
    assert!(query_optional_string(windows_object, "windows.query", "window").is_err());
    assert!(
        query_empty_object_or_null(
            &serde_json::json!({"include_private": true}),
            "profiles.query"
        )
        .is_err()
    );

    let downloads = serde_json::json!({"include_private": "yes"});
    let downloads_object = query_object(&downloads, "downloads.query", &["include_private"])
        .expect("downloads object");
    assert!(
        query_bool_param(
            downloads_object,
            "downloads.query",
            "include_private",
            false
        )
        .is_err()
    );

    let contexts = serde_json::json!({"include_members": "yes"});
    let contexts_object =
        query_object(&contexts, "contexts.query", &["include_members"]).expect("contexts object");
    assert!(query_bool_param(contexts_object, "contexts.query", "include_members", false).is_err());

    let switcher = serde_json::json!({"limit": 0});
    let switcher_object = query_object(
        &switcher,
        "switcher.query",
        &["query", "scope", "limit", "offset", "include_private"],
    )
    .expect("switcher object");
    assert!(query_limit(switcher_object, "switcher.query", 50).is_err());
    let offset_object = serde_json::json!({"offset": 100_001});
    assert!(
        query_offset(
            offset_object.as_object().expect("offset object"),
            "switcher.query"
        )
        .is_err()
    );
    let query = serde_json::json!({"query": false});
    let query_object = query_object(
        &query,
        "switcher.query",
        &["query", "scope", "limit", "offset", "include_private"],
    )
    .expect("query object");
    assert!(query_optional_string(query_object, "switcher.query", "query").is_err());
}

#[test]
fn qml_idle_path_is_event_driven_and_devtools_are_lazy() {
    let qml = include_str!("../qml/Main.qml");
    let script = include_str!("../qml/scripts/BrowserScripts.js");
    assert!(qml.contains("onRuntime_work_available: window.scheduleRuntimeWork(0)"));
    assert!(qml.contains("id: runtimeWorkTimer"));
    assert!(qml.contains("repeat: false"));
    assert!(qml.contains("browserUi.maintenance_delay_ms()"));
    assert!(!qml.contains("interval: 120\n        repeat: true\n        running: true"));
    assert!(qml.contains("id: attachedDevToolsLoader"));
    assert!(qml.contains("id: secondaryDevToolsLoader"));
    assert!(!qml.contains("id: attachedDevToolsView"));
    assert!(!qml.contains("id: secondaryDevToolsView"));
    assert!(script.contains("if(!root)return null"));
    assert!(qml.contains("WebEngineProfilePrototype {\n        id: browserProfilePrototype"));
}
