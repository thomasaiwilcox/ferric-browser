//! Read-only IPC queries and diagnostic projections for `BrowserUi`.

use super::{
    ActionDefinition, ActionRegistry, ActionSource, BTreeSet, BindingResolver, CommandSource,
    Config, ContextSnapshot, CxxQtType, ExistenceState, Instant, IpcQueryError,
    MAX_ACTION_AUDIT_RECORDS, Ordering, ParseInput, ParsedCommand, Pin, PrivacyKind, QString,
    QStringList, SITE_DOCTOR_EXPERIMENT_TIMEOUT, SwitcherCandidate, Uuid, Value,
    action_error_summary, apply_qt_runtime_facts, bounded_navigation_failure_detail, compatibility,
    config_value_at_path, configured_action_target_supports_subject,
    configured_action_target_values, configured_action_targets, configured_switcher_action_values,
    crash_diagnostics, diagnostics, finish_pending_selection_operation, hyprland, ipc_mode_name,
    link_cleaning_policy, live_document_available, load_theme_palette, macro_state_value,
    maintenance, matching_site_rules, operation_is_terminal, operations_query_value, parse_chain,
    parse_external_action_id, parse_ipc_mode, qobject, qt_webengine_version, query_bool_param,
    query_empty_object_or_null, query_limit, query_object, query_offset, query_optional_string,
    runtime_override_value, safe_ipc_url, safe_site_host, safe_site_origin,
    security_deny_rule_count, select_switcher_page, setting_metadata, setting_supported_scopes,
    setting_supports_site_scope, site_doctor_remaining_seconds, switcher_action_allowed,
    switcher_context_boost, switcher_default_action, switcher_rank, theme_contrast_report,
    tokenize_switcher_query, userscript, userscript_action_values,
};

