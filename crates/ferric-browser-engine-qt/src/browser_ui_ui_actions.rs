use super::{
    ActionRegistry, BTreeMap, CommandRegistry, CommandSource, CxxQtType, ParsedCommand, PathBuf,
    Pin, PublicError, QString, QStringList, UserscriptManagerRequest, ValidatedUrl, Value,
    action_failure_category, action_operation_id, current_target, external_action_presentations,
    is_bounded_untrusted_text, is_safe_history_url, parse_external_action_id, qobject,
    safe_ipc_url, typed_ipc_action, ui_action_arguments, userscript, userscript_subject_target,
};

impl qobject::BrowserUi {
    pub(super) fn execute_ui_action(
        self: Pin<&mut Self>,
        action_id: &QString,
        value: &QString,
    ) -> bool {
        self.execute_ui_action_with_source(action_id, value, CommandSource::Ui)
    }

    pub(super) fn execute_ui_action_with_source(
        mut self: Pin<&mut Self>,
        action_id: &QString,
        value: &QString,
        source: CommandSource,
    ) -> bool {
        let action_id = action_id.to_string();
        let value = value.to_string();
        if let Some((target_name, subject)) = parse_external_action_id(&action_id) {
            let mut arguments = vec![target_name];
            match subject.as_str() {
                "selection" => arguments.push("--selection".into()),
                "tab" => arguments.push("--tab".into()),
                "url" => {
                    arguments.push("--url".into());
                    if !value.is_empty() {
                        arguments.push(value);
                    }
                }
                "link" if !value.is_empty() => arguments.push(value),
                "link" => {}
                _ => unreachable!("external action ID parser validated subject"),
            }
            let result = self.as_mut().execute_send_command(&ParsedCommand {
                name: "send".into(),
                arguments,
            });
            let operation_id = action_operation_id(result.as_ref().ok(), "ui-action");
            return match result {
                Ok(_) => {
                    self.as_mut()
                        .record_action_audit(&action_id, &operation_id, "accepted", None);
                    self.as_mut()
                        .set_status_text(QString::from(format!("Executing {action_id}")));
                    true
                }
                Err(error) => {
                    let error = PublicError::engine(error);
                    self.as_mut().record_action_audit(
                        &action_id,
                        &operation_id,
                        "failed",
                        Some(action_failure_category(error.code())),
                    );
                    self.set_status_text(QString::from(format!("{action_id} failed: {error}")));
                    false
                }
            };
        }
        if action_id.starts_with("userscript.") {
            let result = self
                .as_mut()
                .execute_registered_userscript_action(&action_id, &value);
            let operation_id = action_operation_id(result.as_ref().ok(), "ui-userscript");
            return match result {
                Ok(_) => {
                    self.as_mut()
                        .record_action_audit(&action_id, &operation_id, "accepted", None);
                    self.as_mut()
                        .set_status_text(QString::from(format!("Executing {action_id}")));
                    true
                }
                Err(error) => {
                    let error = PublicError::engine(error);
                    self.as_mut().record_action_audit(
                        &action_id,
                        &operation_id,
                        "failed",
                        Some(action_failure_category(error.code())),
                    );
                    self.set_status_text(QString::from(format!("{action_id} failed: {error}")));
                    false
                }
            };
        }
        let arguments = match ui_action_arguments(&action_id, &value) {
            Ok(arguments) => arguments,
            Err(error) => {
                self.set_status_text(QString::from(error));
                return false;
            }
        };
        let (command, route, resolved_id) = match typed_ipc_action(&serde_json::json!({
            "action": action_id,
            "arguments": arguments
        })) {
            Ok(value) => value,
            Err(error) => {
                self.set_status_text(QString::from(error.to_string()));
                return false;
            }
        };
        let mut route = route;
        route.source = source;
        if let Err(error) = self
            .as_ref()
            .validate_action_availability(&resolved_id, route.source)
        {
            self.set_status_text(QString::from(error));
            return false;
        }
        let result = self.as_mut().execute_ipc_command(command, &route);
        let operation_id = action_operation_id(result.as_ref().ok(), "ui-action");
        match result {
            Ok(_) => {
                self.as_mut()
                    .record_action_audit(&resolved_id, &operation_id, "accepted", None);
                self.set_status_text(QString::from(format!("Executing {resolved_id}")));
                true
            }
            Err(error) => {
                let error = PublicError::engine(error);
                self.as_mut().record_action_audit(
                    &resolved_id,
                    &operation_id,
                    "failed",
                    Some(action_failure_category(error.code())),
                );
                self.set_status_text(QString::from(format!("{resolved_id} failed: {error}")));
                false
            }
        }
    }

