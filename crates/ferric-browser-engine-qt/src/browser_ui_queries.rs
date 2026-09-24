//! Read-only IPC queries and diagnostic projections for `BrowserUi`.

use super::{
    ActionDefinition, ActionRegistry, ActionSource, CommandSource, Config, CxxQtType,
    ExistenceState, Value, configured_action_target_supports_subject,
    configured_action_target_values, configured_action_targets, hyprland, live_document_available,
    parse_external_action_id, qobject, query_empty_object_or_null, userscript_action_values,
};

impl qobject::BrowserUi {
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
}
