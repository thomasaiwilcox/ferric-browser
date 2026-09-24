use super::{
    Arc, Command, CommandExt, CxxQtType, Duration, EditorCompletion, EditorWriteWorker, Mutex,
    Ordering, PAGE_SCRIPT_DEADLINE, Path, Pin, QString, Stdio, Uuid, Value,
    cleanup_editor_artifact, current_target, discard_editor_request,
    finish_pending_selection_operation, fs, join_editor_stderr, qobject, read_bounded,
    safe_ipc_url, terminate_editor_process, thread, try_wait_editor_process,
};

impl qobject::BrowserUi {
    pub(super) fn take_clipboard_request(mut self: Pin<&mut Self>) -> QString {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        std::mem::take(&mut this.clipboard_request)
    }

    pub(super) fn write_clipboard(self: Pin<&mut Self>, value: &QString, primary: bool) -> bool {
        if qobject::ferric_browser_write_clipboard(value, primary) {
            true
        } else {
            self.set_status_text(QString::from(if primary {
                "Primary selection is unavailable"
            } else {
                "Clipboard is unavailable"
            }));
            false
        }
    }

    pub(super) fn take_selection_request(mut self: Pin<&mut Self>) -> QString {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let expired = this
            .pending_selection
            .as_ref()
            .is_some_and(|request| request.created_at.elapsed() >= PAGE_SCRIPT_DEADLINE);
        if expired {
            let operation_id = this
                .pending_selection
                .as_ref()
                .and_then(|request| request.operation_id.clone());
            this.pending_selection = None;
            this.pending_spawn = None;
            if let Some(operation_id) = operation_id {
                this.operation_states.insert(
                    operation_id,
                    "failed (selection extraction timed out)".into(),
                );
            }
            if let Some(pending) = this.pending_action_target.take() {
                this.operation_states.insert(
                    pending.operation_id,
                    "failed (selection extraction timed out)".into(),
                );
            }
            if let Some(pending) = this.pending_userscript.take() {
                this.operation_states.insert(
                    pending.operation_id,
                    "failed (selection extraction timed out)".into(),
                );
            }
            self.set_status_text(QString::from("Selection extraction timed out"));
            return QString::default();
        }
        let Some(request) = this.pending_selection.as_mut() else {
            return QString::default();
        };
        if request.issued {
            return QString::default();
        }
        request.issued = true;
        QString::from(&request.token)
    }

