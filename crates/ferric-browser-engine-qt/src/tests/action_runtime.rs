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
        ADAPTER_SOURCE,
        include_str!("../action_catalog.rs"),
        include_str!("../switcher_policy.rs"),
    ]
    .concat();
    let qml = [
        QML_SOURCE,
        include_str!("../../qml/components/FerricContextMenu.qml"),
        include_str!("../../qml/components/FerricSwitcherResults.qml"),
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
    let qml = QML_SOURCE;
    let script = include_str!("../../qml/scripts/BrowserScripts.js");
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
