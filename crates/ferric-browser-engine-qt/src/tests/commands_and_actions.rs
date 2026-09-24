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
    let qml = QML_SOURCE;
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
    let qml = QML_SOURCE;
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
    let qml = QML_SOURCE;
    assert!(qml.contains("action.indexOf(\"new-window\\t\") === 0"));
    assert!(qml.contains("windowPrivateProfile: windowAction[1] === \"true\""));
    assert!(qml.contains("windowStartupUrl: windowAction.slice(3).join(\"\\t\")"));
}

#[test]
fn external_window_focus_reports_bounded_activation_outcome() {
    let source = ADAPTER_SOURCE;
    let qml = QML_SOURCE;
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
    let source = ADAPTER_SOURCE;
    assert!(source.contains("tab-give requires a bounded target window ID"));
    assert!(source.contains("format!(\"tab-give\\t{window_id}\\t{operation_id}\")"));
    assert!(source.contains("format!(\"tab-detach\\t{operation_id}\")"));
    assert!(source.contains("fn complete_transfer_operation("));
    let qml = [
        QML_SOURCE,
        include_str!("../../qml/components/FerricContextMoveDialog.qml"),
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
    let adapter = include_str!("../runtime_bridge.rs");
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
    let qml = QML_SOURCE;
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
    let qml = QML_SOURCE;
    assert!(qml.contains("action.indexOf(\"tab-close-active\\t\") === 0"));
    assert!(qml.contains("function closeTabAtIndex(index)"));
    assert!(
        ADAPTER_SOURCE.contains(".finish_tab_close(index, target.tab, target.generation)")
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
    let qml = QML_SOURCE;
    assert!(qml.contains("function executeHistoryTraversal(view, backwards, requestedCount)"));
    assert!(qml.contains("History boundary reached"));
}

#[test]
fn download_destination_uses_encoded_file_urls() {
    let qml = QML_SOURCE;
    let dialogs = include_str!("../../qml/components/FerricFileDialogSurfaces.qml");
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
