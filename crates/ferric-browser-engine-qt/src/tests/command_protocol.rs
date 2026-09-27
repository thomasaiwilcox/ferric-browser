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

    let qml = QML_SOURCE;
    assert!(qml.contains("action.indexOf(\"command-prefill\\t\") === 0"));
    assert!(qml.contains("commandSurface.cursorPosition = commandSurface.commandText.length"));
}

#[test]
fn scroll_target_command_uses_the_shared_executor_and_owned_view_actions() {
    use crate::browser_ui_browsing_commands::{
        ScrollTargetEffect, dispatch_scroll_target_command,
    };

    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;
    let primary = include_str!("../../qml/components/FerricBrowserRuntimeSurface.qml");
    let secondary = include_str!("../../qml/components/FerricBrowserWindow.qml");
    assert!(source.contains("pub(super) fn execute_scroll_target_command"));
    assert!(source.contains("is_scroll_target_command(&command.name)"));
    assert!(source.contains("context routing is not valid for scroll targeting"));
    assert!(source.contains("hint_chrome_available"));
    assert!(source.contains("\"scroll-target\\t{tab_id}\\t{action_name}\""));
    assert!(source.contains("ScrollTargetEffect::QueueEngineAction(action)"));
    assert!(source.contains("set_hint_options(\"scrollables\", false, \"current\", None, false, 1)"));
    assert!(qml.contains("window.applyScrollTargetAction("));
    assert!(primary.contains("browserUi.tab_index_for_id(scrollTargetParts[1])"));
    assert!(primary.contains("browserUi, scrollTargetView,"));
    assert!(secondary.contains("secondaryUi.tab_index_for_id(scrollTargetParts[1])"));
    assert!(secondary.contains("secondaryUi, scrollTargetView,"));
    assert!(qml.contains("id: secondaryUi"));
    assert!(qml.contains("hint_chrome_available: false"));
    let primary_scroll_target = primary
        .find("if (action.indexOf(\"scroll-target\\t\") === 0)")
        .expect("primary scroll-target branch");
    let generic_active_view = primary
        .find("var webView = window.activeWebView()")
        .expect("generic active-view guard");
    assert!(
        primary_scroll_target < generic_active_view,
        "stable scroll-target routing must run before the generic active-view guard"
    );

    let select = ParsedCommand {
        name: "scroll-target".into(),
        arguments: vec!["select".into()],
    };
    let registry = CommandRegistry::default_v1();
    let issuing_tab = TabId::from_display("tabid-41").expect("stable tab ID");
    let mut mode = Mode::Normal;
    let mut queued_actions = Vec::new();
    let result = dispatch_scroll_target_command(
        &select,
        &registry,
        mode,
        true,
        Some(issuing_tab),
        |effect| match effect {
            ScrollTargetEffect::StartHints => mode = Mode::Hint,
            ScrollTargetEffect::QueueEngineAction(action) => queued_actions.push(action),
        },
    )
    .expect("full-chrome selection should enter Hint mode");
    assert_eq!(
        result,
        serde_json::json!({
            "status": "accepted",
            "action": "select",
            "mode": "hint"
        })
    );
    assert_eq!(mode, Mode::Hint);
    assert!(queued_actions.is_empty());

    let mut mode = Mode::Normal;
    let mut queued_actions = Vec::new();
    let error = dispatch_scroll_target_command(
        &select,
        &registry,
        mode,
        false,
        Some(issuing_tab),
        |effect| match effect {
            ScrollTargetEffect::StartHints => mode = Mode::Hint,
            ScrollTargetEffect::QueueEngineAction(action) => queued_actions.push(action),
        },
    )
    .expect_err("secondary chrome must reject interactive selection");
    assert_eq!(
        error,
        "scroll-target requires full browser chrome and is unavailable in this window"
    );
    assert_eq!(mode, Mode::Normal);
    assert!(queued_actions.is_empty());

    for action in ["auto", "document", "status"] {
        let command = ParsedCommand {
            name: "scroll-target".into(),
            arguments: vec![action.into()],
        };
        let mut effects = Vec::new();
        let result = dispatch_scroll_target_command(
            &command,
            &registry,
            Mode::Normal,
            false,
            Some(issuing_tab),
            |effect| effects.push(effect),
        )
        .expect("non-Hint policy actions should work in reduced chrome");
        assert_eq!(
            result,
            serde_json::json!({
                "status": "accepted",
                "action": action,
                "pending": true
            })
        );
        assert_eq!(
            effects,
            [ScrollTargetEffect::QueueEngineAction(format!(
                "scroll-target\t{issuing_tab}\t{action}"
            ))]
        );
    }

    let no_page_command = ParsedCommand {
        name: "scroll-target".into(),
        arguments: vec!["document".into()],
    };
    let mut no_page_effects = Vec::new();
    dispatch_scroll_target_command(
        &no_page_command,
        &registry,
        Mode::Normal,
        true,
        None,
        |effect| no_page_effects.push(effect),
    )
    .expect("a missing page must reach the QML error path");
    assert_eq!(
        no_page_effects,
        [ScrollTargetEffect::QueueEngineAction(
            "scroll-target\t\tdocument".into()
        )]
    );

    let mut rust = BrowserUiRust::default();
    assert!(rust.hint_chrome_available);
    rust.hint_chrome_available = false;
    assert!(!rust.hint_chrome_available);
}

#[test]
fn chained_scroll_target_policy_actions_keep_the_issuing_tab_identity() {
    use crate::browser_ui_browsing_commands::{
        ScrollTargetEffect, dispatch_scroll_target_command,
    };

    let registry = CommandRegistry::default_v1();
    let issuing_tab = TabId::from_display("tabid-41").expect("issuing tab ID");
    let tab_after_next = TabId::from_display("tabid-42").expect("next tab ID");

    for action in ["document", "auto", "status"] {
        let chain = parse_chain(
            &format!("scroll-target {action} ;; tab-next"),
            ParseInput::Interactive,
        )
        .expect("policy and tab switch command chain");
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].name, "scroll-target");
        assert_eq!(chain[1].name, "tab-next");

        let mut effects = Vec::new();
        dispatch_scroll_target_command(
            &chain[0],
            &registry,
            Mode::Normal,
            true,
            Some(issuing_tab),
            |effect| effects.push(effect),
        )
        .expect("scroll-target policy action");

        assert_ne!(issuing_tab, tab_after_next);
        assert_eq!(
            effects,
            [ScrollTargetEffect::QueueEngineAction(format!(
                "scroll-target\t{issuing_tab}\t{action}"
            ))],
            "the queued policy action must remain bound to tab A after tab-next activates tab B"
        );
    }
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
    let source = ADAPTER_SOURCE;
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
        QML_SOURCE,
        include_str!("../../qml/components/FerricSwitcher.qml"),
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

    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;
    let script = include_str!("../../qml/scripts/BrowserScripts.js");
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

    let qml = QML_SOURCE;
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
