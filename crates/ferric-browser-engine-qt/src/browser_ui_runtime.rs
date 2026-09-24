use super::{
    ActionRegistry, ActionSource, CommandSource, Config, CxxQtType, Duration, Event,
    HyprlandRequest, HyprlandWorker, Instant, ParsedCommand, Pin, QString, StorageRequest,
    SubmitError, ValidatedUrl, Value, configured_switcher_action_values, current_target,
    elapsed_ms, hyprland, is_safe_history_url, parse_external_action_id, parse_switcher_generation,
    qobject, switcher_action_allowed, switcher_default_action, thread, typed_ipc_action,
    validate_switcher_generation,
};

impl qobject::BrowserUi {
    pub(super) fn activate_switcher_result(
        mut self: Pin<&mut Self>,
        kind: &QString,
        id: &QString,
        generation: &QString,
    ) -> bool {
        let kind = kind.to_string();
        let id = id.to_string();
        let generation = match parse_switcher_generation(&generation.to_string()) {
            Ok(generation) => generation,
            Err(error) => {
                self.set_status_text(QString::from(error));
                return false;
            }
        };
        let Some(action) = switcher_default_action(&kind) else {
            self.set_status_text(QString::from("Switcher result has no default action"));
            return false;
        };
        self.as_mut()
            .activate_switcher_action_internal(&kind, &id, action, generation)
    }

    pub(super) fn activate_switcher_action(
        mut self: Pin<&mut Self>,
        kind: &QString,
        id: &QString,
        action: &QString,
        generation: &QString,
    ) -> bool {
        let kind = kind.to_string();
        let id = id.to_string();
        let action = action.to_string();
        let generation = match parse_switcher_generation(&generation.to_string()) {
            Ok(generation) => generation,
            Err(error) => {
                self.set_status_text(QString::from(error));
                return false;
            }
        };
        self.as_mut()
            .activate_switcher_action_internal(&kind, &id, &action, generation)
    }

