use super::{
    Config, ContextsConfig, CxxQtType, MAX_UNTRUSTED_ARGUMENT_BYTES, ParsedCommand, Path, PathBuf,
    Pin, QString, RuntimeOverrides, Uuid, Value, is_bounded_untrusted_text, qobject, resolve_input,
    save_contexts_atomic,
};

impl qobject::BrowserUi {
    pub(super) fn handle_session_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> bool {
        match command.name.as_str() {
            "session-save" => {
                if command.arguments.len() != 1 {
                    self.set_status_text(QString::from("session-save requires one name"));
                    return false;
                }
                self.as_mut()
                    .save_named_session(&QString::from(command.arguments[0].as_str()))
            }
            "session-list" => {
                if !command.arguments.is_empty() {
                    self.set_status_text(QString::from("session-list takes no arguments"));
                    return false;
                }
                let names = self.as_mut().list_named_sessions().to_string();
                let status = if names.is_empty() {
                    "No named sessions".to_owned()
                } else {
                    format!("Sessions: {names}")
                };
                self.as_mut().set_status_text(QString::from(status));
                true
            }
            "session-load" => match self.as_mut().execute_session_load_command(command) {
                Ok(_) => true,
                Err(error) => {
                    self.set_status_text(QString::from(error));
                    false
                }
            },
            "session-delete" => {
                if command.arguments.len() != 1 {
                    self.set_status_text(QString::from("session-delete requires one name"));
                    return false;
                }
                self.set_status_text(QString::from(
                    "Session deletion requires confirmation; use the session manager",
                ));
                false
            }
            _ => false,
        }
    }

