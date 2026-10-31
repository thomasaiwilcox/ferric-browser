//! Recorded commands retain their count and route across native page owners.
use crate::browser_ui_page_windows::{PageContinuation, PageHandoff};
use crate::{CxxQtType, ParsedCommand, Pin, PublicError, RepeatableCommand, qobject};
use ferric_browser_ipc::ErrorCode;

fn preserve_remote_count(command: &mut ParsedCommand, count: u32) {
    if count > 1 && !command.arguments.iter().any(|value| value == "--count") {
        command
            .arguments
            .extend(["--count".into(), count.to_string()]);
    }
}

impl qobject::BrowserUi {
    pub(super) fn execute_page_workflow(
        mut self: Pin<&mut Self>,
        commands: Vec<RepeatableCommand>,
        operation: Option<&str>,
        handoff: &mut Option<PageHandoff>,
    ) -> Result<crate::Value, PublicError> {
        let registered_actions = if commands.iter().any(|step| step.command.name == "action") {
            self.as_ref()
                .rust()
                .cached_userscript_actions()?
                .into_iter()
                .map(|action| (action.subject, action.verb))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let commands = crate::page_workflow_expansion::expand(
            &self.as_ref().rust().registry,
            &registered_actions,
            crate::page_window_macros::snapshot(self.as_ref().rust()),
            commands,
        )
        .map_err(|error| PublicError::new(ErrorCode::InvalidArgument, error.clone(), error))?;
        self.as_mut()
            .execute_page_recorded_commands(commands, operation, handoff)
    }

    pub(super) fn execute_page_recorded_workflow_local(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> bool {
        let result = self.as_mut().execute_page_workflow(
            vec![RepeatableCommand {
                command: command.clone(),
                count: 1,
                route: None,
            }],
            None,
            &mut None,
        );
        if let Err(error) = result {
            self.set_status_text(crate::QString::from(error.user_message()));
            false
        } else {
            true
        }
    }

    pub(super) fn execute_page_recorded_commands(
        mut self: Pin<&mut Self>,
        commands: Vec<RepeatableCommand>,
        operation: Option<&str>,
        handoff: &mut Option<PageHandoff>,
    ) -> Result<crate::Value, PublicError> {
        if commands.len() > crate::MAX_MACRO_COMMANDS {
            return Err("Macro expansion stopped at the 1000-command limit".into());
        }
        let registry = self.as_ref().rust().registry.clone();
        let mut expanded = Vec::new();
        for step in commands {
            for mut command in registry
                .expand_command(step.command.clone())
                .map_err(|e| PublicError::engine(e.to_string()))?
            {
                if crate::browser_ui_page_windows::is_command_workflow(&command)
                    && command.name != "action"
                {
                    return Err("A recorded command contains an unexpanded workflow".into());
                }
                if command.name == "action" {
                    let [subject, verb, arguments @ ..] = command.arguments.as_slice() else {
                        return Err("action requires SUBJECT VERB and typed arguments".into());
                    };
                    let action = self
                        .as_ref()
                        .rust()
                        .cached_userscript_actions()?
                        .into_iter()
                        .find(|action| action.subject == *subject && action.verb == *verb)
                        .ok_or_else(|| format!("unknown action subject/verb: {subject} {verb}"))?;
                    if !crate::userscript_catalog::is_available(
                        &action,
                        self.as_ref().userscript_availability(),
                    ) || crate::userscript_catalog::is_hint_only(&action)
                    {
                        return Err(PublicError::new(
                            ErrorCode::Denied,
                            "This userscript action is unavailable in this window.",
                            "userscript action privacy, storage or local hint policy denies execution",
                        ));
                    }
                    let valid = match subject.as_str() {
                        "link" | "download" | "history-entry" | "bookmark" | "quickmark"
                        | "session" | "command" => arguments.len() == 1,
                        "selection" | "url" => arguments.is_empty(),
                        "tab" | "window" | "context" => arguments.len() <= 1,
                        _ => false,
                    };
                    if !valid {
                        return Err(format!(
                            "Invalid arguments for userscript action {}",
                            action.id
                        )
                        .into());
                    }
                }
                let definition = registry
                    .resolve(&command.name)
                    .map_err(|e| PublicError::engine(e.to_string()))?;
                let valid_count = match definition.count {
                    ferric_browser_core::CountPolicy::NotSupported => step.count == 1,
                    ferric_browser_core::CountPolicy::Supported { maximum } => {
                        (1..=maximum).contains(&step.count)
                    }
                };
                if !valid_count {
                    return Err("The recorded command count is invalid".into());
                }
                if step.route.is_some() {
                    preserve_remote_count(&mut command, step.count);
                }
                registry
                    .validate(&command, self.as_ref().rust().core_mode)
                    .map_err(|e| PublicError::engine(e.to_string()))?;
                if let Some(route) = &step.route {
                    self.as_ref().validate_ipc_route_public(route)?;
                    if command.name == "action" {
                        self.as_ref().validate_action_ipc_route(route)?;
                    }
                    crate::browser_ui_hint_pointer::reject_external_hint_interaction_chain(
                        std::slice::from_ref(&command),
                    )
                    .map_err(|error| {
                        PublicError::new(
                            ErrorCode::Denied,
                            "Use this interaction from the browser window.",
                            error,
                        )
                    })?;
                } else if self
                    .as_mut()
                    .reject_generic_pointer_hint_chain(std::slice::from_ref(&command))
                {
                    return Err(
                        "Pointer click requires a direct local UI or keyboard command".into(),
                    );
                }
                expanded.push(RepeatableCommand {
                    command,
                    count: step.count,
                    route: step.route.clone(),
                });
                if expanded.len() > crate::MAX_MACRO_COMMANDS {
                    return Err("Macro expansion stopped at the 1000-command limit".into());
                }
            }
        }
        let mut result = serde_json::json!({"status":"accepted"});
        for (index, step) in expanded.iter().enumerate() {
            if index > 0 {
                self.as_mut().queue_pending_engine_action();
            }
            if crate::browser_ui_page_creation::handles(&step.command, step.route.as_ref()) {
                let mut commands = expanded[index + 1..].to_vec();
                for step in &mut commands {
                    if let Some(route) = &mut step.route {
                        route.selector = crate::DispatchTarget::Active;
                    }
                }
                return self
                    .execute_page_window_creation(
                        &step.command,
                        step.count,
                        step.route.as_ref(),
                        PageContinuation::Recorded {
                            commands,
                            invocation: step.clone(),
                        },
                        operation,
                        handoff,
                    )
                    .map_err(Into::into);
            }
            if step.command.name == "tab-close" {
                if let Some(route) = &step.route {
                    if route.context.is_some()
                        || Some(self.as_ref().ipc_route_window(route)?)
                            != self.as_ref().rust().window
                    {
                        return Err(
                            "Close pages in their issuing window without context routing".into(),
                        );
                    }
                }
                let mut commands = expanded[index + 1..].to_vec();
                for step in &mut commands {
                    if let Some(route) = &mut step.route {
                        route.selector = crate::DispatchTarget::Active;
                    }
                }
                return self
                    .execute_page_window_close(
                        &step.command,
                        step.count,
                        PageContinuation::Recorded {
                            commands,
                            invocation: step.clone(),
                        },
                        operation,
                        handoff,
                    )
                    .map_err(Into::into);
            }
            if crate::browser_ui_page_regroup::handles(&step.command) {
                if let Some(route) = &step.route {
                    if route.context.is_some()
                        || Some(self.as_ref().ipc_route_window(route)?)
                            != self.as_ref().rust().window
                    {
                        return Err(
                            "Choose the page group in its issuing window without context routing"
                                .into(),
                        );
                    }
                }
                return self
                    .execute_page_window_regroup(
                        &step.command,
                        PageContinuation::Recorded {
                            commands: expanded[index + 1..].to_vec(),
                            invocation: step.clone(),
                        },
                        operation,
                        handoff,
                    )
                    .map_err(Into::into);
            }
            if crate::browser_ui_page_windows::handles(&step.command) {
                if let Some(route) = &step.route {
                    if route.context.is_some() {
                        return Err("context routing is not valid for page selection".into());
                    }
                    if Some(self.as_ref().ipc_route_window(route)?) != self.as_ref().rust().window {
                        return Err("Choose a page in the requested window".into());
                    }
                }
                let mut commands = expanded[index + 1..].to_vec();
                let mut invocation = step.clone();
                for step in std::iter::once(&mut invocation).chain(commands.iter_mut()) {
                    if let Some(route) = &mut step.route {
                        route.selector = crate::DispatchTarget::Active;
                    }
                }
                result = self.as_mut().execute_page_window_selection_with_handoff(
                    &step.command,
                    step.count,
                    PageContinuation::Recorded {
                        commands,
                        invocation,
                    },
                    operation,
                    handoff.clone(),
                )?;
                handoff.take();
                self.as_mut().note_repeatable_invocation(
                    &step.command,
                    step.route.as_ref(),
                    step.count,
                );
                return Ok(result);
            }
            if crate::browser_ui_page_mutations::handles(&step.command) {
                return self
                    .execute_page_window_mutation(
                        &step.command,
                        PageContinuation::Recorded {
                            commands: expanded[index + 1..].to_vec(),
                            invocation: step.clone(),
                        },
                        operation,
                        handoff,
                    )
                    .map_err(Into::into);
            }
            if step.command.name == "action" {
                let (mut value, id) = self
                    .as_mut()
                    .execute_userscript_action_command(&step.command)?
                    .ok_or("The userscript action is no longer available")?;
                value["action_id"] = crate::Value::String(id);
                result = value;
            } else if let Some(route) = &step.route {
                result = self
                    .as_mut()
                    .execute_ipc_command(step.command.clone(), route)?;
            } else if !self.as_mut().execute_counted_parsed_commands_with_handoff(
                vec![step.command.clone()],
                step.count,
                handoff,
            ) {
                return Err(PublicError::engine(
                    self.as_ref().rust().status_text.to_string(),
                ));
            }
        }
        if expanded.len() > 1 {
            self.as_mut().queue_pending_engine_action();
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_replay_keeps_counts_without_replacing_explicit_arguments() {
        let mut command = ParsedCommand {
            name: "scroll".into(),
            arguments: vec!["down".into()],
        };
        preserve_remote_count(&mut command, 7);
        assert_eq!(command.arguments, ["down", "--count", "7"]);
        preserve_remote_count(&mut command, 9);
        assert_eq!(command.arguments, ["down", "--count", "7"]);
        let mut single = ParsedCommand {
            name: "back".into(),
            arguments: Vec::new(),
        };
        preserve_remote_count(&mut single, 1);
        assert!(single.arguments.is_empty());
    }
}