impl qobject::BrowserUi {
    pub(super) fn ipc_tabs_query(&self, params: &Value) -> Result<Value, String> {
        let object = query_object(
            params,
            "tabs.query",
            &["include_private", "active_only", "window"],
        )?;
        let include_private = query_bool_param(object, "tabs.query", "include_private", false)?;
        let active_only = query_bool_param(object, "tabs.query", "active_only", false)?;
        if include_private && !self.ipc_private_queries_enabled() {
            return Err("private tab queries are disabled by configuration".into());
        }
        let Some(state) = self.rust().state.as_ref() else {
            return Err("core state unavailable".into());
        };
        let contexts = self.rust().contexts.as_ref();
        let requested_window = match query_optional_string(object, "tabs.query", "window")? {
            None => None,
            Some(value) => match value {
                "active" => state.active_window(),
                "last-focused" => state.last_focused_window().or(state.active_window()),
                _ => return Err("window query accepts active or last-focused".into()),
            },
        };
        let tabs = state
            .tabs()
            .values()
            .filter(|tab| {
                include_private
                    || state
                        .profiles()
                        .get(&tab.profile)
                        .is_some_and(|profile| !profile.privacy.is_transient())
            })
            .filter(|tab| {
                requested_window.is_none_or(|window| tab.window == window)
            })
            .filter(|tab| {
                !active_only
                    || state
                        .windows()
                        .get(&tab.window)
                        .and_then(|window| window.active_tab)
                        == Some(tab.id)
            })
            .map(|tab| {
                let profile = state.profiles().get(&tab.profile);
                let context = state
                    .windows()
                    .get(&tab.window)
                    .and_then(|window| window.context.as_deref())
                    .and_then(|name| contexts?.contexts().iter().find(|context| context.name == name));
                serde_json::json!({
                    "id": tab.id.to_string(),
                    "window_id": tab.window.to_string(),
                    "profile_id": tab.profile.to_string(),
                    "profile_name": profile.map(|profile| profile.label.clone()),
                    "context_id": context.map(|context| context.id.to_string()),
                    "context_name": context.map(|context| context.name.clone()),
                    "workspace": context.and_then(|context| context.workspace.clone()),
                    "output": Value::Null,
                    "url": tab.url.as_deref().map_or_else(|| "about:blank".into(), safe_ipc_url),
                    "title": tab.title.replace(['\n', '\r'], " "),
                    "selected": state.windows().get(&tab.window).and_then(|window| window.active_tab) == Some(tab.id),
                    "pinned": tab.pinned,
                    "muted": tab.muted,
                    "loading": format!("{:?}", tab.loading).to_ascii_lowercase(),
                    "renderer": format!("{:?}", tab.renderer).to_ascii_lowercase(),
                    "document_revision": state.revision()
                })
            })
            .collect::<Vec<_>>();
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "tabs": tabs
        }))
    }

    pub(super) fn ipc_windows_query(&self, params: &Value) -> Result<Value, String> {
        let object = query_object(params, "windows.query", &["window"])?;
        let Some(state) = self.rust().state.as_ref() else {
            return Err("core state unavailable".into());
        };
        let contexts = self.rust().contexts.as_ref();
        let requested_window = match query_optional_string(object, "windows.query", "window")? {
            None => None,
            Some(value) => match value {
                "active" => state.active_window(),
                "last-focused" => state.last_focused_window().or(state.active_window()),
                _ => return Err("window query accepts active or last-focused".into()),
            },
        };
        let windows = state
            .windows()
            .values()
            .filter(|window| requested_window.is_none_or(|requested| requested == window.id))
            .map(|window| {
                let context = window.context.as_deref().and_then(|name| {
                    contexts?
                        .contexts()
                        .iter()
                        .find(|context| context.name == name)
                });
                serde_json::json!({
                    "id": window.id.to_string(),
                    "profile_id": window.profile.to_string(),
                    "context_id": context.map(|context| context.id.to_string()),
                    "context_name": context.map(|context| context.name.clone()),
                    "workspace": context.and_then(|context| context.workspace.clone()),
                    "tabs": window.tabs.iter().map(ToString::to_string).collect::<Vec<_>>(),
                    "active_tab": window.active_tab.map(|tab| tab.to_string()),
                    "mode": window.modes.last().map(|mode| format!("{mode:?}").to_ascii_lowercase())
                })
            })
            .collect::<Vec<_>>();
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "windows": windows,
            "live_registry": self.rust().live_window_registry.iter().map(|entry| serde_json::json!({
                "id": entry.id,
                "owner_token": entry.owner_token,
                "profile": entry.profile,
                "private": entry.private,
                "ephemeral": entry.ephemeral,
                "tab_count": entry.tab_count,
            })).collect::<Vec<_>>(),
        }))
    }

    pub(super) fn ipc_window_focus(
        mut self: Pin<&mut Self>,
        params: &Value,
    ) -> Result<Value, String> {
        query_empty_object_or_null(params, "window.focus")?;
        let window_id = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(|state| state.last_focused_window().or(state.active_window()))
            .ok_or_else(|| "no live browser window is available to focus".to_owned())?;
        let command = ParsedCommand {
            name: "window-focus".into(),
            arguments: vec![window_id.to_string()],
        };
        let mut result = self.as_mut().execute_window_action(&command)?;
        let operation_id = format!("op-{}", Uuid::new_v4());
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .operation_states
            .insert(operation_id.clone(), "running".into());
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("window-focus\t{window_id}\t{operation_id}"));
        result["operation_id"] = Value::String(operation_id);
        result["status"] = Value::String("accepted".into());
        Ok(serde_json::json!({
            "status": result.get("status").cloned().unwrap_or_else(|| Value::String("accepted".into())),
            "action": "focus",
            "window_id": window_id.to_string(),
            "target": "last-focused",
            "pending": true,
            "operation_id": result["operation_id"].clone()
        }))
    }

    pub(super) fn ipc_profiles_query(&self, params: &Value) -> Result<Value, String> {
        query_empty_object_or_null(params, "profiles.query")?;
        let Some(state) = self.rust().state.as_ref() else {
            return Err("core state unavailable".into());
        };
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "profiles": state.profiles().values().map(|profile| serde_json::json!({
                "id": profile.id.to_string(),
                "name": profile.label,
                "private": profile.privacy == PrivacyKind::Private,
                "ephemeral": profile.privacy.is_ephemeral(),
                "privacy": profile.privacy.name()
            })).collect::<Vec<_>>()
        }))
    }

    #[allow(clippy::unused_self)]
    pub(super) fn ipc_actions_query(&self, params: &Value) -> Result<Value, String> {
        query_empty_object_or_null(params, "actions.query")?;
        let actions = ActionRegistry::default_v1();
        let targets = configured_action_targets(&self.rust().config)?;
        let mut values = actions.definitions().iter().map(|definition| serde_json::json!({
                "id": definition.id,
                "subject": definition.subject.as_str(),
                "verb": definition.verb,
                "label": definition.label,
                "description": definition.description,
                "effect": format!("{:?}", definition.effect).to_ascii_lowercase(),
                "sensitive": definition.sensitive,
                "command": definition.command,
                "sources": definition.sources.iter().map(|source| source.as_str()).collect::<Vec<_>>(),
                "confirmation": definition.confirmation.as_str(),
                "required_capabilities": definition.required_capabilities(),
                "completion_provider": definition.completion_provider(),
                "availability_predicate": definition.availability_predicate(),
                "availability": self.action_availability(definition),
                "examples": definition.examples,
                "arguments": definition.arguments.iter().map(|argument| serde_json::json!({
                    "name": argument.name,
                    "kind": format!("{:?}", argument.kind).to_ascii_lowercase(),
                    "required": argument.required
                })).collect::<Vec<_>>()
            })).collect::<Vec<_>>();
        values.extend(targets.iter().flat_map(|(name, target)| {
            configured_action_target_values(name, target, self.active_profile_is_transient())
        }));
        values.extend(userscript_action_values(
            self.rust()
                .userscript_roots
                .as_ref()
                .map(|roots| roots.config.as_path()),
            None,
            self.rust().state.is_some(),
            self.rust().profile_persistence.is_durable(),
            self.active_profile_is_transient(),
        )?);
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "actions": values
        }))
    }

    pub(super) fn action_availability(&self, definition: &ActionDefinition) -> Value {
        let (available, reason) = match definition.required_capabilities() {
            capabilities if capabilities.contains(&"configured-action-target") => {
                let configured = configured_action_target_supports_subject(
                    &self.rust().config,
                    definition.subject,
                )
                .unwrap_or(false);
                (
                    configured,
                    if configured {
                        "ready"
                    } else {
                        "no-configured-target-for-subject"
                    },
                )
            }
            capabilities
                if capabilities.contains(&"durable-profile-storage")
                    || capabilities.contains(&"durable-download-index") =>
            {
                let available = self.rust().profile_persistence.is_durable();
                (
                    available,
                    if available {
                        "ready"
                    } else {
                        "durable-storage-unavailable"
                    },
                )
            }
            capabilities if capabilities.contains(&"durable-context-registry") => {
                let available =
                    self.rust().profile_persistence.is_durable() && self.rust().contexts.is_some();
                (
                    available,
                    if available {
                        "ready"
                    } else {
                        "durable-context-unavailable"
                    },
                )
            }
            capabilities if capabilities.contains(&"compositor-window-move") => {
                let config = serde_json::from_value::<Config>(self.rust().config.clone())
                    .unwrap_or_default();
                let adapter = hyprland::HyprlandAdapter::from_config(&config.hyprland);
                let available = adapter.workspace_routing_enabled() && adapter.status() == "ready";
                (
                    available,
                    if available {
                        "ready"
                    } else {
                        "compositor-window-move-unavailable"
                    },
                )
            }
            capabilities if capabilities.contains(&"live-document") => {
                let available =
                    live_document_available(self.rust().state.as_ref(), self.rust().tab);
                (
                    available,
                    if available {
                        "ready"
                    } else {
                        "no-live-document"
                    },
                )
            }
            capabilities if capabilities.contains(&"live-window-reparent") => {
                let available = self
                    .rust()
                    .state
                    .as_ref()
                    .and_then(|state| {
                        let window_id = self.rust().window?;
                        let window = state.windows().get(&window_id)?;
                        let tab_id = window.active_tab?;
                        let tab = state.tabs().get(&tab_id)?;
                        (tab.window == window_id && tab.existence == ExistenceState::Live)
                            .then_some(())
                    })
                    .is_some();
                (
                    available,
                    if available {
                        "ready"
                    } else {
                        "no-live-tab-to-transfer"
                    },
                )
            }
            capabilities if capabilities.contains(&"live-window-reparent-unsupported") => {
                (false, "live-window-reparent-unsupported")
            }
            _ if self.rust().state.is_none() => (false, "core-state-unavailable"),
            _ => (true, "ready"),
        };
        serde_json::json!({
            "state": if available { "available" } else { "unavailable" },
            "reason": reason,
            "predicate": definition.availability_predicate(),
            "requires_subject_revalidation": true
        })
    }

    pub(super) fn validate_action_availability(
        &self,
        action_id: &str,
        source: CommandSource,
    ) -> Result<(), String> {
        let registry = ActionRegistry::default_v1();
        let Some(definition) = registry.resolve(action_id) else {
            if let Some((_, subject)) = parse_external_action_id(action_id) {
                if !matches!(
                    source,
                    CommandSource::Ui | CommandSource::Ipc | CommandSource::Switcher
                ) {
                    return Err(format!(
                        "configured external action is not allowed from {}",
                        source.as_str()
                    ));
                }
                let available = configured_action_targets(&self.rust().config)
                    .ok()
                    .into_iter()
                    .flatten()
                    .flat_map(|(name, target)| {
                        configured_action_target_values(
                            &name,
                            &target,
                            self.active_profile_is_transient(),
                        )
                    })
                    .any(|value| {
                        value.get("id").and_then(Value::as_str) == Some(action_id)
                            && value.get("subject").and_then(Value::as_str)
                                == Some(subject.as_str())
                            && value
                                .get("availability")
                                .and_then(|availability| availability.get("state"))
                                .and_then(Value::as_str)
                                == Some("available")
                    });
                return if available {
                    Ok(())
                } else {
                    Err("configured external action is unavailable".to_owned())
                };
            }
            return Err("action is not registered".to_owned());
        };
        if !definition.sources.contains(&match source {
            CommandSource::Ui => ActionSource::Ui,
            CommandSource::Ipc => ActionSource::Ipc,
            CommandSource::Switcher => ActionSource::Switcher,
            CommandSource::Userscript => ActionSource::Userscript,
            CommandSource::Keyboard => ActionSource::Ui,
            CommandSource::Cli => ActionSource::Ipc,
            CommandSource::Macro => ActionSource::Ui,
        }) {
            return Err(format!("action is not allowed from {}", source.as_str()));
        }
        let availability = self.action_availability(definition);
        if availability.get("state").and_then(Value::as_str) == Some("unavailable") {
            return Err(format!(
                "action capability unavailable: {}",
                availability
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            ));
        }
        Ok(())
    }

    pub(super) fn binding_layer_provenance(&self, mode: &str, keychain: &str) -> Value {
        let mut layers = Vec::new();
        let mut add_layer = |layer: &str, value: Option<&str>, source: Option<String>| {
            if let Some(value) = value {
                layers.push(serde_json::json!({
                    "layer": layer,
                    "source": source,
                    "value": value,
                    "status": if value.is_empty() { "mask" } else { "contributing" }
                }));
            }
        };
        let config_binding = |config: &Value| {
            config
                .get("bindings")
                .and_then(Value::as_object)
                .and_then(|bindings| bindings.get(mode))
                .and_then(Value::as_object)
                .and_then(|bindings| bindings.get(keychain))
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        add_layer(
            "global",
            config_binding(&self.rust().base_config).as_deref(),
            Some(self.rust().config_source.to_string()),
        );
        add_layer(
            "profile",
            self.rust()
                .profile_overrides
                .bindings
                .get(mode)
                .and_then(|bindings| bindings.get(keychain))
                .map(String::as_str),
            None,
        );
        add_layer(
            "runtime",
            self.rust()
                .runtime_overrides
                .bindings
                .get(mode)
                .and_then(|bindings| bindings.get(keychain))
                .map(String::as_str),
            None,
        );
        add_layer(
            "cli",
            self.rust()
                .cli_overrides
                .bindings
                .get(mode)
                .and_then(|bindings| bindings.get(keychain))
                .map(String::as_str),
            None,
        );
        add_layer(
            "temporary",
            self.rust()
                .temporary_overrides
                .bindings
                .get(mode)
                .and_then(|bindings| bindings.get(keychain))
                .map(String::as_str),
            None,
        );
        Value::Array(layers)
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn ipc_bindings_query(&self, params: &Value) -> Result<Value, String> {
        let object = query_object(params, "bindings.query", &["mode"])?;
        let mode = query_optional_string(object, "bindings.query", "mode")?
            .map(parse_ipc_mode)
            .transpose()?;
        let definitions = self
            .rust()
            .bindings
            .as_ref()
            .map(BindingResolver::definitions)
            .unwrap_or_default();
        let visible_definitions = definitions
            .iter()
            .filter(|binding| mode.is_none_or(|mode| mode == binding.mode))
            .collect::<Vec<_>>();
        let canonical_name = |command: &str| {
            parse_chain(command, ParseInput::Interactive)
                .ok()
                .and_then(|mut commands| commands.drain(..).next())
                .and_then(|command| self.rust().registry.resolve(&command.name).ok())
                .map(|definition| definition.name.clone())
        };
        let bindings = visible_definitions
            .iter()
            .map(|binding| {
                let command_name = canonical_name(&binding.command);
                let description = command_name
                    .as_deref()
                    .and_then(|name| self.rust().registry.resolve(name).ok())
                    .map_or_else(String::new, |definition| definition.description.clone());
                let keychain = binding.keys.concat();
                let source = self
                    .rust()
                    .config
                    .get("bindings")
                    .and_then(Value::as_object)
                    .and_then(|bindings| bindings.get(ipc_mode_name(binding.mode)))
                    .and_then(Value::as_object)
                    .is_some_and(|bindings| bindings.contains_key(&keychain));
                serde_json::json!({
                    "mode": ipc_mode_name(binding.mode),
                    "keys": binding.keys,
                    "keychain": keychain,
                    "command": binding.command,
                    "command_name": command_name,
                    "description": description,
                    "source": if source { "user" } else { "built-in" },
                    "timeout_ms": ferric_browser_core::DEFAULT_CHORD_TIMEOUT_MS,
                    "provenance": self.binding_layer_provenance(
                        ipc_mode_name(binding.mode),
                        &keychain
                    )
                })
            })
            .collect::<Vec<_>>();

        let mut commands = Vec::new();
        let mut unbound = Vec::new();
        for definition in self.rust().registry.definitions() {
            let applicable_modes = definition
                .modes
                .iter()
                .copied()
                .filter(|candidate| mode.is_none_or(|mode| mode == *candidate))
                .collect::<Vec<_>>();
            if applicable_modes.is_empty() {
                continue;
            }
            commands.push(serde_json::json!({
                "name": definition.name,
                "aliases": definition.aliases,
                "modes": applicable_modes.iter().map(|mode| ipc_mode_name(*mode)).collect::<Vec<_>>(),
                "count": match definition.count {
                    ferric_browser_core::CountPolicy::NotSupported => serde_json::json!({"supported": false}),
                    ferric_browser_core::CountPolicy::Supported { maximum } => serde_json::json!({
                        "supported": true,
                        "maximum": maximum
                    }),
                },
                "sensitive": definition.sensitive,
                "description": definition.description,
                "scope": format!("{:?}", definition.scope).to_ascii_lowercase(),
                "effect": format!("{:?}", definition.effect).to_ascii_lowercase(),
                "completion": format!("{:?}", definition.completion).to_ascii_lowercase(),
                "arguments": definition.arguments.iter().map(|argument| serde_json::json!({
                    "name": argument.name,
                    "kind": format!("{:?}", argument.kind).to_ascii_lowercase(),
                    "required": argument.required
                })).collect::<Vec<_>>(),
                "examples": definition.examples
            }));
            for candidate_mode in applicable_modes {
                let bound = definitions.iter().any(|binding| {
                    binding.mode == candidate_mode
                        && canonical_name(&binding.command).as_deref()
                            == Some(definition.name.as_str())
                });
                if !bound {
                    unbound.push(serde_json::json!({
                        "mode": ipc_mode_name(candidate_mode),
                        "command": definition.name,
                        "description": definition.description
                    }));
                }
            }
        }

        let mut conflicts = Vec::new();
        for (left_index, left) in visible_definitions.iter().enumerate() {
            for right in visible_definitions.iter().skip(left_index + 1) {
                if left.mode != right.mode || left.keys == right.keys {
                    continue;
                }
                let left_prefix = right.keys.starts_with(&left.keys);
                let right_prefix = left.keys.starts_with(&right.keys);
                if left_prefix || right_prefix {
                    conflicts.push(serde_json::json!({
                        "mode": ipc_mode_name(left.mode),
                        "keys": [left.keys.concat(), right.keys.concat()],
                        "commands": [left.command, right.command],
                        "kind": "prefix-ambiguity",
                        "description": "A shorter binding is also a prefix of a longer binding; the configured chord timeout decides the exact-prefix case."
                    }));
                }
            }
        }
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "mode": mode.map_or(Value::Null, |mode| Value::String(ipc_mode_name(mode).into())),
            "bindings": bindings,
            "commands": commands,
            "unbound": unbound,
            "conflicts": conflicts
        }))
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn ipc_bindings_explain(&self, params: &Value) -> Result<Value, String> {
        let object = query_object(params, "bindings.explain", &["keychain", "mode"])?;
        let keychain = object
            .get("keychain")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty() && value.len() <= 128)
            .ok_or_else(|| "bindings.explain requires a bounded keychain".to_owned())?;
        if keychain.chars().any(char::is_control) {
            return Err("bindings.explain keychain contains a control character".into());
        }
        let requested_mode = query_optional_string(object, "bindings.explain", "mode")?
            .map(parse_ipc_mode)
            .transpose()?;
        let current_mode = self.rust().core_mode;
        let mode = requested_mode.unwrap_or(current_mode);

        let mut key_tokens = keychain.chars().peekable();
        let mut count_text = String::new();
        if key_tokens
            .peek()
            .is_some_and(|key| ('1'..='9').contains(key))
        {
            while key_tokens.peek().is_some_and(char::is_ascii_digit) {
                count_text.push(key_tokens.next().expect("peeked count digit"));
            }
        }
        let tokens = key_tokens.map(|key| key.to_string()).collect::<Vec<_>>();
        if tokens.is_empty() {
            return Err("bindings.explain keychain must contain a binding key".into());
        }
        let count = count_text.parse::<u32>().unwrap_or(9_999).clamp(1, 9_999);

        let definitions = self
            .rust()
            .bindings
            .as_ref()
            .map(BindingResolver::definitions)
            .unwrap_or_default();
        let definitions = definitions
            .iter()
            .filter(|binding| binding.mode == mode)
            .collect::<Vec<_>>();
        let exact = definitions
            .iter()
            .find(|binding| binding.keys == tokens)
            .copied();
        let mut continuations = BTreeSet::new();
        for binding in &definitions {
            if binding.keys.starts_with(&tokens)
                && binding.keys.len() > tokens.len()
                && let Some(next) = binding.keys.get(tokens.len())
            {
                continuations.insert(next.clone());
            }
        }
        let command_name = exact.and_then(|binding| {
            parse_chain(&binding.command, ParseInput::Interactive)
                .ok()
                .and_then(|mut commands| commands.drain(..).next())
                .and_then(|command| self.rust().registry.resolve(&command.name).ok())
                .map(|definition| definition.name.clone())
        });
        let count_policy = command_name
            .as_deref()
            .and_then(|name| self.rust().registry.resolve(name).ok())
            .map_or_else(
                || {
                serde_json::json!({
                    "supported": false,
                    "reason": if exact.is_some() { "binding command is unavailable" } else { "no exact binding" }
                })
                },
                |definition| match definition.count {
                    ferric_browser_core::CountPolicy::NotSupported => {
                        serde_json::json!({"supported": false, "reason": "command does not accept counts"})
                    }
                    ferric_browser_core::CountPolicy::Supported { maximum } => {
                        serde_json::json!({"supported": true, "maximum": maximum, "effective": count.min(maximum)})
                    }
                },
            );
        let mode_name = ipc_mode_name(mode);
        let source = exact.map_or("unbound", |_| {
            let configured = self
                .rust()
                .config
                .get("bindings")
                .and_then(Value::as_object)
                .and_then(|bindings| bindings.get(mode_name))
                .and_then(Value::as_object)
                .is_some_and(|bindings| bindings.contains_key(keychain));
            if configured { "user" } else { "built-in" }
        });
        let resolution = if exact.is_some() && !continuations.is_empty() {
            "exact-prefix"
        } else if exact.is_some() {
            "exact"
        } else if !continuations.is_empty() {
            "prefix"
        } else {
            "unmatched"
        };
        let has_continuations = !continuations.is_empty();
        let conflicts = self
            .ipc_bindings_query(&serde_json::json!({"mode": mode_name}))
            .ok()
            .and_then(|value| value.get("conflicts").cloned())
            .unwrap_or_else(|| Value::Array(Vec::new()));

        Ok(serde_json::json!({
            "schema": 1,
            "keychain": keychain,
            "tokens": tokens,
            "requested_mode": object.get("mode").cloned().unwrap_or(Value::Null),
            "current_mode": ipc_mode_name(current_mode),
            "mode": mode_name,
            "count": {
                "explicit": !count_text.is_empty(),
                "value": count,
                "policy": count_policy
            },
            "resolution": resolution,
            "winning_binding": exact.map(|binding| serde_json::json!({
                "command": binding.command,
                "command_name": command_name,
                "source": source,
                "provenance": self.binding_layer_provenance(mode_name, keychain)
            })),
            "continuations": continuations.into_iter().collect::<Vec<_>>(),
            "timeout_ms": ferric_browser_core::DEFAULT_CHORD_TIMEOUT_MS,
            "timeout_behavior": if exact.is_some() && has_continuations {
                "exact prefix wins after the chord timeout"
            } else if has_continuations {
                "waits for a continuation until the chord timeout"
            } else {
                "no prefix ambiguity"
            },
            "shadowed_layers": if source == "user" {
                serde_json::json!([{"layer": "built-in", "status": "shadowed"}])
            } else {
                serde_json::json!([])
            },
            "conflicts": conflicts,
            "reserved": {
                "escape": true,
                "chord": "Ctrl+Shift+Escape",
                "note": "Escape handling remains browser-reserved and is not configurable through the binding trie"
            }
        }))
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn ipc_config_get(&self, params: &Value) -> Result<Value, IpcQueryError> {
        let object = query_object(params, "config.get", &["key", "url", "explain"])
            .map_err(IpcQueryError::Invalid)?;
        let key = object
            .get("key")
            .and_then(Value::as_str)
            .filter(|key| !key.is_empty() && key.len() <= 256 && !key.chars().any(char::is_control))
            .ok_or_else(|| IpcQueryError::Invalid("config.get requires a nonempty key".into()))?;
        if let Some(url) = object.get("url")
            && !url.is_null()
            && url.as_str().is_none()
        {
            return Err(IpcQueryError::Invalid(
                "config.get url must be a string or null".into(),
            ));
        }
        let requested_url = object
            .get("url")
            .and_then(Value::as_str)
            .filter(|url| !url.is_empty());
        if requested_url.is_some_and(|url| {
            url.len() > 8192
                || url.chars().any(char::is_control)
                || !url.starts_with("http://") && !url.starts_with("https://")
        }) {
            return Err(IpcQueryError::Invalid(
                "config.get url must be a bounded HTTP(S) URL".into(),
            ));
        }
        if let Some(explain) = object.get("explain")
            && explain.as_bool().is_none()
        {
            return Err(IpcQueryError::Invalid(
                "config.get explain must be a boolean".into(),
            ));
        }
        let mut value = &self.rust().config;
        for part in key.split('.') {
            value = value
                .as_object()
                .and_then(|object| object.get(part))
                .ok_or_else(|| {
                    IpcQueryError::NotFound(format!("configuration key not found: {key}"))
                })?;
        }
        if requested_url.is_some() && !setting_supports_site_scope(key) {
            return Err(IpcQueryError::Invalid(format!(
                "configuration key {key} does not support site scope"
            )));
        }
        let base_value = value.clone();
        let metadata = setting_metadata(key).ok_or_else(|| {
            IpcQueryError::Invalid(format!(
                "configuration key is not in the setting registry: {key}"
            ))
        })?;
        let pending_value = self
            .rust()
            .pending_config
            .as_ref()
            .and_then(|pending| config_value_at_path(pending, key))
            .filter(|pending| *pending != &base_value)
            .cloned();
        let mut effective_value = base_value.clone();
        let mut effective_source = self.rust().config_source.to_string();
        let mut effective_scope = "global";
        let mut contributors = vec![serde_json::json!({
            "layer": "global",
            "source": self.rust().config_source.to_string(),
            "value": config_value_at_path(&self.rust().base_config, key)
                .cloned()
                .unwrap_or_else(|| base_value.clone()),
            "status": "contributing"
        })];
        for (layer, overrides, scope) in [
            ("profile", &self.rust().profile_overrides, "profile"),
            ("runtime", &self.rust().runtime_overrides, "global"),
            ("cli", &self.rust().cli_overrides, "global"),
            ("temporary", &self.rust().temporary_overrides, "global"),
        ] {
            if let Some(layer_value) = runtime_override_value(overrides, key) {
                effective_source = layer.to_owned();
                effective_scope = scope;
                effective_value = layer_value.clone();
                contributors.push(serde_json::json!({
                    "layer": layer,
                    "value": layer_value,
                    "status": "contributing"
                }));
            }
        }
        if let Some(url) = requested_url {
            let config =
                serde_json::from_value::<Config>(self.rust().config.clone()).map_err(|error| {
                    IpcQueryError::Invalid(format!(
                        "validated configuration could not be decoded: {error}"
                    ))
                })?;
            for rule in matching_site_rules(&config, url) {
                if let Some(site_value) = rule.set.get(key)
                    && let Ok(site_value) = serde_json::to_value(site_value)
                {
                    effective_value = site_value.clone();
                    effective_source = format!("site-rule:{}", rule.id);
                    effective_scope = "site";
                    let contributor = serde_json::json!({
                        "layer": "site-rule",
                        "id": rule.id,
                        "priority": rule.priority,
                        "value": site_value,
                        "status": "contributing"
                    });
                    contributors.push(contributor);
                }
            }
        }
        let mut result = serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "key": key,
            "value": effective_value,
            "metadata": {
                "type": metadata.value_type,
                "default": metadata.default_value,
                "supported_scopes": metadata.supported_scopes,
                "apply_time": metadata.apply_time,
                "prerequisite": metadata.prerequisite,
                "sensitivity": metadata.sensitivity
            },
            "source": effective_source,
            "scope": effective_scope,
            "supported_scopes": setting_supported_scopes(key),
            "site_scope": if setting_supports_site_scope(key) {
                serde_json::json!({"supported": true, "mechanism": "validated-site-rule"})
            } else {
                serde_json::json!({
                    "supported": false,
                    "mechanism": "profile-or-global-value",
                    "reason": "changing an engine-wide value on tab focus would leak across tabs"
                })
            },
            "apply_time": metadata.apply_time,
            "restart_required": metadata.apply_time == "restart",
            "reload_required": matches!(metadata.apply_time, "reload" | "startup"),
            "pending": pending_value.is_some(),
            "pending_value": pending_value,
        });
        if object
            .get("explain")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            for contributor in &mut contributors {
                let layer = contributor.get("layer").and_then(Value::as_str);
                let source = contributor.get("source").and_then(Value::as_str);
                let site_id = contributor.get("id").and_then(Value::as_str);
                let winner = layer == Some(effective_source.as_str())
                    || source == Some(effective_source.as_str())
                    || effective_source
                        .strip_prefix("site-rule:")
                        .is_some_and(|id| layer == Some("site-rule") && site_id == Some(id));
                if winner {
                    contributor["status"] = serde_json::json!("winner");
                } else if contributor.get("status").and_then(Value::as_str) == Some("contributing")
                {
                    contributor["status"] = serde_json::json!("shadowed");
                }
            }
            result["explanation"] = serde_json::json!({
                "contributors": contributors,
                "winner": result["source"].clone(),
                "scope": result["scope"].clone()
            });
        }
        Ok(result)
    }

    pub(super) fn ipc_permissions_query(&self, params: &Value) -> Result<Value, IpcQueryError> {
        let object = params.as_object().ok_or_else(|| {
            IpcQueryError::Invalid("permissions.query params must be an object".into())
        })?;
        if object.keys().any(|key| key != "origin") {
            return Err(IpcQueryError::Invalid(
                "permissions.query contains an unknown field".into(),
            ));
        }
        let origin = object.get("origin").and_then(Value::as_str);
        if object
            .get("origin")
            .is_some_and(|value| !value.is_null() && origin.is_none_or(str::is_empty))
        {
            return Err(IpcQueryError::Invalid(
                "permissions.query origin must be a nonempty string or null".into(),
            ));
        }
        if self.rust().profile_persistence.lacks_durable_storage() {
            return Ok(serde_json::json!({
                "private": true,
                "ready": true,
                "rules": [],
                "sequence": self.rust().ipc_sequence
            }));
        }
        if self.rust().storage_library_dirty.get() || self.rust().storage_library.is_none() {
            return Err(IpcQueryError::Busy(
                "permission metadata is still loading; retry".into(),
            ));
        }
        let normalized_origin = origin
            .map(ferric_browser_storage::normalize_permission_origin)
            .transpose()
            .map_err(|error| IpcQueryError::Invalid(error.to_string()))?;
        let rules = self
            .rust()
            .storage_library
            .as_ref()
            .ok_or_else(|| IpcQueryError::Unavailable("permission metadata is unavailable".into()))?
            .permissions
            .iter()
            .filter(|rule| {
                normalized_origin
                    .as_deref()
                    .is_none_or(|origin| rule.origin == origin)
            })
            .cloned()
            .collect::<Vec<_>>();
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "private": false,
            "ready": true,
            "revision": self.rust().storage_library_revision,
            "rules": rules.into_iter().map(|rule| serde_json::json!({
                "origin": rule.origin,
                "permission": rule.permission,
                "decision": rule.decision,
                "expires_at": rule.expires_at,
                "updated_at": rule.updated_at
            })).collect::<Vec<_>>()
        }))
    }

    pub(super) fn ipc_downloads_query(&self, params: &Value) -> Result<Value, IpcQueryError> {
        let object = query_object(params, "downloads.query", &["include_private"])
            .map_err(IpcQueryError::Invalid)?;
        let include_private = query_bool_param(object, "downloads.query", "include_private", false)
            .map_err(IpcQueryError::Invalid)?;
        if include_private && !self.ipc_private_queries_enabled() {
            return Err(IpcQueryError::Invalid(
                "private download queries are disabled by configuration".into(),
            ));
        }
        if self.rust().profile_persistence.lacks_durable_storage() {
            return Ok(serde_json::json!({
                "downloads": [],
                "private": true,
                "ready": true,
                "sequence": self.rust().ipc_sequence
            }));
        }
        if self.rust().storage_library_dirty.get() || self.rust().storage_library.is_none() {
            return Err(IpcQueryError::Busy(
                "download metadata is still loading; retry".into(),
            ));
        }
        let Some(library) = self.rust().storage_library.as_ref() else {
            return Err(IpcQueryError::Unavailable(
                "download metadata is unavailable".into(),
            ));
        };
        let downloads = library.downloads.clone();
        let downloads = downloads
            .into_iter()
            .map(|download| {
                serde_json::json!({
                    "id": download.id,
                    "source_url": safe_ipc_url(&download.source_url),
                    "destination": download.destination,
                    "state": download.state.as_str(),
                    "bytes_received": download.bytes_received,
                    "created_at": download.created_at,
                    "completed_at": download.completed_at
                })
            })
            .collect::<Vec<_>>();
        Ok(serde_json::json!({
            "downloads": downloads,
            "private": false,
            "ready": true,
            "revision": self.rust().storage_library_revision,
            "sequence": self.rust().ipc_sequence
        }))
    }

    pub(super) fn ipc_private_queries_enabled(&self) -> bool {
        self.rust()
            .config
            .get("ipc")
            .and_then(Value::as_object)
            .and_then(|ipc| ipc.get("private_queries"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    pub(super) fn ipc_contexts_query(&self, params: &Value) -> Result<Value, String> {
        let object = query_object(params, "contexts.query", &["include_members"])?;
        let include_members = query_bool_param(object, "contexts.query", "include_members", false)?;
        let contexts = self
            .rust()
            .contexts
            .as_ref()
            .map(|registry| {
                registry
                    .contexts()
                    .iter()
                    .map(|context| {
                        let mut value = serde_json::json!({
                            "id": context.id.to_string(),
                            "name": context.name,
                            "label": context.label,
                            "profile": context.profile,
                            "sessions": context.sessions,
                            "workspace": context.workspace,
                            "accent": context.accent,
                            "default_target": context.default_target,
                            "created_at": context.created_at,
                            "updated_at": context.updated_at
                        });
                        if include_members {
                            value["members"] = serde_json::to_value(&context.members)
                                .unwrap_or_else(|_| Value::Array(Vec::new()));
                        }
                        value
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "contexts": contexts
        }))
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn ipc_switcher_query(&self, params: &Value) -> Result<Value, String> {
        let object = query_object(
            params,
            "switcher.query",
            &[
                "query",
                "scope",
                "limit",
                "offset",
                "include_private",
                "private_scope",
            ],
        )?;
        let query_input = query_optional_string(object, "switcher.query", "query")?.unwrap_or("");
        let query = tokenize_switcher_query(query_input);
        if query.iter().any(|term| term.len() > 128) {
            return Err("switcher.query terms are too long".into());
        }
        let scope = query_optional_string(object, "switcher.query", "scope")?.unwrap_or("all");
        let scopes = [
            "all",
            "tabs",
            "windows",
            "contexts",
            "commands",
            "history",
            "marks",
            "sessions",
            "downloads",
            "closed",
            "actions",
        ];
        if !scopes.contains(&scope) {
            return Err(format!("unknown switcher scope: {scope}"));
        }
        let limit = query_limit(object, "switcher.query", 50)?;
        let offset = query_offset(object, "switcher.query")?;
        let include_private = query_bool_param(object, "switcher.query", "include_private", false)?;
        if include_private && !self.ipc_private_queries_enabled() {
            // A switcher explicitly opened inside a transient profile is
            // allowed to search only that same in-memory profile. External
            // IPC queries still require the existing opt-in configuration.
            if object.get("private_scope").is_none() {
                return Err("private switcher queries are disabled by configuration".into());
            }
        }
        let Some(state) = self.rust().state.as_ref() else {
            return Err("core state unavailable".into());
        };
        let current_context = state
            .active_tab()
            .and_then(|tab| state.windows().get(&tab.window))
            .and_then(|window| window.context.as_deref());
        let private_scope = query_optional_string(object, "switcher.query", "private_scope")?;
        if let Some(scope) = private_scope {
            let active_scope = state
                .active_tab()
                .and_then(|tab| state.profiles().get(&tab.profile))
                .filter(|profile| profile.privacy.is_transient())
                .map(|profile| profile.label.as_str());
            if active_scope != Some(scope) {
                return Err("private switcher scope is stale or unavailable".into());
            }
        }
        let contexts = self.rust().contexts.as_ref();
        let mut candidates = Vec::<SwitcherCandidate>::new();
        let mut add = |kind: &str,
                       id: String,
                       label: String,
                       secondary: String,
                       profile: Option<String>,
                       context: Option<(String, String)>,
                       workspace: Option<String>,
                       private: bool,
                       recency: i64,
                       generation: Option<u64>,
                       fields: Vec<String>,
                       actions: &[&str]| {
            if let Some(scope) = private_scope
                && !matches!(kind, "command" | "action")
                && profile.as_deref() != Some(scope)
            {
                return;
            }
            if private
                && (!include_private
                    || private_scope.is_some_and(|scope| profile.as_deref() != Some(scope)))
            {
                return;
            }
            let Some(rank) = switcher_rank(&query, &id, &label, &fields) else {
                return;
            };
            let rank = rank.saturating_add(switcher_context_boost(
                current_context,
                context.as_ref().map(|(_, name)| name.as_str()),
            ));
            let value = serde_json::json!({
                "kind": kind,
                "id": id,
                "owner_token": self.rust().window_token.to_string(),
                "generation": generation,
                "label": label,
                "secondary": secondary.replace(['\n', '\r'], " "),
                "profile": profile,
                "context": context.as_ref().map(|(id, name)| serde_json::json!({"id": id, "name": name})),
                "workspace": workspace,
                "privacy": if private { "private" } else { "normal" },
                "recency": recency,
                "rank": rank,
                "match_fields": fields,
                "actions": actions
            });
            candidates.push(SwitcherCandidate {
                rank,
                kind: kind.to_owned(),
                recency,
                value,
            });
        };

        if scope == "all" || scope == "tabs" {
            for tab in state.tabs().values() {
                let Some(profile_state) = state.profiles().get(&tab.profile) else {
                    continue;
                };
                let private = profile_state.privacy.is_transient();
                let context = state
                    .windows()
                    .get(&tab.window)
                    .and_then(|window| window.context.as_deref())
                    .and_then(|name| {
                        contexts?
                            .contexts()
                            .iter()
                            .find(|context| context.name == name)
                    })
                    .map(|context| (context.id.to_string(), context.name.clone()));
                let workspace = context.as_ref().and_then(|(_, name)| {
                    contexts?
                        .contexts()
                        .iter()
                        .find(|context| context.name == *name)?
                        .workspace
                        .clone()
                });
                let url = tab
                    .url
                    .as_deref()
                    .map_or_else(|| "about:blank".into(), safe_ipc_url);
                add(
                    "tab",
                    tab.id.to_string(),
                    if tab.title.is_empty() {
                        url.clone()
                    } else {
                        tab.title.clone()
                    },
                    url.clone(),
                    Some(profile_state.label.clone()),
                    context,
                    workspace,
                    private,
                    i64::try_from(state.revision()).unwrap_or(i64::MAX),
                    Some(tab.generation),
                    vec![tab.title.clone(), url, profile_state.label.clone()],
                    &["focus", "open"],
                );
            }
        }
        if scope == "all" || scope == "windows" {
            for window in state.windows().values() {
                let Some(profile_state) = state.profiles().get(&window.profile) else {
                    continue;
                };
                let private = profile_state.privacy.is_transient();
                let context = window.context.as_deref().and_then(|name| {
                    contexts?
                        .contexts()
                        .iter()
                        .find(|context| context.name == name)
                });
                let context_pair =
                    context.map(|context| (context.id.to_string(), context.name.clone()));
                add(
                    "window",
                    window.id.to_string(),
                    format!("Window {}", window.id),
                    format!("{} tab(s)", window.tabs.len()),
                    Some(profile_state.label.clone()),
                    context_pair,
                    context.and_then(|context| context.workspace.clone()),
                    private,
                    i64::try_from(state.revision()).unwrap_or(i64::MAX),
                    None,
                    vec![profile_state.label.clone(), window.id.to_string()],
                    &["focus"],
                );
            }
        }
        if (scope == "all" || scope == "contexts") && contexts.is_some() {
            for context in contexts.into_iter().flat_map(ContextSnapshot::contexts) {
                let private = state
                    .profiles()
                    .values()
                    .find(|profile| profile.label == context.profile)
                    .is_some_and(|profile| profile.privacy.is_transient());
                add(
                    "context",
                    context.id.to_string(),
                    context.label.clone(),
                    context.name.clone(),
                    Some(context.profile.clone()),
                    Some((context.id.to_string(), context.name.clone())),
                    context.workspace.clone(),
                    private,
                    context.updated_at,
                    None,
                    vec![
                        context.name.clone(),
                        context.label.clone(),
                        context.profile.clone(),
                    ],
                    &["enter"],
                );
            }
        }
        if scope == "all" || scope == "commands" {
            for definition in self.rust().registry.definitions() {
                add(
                    "command",
                    definition.action.to_string(),
                    definition.name.clone(),
                    definition.description.clone(),
                    None,
                    None,
                    None,
                    false,
                    0,
                    None,
                    vec![definition.name.clone(), definition.description.clone()],
                    &["execute", "help"],
                );
            }
        }
        if scope == "all" || scope == "actions" {
            let actions = ActionRegistry::default_v1();
            for definition in actions.definitions() {
                if !definition.sources.contains(&ActionSource::Switcher)
                    || definition
                        .arguments
                        .iter()
                        .any(|argument| argument.required)
                {
                    continue;
                }
                add(
                    "action",
                    definition.id.clone(),
                    definition.label.clone(),
                    definition.description.clone(),
                    None,
                    None,
                    None,
                    false,
                    0,
                    None,
                    vec![
                        definition.id.clone(),
                        definition.subject.as_str().into(),
                        definition.verb.clone(),
                        definition.description.clone(),
                    ],
                    &["execute"],
                );
            }
            for value in configured_switcher_action_values(
                &self.rust().config,
                self.active_profile_is_transient(),
            )? {
                let Some(subject) = value.get("subject").and_then(Value::as_str) else {
                    continue;
                };
                let Some(id) = value.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let description = value
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                add(
                    "action",
                    id.to_owned(),
                    value
                        .get("label")
                        .and_then(Value::as_str)
                        .unwrap_or("Configured action")
                        .to_owned(),
                    description.clone(),
                    None,
                    None,
                    None,
                    false,
                    0,
                    None,
                    vec![
                        id.to_owned(),
                        subject.to_owned(),
                        "send".into(),
                        description,
                    ],
                    &["execute"],
                );
            }
        }
        if scope == "all" || scope == "closed" {
            for closed in &self.rust().closed_tabs {
                add(
                    "closed",
                    closed.id.to_string(),
                    if closed.title.is_empty() {
                        closed.url.clone()
                    } else {
                        closed.title.clone()
                    },
                    closed.url.clone(),
                    Some(closed.profile.clone()),
                    None,
                    None,
                    closed.private,
                    closed.closed_at,
                    None,
                    vec![
                        closed.title.clone(),
                        closed.url.clone(),
                        closed.profile.clone(),
                    ],
                    &["reopen"],
                );
            }
        }
        let indexed_library = self.rust().switcher_library_index.as_ref().filter(|index| {
            self.rust().switcher_library_index_revision == self.rust().storage_library_revision
                && index.profile_name == self.rust().profile_name
        });
        if let Some(index) = indexed_library {
            let profile = index.profile_name.clone();
            for candidate in &index.candidates {
                let actions: &[&str] = match candidate.kind.as_str() {
                    "history" => &["open"],
                    "bookmark" | "quickmark" => &["open", "delete"],
                    "download" => &["show", "open"],
                    _ => &[],
                };
                add(
                    &candidate.kind,
                    candidate.id.clone(),
                    candidate.label.clone(),
                    candidate.secondary.clone(),
                    Some(profile.clone()),
                    None,
                    None,
                    false,
                    candidate.recency,
                    None,
                    candidate.fields.clone(),
                    actions,
                );
            }
        } else if let Some(library) = self.rust().storage_library.as_ref() {
            let profile = self.rust().profile_name.clone();
            if scope == "all" || scope == "history" {
                for page in &library.history {
                    add(
                        "history",
                        page.id.to_string(),
                        if page.title.is_empty() {
                            page.url.clone()
                        } else {
                            page.title.clone()
                        },
                        page.url.clone(),
                        Some(profile.clone()),
                        None,
                        None,
                        false,
                        page.last_visit,
                        None,
                        vec![page.title.clone(), page.url.clone()],
                        &["open"],
                    );
                }
            }
            if scope == "all" || scope == "marks" {
                for mark in &library.bookmarks {
                    add(
                        "bookmark",
                        mark.id.clone(),
                        if mark.title.is_empty() {
                            mark.url.clone()
                        } else {
                            mark.title.clone()
                        },
                        mark.url.clone(),
                        Some(profile.clone()),
                        None,
                        None,
                        false,
                        mark.updated_at,
                        None,
                        vec![mark.title.clone(), mark.url.clone()],
                        &["open", "delete"],
                    );
                }
                for mark in &library.quickmarks {
                    add(
                        "quickmark",
                        mark.name.clone(),
                        mark.name.clone(),
                        mark.url.clone(),
                        Some(profile.clone()),
                        None,
                        None,
                        false,
                        0,
                        None,
                        vec![mark.name.clone(), mark.url.clone()],
                        &["open", "delete"],
                    );
                }
            }
            if scope == "all" || scope == "downloads" {
                for download in &library.downloads {
                    add(
                        "download",
                        download.id.clone(),
                        download.destination.clone(),
                        download.source_url.clone(),
                        Some(profile.clone()),
                        None,
                        None,
                        false,
                        download.created_at,
                        None,
                        vec![download.destination.clone(), download.source_url.clone()],
                        &["show", "open"],
                    );
                }
            }
        } else if include_private && self.active_profile_is_transient() {
            let profile = self.rust().profile_name.clone();
            if scope == "all" || scope == "history" {
                for page in &self.rust().private_history {
                    add(
                        "history",
                        page.id.to_string(),
                        if page.title.is_empty() {
                            page.url.clone()
                        } else {
                            page.title.clone()
                        },
                        page.url.clone(),
                        Some(profile.clone()),
                        None,
                        None,
                        true,
                        page.last_visit,
                        None,
                        vec![page.title.clone(), page.url.clone()],
                        &["open"],
                    );
                }
            }
        }
        if (scope == "all" || scope == "sessions")
            && let Some(profile_id) = self.rust().profile_id
        {
            let profile = self.rust().profile_name.clone();
            for name in &self.rust().session_names {
                add(
                    "session",
                    format!("{profile_id}:{name}"),
                    name.clone(),
                    "saved session".into(),
                    Some(profile.clone()),
                    None,
                    None,
                    false,
                    0,
                    None,
                    vec![name.clone()],
                    &["load-preview", "load"],
                );
            }
        }
        let (total, results) = select_switcher_page(candidates, offset, limit);
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "scope": scope,
            "query": query.join(" "),
            "offset": offset,
            "limit": limit,
            "has_more": offset.saturating_add(results.len()) < total,
            "results": results
        }))
    }

    pub(super) fn ipc_switcher_activate(
        mut self: Pin<&mut Self>,
        params: &Value,
    ) -> Result<Value, String> {
        let object = params
            .as_object()
            .ok_or_else(|| "switcher.activate params must be an object".to_owned())?;
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "kind" | "id" | "action" | "generation"))
        {
            return Err("switcher.activate contains an unknown field".into());
        }
        let kind = object
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| "switcher.activate requires a string kind".to_owned())?;
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| "switcher.activate requires a string id".to_owned())?;
        if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
            return Err("switcher.activate id is invalid".into());
        }
        let action = object
            .get("action")
            .map(|value| {
                value
                    .as_str()
                    .filter(|action| !action.is_empty() && action.len() <= 64)
                    .ok_or_else(|| "switcher.activate action must be a bounded string".to_owned())
            })
            .transpose()?
            .unwrap_or_else(|| switcher_default_action(kind).unwrap_or_default());
        let generation = object
            .get("generation")
            .map(|value| {
                value.as_u64().ok_or_else(|| {
                    "switcher.activate generation must be an unsigned integer".to_owned()
                })
            })
            .transpose()?;
        if generation.is_some() && kind != "tab" {
            return Err("switcher.activate generation is only valid for tab results".into());
        }
        if !switcher_action_allowed(kind, action) {
            return Err(format!(
                "switcher.activate action {action:?} is not allowed for result kind {kind:?}"
            ));
        }
        let binding = self.as_ref();
        let Some(state) = binding.rust().state.as_ref() else {
            return Err("core state unavailable".into());
        };
        let private = match kind {
            "tab" => state
                .tabs()
                .values()
                .find(|tab| tab.id.to_string() == id)
                .and_then(|tab| state.profiles().get(&tab.profile))
                .map(|profile| profile.privacy.is_transient())
                .ok_or_else(|| "switcher target is stale or unsupported".to_owned())?,
            "window" => state
                .windows()
                .values()
                .find(|window| window.id.to_string() == id)
                .and_then(|window| state.profiles().get(&window.profile))
                .map(|profile| profile.privacy.is_transient())
                .ok_or_else(|| "switcher target is stale or unsupported".to_owned())?,
            "closed" => binding
                .rust()
                .closed_tabs
                .iter()
                .find(|closed| closed.id.to_string() == id)
                .map(|closed| closed.private)
                .ok_or_else(|| "switcher target is stale or unsupported".to_owned())?,
            "context" | "history" | "bookmark" | "quickmark" | "session" | "download"
            | "command" | "action" => false,
            _ => return Err(format!("unknown switcher result kind: {kind}")),
        };
        if private {
            return Err("private or ephemeral switcher activation is disabled".into());
        }
        if !self
            .as_mut()
            .activate_switcher_action_internal(kind, id, action, generation)
        {
            return Err("switcher target is stale or unsupported".into());
        }
        Ok(serde_json::json!({
            "status": "accepted",
            "kind": kind,
            "id": id,
            "action": action,
            "sequence": self.rust().ipc_sequence
        }))
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn ipc_site_status(&self, params: &Value) -> Result<Value, String> {
        let Some(state) = self.rust().state.as_ref() else {
            return Err("core state unavailable".into());
        };
        let tab = match params {
            Value::Null => state.active_tab(),
            Value::Object(object) => {
                if object.keys().any(|key| key != "tab") {
                    return Err("site.status contains an unknown field".into());
                }
                match object.get("tab") {
                    None => state.active_tab(),
                    Some(Value::String(id))
                        if !id.is_empty()
                            && id.len() <= 128
                            && !id.chars().any(char::is_control) =>
                    {
                        let tab = state
                            .tabs()
                            .values()
                            .find(|candidate| candidate.id.to_string() == *id)
                            .ok_or_else(|| "site.status tab target is stale".to_owned())?;
                        if tab.existence != ExistenceState::Live {
                            return Err("site.status tab target is stale".into());
                        }
                        Some(tab)
                    }
                    Some(_) => return Err("site.status tab must be a bounded stable tab ID".into()),
                }
            }
            _ => return Err("site.status params must be an object or null".into()),
        };
        let window = tab.and_then(|tab| state.windows().get(&tab.window));
        let profile = tab.and_then(|tab| state.profiles().get(&tab.profile));
        let private = tab
            .and_then(|tab| state.profiles().get(&tab.profile))
            .is_some_and(|profile| profile.privacy.is_transient());
        let origin = tab
            .and_then(|tab| tab.url.as_deref())
            .and_then(safe_site_origin);
        let current_site_url = tab
            .and_then(|tab| tab.url.clone())
            .unwrap_or_else(|| self.rust().current_url.to_string());
        let permission_metadata_ready = private
            || (self.rust().storage_library.is_some() && !self.rust().storage_library_dirty.get());
        let permission_rules = if private {
            Vec::new()
        } else {
            self.rust()
                .storage_library
                .as_ref()
                .into_iter()
                .flat_map(|library| library.permissions.iter())
                .filter(|rule| origin.as_deref().is_none_or(|origin| rule.origin == origin))
                .map(|rule| {
                    serde_json::json!({
                        "permission": rule.permission,
                        "decision": rule.decision,
                        "expires_at": rule.expires_at,
                        "updated_at": rule.updated_at
                    })
                })
                .collect::<Vec<_>>()
        };
        let mut blocking = self.ipc_blocking_status(&Value::Null)?;
        if private {
            if let Some(blocking) = blocking.as_object_mut() {
                blocking.insert("active_site_blocked_requests".into(), Value::from(0));
                blocking.insert("active_site_explanation".into(), serde_json::json!({}));
                blocking.insert("active_site_decisions".into(), serde_json::json!([]));
            }
        }
        let userscripts_available = if private || origin.is_none() {
            false
        } else {
            self.rust().userscript_roots.as_ref().is_some_and(|roots| {
                userscript::matching_page_scripts(&roots.config, &current_site_url, false)
                    .is_ok_and(|scripts| !scripts.is_empty())
            })
        };
        let userscript_inventory = if private {
            serde_json::json!({
                "status": "unavailable",
                "reason": "private-session userscript metadata is withheld",
                "provenance": "privacy-boundary"
            })
        } else if let Some(roots) = self.rust().userscript_roots.as_ref() {
            match userscript::installed_scripts(&roots.config) {
                Ok(installed) => serde_json::json!({
                    "status": "available",
                    "installed": installed,
                    "matching_active_site": userscripts_available,
                    "provenance": "profile-userscript-manifests"
                }),
                Err(reason) => serde_json::json!({
                    "status": "unavailable",
                    "reason": bounded_navigation_failure_detail(&reason),
                    "provenance": "profile-userscript-manifests"
                }),
            }
        } else {
            serde_json::json!({
                "status": "not-configured",
                "installed": [],
                "matching_active_site": false,
                "provenance": "profile-userscript-manifests"
            })
        };
        let compatibility = compatibility::diagnostic_snapshot_for_host_and_engine(
            origin.as_deref().and_then(safe_site_host).as_deref(),
            Some(qt_webengine_version().as_str()),
        );
        let mut site_doctor_experiments = Vec::new();
        if !private && self.rust().blocking_enabled && origin.is_some() {
            site_doctor_experiments.push("blocking-bypass");
        }
        if userscripts_available {
            site_doctor_experiments.push("userscripts-off");
        }
        let compiled_defaults_available = if private || origin.is_none() {
            false
        } else {
            serde_json::from_value::<Config>(self.rust().config.clone()).is_ok_and(|config| {
                matching_site_rules(&config, &current_site_url)
                    .iter()
                    .any(|rule| rule.set.keys().any(|key| setting_supports_site_scope(key)))
            })
        };
        if compiled_defaults_available {
            site_doctor_experiments.push("compiled-defaults");
        }
        if !private && origin.is_some() {
            site_doctor_experiments.push("fresh-view");
        }
        let active_site_experiment = self
            .rust()
            .active_site_experiment
            .as_ref()
            .map(|experiment| {
                serde_json::json!({
                    "id": experiment.id,
                    "kind": experiment.kind,
                    "tab": experiment.tab.to_string(),
                    "temporary_tab": experiment.temporary_tab.map(|tab| tab.to_string()),
                    "origin": experiment.origin,
                    "url": experiment.url,
                    "state": "pending",
                    "timeout_seconds": SITE_DOCTOR_EXPERIMENT_TIMEOUT.as_secs(),
                    "remaining_seconds": site_doctor_remaining_seconds(experiment.created_at, Instant::now())
                        .unwrap_or(0),
                })
            })
            .unwrap_or(Value::Null);
        let mut diagnostics = diagnostics::snapshot_with_primary_selection(Some(
            qobject::ferric_browser_primary_selection_available(),
        ));
        apply_qt_runtime_facts(&mut diagnostics);
        let facts = vec![
            serde_json::json!({
                "id": "network.blocking",
                "value": blocking,
                "provenance": "profile/runtime",
                "scope": "profile/site",
                "apply_time": "live",
                "state": "active",
                "capability": "available",
                "remediation_actions": ["blocking-toggle", "blocklist-update"]
            }),
            serde_json::json!({
                "id": "permissions",
                "value": {
                    "durable_rules": permission_rules,
                    "ready": permission_metadata_ready,
                    "session_grants": [],
                    "pending_requests": []
                },
                "provenance": "permission-store/runtime",
                "scope": "profile/origin",
                "apply_time": "live",
                "state": "active",
                "capability": "available",
                "remediation_actions": ["permission-reset"]
            }),
            serde_json::json!({
                "id": "engine.webengine",
                "value": diagnostics["capabilities"]["webengine"].clone(),
                "provenance": "engine-observation",
                "scope": "application",
                "apply_time": "compiled",
                "state": "active",
                "capability": "available",
                "remediation_actions": ["diagnostics"]
            }),
            serde_json::json!({
                "id": "capture",
                "value": {
                    "status": "unavailable",
                    "reason": "active capture is not exposed by the pinned public Qt adapter",
                    "provenance": "not-probed"
                },
                "provenance": "engine-observation",
                "scope": "tab/document",
                "apply_time": "live",
                "state": "unknown",
                "capability": "unavailable",
                "remediation_actions": []
            }),
            serde_json::json!({
                "id": "page.userscripts",
                "value": userscript_inventory,
                "provenance": "runtime",
                "scope": "profile/site",
                "apply_time": "live",
                "state": "unknown",
                "capability": "unknown",
                "remediation_actions": []
            }),
            serde_json::json!({
                "id": "compatibility.workarounds",
                "value": compatibility,
                "provenance": "compiled-reviewed-data",
                "scope": "application/site",
                "apply_time": "compiled",
                "state": "active",
                "capability": "available",
                "remediation_actions": ["diagnostics"]
            }),
        ];
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "private": private,
            "url": if private { Value::Null } else { Value::String(tab.and_then(|tab| tab.url.as_deref()).map_or_else(|| "about:blank".into(), safe_ipc_url)) },
            "origin": if private { Value::Null } else { origin.as_ref().map_or(Value::Null, |origin| Value::String(origin.clone())) },
            "title": if private { Value::Null } else { Value::String(tab.map_or_else(String::new, |tab| tab.title.replace(['\n', '\r'], " "))) },
            "capture": {
                "tab_id": tab.map(|tab| tab.id.to_string()),
                "window_id": tab.map(|tab| tab.window.to_string()),
                "document_id": tab.map(|tab| tab.document.to_string()),
                "generation": tab.map(|tab| tab.generation),
                "profile_id": if private { Value::Null } else { tab.map_or(Value::Null, |tab| Value::String(tab.profile.to_string())) }
            },
            "profile": {
                "id": if private { Value::Null } else { tab.map_or(Value::Null, |tab| Value::String(tab.profile.to_string())) },
                "label": if private { Value::String("Private".into()) } else { profile.map_or(Value::Null, |profile| Value::String(profile.label.clone())) },
                "privacy": if private { "private" } else { "normal" }
            },
            "context": window.and_then(|window| window.context.clone()).map_or(Value::Null, Value::String),
            "loading": tab.map(|tab| format!("{:?}", tab.loading).to_ascii_lowercase()),
            "renderer": tab.map(|tab| format!("{:?}", tab.renderer).to_ascii_lowercase()),
            "blocking": blocking,
            "facts": facts,
            "safe_remediation_actions": ["blocking-toggle", "blocklist-update", "permission-reset", "diagnostics"],
            "site_doctor": {
                "available": !private && origin.is_some(),
                "experiments": site_doctor_experiments,
                "active": if private {
                    Value::Null
                } else {
                    active_site_experiment
                },
                "last_result": if private {
                    Value::Null
                } else {
                    self.rust()
                        .last_site_doctor_result
                        .clone()
                        .unwrap_or(Value::Null)
                },
                "reason": if private {
                    "private sessions do not expose durable site experiments"
                } else if origin.is_none() {
                    "the active document has no eligible HTTP(S) origin"
                } else if !self.rust().blocking_enabled && !userscripts_available {
                    "one-shot fresh same-profile view is available"
                } else if !userscripts_available {
                    "one-shot blocker and fresh-view experiments are available"
                } else {
                    "one-shot blocker, userscript, and fresh-view experiments are available"
                }
            }
        }))
    }

    pub(super) fn ipc_site_report(&self, include_host: bool) -> Result<Value, String> {
        let ledger = self.ipc_site_status(&Value::Null)?;
        let private = ledger
            .get("private")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let origin_host = if include_host && !private {
            ledger
                .get("origin")
                .and_then(Value::as_str)
                .and_then(safe_site_host)
                .map_or(Value::Null, Value::String)
        } else {
            Value::Null
        };
        let facts = ledger
            .get("facts")
            .and_then(Value::as_array)
            .map(|facts| {
                facts
                    .iter()
                    .filter_map(|fact| {
                        let object = fact.as_object()?;
                        Some(serde_json::json!({
                            "id": object.get("id")?,
                            "state": object.get("state")?,
                            "capability": object.get("capability")?,
                            "provenance": object.get("provenance")?,
                            "scope": object.get("scope")?,
                            "apply_time": object.get("apply_time")?,
                            "status": object.get("value").and_then(Value::as_object).and_then(|value| value.get("status")).cloned().unwrap_or(Value::Null)
                        }))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let blocking = ledger
            .get("blocking")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let active_decisions = blocking
            .get("active_site_decisions")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let has_security_deny_decision = blocking
            .get("active_site_decisions")
            .and_then(Value::as_array)
            .is_some_and(|decisions| {
                decisions.iter().any(|decision| {
                    decision.get("reason").and_then(Value::as_str) == Some("security deny rule")
                })
            });
        let security_deny_rules = security_deny_rule_count(&self.rust().config_json.to_string());
        let mut matched_rule_ids = Vec::new();
        if active_decisions > 0 {
            matched_rule_ids.push("blocking.host-rule");
        }
        if has_security_deny_decision {
            matched_rule_ids.push("blocking.security-deny-host");
        }
        let loaded_lists = blocking
            .get("loaded_lists")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let active_experiment = ledger
            .get("site_doctor")
            .and_then(Value::as_object)
            .and_then(|doctor| doctor.get("active"))
            .filter(|active| !active.is_null())
            .cloned()
            .unwrap_or(Value::Null);
        let last_experiment_result = ledger
            .get("site_doctor")
            .and_then(Value::as_object)
            .and_then(|doctor| doctor.get("last_result"))
            .cloned()
            .unwrap_or(Value::Null);
        let mut diagnostics = diagnostics::snapshot_with_primary_selection(Some(
            qobject::ferric_browser_primary_selection_available(),
        ));
        apply_qt_runtime_facts(&mut diagnostics);
        let quirk_ids = compatibility::load()
            .ok()
            .map(|registry| {
                compatibility::active_ids(
                    &registry,
                    ledger
                        .get("origin")
                        .and_then(Value::as_str)
                        .and_then(safe_site_host)
                        .as_deref(),
                    None,
                )
            })
            .unwrap_or_default();
        Ok(serde_json::json!({
            "schema": 1,
            "correlation_id": format!("site-report-{}", Uuid::new_v4().simple()),
            "application": diagnostics["application"].clone(),
            "engine": diagnostics["capabilities"]["webengine"].clone(),
            "platform": {
                "display": diagnostics["runtime"]["display"].clone(),
                "compositor": diagnostics["runtime"]["compositor"].clone(),
                "graphics_backend": diagnostics["runtime"]["graphics_backend"].clone(),
                "software_rendering": diagnostics["runtime"]["software_rendering"].clone()
            },
            "site": {
                "private": private,
                "host": origin_host
            },
            "facts": facts,
            "blocking": {
                "enabled": blocking.get("enabled").cloned().unwrap_or(Value::Null),
                "security_deny_rules": security_deny_rules,
                "loaded_list_ids": loaded_lists,
                "list_metadata": blocking
                    .get("list_metadata")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!([])),
                "active_decision_count": active_decisions,
                "matched_rule_ids": matched_rule_ids
            },
            "quirk_ids": quirk_ids,
            "experiment": {
                "active": active_experiment,
                "last_result": last_experiment_result
            },
            "excluded": [
                "cookies", "tokens", "form_text", "dom", "full_request_urls",
                "account_identifiers", "profile_names", "private_session_data"
            ]
        }))
    }

    pub(super) fn ipc_blocking_status(&self, params: &Value) -> Result<Value, String> {
        query_empty_object_or_null(params, "blocking.status")?;
        let list_value = |value: &QStringList| {
            Value::Array(
                value
                    .iter()
                    .map(|entry| Value::String(entry.to_string()))
                    .collect(),
            )
        };
        let json_array = |value: &QString| {
            serde_json::from_str::<Value>(&value.to_string())
                .ok()
                .filter(Value::is_array)
                .unwrap_or_else(|| Value::Array(Vec::new()))
        };
        let blocked_rules = list_value(&self.rust().blocking_hosts);
        let exception_rules = list_value(&self.rust().blocking_exceptions);
        let (active_explanation, active_decisions) =
            self.rust().blocking_active_evidence.json_values();
        let security_deny_rules = security_deny_rule_count(&self.rust().config_json.to_string());
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "enabled": self.rust().blocking_enabled,
            "status": self.rust().status_text.to_string(),
            "bypass_sites": list_value(&self.rust().blocking_bypass_sites),
            "loaded_lists": json_array(&self.rust().blocking_loaded_lists),
            "list_metadata": json_array(&self.rust().blocking_list_metadata),
            "skipped_lists": json_array(&self.rust().blocking_skipped_lists),
            "blocked_rules": blocked_rules.as_array().map_or(0, Vec::len),
            "exception_rules": exception_rules.as_array().map_or(0, Vec::len),
            "blocked_requests": self.rust().blocking_blocked_count.max(0),
            "active_site_blocked_requests": self.rust().blocking_active_site_count.max(0),
            "unknown_context_requests": self.rust().blocking_unknown_context_count.max(0),
            "security_deny_rules": security_deny_rules,
            "active_site_explanation": active_explanation,
            "active_site_decisions": active_decisions
        }))
    }

    pub(super) fn blocking_status_message(&self) -> String {
        let Ok(status) = self.ipc_blocking_status(&Value::Null) else {
            return "Blocking status unavailable".into();
        };
        let enabled = status
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let loaded = status
            .get("loaded_lists")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let skipped = status
            .get("skipped_lists")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let rules = status
            .get("blocked_rules")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let blocked = status
            .get("blocked_requests")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let security_deny_rules = status
            .get("security_deny_rules")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let explanation = status
            .get("active_site_explanation")
            .and_then(Value::as_object)
            .and_then(|value| {
                Some(format!(
                    " · last blocked {} by {}",
                    value.get("resource_host")?.as_str()?,
                    value.get("matched_rule")?.as_str()?
                ))
            })
            .unwrap_or_default();
        format!(
            "Blocking {} · {rules} rules · {security_deny_rules} security-deny rules · {blocked} blocked · {loaded} lists loaded · {skipped} skipped{explanation}",
            if enabled { "on" } else { "off" }
        )
    }

    pub(super) fn ipc_operations_query(&self, params: &Value) -> Result<Value, String> {
        operations_query_value(self.rust(), params)
    }

    pub(super) fn ipc_operation_cancel(
        mut self: Pin<&mut Self>,
        params: &Value,
    ) -> Result<Value, String> {
        let object = params
            .as_object()
            .ok_or_else(|| "operations.cancel params must be an object".to_owned())?;
        let id = object
            .get("operation_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| "operations.cancel requires operation_id".to_owned())?;
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if !this.operation_states.contains_key(id) {
            return Err("operation was not found".into());
        }
        if this
            .operation_states
            .get(id)
            .is_some_and(|status| operation_is_terminal(status))
        {
            return Err("operation is no longer cancellable".into());
        }
        if let Ok(values) = this.userscript_cancellations.lock()
            && let Some(cancellation) = values.get(id)
        {
            cancellation.store(true, Ordering::Release);
        }
        if let Ok(values) = this.spawn_cancellations.lock()
            && let Some(cancellation) = values.get(id)
        {
            cancellation.store(true, Ordering::Release);
        }
        if this
            .pending_userscript
            .as_ref()
            .is_some_and(|pending| pending.operation_id == id)
        {
            this.pending_userscript = None;
            this.pending_selection = None;
        }
        if this
            .pending_selection
            .as_ref()
            .and_then(|pending| pending.operation_id.as_deref())
            == Some(id)
        {
            finish_pending_selection_operation(this, "cancelled");
        }
        if let Some(status) = this.operation_states.get_mut(id) {
            *status = "cancelled".into();
        }
        Ok(serde_json::json!({"operation_id": id, "status": "cancelled"}))
    }

    #[allow(clippy::unused_self)]
    pub(super) fn ipc_diagnostics(&self, params: &Value) -> Result<Value, String> {
        query_empty_object_or_null(params, "diagnostics.get")?;
        let mut result = diagnostics::snapshot_with_primary_selection(Some(
            qobject::ferric_browser_primary_selection_available(),
        ));
        apply_qt_runtime_facts(&mut result);
        result["crash"] = self.rust().storage_roots.as_ref().map_or_else(
            || {
                serde_json::json!({
                    "schema": 1,
                    "marker": {
                        "status": "unavailable",
                        "reason": "the active profile has no durable storage root",
                        "provenance": "not-probed"
                    }
                })
            },
            crash_diagnostics,
        );
        result["protocol"] = serde_json::json!({
            "major": ferric_browser_ipc::PROTOCOL_MAJOR,
            "minor": ferric_browser_ipc::PROTOCOL_MINOR
        });
        result["private_state"] = serde_json::json!("memory-only");
        result["macros"] = macro_state_value(self.rust());
        result["action_audit"] = serde_json::json!({
            "status": "available",
            "records": &self.rust().action_audit,
            "limit": MAX_ACTION_AUDIT_RECORDS,
            "fields": "action_id, operation_id, outcome, and redacted category only"
        });
        result["recent_errors"] = action_error_summary(&self.rust().action_audit);
        result["error_correlations"] = serde_json::json!({
            "status": "available",
            "records": ferric_browser_ipc::recent_error_correlations(),
            "limit": 32,
            "fields": "correlation_id and stable error code only",
            "provenance": "process-memory"
        });
        result["request_resolutions"] = serde_json::json!({
            "status": "available",
            "counts": self
                .rust()
                .request_resolution_counts
                .iter()
                .map(|((kind, outcome), count)| {
                    serde_json::json!({
                        "kind": kind,
                        "outcome": outcome,
                        "count": count
                    })
                })
                .collect::<Vec<_>>(),
            "reason": "bounded Qt request resolution outcomes; stale and duplicate responses are no-ops"
        });
        if let Ok(blocking) = self.ipc_blocking_status(&Value::Null) {
            let loaded_lists = blocking
                .get("loaded_lists")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            let skipped_lists = blocking
                .get("skipped_lists")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            let blocked_rules = blocking
                .get("blocked_rules")
                .and_then(Value::as_u64)
                .map_or(0, |value| usize::try_from(value).unwrap_or(usize::MAX));
            result["capabilities"]["network_blocking"] = diagnostics::network_blocking_fact(
                blocking
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                loaded_lists,
                skipped_lists,
                blocked_rules,
            );
        }
        result["link_cleaning"] =
            link_cleaning_policy::diagnostic_snapshot(self.rust().storage_roots.as_ref());
        let config =
            serde_json::from_value::<Config>(self.rust().config.clone()).unwrap_or_default();
        let private_profile = self
            .rust()
            .state
            .as_ref()
            .and_then(|state| {
                let tab = state.active_tab()?;
                let profile = state.profiles().get(&tab.profile)?;
                Some(profile.privacy.is_transient())
            })
            .unwrap_or(false);
        result["maintenance_traffic"] = maintenance::snapshot(
            Some(&self.rust().config),
            !private_profile,
            self.rust().storage_roots.is_some(),
        );
        let contrast = load_theme_palette(&config.theme)
            .map(|palette| serde_json::to_value(theme_contrast_report(&palette)))
            .ok()
            .and_then(Result::ok)
            .unwrap_or_else(|| {
                serde_json::json!({
                    "status": "unknown",
                    "checks": [],
                    "failing": [],
                    "reason": "the configured theme palette could not be assessed"
                })
            });
        result["theme"] = serde_json::json!({
            "contrast": contrast,
            "user_theme_remains_importable": true,
            "security_surfaces": "opaque semantic surfaces retain readable text requirements"
        });
        if let (Some(roots), Some(profile_id)) =
            (self.rust().storage_roots.as_ref(), self.rust().profile_id)
        {
            result["storage"]["health"] = diagnostics::storage_health(
                roots
                    .data
                    .join("profiles")
                    .join(profile_id.to_string())
                    .join("browser.sqlite"),
            );
        }
        Ok(result)
    }

    pub(super) fn ipc_events_subscribe(&self, params: &Value) -> Result<Value, String> {
        Self::ipc_event_filter(params)?;
        Ok(serde_json::json!({
            "status": "subscribed",
            "sequence": self.rust().ipc_sequence
        }))
    }

    pub(super) fn ipc_event_filter(params: &Value) -> Result<Option<Vec<String>>, String> {
        let object = params
            .as_object()
            .ok_or_else(|| "events.subscribe params must be an object".to_owned())?;
        if object.keys().any(|key| key != "event_types") {
            return Err("events.subscribe contains an unknown field".into());
        }
        if let Some(event_types) = object.get("event_types") {
            let types = event_types
                .as_array()
                .ok_or_else(|| "event_types must be an array".to_owned())?;
            if types.len() > 32
                || types.iter().any(|event_type| {
                    event_type.as_str().is_none_or(|event_type| {
                        event_type.is_empty()
                            || event_type.len() > 128
                            || event_type.chars().any(char::is_control)
                    })
                })
            {
                return Err(
                    "event_types must contain at most 32 nonempty strings without control characters"
                        .into(),
                );
            }
            return Ok((!types.is_empty()).then(|| {
                types
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect()
            }));
        }
        Ok(None)
    }
}
