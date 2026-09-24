use super::{
    Arc, AtomicBool, Command, CommandExt, CxxQtType, Event, Instant, JourneyEdgeKind, Mode, Mutex,
    PAGE_SCRIPT_DEADLINE, ParsedCommand, Path, PathBuf, PendingCaret, PendingConfigEdit,
    PendingEditor, PendingLinkNavigation, PendingSelection, PendingSpawn, Pin, QString, Stdio,
    Uuid, ValidatedUrl, Value, clean_link, clean_open_input, configured_search_url, current_target,
    fs, link_cleaning_policy, link_result_value, qobject, read_bounded, resolve_input,
    safe_ipc_url, sanitize_untrusted_title, thread, validate_jseval_script,
};

impl qobject::BrowserUi {
    pub(super) fn execute_link_clean_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_link_navigation = None;
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if command.arguments.len() > 1 {
            return Err(format!("{} accepts at most one URL", command.name));
        }
        let input = if let Some(input) = command.arguments.first() {
            input.clone()
        } else {
            let binding = self.as_ref();
            let rust = binding.rust();
            let state = rust
                .state
                .as_ref()
                .ok_or_else(|| "core state unavailable".to_owned())?;
            let tab = rust
                .tab
                .and_then(|tab| state.tabs().get(&tab))
                .ok_or_else(|| "current tab unavailable".to_owned())?;
            tab.url.as_deref().unwrap_or("about:blank").to_owned()
        };
        let navigation = resolve_input(&input, &self.as_ref().navigation_context())
            .map_err(|error| error.to_string())?;
        let rules = link_cleaning_policy::active_rules(self.as_ref().rust().storage_roots.as_ref());
        let result =
            clean_link(navigation.url.as_str(), &rules).map_err(|error| error.to_string())?;
        let value = link_result_value(&command.name, &result, &rules);
        self.as_mut()
            .publish_link_preview(&command.name, &result, false);
        self.as_mut().set_link_preview_visible(true);
        self.set_status_text(QString::from(if result.changed {
            "Clean-link preview ready"
        } else {
            "Clean-link explanation ready"
        }));
        Ok(value)
    }

    pub(super) fn execute_selection_search(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let requested_engine = match command.arguments.as_slice() {
            [] => None,
            [engine] => Some(engine.as_str()),
            _ => return Err("selection-search accepts at most one engine name".into()),
        };
        let (engine, _) =
            configured_search_url(&self.as_ref().rust().config, requested_engine, "selection")?;
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        )
        .ok_or_else(|| "current document is unavailable".to_owned())?;
        if self.as_ref().rust().pending_selection.is_some() {
            return Err("another selection operation is already pending".into());
        }
        let operation_id = format!("op-{}", Uuid::new_v4());
        let token = format!("selection-{}", Uuid::new_v4());
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.pending_selection = Some(PendingSelection {
            token,
            target,
            primary: false,
            issued: false,
            created_at: Instant::now(),
            search_engine: Some(engine.clone()),
            operation_id: Some(operation_id.clone()),
        });
        this.operation_states
            .insert(operation_id.clone(), "selection pending".into());
        drop(rust);
        if self.as_ref().rust().core_mode == Mode::Caret {
            if let Some(window) = self.as_ref().rust().window {
                let _ = self.as_mut().reduce_event(Event::Escape { window });
            }
            self.as_mut().set_core_mode(Mode::Normal);
            self.as_mut().set_mode(QString::from("normal"));
            self.as_mut().set_caret_selecting(false);
        }
        self.set_status_text(QString::from(format!(
            "Selection search requested with {engine}"
        )));
        Ok(serde_json::json!({
            "status": "accepted",
            "source": "selection",
            "engine": engine,
            "operation_id": operation_id,
            "selection_required": true
        }))
    }

    pub(super) fn deliver_selection_search(
        mut self: Pin<&mut Self>,
        token: &str,
        result: &QString,
    ) -> bool {
        let pending = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(request) = this.pending_selection.as_ref() else {
                self.set_status_text(QString::from("Selection search request expired"));
                return false;
            };
            if request.token != token || request.search_engine.is_none() {
                self.set_status_text(QString::from("Selection search request is stale"));
                return false;
            }
            this.pending_selection.take()
        };
        let Some(pending) = pending else {
            self.set_status_text(QString::from("Selection search request expired"));
            return false;
        };
        let operation_id = pending.operation_id.clone();
        let fail = |mut this: Pin<&mut Self>, message: String| {
            if let Some(operation_id) = operation_id.as_ref() {
                this.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .operation_states
                    .insert(operation_id.clone(), format!("failed ({message})"));
            }
            this.set_status_text(QString::from(message));
            false
        };
        if pending.created_at.elapsed() >= PAGE_SCRIPT_DEADLINE {
            return fail(self, "Selection search extraction timed out".into());
        }
        if current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        ) != Some(pending.target)
        {
            return fail(self, "Selection search target is stale".into());
        }
        let value: Value = match serde_json::from_str(&result.to_string()) {
            Ok(value) => value,
            Err(_) => return fail(self, "Selection search extraction failed".into()),
        };
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            return fail(self, format!("Selection search unavailable: {error}"));
        }
        let Some(text) = value.get("text").and_then(Value::as_str) else {
            return fail(self, "Selection search returned invalid text".into());
        };
        if text.is_empty() {
            return fail(self, "No document selection".into());
        }
        if text.len() > 1024 * 1024 || text.contains('\0') {
            return fail(self, "Selection is too large or invalid".into());
        }
        let engine = pending
            .search_engine
            .as_deref()
            .expect("selection search requests always have an engine");
        let (_, url) = match configured_search_url(&self.as_ref().rust().config, Some(engine), text)
        {
            Ok(value) => value,
            Err(error) => return fail(self, error),
        };
        let url = match ValidatedUrl::parse(url) {
            Ok(url) => url,
            Err(error) => return fail(self, error.to_string()),
        };
        let effects = match self.as_mut().reduce_event(Event::StartNavigation {
            target: pending.target,
            url,
        }) {
            Ok(effects) => effects,
            Err(error) => return fail(self, error.clone()),
        };
        self.as_mut().mark_journey_transition(
            &effects,
            JourneyEdgeKind::Navigate,
            "selection-search",
        );
        self.as_mut().set_pending_engine_action(&effects);
        if let Some(operation_id) = operation_id {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .operation_states
                .insert(operation_id, "accepted".into());
        }
        self.set_status_text(QString::from(format!("Searching selection with {engine}")));
        true
    }

    pub(super) fn execute_selection_yank(
        mut self: Pin<&mut Self>,
        primary: bool,
    ) -> Result<Value, String> {
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        )
        .ok_or_else(|| "current document is unavailable".to_owned())?;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_selection = Some(PendingSelection {
            token: format!("selection-{}", Uuid::new_v4()),
            target,
            primary,
            issued: false,
            created_at: Instant::now(),
            search_engine: None,
            operation_id: None,
        });
        if self.as_ref().rust().core_mode == Mode::Caret {
            if let Some(window) = self.as_ref().rust().window {
                let _ = self.as_mut().reduce_event(Event::Escape { window });
            }
            self.as_mut().set_core_mode(Mode::Normal);
            self.as_mut().set_mode(QString::from("normal"));
            self.as_mut().set_caret_selecting(false);
        }
        self.set_status_text(QString::from("Selection copy requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "source": "selection",
            "primary": primary,
            "pending": true
        }))
    }

    pub(super) fn execute_yank_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let mut source = None;
        let mut input = None;
        let mut clean = false;
        let mut primary = false;
        for argument in &command.arguments {
            match argument.as_str() {
                "url" if source.is_none() => source = Some("url"),
                "title" if source.is_none() => source = Some("title"),
                "selection" if source.is_none() => source = Some("selection"),
                "--clean" if !clean => clean = true,
                "--primary" if !primary => primary = true,
                value if source == Some("url") && input.is_none() => input = Some(value),
                _ => return Err("yank expects URL, title, or selection [--clean]".into()),
            }
        }
        if source == Some("selection") {
            if input.is_some() || clean {
                return Err("selection copying accepts no URL or --clean".into());
            }
            return self.execute_selection_yank(primary);
        }
        if source == Some("title") {
            if input.is_some() || clean {
                return Err("title copying accepts no URL or --clean".into());
            }
            let title = self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .and_then(|state| {
                    self.as_ref()
                        .rust()
                        .tab
                        .and_then(|tab| state.tabs().get(&tab))
                })
                .map(|tab| sanitize_untrusted_title(&tab.title))
                .filter(|title| !title.is_empty())
                .ok_or_else(|| "current tab has no title to copy".to_owned())?;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .clipboard_request = QString::from(&title);
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .clipboard_request_sensitive = false;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .clipboard_request_primary = primary;
            self.as_mut().set_status_text(QString::from("Title copied"));
            return Ok(serde_json::json!({
                "status": "accepted",
                "source": "title",
                "copied": title,
                "primary": primary
            }));
        }
        if source != Some("url") {
            return Err("yank source must be url, title, or selection".into());
        }
        let raw_input = if let Some(input) = input {
            input.to_owned()
        } else {
            self.as_ref()
                .rust()
                .state
                .as_ref()
                .and_then(|state| {
                    self.as_ref()
                        .rust()
                        .tab
                        .and_then(|tab| state.tabs().get(&tab))
                        .and_then(|tab| tab.url.clone())
                })
                .unwrap_or_else(|| "about:blank".into())
        };
        let navigation = resolve_input(&raw_input, &self.as_ref().navigation_context())
            .map_err(|error| error.to_string())?;
        let copied = if clean {
            let rules =
                link_cleaning_policy::active_rules(self.as_ref().rust().storage_roots.as_ref());
            clean_link(navigation.url.as_str(), &rules)
                .map_err(|error| error.to_string())?
                .cleaned
        } else {
            navigation.url.to_string()
        };
        let safe_original = safe_ipc_url(navigation.url.as_str());
        let safe_copied = safe_ipc_url(&copied);
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .clipboard_request = QString::from(&safe_copied);
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .clipboard_request_sensitive = false;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .clipboard_request_primary = primary;
        self.as_mut().set_status_text(QString::from(if clean {
            "Clean URL copied"
        } else {
            "URL copied"
        }));
        Ok(serde_json::json!({
            "status": "accepted",
            "source": "url",
            "original": safe_original,
            "copied": safe_copied,
            "changed": clean && safe_original != safe_copied,
            "clean": clean,
            "primary": primary
        }))
    }

    pub(super) fn queue_caret_operation(
        mut self: Pin<&mut Self>,
        operation: String,
    ) -> Result<(), String> {
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        )
        .ok_or_else(|| "current document is unavailable".to_owned())?;
        self.as_mut().enter_caret();
        self.as_mut().rust_mut().as_mut().get_mut().pending_caret = Some(PendingCaret {
            token: format!("caret-{}", Uuid::new_v4()),
            target,
            operation,
            issued: false,
        });
        Ok(())
    }

    pub(super) fn execute_caret_move(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<(), String> {
        if !(1..=3).contains(&command.arguments.len()) {
            return Err("caret-move expects DIRECTION [--count N]".into());
        }
        let direction = command
            .arguments
            .first()
            .ok_or_else(|| "caret-move requires a direction".to_owned())?;
        if !matches!(
            direction.as_str(),
            "left"
                | "right"
                | "up"
                | "down"
                | "word-next"
                | "word-prev"
                | "line-start"
                | "line-end"
        ) {
            return Err(format!("unknown caret direction: {direction}"));
        }
        let count = match command.arguments.as_slice() {
            [_] => 1,
            [_, flag, value] if flag == "--count" => value
                .parse::<u32>()
                .ok()
                .filter(|count| (1..=9_999).contains(count))
                .ok_or_else(|| "caret-move count must be 1..9999".to_owned())?,
            _ => return Err("caret-move expects DIRECTION [--count N]".into()),
        };
        self.as_mut()
            .queue_caret_operation(format!("move\t{direction}\t{count}"))
    }

    pub(super) fn execute_caret_select(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<(), String> {
        let state = match command.arguments.as_slice() {
            [] => "toggle",
            [state] if matches!(state.as_str(), "on" | "off" | "toggle") => state.as_str(),
            _ => return Err("caret-select expects [on|off|toggle]".into()),
        };
        self.as_mut()
            .queue_caret_operation(format!("select\t{state}"))
    }

    pub(super) fn execute_editor_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<(), String> {
        if !command.arguments.is_empty() {
            return Err("edit-text does not accept arguments".into());
        }
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        )
        .ok_or_else(|| "current document is unavailable".to_owned())?;
        if self.as_ref().rust().pending_editor.is_some() {
            return Err("an external edit is already in progress".into());
        }
        let token = format!("editor-{}", Uuid::new_v4());
        self.as_mut().rust_mut().as_mut().get_mut().pending_editor = Some(PendingEditor {
            token,
            target,
            original: String::new(),
            file: PathBuf::new(),
            process: None,
            cancellation: Arc::new(AtomicBool::new(false)),
            private_temporary: false,
            issued: false,
            created_at: Instant::now(),
        });
        self.set_status_text(QString::from("External editor requested"));
        Ok(())
    }

    pub(super) fn execute_config_edit_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        if !command.arguments.is_empty() {
            return Err("config-edit does not accept arguments".into());
        }
        if self.as_ref().rust().pending_config_edit.is_some() {
            return Err("a configuration edit is already in progress".into());
        }
        let path_text = self.as_ref().rust().config_path.to_string();
        if path_text.is_empty() {
            return Err("no file-backed configuration is active".into());
        }
        let path = fs::canonicalize(&path_text).map_err(|error| {
            format!("configured file-backed configuration is unavailable: {error}")
        })?;
        if !path.is_file() {
            return Err("configured file-backed configuration is not a regular file".into());
        }
        let (executable, arguments) = self.as_ref().editor_argv(&path)?;
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let mut editor_command = Command::new(&executable);
        editor_command
            .args(arguments)
            .current_dir(parent)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        editor_command.process_group(0);
        let mut child = editor_command
            .spawn()
            .map_err(|error| format!("configuration editor could not start: {error}"))?;
        let stderr = child
            .stderr
            .take()
            .map(|stderr| {
                let result = Arc::new(Mutex::new(None));
                let destination = Arc::clone(&result);
                thread::spawn(move || {
                    let value = read_bounded(stderr, 64 * 1024);
                    if let Ok(mut destination) = destination.lock() {
                        *destination = Some(value);
                    }
                });
                result
            })
            .unwrap_or_else(|| {
                Arc::new(Mutex::new(Some(Err(
                    "configuration editor stderr was not captured".into(),
                ))))
            });
        let token = format!("config-edit-{}", Uuid::new_v4());
        let process = Arc::new(Mutex::new(Some(child)));
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_config_edit = Some(PendingConfigEdit {
            token: token.clone(),
            path: path.clone(),
            process,
            exit_status: None,
            stderr,
        });
        self.set_status_text(QString::from("Configuration editor started"));
        Ok(serde_json::json!({
            "status": "accepted",
            "path": path,
            "token": token
        }))
    }

    pub(super) fn execute_spawn_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let mut argv = command.arguments.clone();
        if argv.first().map(String::as_str) == Some("--userscript") {
            let [_, name] = argv.as_slice() else {
                return Err("spawn --userscript requires a manifest name".into());
            };
            return self.execute_userscript(name);
        }
        if argv.first().map(String::as_str) == Some("--") {
            argv.remove(0);
        }
        if argv.is_empty() {
            return Err("spawn requires -- PROGRAM [ARG...]".into());
        }
        if argv.len() > 256 {
            return Err("spawn accepts at most 256 argv values".into());
        }
        let (resolved, needs_selection) = self.resolve_spawn_argv(&argv, None)?;
        if needs_selection {
            let target = current_target(
                self.as_ref().rust().state.as_ref(),
                self.as_ref().rust().tab,
            )
            .ok_or_else(|| "current document is unavailable".to_owned())?;
            let token = format!("spawn-{}", Uuid::new_v4());
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.pending_spawn = Some(PendingSpawn {
                token: token.clone(),
                target,
                argv,
            });
            this.pending_selection = Some(PendingSelection {
                token,
                target,
                primary: false,
                issued: false,
                created_at: Instant::now(),
                search_engine: None,
                operation_id: None,
            });
            self.set_status_text(QString::from("Spawn selection requested"));
            return Ok(serde_json::json!({"status": "accepted", "pending": true}));
        }
        let operation_id = self.as_mut().start_spawn_process(&resolved)?;
        self.set_status_text(QString::from(format!("Spawn started: {operation_id}")));
        Ok(serde_json::json!({
            "status": "accepted",
            "operation_id": operation_id
        }))
    }

    pub(super) fn execute_script_run_command(
        self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let [name] = command.arguments.as_slice() else {
            return Err("script-run requires exactly one userscript name".into());
        };
        self.execute_userscript(name)
    }

    pub(super) fn execute_jseval_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let (world, script) = match command.arguments.as_slice() {
            [script] => ("isolated", script.as_str()),
            [flag, world, script] if flag == "--world" => (world.as_str(), script.as_str()),
            _ => return Err("jseval accepts [--world isolated|page] SCRIPT".into()),
        };
        if !matches!(world, "isolated" | "page") {
            return Err("jseval world must be isolated or page".into());
        }
        validate_jseval_script(script)?;
        let tab_id = self
            .as_ref()
            .rust()
            .tab
            .ok_or_else(|| "jseval requires an active tab".to_owned())?;
        self.as_mut()
            .set_jseval_tab_id(QString::from(tab_id.to_string()));
        self.as_mut().set_jseval_world(QString::from(world));
        self.as_mut().set_jseval_script(QString::from(script));
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some("jseval".into());
        self.as_mut().set_status_text(QString::from(format!(
            "JavaScript evaluation requested in {world} world"
        )));
        Ok(serde_json::json!({
            "status": "accepted",
            "tab_id": tab_id.to_string(),
            "world": world,
            "script_bytes": script.len()
        }))
    }

    pub(super) fn execute_mode_enter(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<(), String> {
        let [mode] = command.arguments.as_slice() else {
            return Err("mode-enter requires normal, insert, caret, or passthrough".into());
        };
        match mode.as_str() {
            "normal" => self.escape(),
            "insert" => self.enter_insert(),
            "caret" => self.enter_caret(),
            "passthrough" | "pass-through" => {
                let Some(window) = self.as_ref().rust().window else {
                    return Err("no active window".into());
                };
                self.as_mut()
                    .reduce_event(Event::PushMode {
                        window,
                        mode: Mode::PassThrough,
                    })
                    .map_err(|error| error.clone())?;
                self.as_mut().set_core_mode(Mode::PassThrough);
                self.as_mut().set_mode(QString::from("pass-through"));
            }
            _ => return Err(format!("unknown mode: {mode}")),
        }
        Ok(())
    }

    pub(super) fn execute_clean_open_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let Some(input) = clean_open_input(command)? else {
            return Err("not a clean-link open command".into());
        };
        let navigation = resolve_input(&input, &self.as_ref().navigation_context())
            .map_err(|error| error.to_string())?;
        let rules = link_cleaning_policy::active_rules(self.as_ref().rust().storage_roots.as_ref());
        let result =
            clean_link(navigation.url.as_str(), &rules).map_err(|error| error.to_string())?;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_link_navigation = None;
        let mut value = link_result_value("open", &result, &rules);
        if !result.changed {
            let url = ValidatedUrl::parse(result.cleaned).map_err(|error| error.to_string())?;
            let target = current_target(
                self.as_ref().rust().state.as_ref(),
                self.as_ref().rust().tab,
            )
            .ok_or_else(|| "tab is no longer live".to_owned())?;
            let effects = self
                .as_mut()
                .reduce_event(Event::StartNavigation { target, url })
                .map_err(|error| error.clone())?;
            self.as_mut().set_pending_engine_action(&effects);
            self.as_mut().sync_core_tabs();
            value["status"] = Value::String("accepted".into());
            return Ok(value);
        }
        let cleaned = ValidatedUrl::parse(result.cleaned.clone())
            .map_err(|error| format!("cleaned URL is invalid: {error}"))?;
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        )
        .ok_or_else(|| "tab is no longer live".to_owned())?;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = None;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_link_navigation = Some(PendingLinkNavigation {
            target: Some(target),
            new_window_action: None,
            journey_parent: None,
            url: cleaned,
        });
        value["requires_confirmation"] = Value::Bool(true);
        self.as_mut().publish_link_preview("open", &result, true);
        self.as_mut().set_link_preview_visible(true);
        self.set_status_text(QString::from("Clean-link navigation awaits confirmation"));
        Ok(value)
    }
}
