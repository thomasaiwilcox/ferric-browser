use super::{
    Arc, AtomicBool, Command, CommandExt, CxxQtType, Duration, MAX_UNTRUSTED_ARGUMENT_BYTES,
    Ordering, Path, Pin, SpawnCompletion, Stdio, Uuid, configured_editor_argv,
    ensure_spawn_working_directory, is_bounded_untrusted_text, qobject, safe_ipc_url,
    terminate_child_process, thread,
};

impl qobject::BrowserUi {
    pub(super) fn editor_argv(&self, file: &Path) -> Result<(String, Vec<String>), String> {
        configured_editor_argv(&self.rust().config, file)
    }

    pub(super) fn resolve_spawn_argv(
        &self,
        templates: &[String],
        selection: Option<&str>,
    ) -> Result<(Vec<String>, bool), String> {
        let url = self
            .rust()
            .state
            .as_ref()
            .and_then(|state| self.rust().tab.and_then(|tab| state.tabs().get(&tab)))
            .and_then(|tab| tab.url.as_deref())
            .map_or_else(|| "about:blank".into(), safe_ipc_url);
        let title = self
            .page_title
            .to_string()
            .chars()
            .map(|character| {
                if character.is_control() {
                    ' '
                } else {
                    character
                }
            })
            .collect::<String>();
        let mut needs_selection = false;
        let argv = templates
            .iter()
            .map(|template| {
                let mut value = template.replace("{url}", &url).replace("{title}", &title);
                if template.contains("{hint_url}") {
                    return Err("{hint_url} is only available from a hint userscript".into());
                }
                if template.contains("{selection}") {
                    if let Some(selection) = selection {
                        value = value.replace("{selection}", selection);
                    } else {
                        needs_selection = true;
                    }
                }
                if value.chars().any(char::is_control) {
                    return Err("spawn argument contains a control character".into());
                }
                Ok(value)
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok((argv, needs_selection))
    }

    pub(super) fn start_spawn_process(
        mut self: Pin<&mut Self>,
        argv: &[String],
    ) -> Result<String, String> {
        let Some((executable, arguments)) = argv.split_first() else {
            return Err("spawn requires an executable".into());
        };
        if !is_bounded_untrusted_text(executable) {
            return Err("spawn executable is empty or invalid".into());
        }
        if arguments.iter().any(|argument| {
            argument.len() > MAX_UNTRUSTED_ARGUMENT_BYTES || argument.chars().any(char::is_control)
        }) {
            return Err("spawn argument is oversized or contains a control character".into());
        }
        let working_directory = {
            let mut rust = self.as_mut().rust_mut();
            ensure_spawn_working_directory(rust.as_mut().get_mut())?
        };
        let operation_id = format!("op-{}", Uuid::new_v4());
        let mut child = Command::new(executable);
        child
            .args(arguments)
            .current_dir(working_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        child.process_group(0);
        let mut child = child
            .spawn()
            .map_err(|error| format!("could not start executable: {error}"))?;
        let completions = Arc::clone(&self.as_ref().rust().spawn_completions);
        let cancellations = Arc::clone(&self.as_ref().rust().spawn_cancellations);
        let cancellation = Arc::new(AtomicBool::new(false));
        if let Ok(mut values) = cancellations.lock() {
            values.insert(operation_id.clone(), Arc::clone(&cancellation));
        }
        let completion_id = operation_id.clone();
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .operation_states
            .insert(operation_id.clone(), "running".into());
        thread::spawn(move || {
            let status = loop {
                if cancellation.load(Ordering::Acquire) {
                    terminate_child_process(&mut child);
                    break "cancelled".to_owned();
                }
                match child.try_wait() {
                    Ok(Some(status)) if status.success() => break "completed".to_owned(),
                    Ok(Some(status)) => break format!("failed ({status})"),
                    Ok(None) => thread::sleep(Duration::from_millis(20)),
                    Err(error) => break format!("failed ({error})"),
                }
            };
            if let Ok(mut values) = cancellations.lock() {
                values.remove(&completion_id);
            }
            if let Ok(mut completions) = completions.lock() {
                completions.push(SpawnCompletion {
                    operation_id: completion_id,
                    status,
                });
            }
        });
        Ok(operation_id)
    }

    pub(super) fn start_action_target_process(
        mut self: Pin<&mut Self>,
        argv: &[String],
        detach: bool,
    ) -> Result<String, String> {
        if detach {
            let Some((executable, arguments)) = argv.split_first() else {
                return Err("action target requires an executable".into());
            };
            if !is_bounded_untrusted_text(executable) {
                return Err("action target executable is empty or invalid".into());
            }
            if arguments.iter().any(|argument| {
                argument.len() > MAX_UNTRUSTED_ARGUMENT_BYTES
                    || argument.chars().any(char::is_control)
            }) {
                return Err(
                    "action target argument is oversized or contains a control character".into(),
                );
            }
            let working_directory = {
                let mut rust = self.as_mut().rust_mut();
                ensure_spawn_working_directory(rust.as_mut().get_mut())?
            };
            let mut command = Command::new(executable);
            command
                .args(arguments)
                .current_dir(working_directory)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            #[cfg(unix)]
            command.process_group(0);
            command
                .spawn()
                .map_err(|error| format!("could not start action target: {error}"))?;
            let operation_id = format!("op-{}", Uuid::new_v4());
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .operation_states
                .insert(operation_id.clone(), "detached".into());
            return Ok(operation_id);
        }
        self.start_spawn_process(argv)
    }
}