    pub(super) fn hyprland_route_workspace(mut self: Pin<&mut Self>, workspace: &QString) -> bool {
        let workspace = workspace.to_string();
        if !hyprland::valid_workspace_selector(&workspace) {
            self.as_mut()
                .set_status_text(QString::from("Hyprland workspace selector is invalid"));
            return false;
        }
        let config = serde_json::from_str::<Config>(&self.as_ref().rust().config_json.to_string())
            .unwrap_or_default()
            .hyprland;
        let adapter = hyprland::HyprlandAdapter::from_config(&config);
        let status = adapter.status();
        self.as_mut()
            .set_hyprland_status(QString::from(status.clone()));
        if !adapter.workspace_routing_enabled() || status != "ready" {
            self.as_mut().set_status_text(QString::from(format!(
                "Hyprland routing unavailable: {status}"
            )));
            return false;
        }
        let request = HyprlandRequest::RouteWorkspace { config, workspace };
        let result = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.hyprland_worker.is_none() {
                this.hyprland_worker = HyprlandWorker::spawn().ok();
            }
            this.hyprland_worker
                .as_mut()
                .map_or(Err(SubmitError::Stopped), |worker| worker.request(request))
        };
        match result {
            Ok(()) => {
                self.as_mut()
                    .set_status_text(QString::from("Hyprland workspace routing request queued"));
                true
            }
            Err(error) => {
                self.as_mut().set_status_text(QString::from(format!(
                    "Hyprland routing request unavailable: {error:?}"
                )));
                false
            }
        }
    }

    pub(super) fn hyprland_move_active_window(
        mut self: Pin<&mut Self>,
        workspace: &QString,
    ) -> bool {
        let workspace = workspace.to_string();
        if !hyprland::valid_workspace_selector(&workspace) {
            self.as_mut()
                .set_status_text(QString::from("Hyprland workspace selector is invalid"));
            return false;
        }
        let config = serde_json::from_str::<Config>(&self.as_ref().rust().config_json.to_string())
            .unwrap_or_default()
            .hyprland;
        let adapter = hyprland::HyprlandAdapter::from_config(&config);
        let status = adapter.status();
        self.as_mut()
            .set_hyprland_status(QString::from(status.clone()));
        if !adapter.workspace_routing_enabled() || status != "ready" {
            self.as_mut().set_status_text(QString::from(format!(
                "Window movement unavailable: {status}"
            )));
            return false;
        }
        let request = HyprlandRequest::MoveActiveWindow { config, workspace };
        let result = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.hyprland_worker.is_none() {
                this.hyprland_worker = HyprlandWorker::spawn().ok();
            }
            this.hyprland_worker
                .as_mut()
                .map_or(Err(SubmitError::Stopped), |worker| worker.request(request))
        };
        match result {
            Ok(()) => {
                self.as_mut()
                    .set_status_text(QString::from("Window workspace-move request queued"));
                true
            }
            Err(error) => {
                self.as_mut().set_status_text(QString::from(format!(
                    "Window movement request unavailable: {error:?}"
                )));
                false
            }
        }
    }

    pub(super) fn hyprland_browser_clients(mut self: Pin<&mut Self>) -> QString {
        let config = serde_json::from_str::<Config>(&self.as_ref().rust().config_json.to_string())
            .unwrap_or_default();
        let adapter = hyprland::HyprlandAdapter::from_config(&config.hyprland);
        let status = adapter.status();
        self.as_mut()
            .set_hyprland_status(QString::from(status.clone()));
        if !adapter.workspace_routing_enabled() || status != "ready" {
            self.as_mut().set_status_text(QString::from(format!(
                "Hyprland client query unavailable: {status}"
            )));
            return QString::from(self.as_ref().hyprland_clients_json().to_string());
        }
        let result = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.hyprland_worker.is_none() {
                this.hyprland_worker = HyprlandWorker::spawn().ok();
            }
            this.hyprland_worker
                .as_mut()
                .map_or(Err(SubmitError::Stopped), |worker| {
                    worker.request(HyprlandRequest::BrowserClients {
                        config: config.hyprland,
                    })
                })
        };
        if let Err(error) = result {
            self.as_mut().set_status_text(QString::from(format!(
                "Hyprland client query unavailable: {error:?}"
            )));
        } else {
            self.as_mut()
                .set_status_text(QString::from("Hyprland browser-client query queued"));
        }
        QString::from(self.as_ref().hyprland_clients_json().to_string())
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn activate_switcher_action_internal(
        mut self: Pin<&mut Self>,
        kind: &str,
        id: &str,
        action: &str,
        generation: Option<u64>,
    ) -> bool {
        if !switcher_action_allowed(kind, action) {
            self.set_status_text(QString::from(
                "Switcher action is not allowed for this result",
            ));
            return false;
        }

        if kind == "closed" {
            let Some(closed) = self
                .as_ref()
                .rust()
                .closed_tabs
                .iter()
                .find(|closed| closed.id.to_string() == id)
                .cloned()
            else {
                self.set_status_text(QString::from("Closed tab descriptor is stale"));
                return false;
            };
            let Some(window) = self.as_ref().rust().window else {
                return false;
            };
            let Ok(url) = ValidatedUrl::parse(closed.url.clone()) else {
                self.set_status_text(QString::from("Closed tab URL is no longer safe to reopen"));
                return false;
            };
            if self
                .as_mut()
                .reduce_event(Event::OpenTab { window })
                .is_err()
            {
                return false;
            }
            let Some(tab) = self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .and_then(|state| state.windows().get(&window))
                .and_then(|window| window.active_tab)
            else {
                return false;
            };
            let Some(target) = current_target(self.as_ref().rust().state.as_ref(), Some(tab))
            else {
                return false;
            };
            let Ok(effects) = self
                .as_mut()
                .reduce_event(Event::StartNavigation { target, url })
            else {
                return false;
            };
            self.as_mut().set_pending_engine_action(&effects);
            {
                let mut rust = self.as_mut().rust_mut();
                rust.as_mut()
                    .get_mut()
                    .closed_tabs
                    .retain(|candidate| candidate.id != closed.id);
            }
            self.as_mut().sync_tab_order_from_core();
            self.set_status_text(QString::from("Closed tab reopened"));
            return true;
        }

        if kind == "action" {
            if let Some((target_name, subject)) = parse_external_action_id(id)
                && matches!(subject.as_str(), "url" | "tab")
            {
                if let Err(error) = self
                    .as_ref()
                    .validate_action_availability(id, CommandSource::Switcher)
                {
                    self.set_status_text(QString::from(format!(
                        "Switcher action unavailable: {error}"
                    )));
                    return false;
                }
                let available = configured_switcher_action_values(
                    &self.as_ref().rust().config,
                    self.as_ref().active_profile_is_transient(),
                )
                .is_ok_and(|values| {
                    values.into_iter().any(|value| {
                        value.get("id").and_then(Value::as_str) == Some(id)
                            && value.get("target").and_then(Value::as_str) == Some(&target_name)
                    })
                });
                if !available {
                    self.as_mut().set_status_text(QString::from(
                        "Configured switcher action is stale or unavailable",
                    ));
                    return false;
                }
                return self.as_mut().execute_ui_action_with_source(
                    &QString::from(id),
                    &QString::default(),
                    CommandSource::Switcher,
                );
            }
            let registry = ActionRegistry::default_v1();
            let Some(definition) = registry.resolve(id) else {
                self.set_status_text(QString::from("Switcher action is stale"));
                return false;
            };
            if !definition.sources.contains(&ActionSource::Switcher)
                || definition
                    .arguments
                    .iter()
                    .any(|argument| argument.required)
            {
                self.set_status_text(QString::from(
                    "Switcher action requires an explicit subject or argument",
                ));
                return false;
            }
            if let Err(error) = self
                .as_ref()
                .validate_action_availability(&definition.id, CommandSource::Switcher)
            {
                self.set_status_text(QString::from(format!(
                    "Switcher action unavailable: {error}"
                )));
                return false;
            }
            let params = serde_json::json!({
                "action": definition.id,
                "arguments": {}
            });
            let Ok((command, mut route, _)) = typed_ipc_action(&params) else {
                self.set_status_text(QString::from("Switcher action could not be typed"));
                return false;
            };
            route.source = CommandSource::Switcher;
            if let Err(error) = self.as_mut().execute_ipc_command(command, &route) {
                self.set_status_text(QString::from(format!("Switcher action failed: {error}")));
                return false;
            }
            self.as_mut().set_status_text(QString::from(format!(
                "Switcher action executed: {}",
                definition.label
            )));
            return true;
        }

        if matches!(kind, "tab" | "window") {
            let binding = self.as_ref();
            let Some(state) = binding.rust().state.as_ref() else {
                return false;
            };
            let target = match kind {
                "tab" => state
                    .tabs()
                    .values()
                    .find(|tab| tab.id.to_string() == id)
                    .map(|tab| (tab.window, Some(tab.id))),
                "window" => state
                    .windows()
                    .values()
                    .find(|window| window.id.to_string() == id)
                    .map(|window| (window.id, window.active_tab)),
                _ => None,
            };
            let Some((window, tab)) = target else {
                self.set_status_text(QString::from("Switcher target is stale or unsupported"));
                return false;
            };
            if let Some(expected_generation) = generation {
                let current_generation =
                    tab.and_then(|tab| state.tabs().get(&tab).map(|tab| tab.generation));
                if let Err(error) =
                    validate_switcher_generation(Some(expected_generation), current_generation)
                {
                    self.set_status_text(QString::from(error));
                    return false;
                }
            }
            if kind == "window" {
                return self.as_mut().execute_ui_action_with_source(
                    &QString::from("browser.window.focus"),
                    &QString::from(id),
                    CommandSource::Switcher,
                );
            }
            if self
                .as_mut()
                .reduce_event(Event::FocusWindow { window })
                .is_err()
            {
                return false;
            }
            if let Some(tab) = tab
                && self
                    .as_mut()
                    .reduce_event(Event::ActivateTab { window, tab })
                    .is_err()
            {
                return false;
            }
            self.as_mut().sync_tab_order_from_core();
            self.set_status_text(QString::from("Switcher target focused"));
            return true;
        }

        if matches!(kind, "history" | "bookmark" | "quickmark") {
            let action_id = match (kind, action) {
                ("history", "open") => Some("browser.history-entry.open"),
                ("bookmark", "open") => Some("browser.bookmark.open"),
                ("bookmark", "delete") => Some("browser.bookmark.delete"),
                ("quickmark", "open") => Some("browser.quickmark.open"),
                ("quickmark", "delete") => Some("browser.quickmark.delete"),
                _ => None,
            };
            if let Some(action_id) = action_id {
                return self.as_mut().execute_ui_action_with_source(
                    &QString::from(action_id),
                    &QString::from(id),
                    CommandSource::Switcher,
                );
            }
            let stored_url = (|| -> Result<String, String> {
                let binding = self.as_ref();
                let rust = binding.rust();
                if rust.storage_library_dirty.get() {
                    return Err("switcher entries are refreshing; retry the selection".to_owned());
                }
                let library = rust.storage_library.as_ref().ok_or_else(|| {
                    "stored switcher entries are still loading; retry the selection".to_owned()
                })?;
                match kind {
                    "history" => library
                        .history
                        .iter()
                        .find(|record| record.id.to_string() == id)
                        .map(|record| record.url.clone())
                        .ok_or_else(|| "history entry is stale".to_owned()),
                    "bookmark" => library
                        .bookmarks
                        .iter()
                        .find(|record| record.id == id)
                        .map(|record| record.url.clone())
                        .ok_or_else(|| "bookmark is stale".to_owned()),
                    "quickmark" => library
                        .quickmarks
                        .iter()
                        .find(|record| record.name == id)
                        .map(|record| record.url.clone())
                        .ok_or_else(|| "quickmark is stale".to_owned()),
                    _ => unreachable!("checked stored switcher kind"),
                }
            })();
            let Ok(stored_url) = stored_url else {
                self.set_status_text(QString::from(stored_url.unwrap_err()));
                return false;
            };
            if !is_safe_history_url(&stored_url) {
                self.set_status_text(QString::from(
                    "Stored entry is no longer eligible for safe navigation",
                ));
                return false;
            }
            let Ok(url) = ValidatedUrl::parse(stored_url) else {
                self.set_status_text(QString::from("Stored entry URL is invalid"));
                return false;
            };
            let Some(target) = current_target(
                self.as_ref().rust().state.as_ref(),
                self.as_ref().rust().tab,
            ) else {
                self.set_status_text(QString::from("Current tab is stale"));
                return false;
            };
            let Ok(effects) = self.as_mut().reduce_event(Event::StartNavigation {
                target,
                url: url.clone(),
            }) else {
                self.set_status_text(QString::from("Stored entry navigation was rejected"));
                return false;
            };
            self.as_mut().set_pending_engine_action(&effects);
            let resolved = QString::from(url.as_str());
            self.as_mut().set_initial_url(resolved.clone());
            self.as_mut().update_current_url(resolved);
            self.as_mut().set_load_state(QString::from("provisional"));
            self.set_status_text(QString::from("Stored entry navigation requested"));
            return true;
        }

        if kind == "context" {
            let Some(context) = self
                .as_ref()
                .rust()
                .contexts
                .as_ref()
                .and_then(|contexts| {
                    contexts
                        .contexts()
                        .iter()
                        .find(|context| context.id.to_string() == id)
                })
                .cloned()
            else {
                self.set_status_text(QString::from("Context is stale"));
                return false;
            };
            if context.profile != self.as_ref().rust().profile_name {
                self.set_status_text(QString::from(
                    "Context belongs to another profile; open it in a profile window",
                ));
                return false;
            }
            if self.as_mut().execute_ui_action_with_source(
                &QString::from("browser.context.enter"),
                &QString::from(context.name),
                CommandSource::Switcher,
            ) {
                self.as_mut().sync_core_tabs();
                self.set_status_text(QString::from("Context entered"));
                true
            } else {
                false
            }
        } else if kind == "session" {
            let Some(profile_id) = self.as_ref().rust().profile_id else {
                self.set_status_text(QString::from("Private profiles have no named sessions"));
                return false;
            };
            let prefix = format!("{profile_id}:");
            let Some(name) = id.strip_prefix(&prefix).filter(|name| !name.is_empty()) else {
                self.set_status_text(QString::from("Session belongs to another profile"));
                return false;
            };
            let name = QString::from(name);
            if !matches!(action, "load-preview" | "load") {
                return false;
            }
            self.as_mut().execute_ui_action_with_source(
                &QString::from("browser.session.load"),
                &name,
                CommandSource::Switcher,
            )
        } else if kind == "download" {
            if action == "show" {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some("show-downloads".into());
                self.set_status_text(QString::from("Downloads manager requested"));
                true
            } else {
                let id = QString::from(id);
                let uri = match self.as_mut().resolve_download_desktop_uri(&id, false) {
                    Ok(uri) => uri,
                    Err(error) => {
                        self.set_status_text(QString::from(error));
                        return false;
                    }
                };
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some(format!("download-open\t{uri}"));
                self.set_status_text(QString::from("Download open requested"));
                true
            }
        } else if kind == "command" {
            if matches!(action, "help" | "execute") {
                let action_id = if action == "help" {
                    "browser.command.help"
                } else {
                    "browser.command.execute"
                };
                return self.as_mut().execute_ui_action_with_source(
                    &QString::from(action_id),
                    &QString::from(id),
                    CommandSource::Switcher,
                );
            }
            let Some(definition) = self
                .as_ref()
                .rust()
                .registry
                .definitions()
                .iter()
                .find(|definition| definition.action.to_string() == id)
                .cloned()
            else {
                self.set_status_text(QString::from("Command is stale"));
                return false;
            };
            if action == "help" {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action =
                    Some(format!("show-binding-help\t{}", definition.name));
                self.set_status_text(QString::from("Command help requested"));
                true
            } else {
                self.as_mut().execute_parsed_commands(vec![ParsedCommand {
                    name: definition.name,
                    arguments: Vec::new(),
                }])
            }
        } else {
            self.set_status_text(QString::from("Switcher target is stale or unsupported"));
            false
        }
    }

    pub(super) fn set_scroll_position(
        mut self: Pin<&mut Self>,
        index: i32,
        x: f64,
        y: f64,
    ) -> bool {
        if index < 0 || !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 {
            return false;
        }
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let Some(tab) = this.tab_ids.get(index as usize).copied() else {
            return false;
        };
        if this.scroll_positions.get(&tab).copied() == Some((x, y)) {
            return true;
        }
        this.scroll_positions.insert(tab, (x, y));
        this.checkpoint.dirty = true;
        if this.checkpoint.dirty_since_ms.is_none() {
            this.checkpoint.dirty_since_ms = Some(elapsed_ms(this.binding_clock));
        }
        true
    }

    pub(super) fn checkpoint_session(mut self: Pin<&mut Self>) -> bool {
        let now = elapsed_ms(self.as_ref().rust().binding_clock);
        let due = {
            let pinned = self.as_ref();
            let rust = pinned.rust();
            !rust.session_save_pending
                && rust.checkpoint.restore_pending.is_none()
                && rust.checkpoint.dirty
                && (rust
                    .checkpoint
                    .dirty_since_ms
                    .is_some_and(|since| now.saturating_sub(since) >= 500)
                    || now.saturating_sub(rust.checkpoint.last_saved_ms) >= 5_000)
        };
        if !due {
            return false;
        }
        let Some(path) = self.as_ref().rust().session_path.clone() else {
            return false;
        };
        self.as_mut()
            .request_session_snapshot_save("last-session", path, true)
    }

    pub(super) fn wait_for_session_save(mut self: Pin<&mut Self>) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            self.as_mut().poll_storage_library();
            if !self.as_ref().rust().session_save_pending {
                return !self.as_ref().rust().checkpoint.dirty;
            }
            if Instant::now() >= deadline {
                self.set_status_text(QString::from(
                    "Session snapshot did not finish before shutdown; changes retained",
                ));
                return false;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub(super) fn wait_for_storage_flush(mut self: Pin<&mut Self>) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            self.as_mut().poll_storage_library();
            let (pending, error) = {
                let this = self.as_ref();
                let rust = this.rust();
                (rust.storage_flush_pending, rust.storage_flush_error.clone())
            };
            if let Some(error) = error {
                self.set_status_text(QString::from(format!(
                    "Durable metadata flush failed: {error}"
                )));
                return false;
            }
            if !pending {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                self.set_status_text(QString::from(
                    "Durable metadata checkpoint did not finish before shutdown",
                ));
                return false;
            }
            if !self.as_ref().storage_is_open() {
                self.set_status_text(QString::from(
                    "Durable metadata flush unavailable; worker stopped",
                ));
                return false;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub(super) fn flush_durable_state(mut self: Pin<&mut Self>) -> bool {
        if self.as_ref().active_profile_is_transient() {
            return true;
        }
        let Some(path) = self.as_ref().rust().session_path.clone() else {
            return false;
        };
        if !self
            .as_mut()
            .request_session_snapshot_save("last-session", path, true)
            || !self.as_mut().wait_for_session_save()
        {
            self.set_status_text(QString::from("Durable session snapshot failed"));
            return false;
        }
        if !self.as_mut().wait_for_history_writes() {
            return false;
        }
        if !self.as_mut().wait_for_permission_writes() {
            return false;
        }
        if !self.as_mut().wait_for_download_writes() {
            return false;
        }
        if !self.as_mut().wait_for_mark_writes() {
            return false;
        }
        if !self.as_mut().wait_for_history_clear() {
            return false;
        }
        if !self.as_mut().wait_for_download_destination() {
            return false;
        }
        if !self.as_mut().wait_for_download_create() {
            return false;
        }
        if !self.as_mut().wait_for_journey_write() {
            return false;
        }
        let request_result = self.as_mut().submit_storage(StorageRequest::Flush);
        match request_result {
            Ok(()) => {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.storage_flush_pending = true;
                this.storage_flush_error = None;
                drop(rust);
                if !self.as_mut().wait_for_storage_flush() {
                    return false;
                }
            }
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Durable metadata flush unavailable: {error}"
                )));
                return false;
            }
        }
        if self
            .as_ref()
            .rust()
            .profile_persistence
            .lacks_durable_storage()
        {
            self.set_status_text(QString::from("Durable profile store is unavailable"));
            return false;
        }
        let now = elapsed_ms(self.as_ref().rust().binding_clock);
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.checkpoint.dirty = false;
        this.checkpoint.dirty_since_ms = None;
        this.checkpoint.restore_pending = None;
        this.checkpoint.last_saved_ms = now;
        self.set_status_text(QString::from("Durable state flushed"));
        true
    }

    pub(super) fn clear_current_session_checkpoints(mut self: Pin<&mut Self>) -> bool {
        let (Some(root), Some(profile_id)) = ({
            let pinned = self.as_ref();
            (
                pinned.rust().session_state_root.clone(),
                pinned.rust().profile_id,
            )
        }) else {
            return true;
        };
        let result = self
            .as_ref()
            .storage_is_open()
            .then_some((root, profile_id));
        let Some((root, profile_id)) = result else {
            self.set_status_text(QString::from(
                "Stale session checkpoint cleanup unavailable; recovery data was retained",
            ));
            return false;
        };
        match Some(
            self.as_mut()
                .submit_storage(StorageRequest::SessionCheckpointClear { root, profile_id }),
        ) {
            Some(Ok(())) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .session_checkpoint_clear_pending = true;
                true
            }
            Some(Err(error)) => {
                self.set_status_text(QString::from(format!(
                    "Stale session checkpoint cleanup unavailable; recovery data was retained: {error}"
                )));
                false
            }
            None => {
                self.set_status_text(QString::from(
                    "Stale session checkpoint cleanup unavailable; recovery data was retained",
                ));
                false
            }
        }
    }
}
