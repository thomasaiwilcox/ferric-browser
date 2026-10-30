//! Expand nested command workflows before their first native or document effect.
use crate::{
    CommandRegistry, IpcRoute, ParsedCommand, RepeatableCommand, page_window_macros::MacroState,
};

pub(super) fn command_action_invocation(
    registry: &CommandRegistry,
    command: &ParsedCommand,
) -> Result<(ParsedCommand, IpcRoute), String> {
    let (id, encoded) = match command.arguments.as_slice() {
        [id] => (id, None),
        [id, encoded] => (id, Some(encoded)),
        _ => return Err("command-execute requires a stable command ID".into()),
    };
    if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
        return Err("command action ID is invalid".into());
    }
    let definition = registry
        .definitions()
        .iter()
        .find(|definition| definition.action.to_string() == *id)
        .ok_or("command action target is stale")?;
    if matches!(
        definition.name.as_str(),
        "action" | "command-help" | "command-execute"
    ) {
        return Err("command action cannot recursively invoke an action command".into());
    }
    let arguments = encoded
        .map(|encoded| {
            serde_json::from_str::<crate::Value>(encoded)
                .map_err(|error| format!("captured command arguments are invalid JSON: {error}"))
        })
        .transpose()?
        .unwrap_or_else(|| serde_json::json!({}));
    if !arguments.is_object() {
        return Err("captured command arguments must be an object".into());
    }
    crate::typed_ipc_command(
        &serde_json::json!({"command": definition.name, "arguments": arguments}),
    )
    .map_err(|error| error.to_string())
}

fn recorded_route(mut saved: RepeatableCommand, caller: Option<&IpcRoute>) -> RepeatableCommand {
    if let Some(caller) = caller {
        let mut route = saved.route.take().unwrap_or_else(|| {
            let mut route = caller.clone();
            if saved.command.name == "open" {
                route.open_target = crate::IpcOpenTarget::Current;
            }
            route
        });
        route.source = caller.source;
        route.selector = crate::DispatchTarget::Active;
        saved.route = Some(route);
    }
    saved
}

struct Expansion<'a> {
    registry: &'a CommandRegistry,
    registered_actions: &'a [(String, String)],
    state: MacroState,
    steps: Vec<RepeatableCommand>,
    bytes: usize,
}

