//! Binding and completion query projections.

#[allow(clippy::wildcard_imports)]
use super::*;

impl qobject::BrowserUi {
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
}