    pub(super) fn deliver_action_target_selection(
        mut self: Pin<&mut Self>,
        token: &str,
        result: &QString,
    ) -> bool {
        let pending = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(pending) = this.pending_action_target.as_ref() else {
                return false;
            };
            if pending.token != token {
                self.set_status_text(QString::from("Action target selection is stale"));
                return false;
            }
            let pending = this.pending_action_target.take();
            finish_pending_selection_operation(this, "cancelled");
            pending
        };
        let Some(pending) = pending else {
            self.set_status_text(QString::from("Action target selection expired"));
            return false;
        };
        let fail = |mut this: Pin<&mut Self>, message: String| {
            this.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .operation_states
                .insert(pending.operation_id.clone(), format!("failed ({message})"));
            this.set_status_text(QString::from(message));
            false
        };
        if current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        ) != Some(pending.target)
        {
            return fail(self, "Action target selection is stale".into());
        }
        if pending.created_at.elapsed() >= PAGE_SCRIPT_DEADLINE {
            return fail(self, "Action target selection timed out".into());
        }
        let value: Value = match serde_json::from_str(&result.to_string()) {
            Ok(value) => value,
            Err(_) => return fail(self, "Action target selection extraction failed".into()),
        };
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            return fail(
                self,
                format!("Action target selection unavailable: {error}"),
            );
        }
        let Some(selection) = value.get("text").and_then(Value::as_str) else {
            return fail(self, "Action target selection returned invalid text".into());
        };
        if selection.is_empty() {
            return fail(self, "Action target selection is empty".into());
        }
        if selection.len() > 1024 * 1024 || selection.contains('\0') {
            return fail(
                self,
                "Action target selection is too large or invalid".into(),
            );
        }
        let url = {
            let binding = self.as_ref();
            let Some(state) = binding.rust().state.as_ref() else {
                return fail(self, "core state unavailable".into());
            };
            let Some(tab) = state.tabs().get(&pending.target.tab) else {
                return fail(self, "current tab is unavailable".into());
            };
            tab.url
                .as_deref()
                .map_or_else(|| "about:blank".into(), safe_ipc_url)
        };
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
        let mut argv = vec![pending.target_config.executable.clone()];
        for argument in &pending.target_config.argv {
            argv.push(match argument.as_str() {
                "{url}" => url.clone(),
                "{title}" => title.clone(),
                "{selection}" => selection.to_owned(),
                _ => argument.clone(),
            });
        }
        match self
            .as_mut()
            .start_action_target_process(&argv, pending.target_config.detach)
        {
            Ok(operation_id) => {
                self.set_status_text(QString::from(format!(
                    "Sent selection to {}: {operation_id}",
                    pending.target_name
                )));
                true
            }
            Err(error) => fail(self, format!("action target failed: {error}")),
        }
    }

    pub(super) fn deliver_selection(
        mut self: Pin<&mut Self>,
        token: &QString,
        result: &QString,
    ) -> bool {
        let token = token.to_string();
        if self
            .as_ref()
            .rust()
            .pending_userscript
            .as_ref()
            .is_some_and(|pending| pending.token == token)
        {
            return self.as_mut().deliver_userscript_selection(&token, result);
        }
        if self
            .as_ref()
            .rust()
            .pending_spawn
            .as_ref()
            .is_some_and(|pending| pending.token == token)
        {
            return self.as_mut().deliver_spawn_selection(&token, result);
        }
        if self
            .as_ref()
            .rust()
            .pending_action_target
            .as_ref()
            .is_some_and(|pending| pending.token == token)
        {
            return self
                .as_mut()
                .deliver_action_target_selection(&token, result);
        }
        if self
            .as_ref()
            .rust()
            .pending_selection
            .as_ref()
            .is_some_and(|pending| pending.token == token && pending.search_engine.is_some())
        {
            return self.as_mut().deliver_selection_search(&token, result);
        }
        let pending = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(request) = this.pending_selection.as_ref() else {
                self.set_status_text(QString::from("Selection request expired"));
                return false;
            };
            if request.token != token {
                self.set_status_text(QString::from("Selection request is stale"));
                return false;
            }
            this.pending_selection.take()
        };
        let Some(pending) = pending else {
            self.set_status_text(QString::from("Selection request expired"));
            return false;
        };
        if pending.created_at.elapsed() >= PAGE_SCRIPT_DEADLINE {
            self.set_status_text(QString::from("Selection extraction timed out"));
            return false;
        }
        let current = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        );
        if current != Some(pending.target) {
            self.set_status_text(QString::from("Selection request is stale"));
            return false;
        }
        let value: Value = if let Ok(value) = serde_json::from_str(&result.to_string()) {
            value
        } else {
            self.set_status_text(QString::from("Selection extraction failed"));
            return false;
        };
        let Some(object) = value.as_object() else {
            self.set_status_text(QString::from("Selection extraction returned invalid data"));
            return false;
        };
        if let Some(error) = object.get("error").and_then(Value::as_str) {
            self.set_status_text(QString::from(format!("Selection not copied: {error}")));
            return false;
        }
        let Some(text) = object.get("text").and_then(Value::as_str) else {
            self.set_status_text(QString::from("Selection extraction returned invalid text"));
            return false;
        };
        if text.is_empty() {
            self.set_status_text(QString::from("No document selection"));
            return false;
        }
        if text.len() > 1024 * 1024 || text.contains('\0') {
            self.set_status_text(QString::from("Selection is too large or invalid"));
            return false;
        }
        let primary = pending.primary;
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.clipboard_request = QString::from(text);
        this.clipboard_request_sensitive = true;
        this.clipboard_request_primary = primary;
        self.set_status_text(QString::from("Selection copied"));
        true
    }

    pub(super) fn deliver_userscript_selection(
        mut self: Pin<&mut Self>,
        token: &str,
        result: &QString,
    ) -> bool {
        let pending = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(pending) = this.pending_userscript.as_ref() else {
                return false;
            };
            if pending.token != token {
                self.set_status_text(QString::from("Userscript selection is stale"));
                return false;
            }
            let pending = this.pending_userscript.take();
            finish_pending_selection_operation(this, "cancelled");
            pending
        };
        let Some(pending) = pending else {
            return false;
        };
        let operation_id = pending.operation_id.clone();
        let fail = |mut this: Pin<&mut Self>, message: String| {
            this.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .operation_states
                .insert(operation_id.clone(), format!("failed ({message})"));
            this.set_status_text(QString::from(message));
            false
        };
        if current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        ) != Some(pending.target)
        {
            return fail(self, "Userscript selection target is stale".into());
        }
        let value: Value = match serde_json::from_str(&result.to_string()) {
            Ok(value) => value,
            Err(_) => return fail(self, "Userscript selection extraction failed".into()),
        };
        if let Some(error) = value.get("error").and_then(Value::as_str) {
            return fail(self, format!("Userscript selection unavailable: {error}"));
        }
        let Some(text) = value.get("text").and_then(Value::as_str) else {
            return fail(self, "Userscript selection returned invalid text".into());
        };
        if text.is_empty() {
            return fail(self, "Userscript selection is empty".into());
        }
        if text.len() > 1024 * 1024 || text.contains('\0') {
            return fail(self, "Userscript selection is too large or invalid".into());
        }
        match self.as_mut().execute_userscript_with_context(
            &pending.name,
            pending.target,
            None,
            Some(text),
            Some(operation_id.clone()),
            None,
            None,
        ) {
            Ok(_) => {
                self.set_status_text(QString::from("Userscript selection started"));
                true
            }
            Err(error) => fail(self, format!("Userscript selection failed: {error}")),
        }
    }

    pub(super) fn take_caret_request(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().set_caret_request_token(QString::default());
        self.as_mut()
            .set_caret_request_operation(QString::default());
        let request = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(request) = this.pending_caret.as_mut() else {
                return false;
            };
            if request.issued {
                return false;
            }
            request.issued = true;
            (request.token.clone(), request.operation.clone())
        };
        self.as_mut()
            .set_caret_request_token(QString::from(request.0));
        self.as_mut()
            .set_caret_request_operation(QString::from(request.1));
        true
    }

    pub(super) fn deliver_caret(
        mut self: Pin<&mut Self>,
        token: &QString,
        result: &QString,
    ) -> bool {
        let token = token.to_string();
        let pending = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(request) = this.pending_caret.as_ref() else {
                self.set_status_text(QString::from("Caret request expired"));
                return false;
            };
            if request.token != token {
                self.set_status_text(QString::from("Caret request is stale"));
                return false;
            }
            this.pending_caret.take()
        };
        let Some(pending) = pending else {
            self.set_status_text(QString::from("Caret request expired"));
            return false;
        };
        if current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        ) != Some(pending.target)
        {
            self.set_status_text(QString::from("Caret request is stale"));
            return false;
        }
        let value: Value = if let Ok(value) = serde_json::from_str(&result.to_string()) {
            value
        } else {
            self.set_status_text(QString::from("Caret operation failed"));
            return false;
        };
        let Some(object) = value.as_object() else {
            self.set_status_text(QString::from("Caret operation returned invalid data"));
            return false;
        };
        if let Some(error) = object.get("error").and_then(Value::as_str) {
            self.set_status_text(QString::from(format!("Caret operation rejected: {error}")));
            return false;
        }
        if object.get("ok").and_then(Value::as_bool) != Some(true) {
            self.set_status_text(QString::from("Caret operation was not applied"));
            return false;
        }
        if pending.operation.starts_with("select\t")
            && let Some(selecting) = object.get("selecting").and_then(Value::as_bool)
        {
            self.as_mut().set_caret_selecting(selecting);
        }
        self.set_status_text(QString::from("Caret updated"));
        true
    }

    pub(super) fn take_editor_request(mut self: Pin<&mut Self>) -> QString {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let expired = this
            .pending_editor
            .as_ref()
            .is_some_and(|request| request.created_at.elapsed() >= PAGE_SCRIPT_DEADLINE);
        if expired {
            if let Some(request) = this.pending_editor.take() {
                discard_editor_request(request);
            }
            self.set_status_text(QString::from("Editor extraction timed out"));
            return QString::default();
        }
        let Some(request) = this.pending_editor.as_mut() else {
            return QString::default();
        };
        if request.issued {
            return QString::default();
        }
        request.issued = true;
        QString::from(&request.token)
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn deliver_editor(
        mut self: Pin<&mut Self>,
        token: &QString,
        result: &QString,
    ) -> bool {
        let token = token.to_string();
        if self
            .as_ref()
            .rust()
            .pending_editor
            .as_ref()
            .is_some_and(|pending| {
                pending.token == token && pending.created_at.elapsed() >= PAGE_SCRIPT_DEADLINE
            })
        {
            self.as_mut()
                .finish_editor_error(&token, "extraction timed out");
            return false;
        }
        let value: Value = if let Ok(value) = serde_json::from_str(&result.to_string()) {
            value
        } else {
            self.set_status_text(QString::from("Editor extraction failed"));
            return false;
        };
        let Some(object) = value.as_object() else {
            self.set_status_text(QString::from("Editor extraction returned invalid data"));
            return false;
        };
        if let Some(error) = object.get("error").and_then(Value::as_str) {
            self.finish_editor_error(&token, error);
            return false;
        }
        let Some(text) = object.get("text").and_then(Value::as_str) else {
            self.finish_editor_error(&token, "focused control did not return text");
            return false;
        };
        if text.len() > 1024 * 1024 || text.contains('\0') {
            self.finish_editor_error(&token, "text is too large or invalid");
            return false;
        }
        let Some((pending_token, pending_target)) = self
            .as_ref()
            .rust()
            .pending_editor
            .as_ref()
            .map(|pending| (pending.token.clone(), pending.target))
        else {
            self.set_status_text(QString::from("Editor request expired"));
            return false;
        };
        if pending_token != token {
            self.set_status_text(QString::from("Editor request is stale"));
            return false;
        }
        if current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        ) != Some(pending_target)
        {
            self.finish_editor_error(&token, "document target changed");
            return false;
        }
        let (editor_dir, private_temporary) =
            self.as_ref().rust().storage_roots.as_ref().map_or_else(
                || {
                    (
                        std::env::temp_dir().join(format!(
                            "ferric-browser-editor-{}",
                            self.as_ref().rust().session_id
                        )),
                        true,
                    )
                },
                |roots| (roots.runtime.join("editor"), false),
            );
        let file = editor_dir.join(format!("{}.txt", Uuid::new_v4()));
        let request = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .editor_write_worker
            .as_mut()
            .ok_or_else(|| "editor writer is unavailable".to_owned())
            .and_then(|worker| worker.request(file.clone(), text.as_bytes().to_vec()));
        if let Err(error) = request {
            self.finish_editor_error(&token, &format!("could not queue editor file: {error}"));
            return false;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let pending = this
                .pending_editor
                .as_mut()
                .expect("editor request checked");
            text.clone_into(&mut pending.original);
            pending.file.clone_from(&file);
            pending.private_temporary = private_temporary;
            this.pending_editor_write = Some((token.clone(), file, private_temporary));
        }
        self.set_status_text(QString::from(if private_temporary {
            "Preparing private external editor; the editor may create its own swap files"
        } else {
            "Preparing external editor"
        }));
        true
    }

    pub(super) fn poll_editor_write(mut self: Pin<&mut Self>) {
        let result = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .editor_write_worker
                .as_mut()
                .and_then(EditorWriteWorker::poll)
        };
        let Some(result) = result else {
            return;
        };
        let pending_write = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_editor_write
            .take();
        let Some((token, file, private_temporary)) = pending_write else {
            if let Ok(path) = result {
                let _ = fs::remove_file(path);
            }
            return;
        };
        let path = match result {
            Ok(path) if path == file => path,
            Ok(path) => {
                cleanup_editor_artifact(&path, private_temporary);
                self.finish_editor_error(&token, "editor file result became stale");
                return;
            }
            Err(error) => {
                self.finish_editor_error(&token, &format!("could not write editor file: {error}"));
                return;
            }
        };
        let Some(pending) = self
            .as_ref()
            .rust()
            .pending_editor
            .as_ref()
            .filter(|pending| pending.token == token)
            .cloned()
        else {
            cleanup_editor_artifact(&path, private_temporary);
            return;
        };
        let editor_dir = path.parent().unwrap_or_else(|| Path::new("."));
        let (executable, args) = match self.editor_argv(&path) {
            Ok(editor) => editor,
            Err(error) => {
                cleanup_editor_artifact(&path, private_temporary);
                self.finish_editor_error(&token, &error);
                return;
            }
        };
        let mut editor_command = Command::new(&executable);
        editor_command
            .args(args)
            .current_dir(editor_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        editor_command.process_group(0);
        match editor_command.spawn() {
            Ok(mut child) => {
                let stderr = child
                    .stderr
                    .take()
                    .map(|stderr| thread::spawn(move || read_bounded(stderr, 64 * 1024)));
                let process = Arc::new(Mutex::new(Some(child)));
                let cancellation = Arc::clone(&pending.cancellation);
                let completions = Arc::clone(&self.as_ref().rust().editor_completions);
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                if let Some(current) = this
                    .pending_editor
                    .as_mut()
                    .filter(|current| current.token == token)
                {
                    current.process = Some(Arc::clone(&process));
                }
                thread::spawn(move || {
                    let mut stderr = stderr;
                    let completion = loop {
                        if cancellation.load(Ordering::Acquire) {
                            terminate_editor_process(&process);
                            cleanup_editor_artifact(&pending.file, pending.private_temporary);
                            return;
                        }
                        match try_wait_editor_process(&process) {
                            Ok(Some(status)) if status.success() => {
                                break match fs::read_to_string(&pending.file) {
                                    Ok(updated)
                                        if updated.len() <= 1024 * 1024
                                            && !updated.contains('\0') =>
                                    {
                                        EditorCompletion {
                                            token: pending.token.clone(),
                                            original: pending.original.clone(),
                                            updated: Some(updated),
                                            error: None,
                                            stderr: join_editor_stderr(&mut stderr),
                                        }
                                    }
                                    Ok(_) => EditorCompletion {
                                        token: pending.token.clone(),
                                        original: pending.original.clone(),
                                        updated: None,
                                        error: Some("edited text is too large or invalid".into()),
                                        stderr: join_editor_stderr(&mut stderr),
                                    },
                                    Err(error) => EditorCompletion {
                                        token: pending.token.clone(),
                                        original: pending.original.clone(),
                                        updated: None,
                                        error: Some(format!("could not read edited text: {error}")),
                                        stderr: join_editor_stderr(&mut stderr),
                                    },
                                };
                            }
                            Ok(Some(status)) => {
                                break EditorCompletion {
                                    token: pending.token.clone(),
                                    original: pending.original.clone(),
                                    updated: None,
                                    error: Some(format!("editor exited with status {status}")),
                                    stderr: join_editor_stderr(&mut stderr),
                                };
                            }
                            Ok(None) => thread::sleep(Duration::from_millis(20)),
                            Err(error) => {
                                break EditorCompletion {
                                    token: pending.token.clone(),
                                    original: pending.original.clone(),
                                    updated: None,
                                    error: Some(error),
                                    stderr: join_editor_stderr(&mut stderr),
                                };
                            }
                        }
                    };
                    if cancellation.load(Ordering::Acquire) {
                        cleanup_editor_artifact(&pending.file, pending.private_temporary);
                        return;
                    }
                    if let Ok(mut completions) = completions.lock() {
                        completions.push(completion);
                    }
                });
                self.set_status_text(QString::from("External editor started"));
            }
            Err(error) => {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                if let Some(request) = this.pending_editor.take() {
                    discard_editor_request(request);
                }
                self.set_status_text(QString::from(format!("Editor could not start: {error}")));
            }
        }
    }

    pub(super) fn take_editor_completion(mut self: Pin<&mut Self>) -> bool {
        self.as_mut()
            .set_editor_completion_token(QString::default());
        self.as_mut()
            .set_editor_completion_original(QString::default());
        self.as_mut()
            .set_editor_completion_updated(QString::default());
        self.as_mut()
            .set_editor_completion_error(QString::default());
        self.as_mut()
            .set_editor_completion_stderr(QString::default());
        let completion = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.editor_completions.lock().ok().and_then(|mut queue| {
                let token = this
                    .pending_editor
                    .as_ref()
                    .map(|request| request.token.as_str());
                queue
                    .iter()
                    .position(|completion| Some(completion.token.as_str()) == token)
                    .map(|index| queue.remove(index))
            })
        };
        let Some(completion) = completion else {
            return false;
        };
        self.as_mut()
            .set_editor_completion_token(QString::from(completion.token));
        self.as_mut()
            .set_editor_completion_original(QString::from(completion.original));
        self.as_mut()
            .set_editor_completion_updated(QString::from(completion.updated.unwrap_or_default()));
        self.as_mut()
            .set_editor_completion_error(QString::from(completion.error.unwrap_or_default()));
        self.as_mut()
            .set_editor_completion_stderr(QString::from(completion.stderr.unwrap_or_default()));
        true
    }

    pub(super) fn deliver_editor_apply(
        mut self: Pin<&mut Self>,
        token: &QString,
        result: &QString,
    ) -> bool {
        let token = token.to_string();
        let pending = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(request) = this.pending_editor.as_ref() else {
                self.set_status_text(QString::from("Editor request expired"));
                return false;
            };
            if request.token != token {
                self.set_status_text(QString::from("Editor request is stale"));
                return false;
            }
            this.pending_editor.take()
        };
        let Some(pending) = pending else {
            self.set_status_text(QString::from("Editor request expired"));
            return false;
        };
        cleanup_editor_artifact(&pending.file, pending.private_temporary);
        if current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        ) != Some(pending.target)
        {
            self.set_status_text(QString::from("Editor target is stale"));
            return false;
        }
        let value: Value = if let Ok(value) = serde_json::from_str(&result.to_string()) {
            value
        } else {
            self.set_status_text(QString::from("Editor write-back failed"));
            return false;
        };
        let Some(object) = value.as_object() else {
            self.set_status_text(QString::from("Editor write-back returned invalid data"));
            return false;
        };
        if let Some(error) = object.get("error").and_then(Value::as_str) {
            self.set_status_text(QString::from(format!("Editor not applied: {error}")));
            return false;
        }
        if object.get("ok").and_then(Value::as_bool) != Some(true) {
            self.set_status_text(QString::from("Editor write-back was not applied"));
            return false;
        }
        self.set_status_text(QString::from("External edit applied"));
        true
    }

    pub(super) fn finish_editor_error(mut self: Pin<&mut Self>, token: &str, error: &str) {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if this
            .pending_editor
            .as_ref()
            .is_some_and(|pending| pending.token == token)
        {
            if let Some(pending) = this.pending_editor.take() {
                discard_editor_request(pending);
            }
        }
        self.set_status_text(QString::from(format!("Editor not started: {error}")));
    }
}