    pub(super) fn select_external_action_subject(
        mut self: Pin<&mut Self>,
        subject: &QString,
    ) -> bool {
        let values = external_action_presentations(
            &self.as_ref().rust().config,
            &subject.to_string(),
            self.as_ref().active_profile_is_transient(),
        );
        match values {
            Ok(values) => {
                self.as_mut().set_external_action_ids(
                    values
                        .iter()
                        .map(|action| QString::from(&action.id))
                        .collect(),
                );
                self.as_mut().set_external_action_labels(
                    values
                        .iter()
                        .map(|action| QString::from(&action.label))
                        .collect(),
                );
                self.as_mut().set_external_action_availability(
                    values
                        .iter()
                        .map(|action| QString::from(action.available.to_string()))
                        .collect(),
                );
                true
            }
            Err(error) => {
                self.as_mut()
                    .set_external_action_ids(QStringList::default());
                self.as_mut()
                    .set_external_action_labels(QStringList::default());
                self.as_mut()
                    .set_external_action_availability(QStringList::default());
                self.set_status_text(QString::from(format!(
                    "External actions unavailable: {error}"
                )));
                false
            }
        }
    }

    pub(super) fn install_userscript_manifest(mut self: Pin<&mut Self>, path: &QString) -> bool {
        let path = path.to_string();
        if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
            self.as_mut().set_userscript_install_state(QString::from(
                "error:userscript manifest path is empty or invalid",
            ));
            return false;
        }
        if self.as_ref().active_profile_is_transient() {
            self.as_mut().set_userscript_install_state(QString::from(
                "error:userscript installation is unavailable in private or ephemeral profiles",
            ));
            return false;
        }
        let Some(root) = self
            .as_ref()
            .rust()
            .userscript_roots
            .as_ref()
            .map(|roots| roots.config.clone())
        else {
            self.as_mut().set_userscript_install_state(QString::from(
                "error:userscripts are unavailable without configured storage",
            ));
            return false;
        };
        let result = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .userscript_manager_worker
            .as_mut()
            .ok_or_else(|| "userscript manager is unavailable".to_owned())
            .and_then(|worker| {
                worker.request(UserscriptManagerRequest::Install {
                    root,
                    source: PathBuf::from(path),
                })
            });
        match result {
            Ok(()) => {
                self.as_mut()
                    .set_userscript_install_state(QString::from("pending"));
                self.as_mut()
                    .set_status_text(QString::from("Installing userscript…"));
                true
            }
            Err(error) => {
                self.as_mut()
                    .set_userscript_install_state(QString::from(format!("error:{error}")));
                self.as_mut().set_status_text(QString::from(format!(
                    "Userscript installation failed: {error}"
                )));
                false
            }
        }
    }

    pub(super) fn execute_userscript_action_command(
        self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Option<(Value, String)>, String> {
        let [subject, verb, arguments @ ..] = command.arguments.as_slice() else {
            return Ok(None);
        };
        let Some(root) = self
            .as_ref()
            .rust()
            .userscript_roots
            .as_ref()
            .map(|roots| roots.config.clone())
        else {
            return Ok(None);
        };
        let Some(action) = userscript::registered_actions(&root)?
            .into_iter()
            .find(|action| action.subject == *subject && action.verb == *verb)
        else {
            return Ok(None);
        };
        let action_id = action.id.clone();
        let result = match action.subject.as_str() {
            "link" => {
                let [url] = arguments else {
                    return Err("userscript link actions require one URL".into());
                };
                self.execute_registered_userscript_action(&action_id, url)?
            }
            "selection" => {
                if !arguments.is_empty() {
                    return Err("userscript selection actions take no arguments".into());
                }
                self.execute_userscript(&action.script)?
            }
            "url" => {
                if !arguments.is_empty() {
                    return Err("userscript URL actions take no arguments".into());
                }
                self.execute_registered_userscript_action(&action_id, "")?
            }
            "tab" | "window" | "context" => {
                if arguments.len() > 1 {
                    return Err(format!(
                        "userscript {} actions accept at most one subject target",
                        action.subject
                    ));
                }
                self.execute_registered_userscript_action(
                    &action_id,
                    arguments.first().map(String::as_str).unwrap_or_default(),
                )?
            }
            "download" | "history-entry" | "bookmark" | "quickmark" | "session" | "command" => {
                let [value] = arguments else {
                    return Err(format!(
                        "userscript {} actions require one typed target",
                        action.subject
                    ));
                };
                self.execute_registered_userscript_action(&action_id, value)?
            }
            _ => {
                return Err(
                        "userscript actions currently support URL, link, selection, tab, window, context, download, history-entry, bookmark, quickmark, session, and command subjects".into(),
                    );
            }
        };
        Ok(Some((result, action_id)))
    }

    pub(super) fn execute_registered_userscript_action(
        self: Pin<&mut Self>,
        action_id: &str,
        value: &str,
    ) -> Result<Value, String> {
        self.execute_registered_userscript_action_with_source(action_id, value, false)
    }

    pub(super) fn execute_registered_userscript_action_for_hint(
        self: Pin<&mut Self>,
        action_id: &str,
        value: &str,
    ) -> Result<Value, String> {
        self.execute_registered_userscript_action_with_source(action_id, value, true)
    }

    pub(super) fn execute_registered_userscript_action_with_source(
        mut self: Pin<&mut Self>,
        action_id: &str,
        value: &str,
        hint_activation: bool,
    ) -> Result<Value, String> {
        let registry = ActionRegistry::default_v1();
        if registry.resolve(action_id).is_some() {
            return Err("userscript action cannot replace a built-in action".into());
        }
        let roots = self
            .as_ref()
            .rust()
            .userscript_roots
            .clone()
            .ok_or_else(|| "userscripts are unavailable without configured storage".to_owned())?;
        let action = userscript::registered_actions(&roots.config)?
            .into_iter()
            .find(|candidate| candidate.id == action_id)
            .ok_or_else(|| "userscript action is not installed".to_owned())?;
        let manifest = userscript::load(&roots.config, &action.script)?.manifest;
        let current = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        )
        .ok_or_else(|| "current document is unavailable".to_owned())?;
        let target = {
            let rust = self.as_ref().get_ref().rust();
            let state = rust.state.as_ref();
            let state = state.ok_or_else(|| "core state unavailable".to_owned())?;
            userscript_subject_target(state, current, &action.subject, value)?
        };
        let rust = self.as_ref().get_ref().rust();
        let state = rust
            .state
            .as_ref()
            .ok_or_else(|| "core state unavailable".to_owned())?;
        let current_profile = state
            .tabs()
            .get(&current.tab)
            .map(|tab| tab.profile)
            .ok_or_else(|| "current userscript profile is unavailable".to_owned())?;
        let target_profile = state
            .tabs()
            .get(&target.tab)
            .map(|tab| tab.profile)
            .ok_or_else(|| "userscript action target is stale".to_owned())?;
        if current_profile != target_profile {
            return Err("userscript action target belongs to another profile".into());
        }
        let bounded_value = || {
            is_bounded_untrusted_text(value)
                .then_some(value)
                .ok_or_else(|| "userscript action subject value is empty or too large".to_owned())
        };
        let mut subject_fields = BTreeMap::new();
        let (subject_url, selection) = match action.subject.as_str() {
            "link" => {
                let accepts_url = action.required_fields.iter().any(|field| field == "url");
                let accepts_hint_url = hint_activation
                    && action
                        .required_fields
                        .iter()
                        .any(|field| field == "hint_url");
                if !accepts_url && !accepts_hint_url {
                    return Err(
                        "link userscript action must require url, or hint_url during hint activation"
                            .into(),
                    );
                }
                let url = ValidatedUrl::parse(bounded_value()?.to_owned())
                    .map_err(|error| format!("userscript action link URL is invalid: {error}"))?;
                if !is_safe_history_url(url.as_str()) {
                    return Err("userscript action link URL is not eligible".into());
                }
                (Some(safe_ipc_url(url.as_str())), None)
            }
            "selection" => {
                if !action
                    .required_fields
                    .iter()
                    .any(|field| field == "selection")
                {
                    return Err(
                        "selection userscript action must require the selection field".into(),
                    );
                }
                (None, Some(bounded_value()?))
            }
            "url" => {
                if !action.required_fields.iter().any(|field| field == "url") {
                    return Err("URL userscript action must require the url field".into());
                }
                if !value.is_empty() {
                    return Err("URL userscript actions do not accept a subject override".into());
                }
                (None, None)
            }
            "tab" => {
                if !action.required_fields.iter().any(|field| field == "tab_id") {
                    return Err("tab userscript action must require the tab_id field".into());
                }
                (None, None)
            }
            "window" => {
                if !action
                    .required_fields
                    .iter()
                    .any(|field| field == "window_id")
                {
                    return Err("window userscript action must require the window_id field".into());
                }
                (None, None)
            }
            "context" => {
                if !action
                    .required_fields
                    .iter()
                    .any(|field| field == "context_name")
                {
                    return Err(
                        "context userscript action must require the context_name field".into(),
                    );
                }
                let target_context = {
                    let rust = self.as_ref().get_ref().rust();
                    rust.state
                        .as_ref()
                        .and_then(|state| {
                            state.tabs().get(&target.tab).map(|tab| (state, tab.window))
                        })
                        .and_then(|(state, window)| state.windows().get(&window))
                        .and_then(|window| window.context.as_deref())
                        .is_some()
                };
                if !target_context {
                    return Err("context userscript action requires an active context".into());
                }
                (None, None)
            }
            "download" => {
                if !action
                    .required_fields
                    .iter()
                    .any(|field| field == "download_id")
                {
                    return Err(
                        "download userscript action must require the download_id field".into(),
                    );
                }
                let id = bounded_value()?;
                let library = self
                    .as_ref()
                    .rust()
                    .storage_library
                    .clone()
                    .filter(|_| !self.as_ref().rust().storage_library_dirty.get())
                    .ok_or_else(|| {
                        "download metadata is unavailable; retry the action".to_owned()
                    })?;
                let download = library
                    .downloads
                    .iter()
                    .find(|download| download.id == id)
                    .ok_or_else(|| "download userscript action target is stale".to_owned())?;
                subject_fields.insert("download_id".into(), Value::String(id.to_owned()));
                let url = is_safe_history_url(&download.source_url)
                    .then(|| safe_ipc_url(&download.source_url));
                (url, None)
            }
            "history-entry" => {
                if !action
                    .required_fields
                    .iter()
                    .any(|field| field == "history_id")
                {
                    return Err(
                        "history-entry userscript action must require the history_id field".into(),
                    );
                }
                let id = bounded_value()?
                    .parse::<i64>()
                    .ok()
                    .filter(|id| *id >= 0)
                    .ok_or_else(|| {
                        "history-entry userscript action requires a valid ID".to_owned()
                    })?;
                let library = self
                    .as_ref()
                    .rust()
                    .storage_library
                    .clone()
                    .filter(|_| !self.as_ref().rust().storage_library_dirty.get())
                    .ok_or_else(|| {
                        "history metadata is unavailable; retry the action".to_owned()
                    })?;
                let history = library
                    .history
                    .iter()
                    .find(|record| record.id == id)
                    .ok_or_else(|| "history-entry userscript action target is stale".to_owned())?;
                if !is_safe_history_url(&history.url) {
                    return Err("history-entry userscript action URL is not eligible".into());
                }
                subject_fields.insert("history_id".into(), Value::String(id.to_string()));
                (Some(safe_ipc_url(&history.url)), None)
            }
            "bookmark" => {
                if !action
                    .required_fields
                    .iter()
                    .any(|field| field == "bookmark_id")
                {
                    return Err(
                        "bookmark userscript action must require the bookmark_id field".into(),
                    );
                }
                let id = bounded_value()?;
                let library = self
                    .as_ref()
                    .rust()
                    .storage_library
                    .clone()
                    .filter(|_| !self.as_ref().rust().storage_library_dirty.get())
                    .ok_or_else(|| {
                        "bookmark metadata is unavailable; retry the action".to_owned()
                    })?;
                let bookmark = library
                    .bookmarks
                    .iter()
                    .find(|bookmark| bookmark.id == id)
                    .ok_or_else(|| "bookmark userscript action target is stale".to_owned())?;
                if !is_safe_history_url(&bookmark.url) {
                    return Err("bookmark userscript action URL is not eligible".into());
                }
                subject_fields.insert("bookmark_id".into(), Value::String(id.to_owned()));
                (Some(safe_ipc_url(&bookmark.url)), None)
            }
            "quickmark" => {
                if !action
                    .required_fields
                    .iter()
                    .any(|field| field == "quickmark_name")
                {
                    return Err(
                        "quickmark userscript action must require the quickmark_name field".into(),
                    );
                }
                let name = bounded_value()?;
                let library = self
                    .as_ref()
                    .rust()
                    .storage_library
                    .clone()
                    .filter(|_| !self.as_ref().rust().storage_library_dirty.get())
                    .ok_or_else(|| {
                        "quickmark metadata is unavailable; retry the action".to_owned()
                    })?;
                let quickmark = library
                    .quickmarks
                    .iter()
                    .find(|quickmark| quickmark.name == name)
                    .ok_or_else(|| "quickmark userscript action target is stale".to_owned())?;
                if !is_safe_history_url(&quickmark.url) {
                    return Err("quickmark userscript action URL is not eligible".into());
                }
                subject_fields.insert("quickmark_name".into(), Value::String(name.to_owned()));
                (Some(safe_ipc_url(&quickmark.url)), None)
            }
            "session" => {
                if !action
                    .required_fields
                    .iter()
                    .any(|field| field == "session_name")
                {
                    return Err(
                        "session userscript action must require the session_name field".into(),
                    );
                }
                let name = bounded_value()?;
                self.as_mut().request_session_names();
                if self.as_ref().rust().session_names_dirty.get() {
                    return Err("named session catalog is still loading; retry the action".into());
                }
                if self.as_ref().rust().profile_id.is_none() {
                    return Err("named sessions are unavailable in this profile".to_owned());
                }
                if !self
                    .as_ref()
                    .rust()
                    .session_names
                    .iter()
                    .any(|candidate| candidate == name)
                {
                    return Err("session userscript action target is stale".into());
                }
                subject_fields.insert("session_name".into(), Value::String(name.to_owned()));
                (None, None)
            }
            "command" => {
                if !action
                    .required_fields
                    .iter()
                    .any(|field| field == "command_id")
                {
                    return Err(
                        "command userscript action must require the command_id field".into(),
                    );
                }
                let id = bounded_value()?;
                if CommandRegistry::default_v1().resolve(id).is_err() {
                    return Err("command userscript action target is unknown".into());
                }
                subject_fields.insert("command_id".into(), Value::String(id.to_owned()));
                (None, None)
            }
            _ => {
                return Err(
                    "userscript actions currently support URL, link, selection, tab, window, context, download, history-entry, bookmark, quickmark, session, and command subjects".into(),
                );
            }
        };
        let hint_url = if hint_activation
            && action.subject == "link"
            && manifest
                .context_fields
                .iter()
                .any(|field| field == "hint_url")
        {
            subject_url.clone()
        } else {
            None
        };
        self.execute_userscript_with_context(
            &action.script,
            target,
            hint_url,
            selection,
            None,
            subject_url,
            Some(subject_fields),
        )
    }
}