    pub(super) fn execute_session_load_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        if self.as_ref().active_profile_is_transient() {
            return Err("private and ephemeral profiles cannot load durable sessions".into());
        }
        let (append, name) = match command.arguments.as_slice() {
            [name] if is_bounded_untrusted_text(name) => (false, name.as_str()),
            [flag, name] if flag == "--append" && is_bounded_untrusted_text(name) => {
                (true, name.as_str())
            }
            _ => return Err("session-load expects [--append] NAME".into()),
        };
        let name = QString::from(name);
        if !self.as_mut().request_session_preview(&name) {
            return Err("named session preview is unavailable or busy".into());
        }
        self.as_mut().set_session_preview(QString::from(format!(
            "{}\n{}",
            if append { "append" } else { "replace" },
            name
        )));
        self.as_mut()
            .set_status_text(QString::from("Session preview requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "pending": "session-preview",
            "requires_confirmation": true,
            "session": name.to_string(),
            "append": append
        }))
    }

    pub(super) fn execute_window_new_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let mut requested_profile = None;
        let mut private = false;
        let mut index = 0;
        while index < command.arguments.len() {
            match command.arguments[index].as_str() {
                "--profile" if requested_profile.is_none() => {
                    let name = command
                        .arguments
                        .get(index + 1)
                        .filter(|name| is_bounded_untrusted_text(name) && !name.starts_with("--"))
                        .ok_or_else(|| "window-new --profile requires a name".to_owned())?;
                    requested_profile = Some(name.clone());
                    index += 1;
                }
                "--private" if !private => private = true,
                option => return Err(format!("unknown window-new option: {option}")),
            }
            index += 1;
        }

        let profile_name = if let Some(name) = requested_profile {
            if self.as_ref().rust().storage_roots.is_none() {
                return Err("private profiles cannot select a durable profile".into());
            }
            let queued = self.as_mut().request_profile_list_with_mode(false);
            if !queued {
                return Err("profile list is unavailable or busy".into());
            }
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_window_new_profile = Some((name.clone(), private));
            self.as_mut()
                .set_status_text(QString::from("Profile lookup requested"));
            return Ok(serde_json::json!({
                "status": "accepted",
                "pending": "profile-lookup",
                "profile": name,
                "private": private
            }));
        } else if self.as_ref().rust().storage_roots.is_some() {
            self.as_ref().rust().profile_name.clone()
        } else if private {
            "private".into()
        } else {
            return Err("normal window creation requires a named profile".into());
        };

        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!(
            "new-window\t{private}\t{profile_name}\tabout:blank"
        ));
        self.as_mut().set_status_text(QString::from(if private {
            "Private window requested"
        } else {
            "New window requested"
        }));
        Ok(serde_json::json!({
            "status": "accepted",
            "pending": true,
            "profile": profile_name,
            "private": private
        }))
    }

    pub(super) fn handle_profile_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> bool {
        match command.name.as_str() {
            "profile-list" => {
                if !command.arguments.is_empty() {
                    self.set_status_text(QString::from("profile-list takes no arguments"));
                    return false;
                }
                let queued = self.as_mut().request_profile_list_with_mode(true);
                if queued {
                    self.as_mut()
                        .set_status_text(QString::from("Profile list requested"));
                }
                queued
            }
            "profile-open" => {
                if !(1..=2).contains(&command.arguments.len()) {
                    self.set_status_text(QString::from(
                        "profile-open requires NAME and optional URL or search input",
                    ));
                    return false;
                }
                let name = command.arguments[0].as_str();
                if self.as_ref().rust().profile_registry_roots.is_none()
                    && self.as_ref().rust().storage_roots.is_none()
                {
                    self.set_status_text(QString::from(
                        "Private profiles cannot open durable profile windows",
                    ));
                    return false;
                }
                if self.as_ref().active_profile_is_transient() && command.arguments.len() != 2 {
                    self.set_status_text(QString::from(
                        "Private profile reopen requires an explicit safe URL",
                    ));
                    return false;
                }
                let requested = command
                    .arguments
                    .get(1)
                    .cloned()
                    .unwrap_or_else(|| self.as_ref().rust().initial_url.to_string());
                let url = match resolve_input(&requested, &self.as_ref().navigation_context()) {
                    Ok(result) => result.url,
                    Err(error) => {
                        self.set_status_text(QString::from(format!(
                            "Profile window navigation rejected: {error}"
                        )));
                        return false;
                    }
                };
                let name = name.to_owned();
                let queued = self.as_mut().request_profile_list_with_mode(false);
                if queued {
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .pending_profile_open = Some((name, url.to_string()));
                    self.as_mut()
                        .set_status_text(QString::from("Profile lookup requested"));
                }
                queued
            }
            "profile-create" => {
                let ephemeral = match command.arguments.as_slice() {
                    [_] => false,
                    [_, flag] if flag == "--ephemeral" => true,
                    _ => {
                        self.set_status_text(QString::from(
                            "profile-create requires NAME with optional --ephemeral",
                        ));
                        return false;
                    }
                };
                if ephemeral {
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .pending_engine_action = Some("ephemeral-window\tabout:blank".into());
                    self.set_status_text(QString::from(
                        "Created a fresh ephemeral profile window; it is not durable",
                    ));
                    return true;
                }
                if command.arguments.len() != 1 {
                    self.set_status_text(QString::from("profile-create requires one name"));
                    return false;
                }
                let name = QString::from(command.arguments[0].as_str());
                self.as_mut().create_profile(&name, &name)
            }
            "profile-delete" => {
                if command.arguments.len() != 1 {
                    self.set_status_text(QString::from("profile-delete requires one name"));
                    return false;
                }
                let name = QString::from(command.arguments[0].as_str());
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some(format!("profile-delete\t{name}"));
                self.set_status_text(QString::from(
                    "Profile deletion preview requested; confirmation is required",
                ));
                true
            }
            _ => false,
        }
    }

    pub(super) fn context_command_arguments(
        &self,
        command: &ParsedCommand,
    ) -> Result<(String, String, String, Option<String>, bool), String> {
        match command.name.as_str() {
            "context-list" => {
                if !command.arguments.is_empty() {
                    return Err("context-list takes no arguments".into());
                }
                Ok((String::new(), String::new(), String::new(), None, false))
            }
            "context-create" => {
                let Some(name) = command.arguments.first().cloned() else {
                    return Err("context-create requires NAME".into());
                };
                let mut label = name.clone();
                let mut profile = self.rust().profile_name.clone();
                let mut workspace = None;
                let mut profile_set = false;
                let mut index = 1;
                while index < command.arguments.len() {
                    let option = command.arguments[index].as_str();
                    let value = command
                        .arguments
                        .get(index + 1)
                        .filter(|value| !value.is_empty() && !value.chars().any(char::is_control))
                        .cloned()
                        .ok_or_else(|| format!("{option} requires a value"))?;
                    match option {
                        "--label" => label = value,
                        "--profile" => {
                            profile = value;
                            profile_set = true;
                        }
                        "--workspace" => workspace = Some(value),
                        _ => return Err(format!("unknown context-create option: {option}")),
                    }
                    index += 2;
                }
                if !profile_set {
                    return Err("context-create requires --profile NAME".into());
                }
                Ok((name, label, profile, workspace, false))
            }
            "context-delete" => {
                let Some(name) = command.arguments.first().cloned() else {
                    return Err("context-delete requires NAME".into());
                };
                if command.arguments.len() > 2
                    || (command.arguments.len() == 2 && command.arguments[1] != "--confirm")
                {
                    return Err("context-delete expects NAME [--confirm]".into());
                }
                Ok((
                    name,
                    String::new(),
                    String::new(),
                    None,
                    command.arguments.len() == 2,
                ))
            }
            "context-enter" => {
                let Some(name) = command.arguments.first().cloned() else {
                    return Err("context-enter requires NAME".into());
                };
                if command.arguments.len() != 1 {
                    return Err("context-enter expects NAME".into());
                }
                Ok((name, String::new(), String::new(), None, false))
            }
            "context-save" => {
                if command.arguments.len() > 1 {
                    return Err("context-save expects an optional NAME".into());
                }
                Ok((
                    command.arguments.first().cloned().unwrap_or_default(),
                    String::new(),
                    String::new(),
                    None,
                    false,
                ))
            }
            _ => Err("not a context command".into()),
        }
    }

    pub(super) fn context_routes_path(&self) -> Result<PathBuf, String> {
        if self.rust().storage_roots.is_none() {
            return Err("transient profiles cannot manage durable context routes".into());
        }
        let config_path = self.rust().config_path.to_string();
        if config_path.is_empty() {
            return Err(
                "context routes require a user configuration path; safe mode has no durable config"
                    .into(),
            );
        }
        Ok(PathBuf::from(config_path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("contexts.toml"))
    }

    pub(super) fn current_contexts_config(&self) -> Result<ContextsConfig, String> {
        serde_json::from_str::<ContextsConfig>(&self.rust().contexts_json.to_string())
            .map_err(|error| format!("contexts configuration is unavailable: {error}"))
    }

    pub(super) fn set_contexts_configuration(mut self: Pin<&mut Self>, input: &QString) -> bool {
        match serde_json::from_str::<ContextsConfig>(&input.to_string()) {
            Ok(config) => match self.as_mut().publish_contexts_config(&config) {
                Ok(()) => true,
                Err(error) => {
                    self.set_status_text(QString::from(error));
                    false
                }
            },
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "contexts configuration is invalid: {error}"
                )));
                false
            }
        }
    }

    /// Records an explicit bootstrap decision before context-enter is dispatched.
    ///
    /// QML owns window construction, but cannot write application policy state
    /// directly. Keeping this at the bridge boundary makes the one-shot reuse
    /// decision visible and keeps context entry under the normal command path.
    pub(super) fn set_context_entry_reuse(mut self: Pin<&mut Self>, force_reuse: bool) {
        self.as_mut().set_context_entry_force_reuse(force_reuse);
    }

    pub(super) fn set_startup_configuration(
        mut self: Pin<&mut Self>,
        config: &QString,
        base_config: &QString,
        cli_overrides: &QString,
        profile_overrides: &QString,
        path: &QString,
        source: &QString,
    ) -> bool {
        let config_text = config.to_string();
        let base_config_text = base_config.to_string();
        let cli_overrides_text = cli_overrides.to_string();
        let profile_overrides_text = profile_overrides.to_string();
        let path = path.to_string();
        let source = source.to_string();
        let valid_metadata = |value: &str| {
            value.len() <= MAX_UNTRUSTED_ARGUMENT_BYTES && !value.chars().any(char::is_control)
        };
        let valid = serde_json::from_str::<Config>(&config_text)
            .and_then(|_| serde_json::from_str::<Config>(&base_config_text))
            .and_then(|_| serde_json::from_str::<RuntimeOverrides>(&cli_overrides_text))
            .and_then(|_| serde_json::from_str::<RuntimeOverrides>(&profile_overrides_text))
            .is_ok()
            && valid_metadata(&path)
            && valid_metadata(&source);
        if !valid {
            self.set_status_text(QString::from("Startup configuration is invalid"));
            return false;
        }
        self.as_mut().set_config_json(QString::from(config_text));
        self.as_mut()
            .set_config_base_json(QString::from(base_config_text));
        self.as_mut()
            .set_cli_overrides_json(QString::from(cli_overrides_text));
        self.as_mut()
            .set_profile_overrides_json(QString::from(profile_overrides_text));
        self.as_mut().set_config_path(QString::from(path));
        self.as_mut().set_config_source(QString::from(source));
        true
    }

    pub(super) fn publish_contexts_config(
        mut self: Pin<&mut Self>,
        config: &ContextsConfig,
    ) -> Result<(), String> {
        let json = serde_json::to_string(config)
            .map_err(|error| format!("could not serialize contexts configuration: {error}"))?;
        self.as_mut().set_contexts_json(QString::from(json));
        self.as_mut().set_context_choice_names(
            config
                .contexts
                .iter()
                .map(|context| QString::from(&context.name))
                .collect(),
        );
        self.as_mut().set_context_choice_labels(
            config
                .contexts
                .iter()
                .map(|context| QString::from(&context.label))
                .collect(),
        );
        self.as_mut().set_context_choice_profiles(
            config
                .contexts
                .iter()
                .map(|context| QString::from(&context.profile))
                .collect(),
        );
        Ok(())
    }

    pub(super) fn execute_context_route_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let action = command
            .arguments
            .first()
            .map(String::as_str)
            .ok_or_else(|| "context-route requires ADD, REMOVE, or LIST".to_owned())?;
        let mut config = self.as_ref().current_contexts_config()?;
        match action {
            "list" => {
                if command.arguments.len() != 1 {
                    return Err("context-route list takes no arguments".into());
                }
                let routes = config
                    .routes
                    .iter()
                    .map(|route| {
                        serde_json::json!({
                            "id": route.id,
                            "pattern": route.pattern,
                            "context": route.context,
                            "priority": route.priority,
                            "behavior": route.behavior,
                            "entry_points": route.entry_points,
                        })
                    })
                    .collect::<Vec<_>>();
                let lines = config
                    .routes
                    .iter()
                    .map(|route| {
                        format!(
                            "{}	{}	{}	priority={}	{}	{}",
                            route.id,
                            route.pattern,
                            route.context,
                            route.priority,
                            route.behavior,
                            route.entry_points.join(",")
                        )
                    })
                    .collect::<Vec<_>>();
                self.as_mut()
                    .set_library_kind(QString::from("context-routes"));
                self.as_mut()
                    .set_library_values(QString::from(lines.join("\n")));
                self.set_status_text(QString::from(format!(
                    "{} context route(s); route changes are pre-navigation only",
                    routes.len()
                )));
                Ok(serde_json::json!({
                    "status": "listed",
                    "routes": routes,
                    "authentication_guard": "pre-navigation-only",
                }))
            }
            "add" => {
                if command.arguments.len() < 3 {
                    return Err("context-route add requires PATTERN and CONTEXT".into());
                }
                let pattern = &command.arguments[1];
                let context_name = &command.arguments[2];
                let mut priority = 0;
                let mut behavior = "prompt".to_owned();
                let mut entry_points = None;
                let mut seen_priority = false;
                let mut seen_behavior = false;
                let mut index = 3;
                while index < command.arguments.len() {
                    let option = command.arguments[index].as_str();
                    let value = command
                        .arguments
                        .get(index + 1)
                        .ok_or_else(|| format!("{option} requires a value"))?;
                    match option {
                        "--priority" => {
                            if seen_priority {
                                return Err(
                                    "context-route --priority was specified more than once".into(),
                                );
                            }
                            priority = value.parse::<i32>().map_err(|_| {
                                "context-route priority must be a 32-bit integer".to_owned()
                            })?;
                            seen_priority = true;
                        }
                        "--behavior" => {
                            if seen_behavior {
                                return Err(
                                    "context-route --behavior was specified more than once".into(),
                                );
                            }
                            if !matches!(value.as_str(), "prompt" | "suggest") {
                                return Err(
                                    "context-route behavior must be prompt or suggest".into()
                                );
                            }
                            behavior = value.clone();
                            seen_behavior = true;
                        }
                        "--entry-point" => {
                            let points = entry_points.get_or_insert_with(Vec::new);
                            if !matches!(
                                value.as_str(),
                                "external-open" | "explicit-open" | "typed-initial-url"
                            ) {
                                return Err(format!(
                                    "unsupported context route entry point: {value}"
                                ));
                            }
                            if points.iter().any(|point: &String| point == value) {
                                return Err(format!(
                                    "context route entry point specified more than once: {value}"
                                ));
                            }
                            if points.len() >= 3 {
                                return Err(
                                    "context-route accepts at most three entry points".into()
                                );
                            }
                            points.push(value.clone());
                        }
                        _ => return Err(format!("unknown context-route option: {option}")),
                    }
                    index += 2;
                }
                let context_exists =
                    self.as_ref()
                        .rust()
                        .contexts
                        .as_ref()
                        .is_some_and(|contexts| {
                            contexts
                                .contexts()
                                .iter()
                                .any(|context| context.name == *context_name)
                        });
                if !context_exists {
                    return Err(format!("context not found: {context_name}"));
                }
                if config
                    .routes
                    .iter()
                    .any(|route| route.pattern == *pattern && route.context == *context_name)
                {
                    return Err("an identical context route already exists".into());
                }
                if let Some(conflict) = config.routes.iter().find(|route| {
                    route.pattern == *pattern
                        && route.context != *context_name
                        && route.priority == priority
                        && if let Some(requested) = entry_points.as_ref() {
                            route
                                .entry_points
                                .iter()
                                .any(|entry| requested.iter().any(|candidate| candidate == entry))
                        } else {
                            route.entry_points.iter().any(|entry| {
                                matches!(
                                    entry.as_str(),
                                    "external-open" | "explicit-open" | "typed-initial-url"
                                )
                            })
                        }
                }) {
                    return Err(format!(
                        "route conflicts with {} at priority {}; choose a unique priority before adding another target",
                        conflict.id, priority
                    ));
                }
                let id = loop {
                    let candidate = format!("route-{}", Uuid::new_v4().simple());
                    if !config.routes.iter().any(|route| route.id == candidate) {
                        break candidate;
                    }
                };
                let route = ferric_browser_config::ContextRoute {
                    id: id.clone(),
                    pattern: pattern.clone(),
                    context: context_name.clone(),
                    priority,
                    behavior,
                    entry_points: entry_points.unwrap_or_else(|| {
                        vec![
                            "external-open".into(),
                            "explicit-open".into(),
                            "typed-initial-url".into(),
                        ]
                    }),
                };
                let conflicts = config
                    .routes
                    .iter()
                    .filter(|existing| existing.pattern == *pattern)
                    .map(|existing| {
                        serde_json::json!({
                            "id": existing.id,
                            "context": existing.context,
                            "priority": existing.priority,
                            "behavior": existing.behavior,
                            "entry_points": existing.entry_points,
                        })
                    })
                    .collect::<Vec<_>>();
                config.routes.push(route.clone());
                let path = self.as_ref().context_routes_path()?;
                save_contexts_atomic(&path, &config).map_err(|error| error.to_string())?;
                self.as_mut().publish_contexts_config(&config)?;
                self.as_mut().set_status_text(QString::from(format!(
                    "Context route {id} added; prompt applies before navigation only"
                )));
                Ok(serde_json::json!({
                    "status": "queued",
                    "route": route,
                    "conflicts": conflicts,
                    "resolution": "priority-then-specificity-then-authored-order",
                    "authentication_guard": "pre-navigation-only",
                }))
            }
            "remove" => {
                if command.arguments.len() != 2 {
                    return Err("context-route remove requires ROUTE_ID".into());
                }
                let route_id = &command.arguments[1];
                let original_len = config.routes.len();
                let removed = config
                    .routes
                    .iter()
                    .find(|route| route.id == *route_id)
                    .cloned();
                if removed.is_none() {
                    return Err(format!("context route not found: {route_id}"));
                }
                config.routes.retain(|route| route.id != *route_id);
                debug_assert_eq!(config.routes.len() + 1, original_len);
                let path = self.as_ref().context_routes_path()?;
                save_contexts_atomic(&path, &config).map_err(|error| error.to_string())?;
                self.as_mut().publish_contexts_config(&config)?;
                self.as_mut().set_status_text(QString::from(format!(
                    "Context route {route_id} removed; existing navigation chains were not changed"
                )));
                Ok(serde_json::json!({
                    "status": "removed",
                    "route": removed,
                    "authentication_guard": "pre-navigation-only",
                }))
            }
            _ => Err("context-route action must be add, remove, or list".into()),
        }
    }
}
