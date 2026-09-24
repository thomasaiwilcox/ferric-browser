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
    assert!(QML_SOURCE.contains("windowStartupContext: windowAction[3]"));
    assert!(ADAPTER_SOURCE.contains("private-window cannot use a durable context"));
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
        QML_SOURCE,
        include_str!("../../qml/components/FerricSettings.qml"),
        include_str!("../../qml/components/FerricJourneySearch.qml"),
        include_str!("../../qml/components/FerricSettingRows.qml"),
        include_str!("../../qml/components/FerricLibraryManager.qml"),
        include_str!("../../qml/components/FerricLibraryEntries.qml"),
        include_str!("../../qml/components/FerricCommandLine.qml"),
        include_str!("../../qml/scripts/SpellcheckPresentation.js"),
    ]
    .concat();
    let reopen_confirmation = include_str!("../../qml/components/FerricReopenWindowConfirmation.qml");
    let downloads = include_str!("../../qml/components/FerricDownloadManager.qml");
    let source = ADAPTER_SOURCE;
    let hint_policy = include_str!("../hint_policy.rs");
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
