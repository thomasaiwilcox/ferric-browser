#[allow(clippy::wildcard_imports)]
use super::*;
use crate::cli_parse::log_level_rank;
use crate::ipc_client::{add_cli_context, storage_diagnostics};

#[test]
fn generated_command_usage_preserves_typed_flag_shapes() {
    let registry = CommandRegistry::default_v1();
    let open = registry.resolve("open").expect("open definition");
    assert_eq!(
        command_usage(open),
        "open <input> [--target <TARGET>] [--profile <PROFILE>] [--context <CONTEXT>] [--ephemeral] [--clean-link]"
    );
    let search = registry.resolve("search").expect("search definition");
    assert_eq!(
        command_usage(search),
        "search <query> [--backward] [--case <CASE>]"
    );
    let search_next = registry
        .resolve("search-next")
        .expect("search-next definition");
    assert_eq!(
        command_usage(search_next),
        "search-next [--backward] [--count <COUNT>]"
    );
    let context = registry
        .resolve("context-create")
        .expect("context-create definition");
    assert_eq!(
        command_usage(context),
        "context-create <name> [--label <LABEL>] --profile <PROFILE> [--workspace <WORKSPACE>]"
    );
}

#[test]
fn structured_logging_uses_bounded_levels_and_safe_event_names() {
    assert_eq!(log_level_rank("error"), Some(0));
    assert_eq!(log_level_rank("warn"), Some(1));
    assert_eq!(log_level_rank("info"), Some(2));
    assert_eq!(log_level_rank("debug"), Some(3));
    assert_eq!(log_level_rank("trace"), None);
    assert_eq!(cli_action_name(&CliAction::ConfigCheck), "config-check");
    assert_eq!(
        cli_action_name(&CliAction::Open {
            input: "https://private.example/?token=secret".into(),
            additional_inputs: Vec::new(),
            target: None,
            clean_link: false,
            explicit_input: true,
            ephemeral: false,
        }),
        "open"
    );
}

