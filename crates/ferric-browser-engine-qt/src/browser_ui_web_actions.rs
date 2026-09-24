use super::{
    Arc, AtomicBool, BTreeMap, Command, CommandExt, CxxQtType, DispatchTarget, Duration, Instant,
    MAX_UNTRUSTED_ARGUMENT_BYTES, Ordering, ParsedCommand, PendingActionTarget, PendingDownload,
    PendingSelection, PendingUserscript, Pin, QString, Stdio, Target, UserscriptCompletion, Uuid,
    Value, Write, captured_target_is_current, configured_action_targets, current_target,
    finish_pending_selection_operation, ipc_mode_name, is_bounded_untrusted_text,
    is_safe_history_url, qobject, read_bounded, resolve_input, safe_ipc_url,
    sanitize_process_stderr, terminate_child_process, thread, typed_ipc_command, userscript,
    validate_print_pdf_path, validate_save_page_path,
};

impl qobject::BrowserUi {
    pub(super) fn execute_send_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let [target_name, input @ ..] = command.arguments.as_slice() else {
            return Err("send requires a target name".into());
        };
        if !is_bounded_untrusted_text(target_name)
            || !target_name.chars().all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
            })
        {
            return Err("send target name must be a lowercase slug".into());
        }
        if input.len() > 2 {
            return Err("send accepts at most two bounded subject arguments".into());
        }
        if input.iter().any(|argument| {
            argument.len() > MAX_UNTRUSTED_ARGUMENT_BYTES || argument.chars().any(char::is_control)
        }) {
            return Err(
                "send subject argument is oversized or contains a control character".into(),
            );
        }
        let marker = input.first().map(String::as_str);
        let (required_subject, explicit_url) = match marker {
            Some("--selection") if input.len() == 1 => ("selection", None),
            Some("--tab") if input.len() == 1 => ("tab", None),
            Some("--url") => ("url", input.get(1)),
            Some(_) => ("link", input.first()),
            None => ("link", None),
        };
        let selection_requested = required_subject == "selection";
        let targets = configured_action_targets(&self.as_ref().rust().config)?;
        let target = targets
            .into_iter()
            .find(|(name, _)| name == target_name)
            .map(|(_, target)| target)
            .ok_or_else(|| format!("configured action target not found: {target_name}"))?;
        if !target
            .subject_types
            .iter()
            .any(|subject| subject == required_subject)
        {
            return Err(format!(
                "action target {target_name} does not accept {required_subject} subjects"
            ));
        }
        let current = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        )
        .ok_or_else(|| "current document is unavailable".to_owned())?;
        let binding = self.as_ref();
        let state = binding
            .rust()
            .state
            .as_ref()
            .ok_or_else(|| "core state unavailable".to_owned())?;
        let tab = state
            .tabs()
            .get(&current.tab)
            .ok_or_else(|| "current tab is unavailable".to_owned())?;
        let private = state
            .profiles()
            .get(&tab.profile)
            .is_some_and(|profile| profile.privacy.is_transient());
        if private && !target.allow_private {
            return Err("action target is disabled in private or ephemeral mode".into());
        }
        if selection_requested {
            let operation_id = format!("op-{}", Uuid::new_v4());
            let token = format!("selection-{}", Uuid::new_v4());
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.pending_action_target = Some(PendingActionTarget {
                token: token.clone(),
                target: current,
                target_name: target_name.clone(),
                target_config: target,
                operation_id: operation_id.clone(),
                created_at: Instant::now(),
            });
            this.pending_selection = Some(PendingSelection {
                token,
                target: current,
                primary: false,
                issued: false,
                created_at: Instant::now(),
                search_engine: None,
                operation_id: None,
            });
            this.operation_states
                .insert(operation_id.clone(), "selection pending".into());
            return Ok(serde_json::json!({
                "status": "accepted",
                "target": target_name,
                "operation_id": operation_id,
                "selection_required": true
            }));
        }
        let url = explicit_url
            .map(|value| resolve_input(value, &self.as_ref().navigation_context()))
            .transpose()
            .map_err(|error| error.to_string())?
            .map(|navigation| navigation.url.to_string())
            .or_else(|| tab.url.clone())
            .ok_or_else(|| "current URL is unavailable".to_owned())?;
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("action target accepts only HTTP(S) URLs".into());
        }
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
        let mut argv = vec![target.executable.clone()];
        for argument in &target.argv {
            argv.push(match argument.as_str() {
                "{url}" => safe_ipc_url(&url),
                "{title}" => title.clone(),
                "{selection}" => {
                    return Err(
                        "action target selection fields require a selection action".to_owned()
                    );
                }
                _ => argument.clone(),
            });
        }
        let operation_id = self
            .as_mut()
            .start_action_target_process(&argv, target.detach)?;
        self.as_mut().set_status_text(QString::from(format!(
            "Sent to {target_name}: {operation_id}"
        )));
        Ok(serde_json::json!({
            "status": "accepted",
            "target": target_name,
            "operation_id": operation_id,
            "url": safe_ipc_url(&url),
            "detached": target.detach
        }))
    }

    pub(super) fn execute_download_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if command.arguments.len() != 1 {
            return Err("download requires exactly one URL".into());
        }
        let navigation = resolve_input(&command.arguments[0], &self.as_ref().navigation_context())
            .map_err(|error| error.to_string())?;
        let url = navigation.url.to_string();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("download supports only HTTP(S) URLs".into());
        }
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        )
        .ok_or_else(|| "current document is unavailable".to_owned())?;
        Ok(self
            .as_mut()
            .queue_download_request(target, &url, "Download request queued"))
    }

    pub(super) fn execute_print_pdf_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if command.arguments.len() != 1 {
            return Err("print-pdf requires exactly one path".into());
        }
        let path = validate_print_pdf_path(&command.arguments[0])?;
        let tab_index = self.as_ref().rust().active_tab_index;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!(
            "print-pdf\t{tab_index}\t{}",
            path.to_string_lossy()
        ));
        self.as_mut()
            .set_status_text(QString::from("PDF generation requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "path": path.to_string_lossy(),
            "tab_index": tab_index
        }))
    }

    pub(super) fn execute_save_page_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if command.arguments.len() != 1 {
            return Err("save-page requires exactly one path".into());
        }
        let path = validate_save_page_path(&command.arguments[0])?;
        let tab_index = self.as_ref().rust().active_tab_index;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!(
            "save-page\t{tab_index}\t{}",
            path.to_string_lossy()
        ));
        self.as_mut()
            .set_status_text(QString::from("Page save requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "path": path.to_string_lossy(),
            "tab_index": tab_index
        }))
    }

    pub(super) fn execute_view_source_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if !command.arguments.is_empty() {
            return Err("view-source does not accept arguments".into());
        }
        let url = self.as_ref().rust().current_url.to_string();
        if !is_safe_history_url(&url)
            || !(url.starts_with("http://") || url.starts_with("https://"))
        {
            return Err("view-source requires the current safe HTTP(S) document".into());
        }
        let tab_index = self.as_ref().rust().active_tab_index;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("view-source\t{tab_index}\t{url}"));
        self.as_mut()
            .set_status_text(QString::from("Source view requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "tab_index": tab_index,
            "source_url": url
        }))
    }

    pub(super) fn execute_devtools_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let detach = match command.arguments.as_slice() {
            [] => false,
            [flag] if flag == "--detach" => true,
            _ => return Err("devtools accepts only the optional --detach flag".into()),
        };
        let tab_id = self
            .as_ref()
            .rust()
            .tab
            .ok_or_else(|| "devtools requires an active tab".to_owned())?;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("devtools\t{detach}"));
        self.as_mut().set_status_text(QString::from(if detach {
            "DevTools detach requested"
        } else {
            "DevTools toggle requested"
        }));
        Ok(serde_json::json!({
            "status": "accepted",
            "tab_id": tab_id.to_string(),
            "detach": detach
        }))
    }

    pub(super) fn execute_print_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if !command.arguments.is_empty() {
            return Err("print does not accept arguments".into());
        }
        let tab_index = self.as_ref().rust().active_tab_index;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("print\t{tab_index}"));
        self.as_mut()
            .set_status_text(QString::from("Print requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "tab_index": tab_index,
            "path": "private temporary PDF"
        }))
    }

    pub(super) fn execute_download_desktop_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if command.arguments.len() != 1 {
            return Err(format!("{} requires exactly one download ID", command.name));
        }
        let id = QString::from(command.arguments[0].clone());
        let uri = self
            .as_mut()
            .resolve_download_desktop_uri(&id, command.name == "download-show")?;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("{}\t{uri}", command.name));
        self.as_mut()
            .set_status_text(QString::from(if command.name == "download-show" {
                "Download reveal requested"
            } else {
                "Download open requested"
            }));
        Ok(serde_json::json!({
            "status": "accepted",
            "download_id": command.arguments[0],
            "action": command.name
        }))
    }

    pub(super) fn queue_download_request(
        mut self: Pin<&mut Self>,
        target: Target,
        url: &str,
        status: &str,
    ) -> Value {
        let token = format!("download-{}", Uuid::new_v4());
        self.as_mut().rust_mut().as_mut().get_mut().pending_download = Some(PendingDownload {
            token: token.clone(),
            target,
            url: url.to_owned(),
            issued: false,
        });
        self.set_status_text(QString::from(status));
        serde_json::json!({
            "status": "accepted",
            "token": token,
            "url": safe_ipc_url(url)
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn execute_userscript(
        mut self: Pin<&mut Self>,
        name: &str,
    ) -> Result<Value, String> {
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        )
        .ok_or_else(|| "current document is unavailable".to_owned())?;
        let roots = self
            .as_ref()
            .rust()
            .userscript_roots
            .clone()
            .ok_or_else(|| "userscripts are unavailable without configured storage".to_owned())?;
        let loaded = userscript::load(&roots.config, name)?;
        userscript::validate_executable(&loaded.executable)?;
        if loaded.manifest.argv.iter().any(|argument| {
            argument.len() > userscript::MAX_ARGV_VALUE_BYTES
                || argument.is_empty()
                || argument.chars().any(char::is_control)
        }) {
            return Err("userscript argv is oversized or contains an invalid value".into());
        }
        let binding = self.as_ref();
        let state = binding
            .rust()
            .state
            .as_ref()
            .ok_or_else(|| "core state unavailable".to_owned())?;
        let tab = state
            .tabs()
            .get(&target.tab)
            .ok_or_else(|| "current tab is unavailable".to_owned())?;
        let private = state
            .profiles()
            .get(&tab.profile)
            .is_some_and(|profile| profile.privacy.is_transient());
        if private && !loaded.manifest.allow_private {
            return Err(
                "userscript is disabled in private or ephemeral mode by its manifest".into(),
            );
        }
        if loaded
            .manifest
            .context_fields
            .iter()
            .any(|field| field == "selection")
        {
            let operation_id = format!("op-{}", Uuid::new_v4());
            let token = format!("selection-{}", Uuid::new_v4());
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.pending_selection = Some(PendingSelection {
                token: token.clone(),
                target,
                primary: false,
                issued: false,
                created_at: Instant::now(),
                search_engine: None,
                operation_id: None,
            });
            this.pending_userscript = Some(PendingUserscript {
                token,
                target,
                name: name.to_owned(),
                operation_id: operation_id.clone(),
            });
            this.operation_states
                .insert(operation_id.clone(), "selection pending".into());
            return Ok(serde_json::json!({
                "status": "accepted",
                "operation_id": operation_id,
                "userscript": name,
                "selection_required": true
            }));
        }
        self.execute_userscript_with_context(name, target, None, None, None, None, None)
    }

    pub(super) fn execute_userscript_for_hint(
        self: Pin<&mut Self>,
        name: &str,
        target: Target,
        hint_url: &str,
    ) -> Result<Value, String> {
        self.execute_userscript_with_context(
            name,
            target,
            Some(hint_url.to_owned()),
            None,
            None,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(super) fn execute_userscript_with_context(
        mut self: Pin<&mut Self>,
        name: &str,
        target: Target,
        hint_url: Option<String>,
        selection: Option<&str>,
        operation_id: Option<String>,
        subject_url: Option<String>,
        subject_fields: Option<BTreeMap<String, Value>>,
    ) -> Result<Value, String> {
        let roots = self
            .as_ref()
            .rust()
            .userscript_roots
            .clone()
            .ok_or_else(|| "userscripts are unavailable without configured storage".to_owned())?;
        let loaded = userscript::load(&roots.config, name)?;
        userscript::validate_executable(&loaded.executable)?;
        if loaded.manifest.argv.iter().any(|argument| {
            argument.len() > userscript::MAX_ARGV_VALUE_BYTES
                || argument.is_empty()
                || argument.chars().any(char::is_control)
        }) {
            return Err("userscript argv is oversized or contains an invalid value".into());
        }
        let binding = self.as_ref();
        let state = binding
            .rust()
            .state
            .as_ref()
            .ok_or_else(|| "core state unavailable".to_owned())?;
        let tab = state
            .tabs()
            .get(&target.tab)
            .ok_or_else(|| "current tab is unavailable".to_owned())?;
        let target_window = state
            .windows()
            .get(&tab.window)
            .ok_or_else(|| "userscript target window is unavailable".to_owned())?;
        let target_profile = state
            .profiles()
            .get(&tab.profile)
            .ok_or_else(|| "userscript target profile is unavailable".to_owned())?;
        let private = state
            .profiles()
            .get(&tab.profile)
            .is_some_and(|profile| profile.privacy.is_transient());
        if private && !loaded.manifest.allow_private {
            return Err(
                "userscript is disabled in private or ephemeral mode by its manifest".into(),
            );
        }
        let wants_hint_url = loaded
            .manifest
            .context_fields
            .iter()
            .any(|field| field == "hint_url");
        if hint_url.is_some() != wants_hint_url {
            return Err(if hint_url.is_some() {
                "userscript must declare hint_url context for hint activation".into()
            } else {
                "userscript hint_url context is only available from a hint activation".into()
            });
        }
        let wants_selection = loaded
            .manifest
            .context_fields
            .iter()
            .any(|field| field == "selection");
        if selection.is_some() != wants_selection {
            return Err(if selection.is_some() {
                "userscript must declare selection context for selection activation".into()
            } else {
                "userscript selection context is unavailable".into()
            });
        }
        if hint_url.is_some() && selection.is_some() {
            return Err("userscript hint and selection contexts cannot be combined".into());
        }

        let subject_fields = subject_fields.unwrap_or_default();
        let mut context = serde_json::Map::new();
        context.insert("tab_id".into(), Value::String(target.tab.to_string()));
        context.insert("document_revision".into(), Value::from(state.revision()));
        context.insert(
            "profile".into(),
            Value::String(target_profile.label.clone()),
        );
        context.insert("private".into(), Value::Bool(private));
        let page_url = tab
            .url
            .as_deref()
            .map_or_else(|| "about:blank".to_owned(), safe_ipc_url);
        let url = subject_url.unwrap_or_else(|| page_url.clone());
        let title = tab.title.replace(['\n', '\r'], " ");
        let hint_url = hint_url.map(|value| safe_ipc_url(&value));
        for field in &loaded.manifest.context_fields {
            let value = match field.as_str() {
                "url" => Value::String(url.clone()),
                "title" => Value::String(title.clone()),
                "mode" => Value::String(ipc_mode_name(self.as_ref().rust().core_mode).into()),
                "profile" => Value::String(target_profile.label.clone()),
                "tab_id" => Value::String(target.tab.to_string()),
                "window_id" => Value::String(tab.window.to_string()),
                "context_name" => Value::String(target_window.context.clone().unwrap_or_default()),
                "document_revision" => Value::from(state.revision()),
                "private" => Value::Bool(private),
                "hint_url" => Value::String(
                    hint_url
                        .clone()
                        .ok_or_else(|| "userscript hint_url context is unavailable".to_owned())?,
                ),
                "selection" => Value::String(
                    selection
                        .map(ToOwned::to_owned)
                        .ok_or_else(|| "userscript selection context is unavailable".to_owned())?,
                ),
                field => subject_fields.get(field).cloned().ok_or_else(|| {
                    format!("userscript {field} context is unavailable for this invocation")
                })?,
            };
            context.insert(field.clone(), value);
        }
        let operation_id = operation_id.unwrap_or_else(|| format!("op-{}", Uuid::new_v4()));
        let context = Value::Object(context);
        let input = serde_json::to_vec(&userscript::protocol_input(&operation_id, &context))
            .map_err(|error| format!("could not encode userscript context: {error}"))?;
        let mut command = Command::new(&loaded.executable);
        command
            .args(&loaded.manifest.argv)
            .current_dir(loaded.path.parent().unwrap_or(&roots.config))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for variable in ["RB_URL", "RB_TITLE", "RB_MODE", "RB_PROFILE"] {
            command.env_remove(variable);
        }
        for field in &loaded.manifest.context_fields {
            match field.as_str() {
                "url" => {
                    command.env("RB_URL", &url);
                }
                "title" => {
                    command.env("RB_TITLE", &title);
                }
                "mode" => {
                    command.env("RB_MODE", ipc_mode_name(self.as_ref().rust().core_mode));
                }
                "profile" => {
                    command.env("RB_PROFILE", &target_profile.label);
                }
                _ => {}
            }
        }
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command
            .spawn()
            .map_err(|error| format!("could not start userscript: {error}"))?;
        // A userscript may read its context slowly (or not at all). Keep the
        // bounded protocol write off the Qt thread so a full pipe cannot make
        // launching a script block the browser UI.
        let stdin = child.stdin.take();
        let input_writer = thread::spawn(move || {
            let Some(mut stdin) = stdin else {
                return Ok::<(), String>(());
            };
            stdin
                .write_all(&input)
                .map_err(|error| format!("could not write userscript context: {error}"))
        });
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "userscript stdout was not captured".to_owned())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "userscript stderr was not captured".to_owned())?;
        let stdout_reader =
            thread::spawn(move || read_bounded(stdout, userscript::MAX_PROTOCOL_BYTES));
        let stderr_reader = thread::spawn(move || read_bounded(stderr, 64 * 1024));
        let completions = Arc::clone(&self.as_ref().rust().userscript_completions);
        let cancellation = Arc::new(AtomicBool::new(false));
        let cancellations = Arc::clone(&self.as_ref().rust().userscript_cancellations);
        let manifest = loaded.manifest;
        let completion_id = operation_id.clone();
        let completion_target = target;
        let timeout = Duration::from_secs(manifest.timeout_seconds);
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .operation_states
            .insert(operation_id.clone(), "running".into());
        if let Ok(mut values) = cancellations.lock() {
            values.insert(operation_id.clone(), Arc::clone(&cancellation));
        }
        thread::spawn(move || {
            let started = Instant::now();
            let (mut status, mut action) = loop {
                if cancellation.load(Ordering::Acquire) {
                    terminate_child_process(&mut child);
                    break ("cancelled".into(), None);
                }
                match child.try_wait() {
                    Ok(Some(status)) => {
                        break if status.success() {
                            // A successful one-shot script may still have
                            // forked a descendant that inherited stdin. End
                            // the private process group before joining the
                            // writer so completion cannot wait forever for a
                            // leaked pipe endpoint.
                            terminate_child_process(&mut child);
                            match stdout_reader.join() {
                                Ok(Ok(output)) => {
                                    match userscript::parse_output(&output, &manifest) {
                                        Ok(output) => {
                                            let status = if output.message.is_empty() {
                                                "completed".to_owned()
                                            } else {
                                                format!("completed: {}", output.message)
                                            };
                                            (status, output.action)
                                        }
                                        Err(error) => (format!("failed ({error})"), None),
                                    }
                                }
                                Ok(Err(error)) => (format!("failed ({error})"), None),
                                Err(_) => {
                                    ("failed (userscript output reader stopped)".into(), None)
                                }
                            }
                        } else {
                            (format!("failed ({status})"), None)
                        };
                    }
                    Ok(None) if started.elapsed() >= timeout => {
                        terminate_child_process(&mut child);
                        break ("failed (userscript timed out)".into(), None);
                    }
                    Ok(None) => thread::sleep(Duration::from_millis(10)),
                    Err(error) => break (format!("failed ({error})"), None),
                }
            };
            let input_result = input_writer
                .join()
                .ok()
                .unwrap_or_else(|| Err("userscript context writer stopped".into()));
            if status == "completed" || status.starts_with("completed: ") {
                if let Err(error) = input_result {
                    status = format!("failed ({error})");
                    action = None;
                }
            }
            let stderr = stderr_reader
                .join()
                .ok()
                .and_then(Result::ok)
                .map(|output| sanitize_process_stderr(&output))
                .unwrap_or_default();
            if let Ok(mut values) = cancellations.lock() {
                values.remove(&completion_id);
            }
            if let Ok(mut completions) = completions.lock() {
                completions.push(UserscriptCompletion {
                    operation_id: completion_id,
                    status,
                    target: completion_target,
                    action,
                    stderr,
                });
            }
        });
        Ok(serde_json::json!({
            "status": "accepted",
            "operation_id": operation_id,
            "userscript": name
        }))
    }

    pub(super) fn deliver_spawn_selection(
        mut self: Pin<&mut Self>,
        token: &str,
        result: &QString,
    ) -> bool {
        let pending = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(pending) = this.pending_spawn.as_ref() else {
                return false;
            };
            if pending.token != token {
                self.set_status_text(QString::from("Spawn selection is stale"));
                return false;
            }
            this.pending_spawn.take()
        };
        let Some(pending) = pending else {
            self.set_status_text(QString::from("Spawn selection expired"));
            return false;
        };
        {
            let mut rust = self.as_mut().rust_mut();
            finish_pending_selection_operation(rust.as_mut().get_mut(), "cancelled");
        }
        if current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        ) != Some(pending.target)
        {
            self.set_status_text(QString::from("Spawn target is stale"));
            return false;
        }
        let value: Value = if let Ok(value) = serde_json::from_str(&result.to_string()) {
            value
        } else {
            self.set_status_text(QString::from("Spawn selection extraction failed"));
            return false;
        };
        let Some(text) = value.get("text").and_then(Value::as_str) else {
            self.set_status_text(QString::from("Spawn selection is unavailable"));
            return false;
        };
        if text.is_empty() || text.len() > 1024 * 1024 || text.contains('\0') {
            self.set_status_text(QString::from("Spawn selection is empty or invalid"));
            return false;
        }
        let argv = match self.resolve_spawn_argv(&pending.argv, Some(text)) {
            Ok((argv, false)) => argv,
            Ok(_) => {
                self.set_status_text(QString::from("Spawn selection could not be resolved"));
                return false;
            }
            Err(error) => {
                self.set_status_text(QString::from(error));
                return false;
            }
        };
        match self.as_mut().start_spawn_process(&argv) {
            Ok(operation_id) => {
                self.set_status_text(QString::from(format!("Spawn started: {operation_id}")));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Spawn failed: {error}")));
                false
            }
        }
    }

    pub(super) fn execute_userscript_action(
        mut self: Pin<&mut Self>,
        target: Target,
        action: userscript::UserscriptAction,
    ) -> Result<(), String> {
        if !captured_target_is_current(self.as_ref().rust().state.as_ref(), target) {
            return Err("userscript result target is stale".into());
        }
        let params = match action {
            userscript::UserscriptAction::Open { url } => {
                serde_json::json!({"command": "open", "arguments": {"input": url}})
            }
            userscript::UserscriptAction::Yank { url, clean } => {
                serde_json::json!({"command": "yank", "arguments": {
                    "source": "url",
                    "input": url,
                    "clean": clean
                }})
            }
            userscript::UserscriptAction::Command { name, arguments } => {
                serde_json::json!({"command": name, "arguments": arguments})
            }
        };
        let (command, mut route) = typed_ipc_command(&params).map_err(|error| error.to_string())?;
        route.selector = DispatchTarget::Tab(target.tab);
        route.context = None;
        route.profile = None;
        self.as_mut().execute_ipc_command(command, &route)?;
        Ok(())
    }
}