impl Expansion<'_> {
    fn step(&mut self, step: RepeatableCommand, depth: u8, recorded: bool) -> Result<(), String> {
        if depth > crate::MAX_MACRO_DEPTH {
            return Err("macro nesting limit reached".into());
        }
        for command in self
            .registry
            .expand_command(step.command.clone())
            .map_err(|error| error.to_string())?
        {
            let mut step = RepeatableCommand {
                command,
                ..step.clone()
            };
            if crate::browser_ui_page_windows::is_command_workflow(&step.command)
                && step
                    .route
                    .as_ref()
                    .is_some_and(|route| route.context.is_some())
            {
                return Err("context routing is not valid for input workflow commands".into());
            }
            if recorded && !crate::is_repeatable_command(&step.command.name) {
                return Err("A recording contains a command that cannot be replayed".into());
            }
            match step.command.name.as_str() {
                "repeat" => {
                    let count = match step.command.arguments.as_slice() {
                        [] => 1,
                        [flag, count] if flag == "--count" => count
                            .parse::<usize>()
                            .ok()
                            .filter(|count| (1..=100).contains(count))
                            .unwrap_or(0),
                        _ => 0,
                    };
                    if count == 0 {
                        return Err("repeat accepts optional --count N (1..100)".into());
                    }
                    let last = self
                        .state
                        .last
                        .clone()
                        .ok_or("No eligible command to repeat")?;
                    for _ in 0..count {
                        self.step(
                            recorded_route(last.clone(), step.route.as_ref()),
                            depth + 1,
                            true,
                        )?;
                    }
                    continue;
                }
                "macro-play" => {
                    let [register] = step.command.arguments.as_slice() else {
                        return Err("macro-play requires REGISTER".into());
                    };
                    if !crate::valid_macro_register(register) {
                        return Err("macro register must be one ASCII letter or digit".into());
                    }
                    let commands = self
                        .state
                        .registers
                        .get(register)
                        .filter(|commands| !commands.is_empty())
                        .cloned()
                        .ok_or_else(|| format!("Macro {register} is empty or missing"))?;
                    for command in commands {
                        self.step(
                            recorded_route(command, step.route.as_ref()),
                            depth + 1,
                            true,
                        )?;
                    }
                    continue;
                }
                "action" => {
                    // Registered userscript actions remain leaves in the caller;
                    // built-in command actions are the ones that can select owners.
                    let registered = step.command.arguments.get(0..2).is_some_and(|arguments| {
                        self.registered_actions.iter().any(|(subject, verb)| {
                            *subject == arguments[0] && *verb == arguments[1]
                        })
                    });
                    if !(registered && step.route.is_some())
                        && let Ok((mapped, _)) = crate::parse_action_invocation(&step.command)
                    {
                        step.command = mapped;
                        self.step(step, depth + 1, false)?;
                        continue;
                    }
                }
                "command-execute" => {
                    let (mapped, mut decoded_route) =
                        command_action_invocation(self.registry, &step.command)?;
                    let routed =
                        step.route.is_some() || matches!(mapped.name.as_str(), "open" | "tab-open");
                    if let Some(route) = &step.route {
                        decoded_route.source = route.source;
                        decoded_route.selector = route.selector;
                        decoded_route.profile.clone_from(&route.profile);
                        decoded_route.context.clone_from(&route.context);
                    } else {
                        decoded_route.source = crate::CommandSource::Ui;
                    }
                    step.command = mapped;
                    step.route = routed.then_some(decoded_route);
                    self.step(step, depth + 1, false)?;
                    continue;
                }
                "macro-record" => {
                    let [register] = step.command.arguments.as_slice() else {
                        return Err("macro-record requires REGISTER".into());
                    };
                    if !crate::valid_macro_register(register) {
                        return Err("macro register must be one ASCII letter or digit".into());
                    }
                    self.state.recording = Some((register.clone(), Vec::new()));
                }
                "macro-stop" => {
                    if !step.command.arguments.is_empty() {
                        return Err("macro-stop does not accept arguments".into());
                    }
                    let (register, commands) =
                        self.state.recording.take().ok_or("No macro is recording")?;
                    self.state.registers.insert(register, commands);
                }
                _ => {}
            }
            if let Some((command, mut route)) = crate::interactive_open_command(&step.command)? {
                if let Some(caller) = &step.route {
                    route.source = caller.source;
                    route.selector = caller.selector;
                    route.external_open = caller.external_open;
                    if route.profile.is_none() {
                        route.profile.clone_from(&caller.profile);
                    }
                    if route.context.is_none() {
                        route.context.clone_from(&caller.context);
                    }
                } else {
                    route.source = crate::CommandSource::Ui;
                }
                step.command = command;
                step.route = Some(route);
            }
            self.bytes = self
                .bytes
                .saturating_add(crate::page_window_macros::command_bytes(&step));
            if self.bytes > 8 * 1024 * 1024 {
                return Err("Macro expansion stopped at the 8 MiB memory limit".into());
            }
            if crate::is_repeatable_command(&step.command.name) {
                self.state.last = Some(step.clone());
                if let Some((_, commands)) = &mut self.state.recording {
                    if commands.len() >= crate::MAX_MACRO_COMMANDS {
                        return Err("Macro recording stopped at the 1000-command limit".into());
                    }
                    commands.push(step.clone());
                }
            }
            self.steps.push(step);
            if self.steps.len() > crate::MAX_MACRO_COMMANDS {
                return Err("Macro expansion stopped at the 1000-command limit".into());
            }
        }
        Ok(())
    }
}