#[test]
fn diagnostics_bound_and_order_profile_inspection() {
    let base = std::env::temp_dir().join(format!(
        "ferric-browser-diagnostics-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("storage roots");
    let profiles = roots.data.join("profiles");
    for index in 0..(MAX_DIAGNOSTIC_PROFILES + 2) {
        let id = Uuid::from_u128(index as u128).to_string();
        std::fs::create_dir_all(profiles.join(id)).expect("profile directory");
    }

    let report = storage_diagnostics(&roots);
    assert_eq!(report["profiles"].as_array().map(Vec::len), Some(128));
    assert_eq!(report["profile_limit"], MAX_DIAGNOSTIC_PROFILES);
    assert_eq!(report["profiles_truncated"], true);
    let ids = report["profiles"]
        .as_array()
        .expect("profile report")
        .iter()
        .map(|profile| profile["id"].as_str().expect("profile ID"))
        .collect::<Vec<_>>();
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        ids.first().copied(),
        Some("00000000-0000-0000-0000-000000000000")
    );
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn default_browser_is_an_explicit_cli_operation() {
    let (_, status) = parse_cli(&["default-browser".into(), "status".into()])
        .expect("valid default-browser status");
    assert!(matches!(
        status,
        CliAction::DefaultBrowser {
            operation: DefaultBrowserOperation::Status
        }
    ));

    let (_, set) =
        parse_cli(&["default-browser".into(), "set".into()]).expect("valid default-browser set");
    assert!(matches!(
        set,
        CliAction::DefaultBrowser {
            operation: DefaultBrowserOperation::Set
        }
    ));
    assert!(parse_cli(&["default-browser".into()]).is_err());
    assert!(parse_cli(&["default-browser".into(), "set".into(), "extra".into()]).is_err());
}

#[test]
fn reset_data_requires_explicit_confirmation() {
    let (_, action) =
        parse_cli(&["reset-data".into(), "--confirm".into()]).expect("explicit reset command");
    assert!(matches!(action, CliAction::ResetData));
    assert!(parse_cli(&["reset-data".into()]).is_err());
    assert!(parse_cli(&["reset-data".into(), "--force".into()]).is_err());
}

#[test]
fn cli_startup_scalar_options_are_not_overwritten() {
    for arguments in [
        vec![
            "--basedir".into(),
            "/tmp/one".into(),
            "--basedir".into(),
            "/tmp/two".into(),
        ],
        vec![
            "--config".into(),
            "/tmp/one.toml".into(),
            "--config".into(),
            "/tmp/two.toml".into(),
        ],
        vec![
            "--profile".into(),
            "one".into(),
            "--profile".into(),
            "two".into(),
        ],
        vec![
            "--context".into(),
            "one".into(),
            "--context".into(),
            "two".into(),
        ],
        vec![
            "--instance".into(),
            "one".into(),
            "--instance".into(),
            "two".into(),
        ],
        vec![
            "--log-level".into(),
            "info".into(),
            "--log-level".into(),
            "debug".into(),
        ],
        vec!["--safe-mode".into(), "--safe-mode".into()],
    ] {
        assert!(parse_cli(&arguments).is_err(), "arguments: {arguments:?}");
    }
}

#[test]
fn cli_startup_settings_are_bounded() {
    let mut arguments = Vec::new();
    for index in 0..MAX_STARTUP_SETTINGS {
        arguments.push("--set".into());
        arguments.push(format!("setting{index}=value"));
    }
    assert!(parse_cli(&arguments).is_ok());
    arguments.push("--set".into());
    arguments.push("overflow=value".into());
    assert!(parse_cli(&arguments).is_err());
}

#[test]
fn cli_selection_search_validates_engine_name_at_the_envelope_boundary() {
    let valid = command_params(&ParsedCommand {
        name: "selection-search".into(),
        arguments: vec!["--engine".into(), "ddg".into()],
    })
    .expect("valid selection-search envelope");
    assert_eq!(valid["arguments"]["engine"], "ddg");
    for engine in [String::new(), "bad\nengine".into()] {
        assert!(
            command_params(&ParsedCommand {
                name: "selection-search".into(),
                arguments: vec!["--engine".into(), engine],
            })
            .is_err()
        );
    }
}

#[test]
fn cli_paste_open_preserves_target_and_primary_channel() {
    let typed = command_params(&ParsedCommand {
        name: "paste-open".into(),
        arguments: vec!["--target".into(), "tab".into(), "--primary".into()],
    })
    .expect("typed paste-open envelope");
    assert_eq!(
        typed["arguments"],
        serde_json::json!({"target": "tab", "primary": true})
    );
    assert!(
        command_params(&ParsedCommand {
            name: "paste-open".into(),
            arguments: vec!["--target".into(), "window".into()],
        })
        .is_err()
    );
}

#[test]
fn cli_context_create_validates_text_fields_at_the_envelope_boundary() {
    let valid = command_params(&ParsedCommand {
        name: "context-create".into(),
        arguments: vec![
            "work".into(),
            "--label".into(),
            "Work".into(),
            "--profile".into(),
            "work".into(),
            "--workspace".into(),
            "2".into(),
        ],
    })
    .expect("valid context-create envelope");
    assert_eq!(valid["arguments"]["name"], "work");
    assert_eq!(valid["arguments"]["label"], "Work");

    assert!(
        command_params(&ParsedCommand {
            name: "context-create".into(),
            arguments: vec!["work".into()],
        })
        .is_err()
    );

    for arguments in [
        vec![String::new()],
        vec!["work".into(), "--label".into(), "bad\nlabel".into()],
        vec!["work".into(), "--profile".into(), String::new()],
        vec!["work".into(), "--workspace".into(), "bad\tworkspace".into()],
    ] {
        assert!(
            command_params(&ParsedCommand {
                name: "context-create".into(),
                arguments,
            })
            .is_err()
        );
    }
}

#[test]
fn cli_tab_move_context_preserves_optional_index() {
    let active = command_params(&ParsedCommand {
        name: "tab-move".into(),
        arguments: vec!["--context".into(), "work".into()],
    })
    .expect("active tab context move envelope");
    assert_eq!(active["arguments"], serde_json::json!({"context": "work"}));
    let indexed = command_params(&ParsedCommand {
        name: "tab-move".into(),
        arguments: vec!["2".into(), "--context".into(), "work".into()],
    })
    .expect("indexed context move envelope");
    assert_eq!(
        indexed["arguments"],
        serde_json::json!({"id": "2", "context": "work"})
    );
}

#[test]
fn cli_tab_move_context_rejects_untrusted_text() {
    for arguments in [
        vec!["--context".into(), "work\nprod".into()],
        vec!["2\tstale".into(), "--context".into(), "work".into()],
    ] {
        assert!(
            command_params(&ParsedCommand {
                name: "tab-move".into(),
                arguments,
            })
            .is_err()
        );
    }
}

#[test]
fn cli_tab_window_and_download_ids_reject_untrusted_text() {
    for (name, arguments) in [
        ("tab-give", vec!["window\n1".into()]),
        ("tab-select", vec!["tab\t1".into()]),
        ("tab-focus", vec!["tab\n1".into()]),
        ("tab-close", vec!["--id".into(), "tab\n1".into()]),
        ("window-focus", vec!["window\n1".into()]),
        (
            "window-move",
            vec!["window-1".into(), "workspace\t1".into()],
        ),
        ("download-open", vec!["download\n1".into()]),
    ] {
        assert!(
            command_params(&ParsedCommand {
                name: name.into(),
                arguments,
            })
            .is_err(),
            "{name} accepted untrusted identifier text"
        );
    }
}

#[test]
fn cli_profile_and_context_names_validate_at_the_envelope_boundary() {
    for command_name in [
        "context-delete",
        "context-enter",
        "context-save",
        "profile-create",
        "profile-delete",
        "profile-open",
        "session-delete",
        "session-load",
        "session-save",
    ] {
        assert!(
            command_params(&ParsedCommand {
                name: command_name.into(),
                arguments: vec!["bad\nname".into()],
            })
            .is_err(),
            "command should reject control characters: {command_name}"
        );
    }
}

#[test]
fn cli_library_identifiers_validate_at_the_envelope_boundary() {
    for (name, arguments) in [
        ("bookmark-delete", vec!["bad\nID".into()]),
        (
            "bookmark-edit",
            vec!["bad\nID".into(), "--title".into(), "Title".into()],
        ),
        ("bookmark-open", vec!["bad\nID".into()]),
        ("history-open", vec!["bad\nID".into()]),
        ("quickmark-add", vec!["bad\nname".into()]),
        ("quickmark-delete", vec!["bad\nname".into()]),
        (
            "quickmark-edit",
            vec!["bad\nname".into(), "https://example.test".into()],
        ),
        ("quickmark-open", vec!["bad\nname".into()]),
    ] {
        assert!(
            command_params(&ParsedCommand {
                name: name.into(),
                arguments,
            })
            .is_err(),
            "command should reject control characters: {name}"
        );
    }
}
use ferric_browser_config::{ContextDefinition, ProfileDefinition};
use std::collections::BTreeMap;

#[test]
fn cli_open_keeps_colon_input_as_data() {
    let arguments = vec!["open".into(), "--".into(), ":https://example.test".into()];
    let (_, action) = parse_cli(&arguments).expect("valid open command");
    assert!(matches!(
        action,
        CliAction::Open { input, target: None, .. } if input == ":https://example.test"
    ));
}

#[test]
fn cli_open_accepts_explicit_clean_link() {
    let arguments = vec![
        "open".into(),
        "--clean-link".into(),
        "https://example.test/?utm_source=demo".into(),
    ];
    let (_, action) = parse_cli(&arguments).expect("valid clean-link open");
    assert!(matches!(
        action,
        CliAction::Open {
            input,
            clean_link: true,
            explicit_input: true,
            ..
        } if input == "https://example.test/?utm_source=demo"
    ));
}

#[test]
fn cli_open_accepts_typed_route_options_and_joins_input_tail() {
    let arguments = vec![
        "open".into(),
        "--target".into(),
        "current".into(),
        "--profile".into(),
        "work".into(),
        "--context".into(),
        "calendar".into(),
        "quarterly".into(),
        "report".into(),
    ];
    let (options, action) = parse_cli(&arguments).expect("valid typed open command");
    assert_eq!(options.profile.as_deref(), Some("work"));
    assert_eq!(options.context.as_deref(), Some("calendar"));
    assert!(matches!(
        action,
        CliAction::Open {
            input,
            target: Some(target),
            explicit_input: true,
            ..
        } if input == "quarterly report" && target == "tab"
    ));
}

#[test]
fn cli_open_accepts_cold_start_background_target() {
    let (_, action) = parse_cli(&[
        "open".into(),
        "--target".into(),
        "tab-bg".into(),
        "https://example.test/one".into(),
    ])
    .expect("valid cold-start background open");
    assert!(matches!(
        action,
        CliAction::Open {
            input,
            target: Some(target),
            explicit_input: true,
            ..
        } if input == "https://example.test/one" && target == "tab-bg"
    ));
}

#[test]
fn cli_open_rejects_clean_link_with_cold_start_background_target() {
    let error = parse_cli(&[
        "open".into(),
        "--target".into(),
        "tab-bg".into(),
        "--clean-link".into(),
        "https://example.test/one?utm_source=demo".into(),
    ])
    .expect_err("clean-link background open must be rejected before startup");
    assert!(error.contains("cannot be combined with --target tab-bg"));
}

#[test]
fn cli_open_keeps_multiple_url_arguments_as_separate_inputs() {
    let (_, action) = parse_cli(&[
        "open".into(),
        "https://example.test/one".into(),
        "https://example.test/two".into(),
        "file:///tmp/three.html".into(),
    ])
    .expect("valid multiple-url open command");
    assert!(matches!(
        action,
        CliAction::Open {
            input,
            additional_inputs,
            explicit_input: true,
            ..
        } if input == "https://example.test/one"
            && additional_inputs == [
                "https://example.test/two".to_owned(),
                "file:///tmp/three.html".to_owned()
            ]
    ));

    let (_, action) =
        parse_cli(&["open".into(), "hello".into(), "world".into()]).expect("valid search input");
    assert!(matches!(
        action,
        CliAction::Open {
            input,
            additional_inputs,
            ..
        } if input == "hello world" && additional_inputs.is_empty()
    ));
}

#[test]
fn cli_open_option_terminator_preserves_option_like_input_tail() {
    let arguments = vec![
        "open".into(),
        "--".into(),
        "--profile".into(),
        "literal".into(),
    ];
    let (_, action) = parse_cli(&arguments).expect("valid data-only open command");
    assert!(matches!(
        action,
        CliAction::Open { input, .. } if input == "--profile literal"
    ));
}

#[test]
fn cli_catalog_commands_are_forwarded_instead_of_becoming_open_input() {
    let (_, action) = parse_cli(&["reload".into(), "--bypass-cache".into()])
        .expect("registered commands use the direct CLI entry point");
    assert!(matches!(
        action,
        CliAction::Command { text, window: None } if text == "reload --bypass-cache"
    ));

    let (_, action) = parse_cli(&["profile-list".into()])
        .expect("registered no-argument commands use the direct CLI entry point");
    assert!(matches!(
        action,
        CliAction::Command { text, window: None } if text == "profile-list"
    ));
}

#[test]
fn cli_builtin_aliases_expand_before_typed_forwarding() {
    let commands =
        parse_chain("o https://example.test", ParseInput::Cli).expect("alias command parses");
    let expanded = CommandRegistry::default_v1()
        .expand_chain(commands)
        .expect("built-in alias expands");
    assert_eq!(expanded[0].name, "open");
    assert_eq!(expanded[0].arguments, vec!["https://example.test"]);
}

#[test]
fn startup_profile_selection_follows_context_profile_definition() {
    let options = CliOptions {
        context: Some("calendar".into()),
        ..CliOptions::default()
    };
    let profiles = ProfilesConfig {
        profiles: vec![ProfileDefinition {
            name: "work".into(),
            label: "Work".into(),
            default: false,
            overrides: BTreeMap::default(),
        }],
        ..ProfilesConfig::default()
    };
    let contexts = ContextsConfig {
        contexts: vec![ContextDefinition {
            name: "calendar".into(),
            label: "Calendar".into(),
            profile: "work".into(),
            sessions: Vec::new(),
            workspace: None,
            accent: None,
            default_target: "reuse-or-window".into(),
        }],
        ..ContextsConfig::default()
    };
    let (_, name, label, startup_context) =
        select_startup_profile(&options, &profiles, &contexts, "default", "Default")
            .expect("context selects a valid startup profile");
    assert_eq!(name, "work");
    assert_eq!(label, "Work");
    assert_eq!(startup_context.as_deref(), Some("calendar"));
}

#[test]
fn startup_profile_selection_rejects_conflicting_context_profile() {
    let options = CliOptions {
        profile: Some("personal".into()),
        context: Some("calendar".into()),
        ..CliOptions::default()
    };
    let contexts = ContextsConfig {
        contexts: vec![ContextDefinition {
            name: "calendar".into(),
            label: "Calendar".into(),
            profile: "work".into(),
            sessions: Vec::new(),
            workspace: None,
            accent: None,
            default_target: "reuse-or-window".into(),
        }],
        ..ContextsConfig::default()
    };
    let error = select_startup_profile(
        &options,
        &ProfilesConfig::default(),
        &contexts,
        "default",
        "Default",
    )
    .expect_err("conflicting selectors must be rejected");
    assert!(error.contains("profile and context selectors disagree"));
}

#[test]
fn cli_open_accepts_ephemeral_profile_flag() {
    let (_, action) = parse_cli(&[
        "open".into(),
        "--ephemeral".into(),
        "--".into(),
        "https://example.test".into(),
    ])
    .expect("valid ephemeral open");
    assert!(matches!(
        action,
        CliAction::Open {
            ephemeral: true,
            ..
        }
    ));
}

#[test]
fn cli_hint_accepts_ephemeral_target_without_rapid_mode() {
    let command = parse_chain("hint --target ephemeral links", ParseInput::Cli)
        .expect("valid ephemeral hint command")
        .pop()
        .expect("hint command");
    let params = command_params(&command).expect("typed ephemeral hint parameters");
    assert_eq!(params["arguments"]["target"], "ephemeral");
    assert_eq!(params["arguments"]["kind"], "links");
}

#[test]
fn cli_profile_create_accepts_ephemeral_flag() {
    let command = parse_chain("profile-create task --ephemeral", ParseInput::Cli)
        .expect("valid ephemeral profile command")
        .pop()
        .expect("profile command");
    let params = command_params(&command).expect("typed profile parameters");
    assert_eq!(
        params["arguments"],
        serde_json::json!({"name": "task", "ephemeral": true})
    );
}

#[test]
fn cli_profile_open_accepts_an_optional_url() {
    let command = parse_chain("profile-open work https://example.test", ParseInput::Cli)
        .expect("valid profile-open command")
        .pop()
        .expect("profile-open command");
    let params = command_params(&command).expect("typed profile-open parameters");
    assert_eq!(
        params["arguments"],
        serde_json::json!({"name": "work", "input": "https://example.test"})
    );
}

#[test]
fn cli_profile_delete_keeps_the_name_typed() {
    let command = parse_chain("profile-delete work", ParseInput::Cli)
        .expect("valid profile-delete command")
        .pop()
        .expect("profile-delete command");
    let params = command_params(&command).expect("typed profile-delete parameters");
    assert_eq!(params["arguments"], serde_json::json!({"name": "work"}));
}

#[test]
fn cli_fullscreen_defaults_to_toggle_and_validates_state() {
    let command = parse_chain("fullscreen", ParseInput::Cli)
        .expect("valid fullscreen command")
        .pop()
        .expect("fullscreen command");
    let params = command_params(&command).expect("typed fullscreen parameters");
    assert_eq!(params["arguments"], serde_json::json!({"state": "toggle"}));
    let command = parse_chain("fullscreen off", ParseInput::Cli)
        .expect("valid fullscreen off command")
        .pop()
        .expect("fullscreen command");
    assert_eq!(
        command_params(&command).expect("fullscreen off parameters")["arguments"],
        serde_json::json!({"state": "off"})
    );
    assert!(
        parse_chain("fullscreen sideways", ParseInput::Cli)
            .ok()
            .and_then(|mut commands| commands.pop())
            .and_then(|command| command_params(&command).ok())
            .is_none()
    );
}

#[test]
fn cli_window_new_types_profile_and_private_options() {
    let command = parse_chain("window-new --profile work --private", ParseInput::Cli)
        .expect("valid window-new command")
        .pop()
        .expect("window-new command");
    let params = command_params(&command).expect("typed window-new parameters");
    assert_eq!(
        params["arguments"],
        serde_json::json!({"profile": "work", "private": true})
    );
    assert!(
        command_params(&ParsedCommand {
            name: "window-new".into(),
            arguments: vec!["--private".into(), "--private".into()],
        })
        .is_err()
    );
}

#[test]
fn cli_tab_open_types_input_and_background_flag() {
    let command = parse_chain(
        "tab-open --background -- https://example.test",
        ParseInput::Cli,
    )
    .expect("valid tab-open command")
    .pop()
    .expect("tab-open command");
    assert_eq!(
        command_params(&command).expect("typed tab-open parameters")["arguments"],
        serde_json::json!({"input": "https://example.test", "background": true})
    );
    assert!(
        command_params(&ParsedCommand {
            name: "tab-open".into(),
            arguments: vec!["--background".into(), "--background".into(), "x".into()],
        })
        .is_err()
    );
}

#[test]
fn cli_history_commands_type_bounded_counts() {
    let command = parse_chain("back --count 4", ParseInput::Cli)
        .expect("valid history command")
        .pop()
        .expect("back command");
    assert_eq!(
        command_params(&command).expect("typed history parameters")["arguments"],
        serde_json::json!({"count": 4})
    );
    assert!(
        command_params(&ParsedCommand {
            name: "forward".into(),
            arguments: vec!["--count".into(), "101".into()],
        })
        .is_err()
    );
    let command = parse_chain("tab-next --count 4", ParseInput::Cli)
        .expect("valid tab traversal command")
        .pop()
        .expect("tab-next command");
    assert_eq!(
        command_params(&command).expect("typed tab traversal parameters")["arguments"],
        serde_json::json!({"count": 4})
    );
    assert!(
        command_params(&ParsedCommand {
            name: "tab-prev".into(),
            arguments: vec!["--count".into(), "101".into()],
        })
        .is_err()
    );
}

#[test]
fn cli_history_clear_forwards_exact_scope_filters() {
    let command = parse_chain(
        "history-clear --since 1757894400 --origin https://example.test --confirm",
        ParseInput::Cli,
    )
    .expect("valid history clear command")
    .pop()
    .expect("history clear command");
    assert_eq!(
        command_params(&command).expect("typed history clear parameters"),
        serde_json::json!({
            "command": "history-clear",
            "arguments": {
                "since": 1_757_894_400,
                "origin": "https://example.test",
                "confirmed": true
            }
        })
    );
    assert!(
        command_params(&ParsedCommand {
            name: "history-clear".into(),
            arguments: vec![
                "--origin".into(),
                "https://example.test/path".into(),
                "--confirm".into()
            ],
        })
        .is_err()
    );
}

#[test]
fn cli_tab_give_types_the_target_window_id() {
    let command = parse_chain("tab-give window-42", ParseInput::Cli)
        .expect("valid tab-give command")
        .pop()
        .expect("tab-give command");
    let params = command_params(&command).expect("typed tab-give parameters");
    assert_eq!(
        params["arguments"],
        serde_json::json!({"window_id": "window-42"})
    );
    assert!(
        command_params(&ParsedCommand {
            name: "tab-give".into(),
            arguments: Vec::new(),
        })
        .is_err()
    );
}

#[test]
fn cli_reload_types_bypass_cache() {
    let command = parse_chain("reload --bypass-cache", ParseInput::Cli)
        .expect("valid reload command")
        .pop()
        .expect("reload command");
    let params = command_params(&command).expect("typed reload parameters");
    assert_eq!(
        params["arguments"],
        serde_json::json!({"bypass_cache": true})
    );
    let command = parse_chain("reload", ParseInput::Cli)
        .expect("valid default reload command")
        .pop()
        .expect("reload command");
    assert_eq!(
        command_params(&command).expect("typed default reload parameters")["arguments"],
        serde_json::json!({"bypass_cache": false})
    );
    assert!(
        command_params(&ParsedCommand {
            name: "reload".into(),
            arguments: vec!["--bypass-cache".into(), "--bypass-cache".into()],
        })
        .is_err()
    );
}

#[test]
fn cli_tab_close_types_optional_id_and_count() {
    let command = parse_chain("tab-close --id tab-42", ParseInput::Cli)
        .expect("valid targeted tab-close command")
        .pop()
        .expect("tab-close command");
    assert_eq!(
        command_params(&command).expect("typed targeted tab-close parameters")["arguments"],
        serde_json::json!({"id": "tab-42"})
    );
    let command = parse_chain("tab-close --count 3", ParseInput::Cli)
        .expect("valid bulk tab-close command")
        .pop()
        .expect("tab-close command");
    assert_eq!(
        command_params(&command).expect("typed bulk tab-close parameters")["arguments"],
        serde_json::json!({"count": 3})
    );
    assert!(
        command_params(&ParsedCommand {
            name: "tab-close".into(),
            arguments: vec!["--id".into(), "tab-42".into(), "--count".into(), "2".into()],
        })
        .is_err()
    );
}

#[test]
fn cli_search_types_query_direction_and_case() {
    let command = parse_chain(
        "search --backward --case sensitive \"Ferric Browser\"",
        ParseInput::Cli,
    )
    .expect("valid search command")
    .pop()
    .expect("search command");
    let params = command_params(&command).expect("typed search parameters");
    assert_eq!(
        params["arguments"],
        serde_json::json!({
            "query": "Ferric Browser",
            "backward": true,
            "case": "sensitive"
        })
    );
    assert!(
        command_params(&ParsedCommand {
            name: "search".into(),
            arguments: vec!["--case".into(), "bogus".into(), "query".into()],
        })
        .is_err()
    );
    assert!(
        command_params(&ParsedCommand {
            name: "search".into(),
            arguments: vec![
                "--case".into(),
                "sensitive".into(),
                "--case".into(),
                "smart".into(),
                "query".into(),
            ],
        })
        .is_err()
    );
}

#[test]
fn cli_journey_accepts_current_filter() {
    let command = parse_chain("journey --current", ParseInput::Cli)
        .expect("valid journey command")
        .pop()
        .expect("journey command");
    let params = command_params(&command).expect("typed journey parameters");
    assert_eq!(params["arguments"], serde_json::json!({"current": true}));

    for arguments in [
        vec!["--current".into(), "--current".into()],
        vec![
            "--search".into(),
            "one".into(),
            "--search".into(),
            "two".into(),
        ],
        vec![
            "--expand".into(),
            "one".into(),
            "--expand".into(),
            "two".into(),
        ],
    ] {
        assert!(
            command_params(&ParsedCommand {
                name: "journey".into(),
                arguments,
            })
            .is_err()
        );
    }
}

#[test]
fn cli_journey_accepts_bounded_search_and_expansion_filters() {
    let command = parse_chain("journey --search \"example.test\"", ParseInput::Cli)
        .expect("valid journey search command")
        .pop()
        .expect("journey command");
    let params = command_params(&command).expect("typed journey search parameters");
    assert_eq!(params["arguments"]["search"], "example.test");

    for search in ["", "bad\nsearch", &"x".repeat(257)] {
        assert!(
            command_params(&ParsedCommand {
                name: "journey".into(),
                arguments: vec!["--search".into(), search.into()],
            })
            .is_err()
        );
    }

    let command = parse_chain(
        "journey --expand 123e4567-e89b-12d3-a456-426614174000",
        ParseInput::Cli,
    )
    .expect("valid journey expansion command")
    .pop()
    .expect("journey command");
    let params = command_params(&command).expect("typed journey expansion parameters");
    assert_eq!(
        params["arguments"]["expand"],
        "123e4567-e89b-12d3-a456-426614174000"
    );
    assert!(
        command_params(&ParsedCommand {
            name: "journey".into(),
            arguments: vec!["--expand".into(), "not-a-uuid".into()],
        })
        .is_err()
    );
}

#[test]
fn cli_journey_reopen_accepts_uuid_and_target() {
    let command = parse_chain(
        "journey-reopen 123e4567-e89b-12d3-a456-426614174000 --target tab",
        ParseInput::Cli,
    )
    .expect("valid journey reopen command")
    .pop()
    .expect("journey reopen command");
    let params = command_params(&command).expect("typed journey reopen parameters");
    assert_eq!(
        params["arguments"],
        serde_json::json!({
            "node": "123e4567-e89b-12d3-a456-426614174000",
            "target": "tab"
        })
    );
    assert!(
        command_params(&ParsedCommand {
            name: "journey-reopen".into(),
            arguments: vec!["not-a-uuid".into()],
        })
        .is_err()
    );
}

#[test]
fn cli_journey_reopen_accepts_window_target() {
    let command = parse_chain(
        "journey-reopen 123e4567-e89b-12d3-a456-426614174000 --target window",
        ParseInput::Cli,
    )
    .expect("valid window journey reopen command")
    .pop()
    .expect("journey command");
    let params = command_params(&command).expect("typed window journey parameters");
    assert_eq!(params["arguments"]["target"], "window");
}

#[test]
fn cli_query_selects_active_tab_without_starting_a_gui() {
    let arguments = vec![
        "query".into(),
        "active-tab".into(),
        "--format".into(),
        "json".into(),
    ];
    let (_, action) = parse_cli(&arguments).expect("valid query command");
    assert!(matches!(
        action,
        CliAction::Query { method, params, format }
            if method == "tabs.query"
                && params["active_only"] == true
                && format == "json"
    ));
}

#[test]
fn cli_query_exposes_live_window_registry() {
    let arguments = vec![
        "query".into(),
        "windows".into(),
        "--format".into(),
        "json".into(),
    ];
    let (_, action) = parse_cli(&arguments).expect("valid windows query command");
    assert!(matches!(
        action,
        CliAction::Query { method, params, format }
            if method == "windows.query" && params == serde_json::json!({}) && format == "json"
    ));
}

#[test]
fn cli_switcher_command_uses_typed_scope_and_query_fields() {
    let command = parse_chain("switcher --scope tabs rust", ParseInput::Cli)
        .expect("valid switcher command")
        .remove(0);
    let params = command_params(&command).expect("typed switcher parameters");
    assert_eq!(params["arguments"]["scope"], "tabs");
    assert_eq!(params["arguments"]["query"], "rust");
    assert!(
        command_params(&ParsedCommand {
            name: "switcher".into(),
            arguments: vec![
                "--scope".into(),
                "tabs".into(),
                "--scope".into(),
                "history".into(),
            ],
        })
        .is_err()
    );
}

#[test]
fn cli_activate_accepts_only_stable_tab_ids() {
    let (_, action) = parse_cli(&["activate".into(), "tab".into(), "tab-123".into()])
        .expect("valid tab activation");
    assert!(matches!(
        action,
        CliAction::Activate { kind, id } if kind == "tab" && id == "tab-123"
    ));
    assert!(parse_cli(&["activate".into(), "window".into(), "window-123".into()]).is_err());
}

#[test]
fn cli_tab_and_window_commands_use_typed_subject_fields() {
    let select = command_params(&ParsedCommand {
        name: "tab-select".into(),
        arguments: vec!["tab-42".into()],
    })
    .expect("typed tab-select parameters");
    assert_eq!(select["arguments"]["selector"], "tab-42");

    let mute = command_params(&ParsedCommand {
        name: "tab-mute".into(),
        arguments: vec!["tab-42".into(), "off".into()],
    })
    .expect("typed tab-mute parameters");
    assert_eq!(mute["arguments"]["id"], "tab-42");
    assert_eq!(mute["arguments"]["state"], "off");
    let mute_active = command_params(&ParsedCommand {
        name: "tab-mute".into(),
        arguments: vec!["toggle".into()],
    })
    .expect("typed active-tab mute parameters");
    assert!(mute_active["arguments"].get("id").is_none());
    assert_eq!(mute_active["arguments"]["state"], "toggle");

    let move_tab = command_params(&ParsedCommand {
        name: "tab-move".into(),
        arguments: vec!["tab-42".into(), "left".into()],
    })
    .expect("typed tab-move parameters");
    assert_eq!(move_tab["arguments"]["direction"], "left");
    let move_active = command_params(&ParsedCommand {
        name: "tab-move".into(),
        arguments: vec!["right".into()],
    })
    .expect("typed active-tab move parameters");
    assert!(move_active["arguments"].get("id").is_none());
    assert_eq!(move_active["arguments"]["direction"], "right");

    let focus = command_params(&ParsedCommand {
        name: "window-focus".into(),
        arguments: vec!["window-7".into()],
    })
    .expect("typed window-focus parameters");
    assert_eq!(focus["arguments"]["id"], "window-7");
}

#[test]
fn cli_navigation_and_configuration_commands_use_typed_arguments() {
    let tab_open = command_params(&ParsedCommand {
        name: "tab-open".into(),
        arguments: vec!["--background".into(), "https://example.test/a".into()],
    })
    .expect("typed tab-open parameters");
    assert_eq!(tab_open["arguments"]["input"], "https://example.test/a");
    assert_eq!(tab_open["arguments"]["background"], true);

    let scroll = command_params(&ParsedCommand {
        name: "scroll-page".into(),
        arguments: vec!["down".into(), "--half".into(), "--count".into(), "3".into()],
    })
    .expect("typed scroll-page parameters");
    assert_eq!(scroll["arguments"]["direction"], "down");
    assert_eq!(scroll["arguments"]["half"], true);
    assert_eq!(scroll["arguments"]["count"], 3);

    let binding = command_params(&ParsedCommand {
        name: "bind".into(),
        arguments: vec![
            "--mode".into(),
            "normal".into(),
            "gg".into(),
            "scroll-page down".into(),
        ],
    })
    .expect("typed bind parameters");
    assert_eq!(binding["arguments"]["mode"], "normal");
    assert_eq!(binding["arguments"]["keychain"], "gg");
    assert_eq!(binding["arguments"]["command"], "scroll-page down");

    let search = command_params(&ParsedCommand {
        name: "search-next".into(),
        arguments: vec!["--backward".into(), "--count".into(), "4".into()],
    })
    .expect("typed search-next parameters");
    assert_eq!(search["arguments"]["direction"], "backward");
    assert_eq!(search["arguments"]["count"], 4);

    assert!(
        command_params(&ParsedCommand {
            name: "zoom".into(),
            arguments: vec!["9".into()],
        })
        .is_err()
    );
    assert!(
        command_params(&ParsedCommand {
            name: "search-next".into(),
            arguments: vec!["--count".into(), "2".into(), "--count".into(), "3".into()],
        })
        .is_err()
    );
    assert!(
        command_params(&ParsedCommand {
            name: "bind".into(),
            arguments: vec!["--mode".into(), "invalid".into(), "g".into(), "help".into()],
        })
        .is_err()
    );
}

#[test]
fn every_registered_command_example_has_a_typed_cli_envelope() {
    let registry = CommandRegistry::default_v1();
    let mut checked = 0;
    for definition in registry.definitions() {
        for example in &definition.examples {
            let commands = parse_chain(example, ParseInput::Cli).unwrap_or_else(|error| {
                panic!("{} example does not parse: {error}", definition.name)
            });
            let commands = registry.expand_chain(commands).unwrap_or_else(|error| {
                panic!("{} example does not expand: {error}", definition.name)
            });
            for command in commands {
                command_params(&command).unwrap_or_else(|error| {
                    panic!(
                        "{} example has no typed CLI envelope: {error}",
                        definition.name
                    )
                });
                checked += 1;
            }
        }
    }
    assert!(
        checked >= 100,
        "expected broad command example coverage, got {checked}"
    );
}

#[test]
fn cli_query_supports_bindings_and_config_keys() {
    let (_, action) = parse_cli(&[
        "query".into(),
        "bindings".into(),
        "--mode".into(),
        "normal".into(),
    ])
    .expect("valid binding query");
    assert!(matches!(
        action,
        CliAction::Query { method, params, .. }
            if method == "bindings.query" && params["mode"] == "normal"
    ));

    let (_, action) =
        parse_cli(&["query".into(), "operations".into()]).expect("valid operations query");
    assert!(matches!(
        action,
        CliAction::Query { method, params, .. }
            if method == "operations.query" && params == serde_json::json!({})
    ));

    let (_, action) =
        parse_cli(&["query".into(), "blocking".into()]).expect("valid blocking query");
    assert!(matches!(
        action,
        CliAction::Query { method, params, .. }
            if method == "blocking.status" && params == serde_json::json!({})
    ));

    let (_, action) = parse_cli(&[
        "query".into(),
        "config".into(),
        "ui.font_size_pt".into(),
        "--explain".into(),
    ])
    .expect("valid config query");
    assert!(matches!(
        action,
        CliAction::Query { method, params, .. }
            if method == "config.get"
                && params["key"] == "ui.font_size_pt"
                && params["explain"] == true
    ));

    let (_, action) = parse_cli(&[
        "query".into(),
        "config".into(),
        "content.zoom".into(),
        "--url".into(),
        "https://example.test/docs".into(),
    ])
    .expect("valid site config query");
    assert!(matches!(
        action,
        CliAction::Query { method, params, .. }
            if method == "config.get"
                && params["key"] == "content.zoom"
                && params["url"] == "https://example.test/docs"
    ));

    let (_, action) = parse_cli(&["query".into(), "contexts".into(), "--members".into()])
        .expect("valid context query");
    assert!(matches!(
        action,
        CliAction::Query { method, params, .. }
            if method == "contexts.query" && params["include_members"] == true
    ));

    let (_, action) = parse_cli(&[
        "query".into(),
        "switcher".into(),
        "--scope".into(),
        "history".into(),
        "rust".into(),
        "--limit".into(),
        "12".into(),
    ])
    .expect("valid switcher query");
    assert!(matches!(
        action,
        CliAction::Query { method, params, .. }
            if method == "switcher.query"
                && params["scope"] == "history"
                && params["query"] == "rust"
                && params["limit"] == 12
    ));

    let (_, action) = parse_cli(&[
        "query".into(),
        "binding-explain".into(),
        "2gt".into(),
        "--mode".into(),
        "normal".into(),
    ])
    .expect("valid binding explanation query");
    assert!(matches!(
        action,
        CliAction::Query { method, params, .. }
            if method == "bindings.explain"
                && params["keychain"] == "2gt"
                && params["mode"] == "normal"
    ));
}

#[test]
fn cli_blocking_toggle_forwards_optional_site_flag() {
    let site = ParsedCommand {
        name: "blocking-toggle".into(),
        arguments: vec!["--site".into()],
    };
    assert_eq!(
        command_params(&site).expect("site toggle params")["arguments"],
        serde_json::json!({"site": true})
    );
    let global = ParsedCommand {
        name: "blocking-toggle".into(),
        arguments: Vec::new(),
    };
    assert_eq!(
        command_params(&global).expect("global toggle params")["arguments"],
        serde_json::json!({})
    );
}

#[test]
fn cli_site_status_forwards_optional_tab_target() {
    let active = ParsedCommand {
        name: "site-status".into(),
        arguments: Vec::new(),
    };
    assert_eq!(
        command_params(&active).expect("active site status params")["arguments"],
        serde_json::json!({})
    );

    let targeted = ParsedCommand {
        name: "site-status".into(),
        arguments: vec!["--tab".into(), "tab-42".into()],
    };
    assert_eq!(
        command_params(&targeted).expect("targeted site status params")["arguments"],
        serde_json::json!({"tab": "tab-42"})
    );
    assert!(
        command_params(&ParsedCommand {
            name: "site-status".into(),
            arguments: vec!["--tab".into(), "bad\nid".into()],
        })
        .is_err()
    );
}

#[test]
fn cli_link_commands_forward_an_optional_url_field() {
    let command = parse_chain(
        "url-explain https://example.test/?utm_source=demo",
        ParseInput::Cli,
    )
    .expect("valid URL explanation command")
    .remove(0);
    assert_eq!(
        command_params(&command).expect("typed command parameters"),
        serde_json::json!({
            "command": "url-explain",
            "arguments": {"url": "https://example.test/?utm_source=demo"}
        })
    );
    let current = parse_chain("url-clean", ParseInput::Cli)
        .expect("valid current URL command")
        .remove(0);
    assert_eq!(
        command_params(&current).expect("current URL parameters"),
        serde_json::json!({"command": "url-clean", "arguments": {}})
    );
}

#[test]
fn cli_hint_commands_forward_rapid_target_options() {
    let command = parse_chain("hint --rapid --target yank links", ParseInput::Cli)
        .expect("valid rapid hint command")
        .remove(0);
    assert_eq!(
        command_params(&command).expect("typed rapid hint parameters"),
        serde_json::json!({
            "command": "hint",
            "arguments": {"kind": "links", "target": "yank", "rapid": true}
        })
    );
}

#[test]
fn cli_hint_commands_accept_qutebrowser_one_shot_targets() {
    for target in ["tab", "tab-bg", "window", "yank", "clean-yank", "download"] {
        let command = parse_chain(&format!("hint --target {target} links"), ParseInput::Cli)
            .expect("valid one-shot hint command")
            .remove(0);
        let params = command_params(&command).expect("typed one-shot hint parameters");
        assert_eq!(params["arguments"]["target"], target);
        assert_eq!(params["arguments"]["kind"], "links");
    }
}

#[test]
fn cli_hint_commands_forward_userscript_target() {
    let command = parse_chain(
        "hint --target userscript --script video links",
        ParseInput::Cli,
    )
    .expect("valid userscript hint command")
    .remove(0);
    assert_eq!(
        command_params(&command).expect("typed userscript hint parameters"),
        serde_json::json!({
            "command": "hint",
            "arguments": {
                "kind": "links",
                "target": "userscript",
                "script": "video"
            }
        })
    );
    let command = parse_chain(
        "hint --rapid --target userscript --script video links",
        ParseInput::Cli,
    )
    .expect("valid rapid userscript hint command")
    .remove(0);
    assert_eq!(
        command_params(&command).expect("typed rapid userscript hint parameters"),
        serde_json::json!({
            "command": "hint",
            "arguments": {
                "kind": "links",
                "target": "userscript",
                "script": "video",
                "rapid": true
            }
        })
    );
}

#[test]
fn cli_download_commands_use_a_typed_url_field() {
    let command = parse_chain("download https://example.test/file.zip", ParseInput::Cli)
        .expect("valid download command")
        .remove(0);
    assert_eq!(
        command_params(&command).expect("typed download parameters"),
        serde_json::json!({
            "command": "download",
            "arguments": {"input": "https://example.test/file.zip"}
        })
    );
}

#[test]
fn cli_print_pdf_command_uses_a_typed_path_field() {
    let command = parse_chain("print-pdf /tmp/page.pdf", ParseInput::Cli)
        .expect("valid print-pdf command")
        .remove(0);
    assert_eq!(
        command_params(&command).expect("typed print-pdf parameters"),
        serde_json::json!({
            "command": "print-pdf",
            "arguments": {"path": "/tmp/page.pdf"}
        })
    );
}

#[test]
fn cli_save_page_command_uses_a_typed_path_field() {
    let command = parse_chain("save-page /tmp/page.html", ParseInput::Cli)
        .expect("valid save-page command")
        .remove(0);
    assert_eq!(
        command_params(&command).expect("typed save-page parameters"),
        serde_json::json!({
            "command": "save-page",
            "arguments": {"path": "/tmp/page.html"}
        })
    );
}

#[test]
fn cli_script_run_command_uses_a_typed_name_field() {
    let command = parse_chain("script-run video", ParseInput::Cli)
        .expect("valid script-run command")
        .remove(0);
    assert_eq!(
        command_params(&command).expect("typed script-run parameters"),
        serde_json::json!({
            "command": "script-run",
            "arguments": {"name": "video"}
        })
    );
}

#[test]
fn cli_jseval_command_uses_typed_world_and_script_fields() {
    let command = parse_chain(
        "jseval --world page \"document.body.dataset.ferric-browser = '1'\"",
        ParseInput::Cli,
    )
    .expect("valid jseval command")
    .remove(0);
    assert_eq!(
        command_params(&command).expect("typed jseval parameters"),
        serde_json::json!({
            "command": "jseval",
            "arguments": {
                "world": "page",
                "script": "document.body.dataset.ferric-browser = '1'"
            }
        })
    );
    let default_world = parse_chain("jseval \"1 + 1\"", ParseInput::Cli)
        .expect("default jseval world")
        .remove(0);
    assert_eq!(
        command_params(&default_world).expect("default jseval parameters"),
        serde_json::json!({"command": "jseval", "arguments": {"world": "isolated", "script": "1 + 1"}})
    );
}

#[test]
fn cli_print_command_uses_an_empty_typed_argument_object() {
    let command = parse_chain("print", ParseInput::Cli)
        .expect("valid print command")
        .remove(0);
    assert_eq!(
        command_params(&command).expect("typed print parameters"),
        serde_json::json!({
            "command": "print",
            "arguments": {}
        })
    );
}

#[test]
fn cli_devtools_command_uses_typed_detach_flag() {
    let command = parse_chain("devtools --detach", ParseInput::Cli)
        .expect("valid devtools command")
        .remove(0);
    assert_eq!(
        command_params(&command).expect("typed devtools parameters"),
        serde_json::json!({
            "command": "devtools",
            "arguments": {"detach": true}
        })
    );
    let toggle = parse_chain("devtools", ParseInput::Cli)
        .expect("valid devtools toggle")
        .remove(0);
    assert_eq!(
        command_params(&toggle).expect("typed devtools toggle parameters"),
        serde_json::json!({
            "command": "devtools",
            "arguments": {"detach": false}
        })
    );
}

#[test]
fn cli_configuration_lifecycle_commands_use_typed_paths_and_no_arguments() {
    for name in [
        "config-edit",
        "config-reload",
        "config-check",
        "theme-reload",
    ] {
        let command = ParsedCommand {
            name: name.into(),
            arguments: Vec::new(),
        };
        assert_eq!(
            command_params(&command).expect("typed lifecycle command")["arguments"],
            serde_json::json!({})
        );
        assert!(
            command_params(&ParsedCommand {
                name: name.into(),
                arguments: vec!["unexpected".into()],
            })
            .is_err()
        );
    }
    let command = parse_chain(
        "config-write-defaults /tmp/ferric-browser-defaults.toml",
        ParseInput::Cli,
    )
    .expect("valid config-write-defaults command")
    .pop()
    .expect("config-write-defaults command");
    assert_eq!(
        command_params(&command).expect("typed config-write-defaults")["arguments"],
        serde_json::json!({"path": "/tmp/ferric-browser-defaults.toml"})
    );
}

#[test]
fn cli_get_command_uses_typed_key_url_and_explain_fields() {
    let command = parse_chain(
        "get content.zoom --url https://example.test/docs --explain",
        ParseInput::Cli,
    )
    .expect("valid get command")
    .remove(0);
    assert_eq!(
        command_params(&command).expect("typed get parameters")["arguments"],
        serde_json::json!({
            "key": "content.zoom",
            "url": "https://example.test/docs",
            "explain": true
        })
    );
    assert!(
        command_params(&ParsedCommand {
            name: "get".into(),
            arguments: vec![
                "content.zoom".into(),
                "--explain".into(),
                "--explain".into()
            ],
        })
        .is_err()
    );
}

#[test]
fn cli_support_commands_use_bounded_typed_arguments() {
    let command = parse_chain("help content.zoom", ParseInput::Cli)
        .expect("valid help command")
        .pop()
        .expect("help command");
    assert_eq!(
        command_params(&command).expect("typed help parameters")["arguments"],
        serde_json::json!({"topic": "content.zoom"})
    );
    for name in ["version", "diagnostics"] {
        let command = ParsedCommand {
            name: name.into(),
            arguments: Vec::new(),
        };
        assert_eq!(
            command_params(&command).expect("typed support parameters")["arguments"],
            serde_json::json!({})
        );
    }
    assert!(
        command_params(&ParsedCommand {
            name: "help".into(),
            arguments: vec!["one".into(), "two".into()],
        })
        .is_err()
    );
}

#[test]
fn cli_permission_commands_use_typed_fields() {
    let command = parse_chain("permissions https://example.test", ParseInput::Cli)
        .expect("valid permissions command")
        .remove(0);
    assert_eq!(
        command_params(&command).expect("typed permissions parameters"),
        serde_json::json!({
            "command": "permissions",
            "arguments": {"origin": "https://example.test"}
        })
    );
    let command = parse_chain(
        "permission-reset https://example.test notifications",
        ParseInput::Cli,
    )
    .expect("valid permission reset command")
    .remove(0);
    assert_eq!(
        command_params(&command).expect("typed permission reset parameters"),
        serde_json::json!({
            "command": "permission-reset",
            "arguments": {
                "origin": "https://example.test",
                "permission": "notifications"
            }
        })
    );
}

#[test]
fn cli_site_data_clear_uses_typed_origin_and_confirmation() {
    let command = parse_chain(
        "site-data-clear https://example.test --confirm",
        ParseInput::Cli,
    )
    .expect("valid site-data clear command")
    .remove(0);
    assert_eq!(
        command_params(&command).expect("typed site-data clear parameters"),
        serde_json::json!({
            "command": "site-data-clear",
            "arguments": {"origin": "https://example.test", "confirmed": true}
        })
    );
}

#[test]
fn cli_download_desktop_commands_use_a_typed_id_field() {
    for name in [
        "download-open",
        "download-show",
        "download-cancel",
        "download-pause",
        "download-resume",
        "download-retry",
    ] {
        let command = parse_chain(&format!("{name} download-1"), ParseInput::Cli)
            .expect("valid download desktop command")
            .remove(0);
        assert_eq!(
            command_params(&command).expect("typed download desktop parameters"),
            serde_json::json!({
                "command": name,
                "arguments": {"id": "download-1"}
            })
        );
    }
}

#[test]
fn cli_action_commands_use_typed_subject_and_verb_fields() {
    let command = parse_chain(
        "action url explain https://example.test/?utm_source=demo",
        ParseInput::Cli,
    )
    .expect("valid action command")
    .remove(0);
    assert_eq!(
        command_params(&command).expect("typed action parameters"),
        serde_json::json!({
            "command": "action",
            "arguments": {
                "subject": "url",
                "verb": "explain",
                "url": "https://example.test/?utm_source=demo"
            }
        })
    );
    let listing = parse_chain("action-list url", ParseInput::Cli)
        .expect("valid action list")
        .remove(0);
    assert_eq!(
        command_params(&listing).expect("typed action list parameters"),
        serde_json::json!({"command": "action-list", "arguments": {"subject": "url"}})
    );
}

#[test]
fn cli_send_and_command_management_use_typed_envelopes() {
    for (text, expected) in [
        (
            "send mpv --selection",
            serde_json::json!({
                "target": "mpv",
                "selection": true,
                "send_subject": "selection"
            }),
        ),
        (
            "send mpv --url https://example.test",
            serde_json::json!({
                "target": "mpv",
                "url": "https://example.test",
                "send_subject": "url"
            }),
        ),
    ] {
        let command = parse_chain(text, ParseInput::Cli)
            .expect("valid send command")
            .remove(0);
        assert_eq!(
            command_params(&command).expect("typed send parameters")["arguments"],
            expected
        );
    }

    let help = parse_chain("command-help open", ParseInput::Cli)
        .expect("valid command help")
        .remove(0);
    assert_eq!(
        command_params(&help).expect("typed command help parameters")["arguments"],
        serde_json::json!({"id": "open"})
    );
    let execute = parse_chain(
        r#"command-execute open '{"target":"tab"}'"#,
        ParseInput::Cli,
    )
    .expect("valid command execute")
    .remove(0);
    assert_eq!(
        command_params(&execute).expect("typed command execute parameters")["arguments"],
        serde_json::json!({"id": "open", "arguments": {"target": "tab"}})
    );
    assert!(
        command_params(&ParsedCommand {
            name: "send".into(),
            arguments: vec!["mpv".into(), "--selection".into(), "extra".into()],
        })
        .is_err()
    );
}

#[test]
fn cli_context_route_commands_use_typed_fields() {
    let add = parse_chain(
        "context-route add 'https://*.company.test/*' work",
        ParseInput::Cli,
    )
    .expect("valid context route add")
    .remove(0);
    assert_eq!(
        command_params(&add).expect("typed context route add parameters"),
        serde_json::json!({
            "command": "context-route",
            "arguments": {
                "action": "add",
                "pattern": "https://*.company.test/*",
                "context": "work"
            }
        })
    );
    let configured = parse_chain(
        "context-route add 'https://*.company.test/*' work --priority -4 --behavior suggest --entry-point explicit-open --entry-point typed-initial-url",
        ParseInput::Cli,
    )
    .expect("valid configured context route add")
    .remove(0);
    assert_eq!(
        command_params(&configured).expect("typed configured route parameters"),
        serde_json::json!({
            "command": "context-route",
            "arguments": {
                "action": "add",
                "pattern": "https://*.company.test/*",
                "context": "work",
                "priority": -4,
                "behavior": "suggest",
                "entry_points": ["explicit-open", "typed-initial-url"]
            }
        })
    );
    let remove = parse_chain("context-route remove company-work", ParseInput::Cli)
        .expect("valid context route remove")
        .remove(0);
    assert_eq!(
        command_params(&remove).expect("typed context route remove parameters"),
        serde_json::json!({
            "command": "context-route",
            "arguments": {"action": "remove", "id": "company-work"}
        })
    );
    let open = parse_chain("open https://example.test", ParseInput::Cli)
        .expect("valid external open")
        .remove(0);
    assert_eq!(
        command_params(&open).expect("typed external open parameters"),
        serde_json::json!({
            "command": "open",
            "arguments": {
                "input": "https://example.test",
                "external": true
            }
        })
    );

    for arguments in [
        vec!["add".into(), "bad\npattern".into(), "work".into()],
        vec![
            "add".into(),
            "https://*.company.test/*".into(),
            "bad\tcontext".into(),
        ],
        vec!["remove".into(), String::new()],
    ] {
        assert!(
            command_params(&ParsedCommand {
                name: "context-route".into(),
                arguments,
            })
            .is_err()
        );
    }
}

#[test]
fn cli_yank_commands_use_typed_url_and_clean_fields() {
    let command = parse_chain("yank url --clean", ParseInput::Cli)
        .expect("valid clean URL copy command")
        .remove(0);
    assert_eq!(
        command_params(&command).expect("typed URL copy parameters"),
        serde_json::json!({
            "command": "yank",
            "arguments": {"source": "url", "clean": true}
        })
    );

    let command = parse_chain("yank selection --primary", ParseInput::Cli)
        .expect("valid primary selection copy command")
        .remove(0);
    assert_eq!(
        command_params(&command).expect("typed primary selection parameters"),
        serde_json::json!({
            "command": "yank",
            "arguments": {"source": "selection", "primary": true}
        })
    );
    let command = parse_chain("yank title --primary", ParseInput::Cli)
        .expect("valid title copy command")
        .remove(0);
    assert_eq!(
        command_params(&command).expect("typed title parameters"),
        serde_json::json!({
            "command": "yank",
            "arguments": {"source": "title", "primary": true}
        })
    );
    let duplicate = parse_chain("yank url --primary --primary", ParseInput::Cli)
        .expect("parse duplicate option for typed validation")
        .remove(0);
    assert!(command_params(&duplicate).is_err());
}

#[test]
fn cli_caret_commands_use_typed_fields() {
    let mode = parse_chain("mode-enter caret", ParseInput::Cli)
        .expect("valid mode command")
        .remove(0);
    assert_eq!(
        command_params(&mode).expect("typed mode parameters"),
        serde_json::json!({
            "command": "mode-enter",
            "arguments": {"mode": "caret"}
        })
    );

    let movement = parse_chain("caret-move word-next --count 2", ParseInput::Cli)
        .expect("valid caret movement")
        .remove(0);
    assert_eq!(
        command_params(&movement).expect("typed movement parameters"),
        serde_json::json!({
            "command": "caret-move",
            "arguments": {"direction": "word-next", "count": 2}
        })
    );

    let selection = parse_chain("caret-select toggle", ParseInput::Cli)
        .expect("valid caret selection")
        .remove(0);
    assert_eq!(
        command_params(&selection).expect("typed selection parameters"),
        serde_json::json!({
            "command": "caret-select",
            "arguments": {"state": "toggle"}
        })
    );
}

#[test]
fn cli_spawn_commands_forward_direct_argv() {
    let command = parse_chain("spawn -- /usr/bin/printf {title}", ParseInput::Cli)
        .expect("valid spawn command")
        .remove(0);
    assert_eq!(
        command_params(&command).expect("typed spawn parameters"),
        serde_json::json!({
            "command": "spawn",
            "arguments": {"argv": ["/usr/bin/printf", "{title}"]}
        })
    );
}

#[test]
fn cli_spawn_userscript_forwards_manifest_name() {
    let command = parse_chain("spawn --userscript video", ParseInput::Cli)
        .expect("valid userscript command")
        .remove(0);
    assert_eq!(
        command_params(&command).expect("typed userscript parameters"),
        serde_json::json!({
            "command": "spawn",
            "arguments": {"userscript": "video"}
        })
    );
}

#[test]
fn implicit_config_uses_basedir_but_temp_launches_ignore_it() {
    let directory = std::env::temp_dir().join(format!(
        "ferric-browser-implicit-config-{}",
        std::process::id()
    ));
    fs::create_dir_all(directory.join("config")).expect("config directory");
    fs::write(
        directory.join("config/config.toml"),
        "[ui]\nfont_size_pt = 11.0\n",
    )
    .expect("config");
    let options = CliOptions {
        basedir: Some(directory.to_string_lossy().into_owned()),
        ..CliOptions::default()
    };
    assert!(default_config_path(&options).is_some());
    let temporary = CliOptions {
        temp_basedir: true,
        basedir: options.basedir.clone(),
        ..CliOptions::default()
    };
    assert!(default_config_path(&temporary).is_none());
    let safe = CliOptions {
        safe_mode: true,
        basedir: options.basedir.clone(),
        ..CliOptions::default()
    };
    assert!(default_config_path(&safe).is_none());
    let _ = fs::remove_file(directory.join("config/config.toml"));
    let _ = fs::remove_dir(directory.join("config"));
    let _ = fs::remove_dir(directory);
}

#[test]
fn safe_mode_is_a_gui_only_startup_option() {
    let (options, action) = parse_cli(&["--safe-mode".into()]).expect("safe mode startup");
    assert!(options.safe_mode);
    assert!(matches!(action, CliAction::Open { .. }));
}

#[test]
fn userscripts_off_is_a_normal_profile_gui_only_startup_option() {
    let (options, action) =
        parse_cli(&["--userscripts-off".into()]).expect("userscripts-off startup");
    assert!(options.userscripts_off);
    assert!(!options.safe_mode);
    assert!(matches!(action, CliAction::Open { .. }));
}

#[test]
fn software_rendering_is_a_gui_only_startup_option() {
    let (options, action) =
        parse_cli(&["--software-rendering".into()]).expect("software rendering startup");
    assert!(options.software_rendering);
    assert!(matches!(action, CliAction::Open { .. }));
}

#[test]
fn cli_errors_use_stable_codes() {
    let error = run(&["--unknown".into()]).expect_err("unknown option is rejected");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert_eq!(error.status(), 2);
    let config_error = CliError::from_run(RunError::config("invalid test config"));
    assert_eq!(config_error.code, ErrorCode::Config);
    assert_eq!(config_error.status(), 2);
    let storage_error = CliError::from_run(RunError::storage("unavailable test storage"));
    assert_eq!(storage_error.code, ErrorCode::Storage);
    assert_eq!(storage_error.status(), 1);
    let instance_error = CliError::from_run(RunError::no_instance("missing test instance"));
    assert_eq!(instance_error.code, ErrorCode::NoInstance);
    assert_eq!(instance_error.status(), 3);
    assert_eq!(error.user_message, "The command-line request is invalid.");
    let error = run(&[
        "--safe-mode".into(),
        "--basedir".into(),
        "/tmp/ferric-invalid-combination".into(),
    ])
    .expect_err("post-parse option conflict is rejected");
    assert_eq!(error.code, ErrorCode::InvalidArgument);
    assert_eq!(error.status(), 2);
    assert!(wants_structured_error(&[
        "query".into(),
        "tabs".into(),
        "--format".into(),
        "json".into(),
    ]));
    assert!(!wants_structured_error(&["query".into(), "tabs".into()]));
    let structured = serde_json::from_str::<serde_json::Value>(&structured_cli_error_with_id(
        ErrorCode::Timeout,
        6,
        "slow",
        "err-test",
    ))
    .expect("structured CLI errors are JSON");
    assert_eq!(structured["error"]["code"], "E_TIMEOUT");
    assert_eq!(structured["error"]["status"], 6);
    assert_eq!(structured["error"]["message"], "slow");
    assert_eq!(
        structured["error"]["preserved"],
        "the timed-out operation was not committed"
    );
    assert_eq!(
        structured["error"]["next_action"],
        "check the target and retry"
    );
    assert!(
        structured["error"]["correlation_id"]
            .as_str()
            .is_some_and(|value| value.starts_with("err-"))
    );
}

#[test]
fn cli_command_parses_chain_as_data_for_ipc() {
    let arguments = vec![
        "command".into(),
        "--".into(),
        ":open".into(),
        "https://example.test".into(),
    ];
    let (_, action) = parse_cli(&arguments).expect("valid command command");
    assert!(matches!(
        action,
        CliAction::Command { text, window: None }
            if text == ":open https://example.test"
    ));
}

#[test]
fn cli_binding_commands_use_typed_mode_and_keychain_fields() {
    let list = parse_chain("binding-list --mode normal", ParseInput::Cli)
        .expect("binding list command")
        .pop()
        .expect("binding list parsed command");
    assert_eq!(
        command_params(&list).expect("typed binding list")["arguments"],
        serde_json::json!({"mode": "normal"})
    );
    let explain = parse_chain("binding-explain gg --mode normal", ParseInput::Cli)
        .expect("binding explain command")
        .pop()
        .expect("binding explain parsed command");
    assert_eq!(
        command_params(&explain).expect("typed binding explain")["arguments"],
        serde_json::json!({"keychain": "gg", "mode": "normal"})
    );
    assert!(
        command_params(&ParsedCommand {
            name: "binding-explain".into(),
            arguments: Vec::new(),
        })
        .is_err()
    );
    let learning = parse_chain("learning-mode toggle", ParseInput::Cli)
        .expect("learning mode command")
        .pop()
        .expect("learning mode parsed command");
    assert_eq!(
        command_params(&learning).expect("typed learning mode")["arguments"],
        serde_json::json!({"state": "toggle"})
    );
    assert!(
        command_params(&ParsedCommand {
            name: "learning-mode".into(),
            arguments: vec!["invalid".into()],
        })
        .is_err()
    );
}

#[test]
fn forwarded_cli_context_preserves_cli_source() {
    let mut params = serde_json::json!({
        "command": "back",
        "arguments": {}
    });
    add_cli_context(&mut params, &CliOptions::default(), None);
    assert_eq!(params["context"]["source"], "cli");
}

#[test]
fn engine_security_flag_guard_rejects_boolean_forms() {
    assert_eq!(
        find_disabling_engine_flag("--disable-web-security --foo"),
        Some("--disable-web-security")
    );
    assert_eq!(
        find_disabling_engine_flag("--disable-web-security=true"),
        Some("--disable-web-security")
    );
    assert_eq!(
        find_disabling_engine_flag("--single-process=false"),
        Some("--single-process")
    );
    assert_eq!(find_disabling_engine_flag("--disable-gpu"), None);
}