pub(super) fn expand(
    registry: &CommandRegistry,
    registered_actions: &[(String, String)],
    state: MacroState,
    commands: Vec<RepeatableCommand>,
) -> Result<Vec<RepeatableCommand>, String> {
    let mut expansion = Expansion {
        registry,
        registered_actions,
        state,
        steps: Vec::new(),
        bytes: 0,
    };
    for command in commands {
        expansion.step(command, 0, false)?;
    }
    Ok(expansion.steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn step(name: &str, args: &[&str]) -> RepeatableCommand {
        RepeatableCommand {
            command: ParsedCommand {
                name: name.into(),
                arguments: args.iter().map(|value| (*value).into()).collect(),
            },
            count: 1,
            route: None,
        }
    }
    fn command_id(registry: &CommandRegistry, name: &str) -> String {
        registry.resolve(name).unwrap().action.to_string()
    }

    #[test]
    fn nested_wrappers_expand_in_order_with_the_preceding_repeat_target() {
        let registry = CommandRegistry::default_v1();
        let previous = command_id(&registry, "tab-prev");
        let mut state = MacroState::default();
        state.registers.insert(
            "a".into(),
            vec![step("tab-prev", &[]), step("tab-next", &[])],
        );
        let commands = expand(
            &registry,
            &[],
            state,
            vec![
                step("tab-next", &[]),
                step("repeat", &["--count", "2"]),
                step("macro-play", &["a"]),
                step("action", &["tab", "previous"]),
                step("command-execute", &[&previous]),
            ],
        )
        .unwrap();
        assert_eq!(
            commands
                .iter()
                .map(|step| step.command.name.as_str())
                .collect::<Vec<_>>(),
            [
                "tab-next", "tab-next", "tab-next", "tab-prev", "tab-next", "tab-prev", "tab-prev"
            ]
        );
    }

    #[test]
    fn registered_action_collisions_keep_the_existing_local_and_ipc_precedence() {
        let registry = CommandRegistry::default_v1();
        let registered = [("tab".into(), "next".into())];
        let local = step("action", &["tab", "next"]);
        let mut remote = local.clone();
        remote.route = Some(
            crate::typed_ipc_command(
                &serde_json::json!({"command":"repeat","context":{"source":"cli"}}),
            )
            .unwrap()
            .1,
        );
        let local_steps =
            expand(&registry, &registered, MacroState::default(), vec![local]).unwrap();
        let remote_steps =
            expand(&registry, &registered, MacroState::default(), vec![remote]).unwrap();
        assert_eq!(local_steps[0].command.name, "tab-next");
        assert_eq!(remote_steps[0].command.name, "action");
        assert_eq!(remote_steps[0].command.arguments, ["tab", "next"]);
    }

    #[test]
    fn remote_replay_cannot_acquire_saved_local_authority_or_owner_selector() {
        let registry = CommandRegistry::default_v1();
        let mut local = step("tab-next", &[]);
        local.count = 3;
        let mut local_route = crate::typed_ipc_command(&serde_json::json!({"command":"tab-next","context":{"source":"cli","window":"windowid-123"}})).unwrap().1;
        local_route.source = crate::CommandSource::Ui;
        local.route = Some(local_route);
        let mut caller = step("repeat", &[]);
        caller.route = Some(
            crate::typed_ipc_command(
                &serde_json::json!({"command":"repeat","context":{"source":"cli"}}),
            )
            .unwrap()
            .1,
        );
        let commands = expand(
            &registry,
            &[],
            MacroState {
                last: Some(local),
                ..MacroState::default()
            },
            vec![caller],
        )
        .unwrap();
        assert_eq!(commands[0].count, 3);
        assert_eq!(
            commands[0].route.as_ref().unwrap().source,
            crate::CommandSource::Cli
        );
        assert_eq!(
            commands[0].route.as_ref().unwrap().selector,
            crate::DispatchTarget::Active
        );
    }

    #[test]
    fn typed_command_wrapper_preserves_its_open_target_and_caller_source() {
        let registry = CommandRegistry::default_v1();
        let open_id = command_id(&registry, "open");
        let mut wrapper = step(
            "command-execute",
            &[
                &open_id,
                r#"{"input":"https://test.invalid","target":"window"}"#,
            ],
        );
        wrapper.route = Some(
            crate::typed_ipc_command(
                &serde_json::json!({"command":"repeat","context":{"source":"cli"}}),
            )
            .unwrap()
            .1,
        );
        let commands = expand(&registry, &[], MacroState::default(), vec![wrapper]).unwrap();
        assert_eq!(commands[0].command.arguments, ["https://test.invalid"]);
        let route = commands[0].route.as_ref().unwrap();
        assert_eq!(route.open_target, crate::IpcOpenTarget::Window);
        assert_eq!(route.source, crate::CommandSource::Cli);
    }

    #[test]
    fn a_local_command_wrapper_keeps_local_dispatch_without_promoting_remote_input() {
        let registry = CommandRegistry::default_v1();
        let id = command_id(&registry, "target-search");
        let local = step("command-execute", &[&id]);
        let mut remote = local.clone();
        remote.route = Some(
            crate::typed_ipc_command(
                &serde_json::json!({"command":"repeat","context":{"source":"cli"}}),
            )
            .unwrap()
            .1,
        );
        assert!(
            expand(&registry, &[], MacroState::default(), vec![local]).unwrap()[0]
                .route
                .is_none()
        );
        assert_eq!(
            expand(&registry, &[], MacroState::default(), vec![remote]).unwrap()[0]
                .route
                .as_ref()
                .unwrap()
                .source,
            crate::CommandSource::Cli
        );
        let mut huge = step("open", &[]);
        huge.command.arguments.push("x".repeat(8 * 1024 * 1024));
        assert!(
            expand(&registry, &[], MacroState::default(), vec![huge])
                .unwrap_err()
                .contains("memory limit")
        );
    }

    #[test]
    fn same_chain_record_stop_and_play_uses_the_captured_recording_and_obeys_limits() {
        let registry = CommandRegistry::default_v1();
        let commands = expand(
            &registry,
            &[],
            MacroState::default(),
            vec![
                step("macro-record", &["a"]),
                step("tab-next", &[]),
                step("macro-stop", &[]),
                step("macro-play", &["a"]),
            ],
        )
        .unwrap();
        assert_eq!(
            commands
                .iter()
                .map(|step| step.command.name.as_str())
                .collect::<Vec<_>>(),
            ["macro-record", "tab-next", "macro-stop", "tab-next"]
        );
        let mut state = MacroState::default();
        state.registers.insert(
            "a".into(),
            vec![step("tab-next", &[]); crate::MAX_MACRO_COMMANDS],
        );
        assert!(
            expand(
                &registry,
                &[],
                state,
                vec![step("macro-play", &["a"]), step("tab-next", &[])]
            )
            .is_err()
        );
        let mut state = MacroState::default();
        state
            .registers
            .insert("a".into(), vec![step("target-search", &[])]);
        assert!(expand(&registry, &[], state, vec![step("macro-play", &["a"])]).is_err());
        assert!(
            expand(
                &registry,
                &[],
                MacroState::default(),
                vec![step("command-execute", &["unknown"])]
            )
            .is_err()
        );
    }
}
