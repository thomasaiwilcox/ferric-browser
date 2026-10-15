//! Binding and completion query projections.

#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn binding_explanation_status(value: &Value) -> String {
    let routing = value.get("mixed_routing").unwrap_or(&Value::Null);
    let owner = routing
        .get("owner")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let reason = routing
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let resolution = value
        .get("resolution")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let source = routing
        .get("shortcut_policy_source")
        .and_then(Value::as_str)
        .unwrap_or("default");
    let site_rule = routing
        .get("shortcut_policy_site_rule")
        .and_then(Value::as_str)
        .or_else(|| routing.get("page_keys_site_rule").and_then(Value::as_str));
    let qml_special = routing
        .get("qml_special")
        .and_then(Value::as_str)
        .unwrap_or("none");
    format!(
        "Key routing: {owner} ({reason}); binding {resolution}; policy {source}{}; QML {qml_special}",
        site_rule.map_or_else(String::new, |rule| format!("; site rule {rule}"))
    )
}

pub(super) fn binding_command_status(command: &str) -> &'static str {
    if command == "binding-list" {
        "Binding list requested"
    } else {
        "Binding explanation requested"
    }
}

impl qobject::BrowserUi {
    pub(super) fn binding_explanation_feedback(
        &self,
        keychain: Option<&str>,
        mode: Option<&str>,
    ) -> Option<String> {
        let keychain = keychain?;
        let explanation = self
            .ipc_bindings_explain(&serde_json::json!({"keychain": keychain, "mode": mode}))
            .ok()?;
        Some(binding_explanation_status(&explanation))
    }

    pub(super) fn set_binding_explanation_status(mut self: Pin<&mut Self>, explanation: &Value) {
        self.as_mut()
            .set_status_text(QString::from(binding_explanation_status(explanation)));
    }
}

fn parse_explain_keychain(keychain: &str) -> Result<(u32, bool, Vec<String>), String> {
    let is_single_logical_key = ferric_browser_core::LogicalKeyToken::parse(keychain).is_ok();
    let (count_text, remaining) = if is_single_logical_key {
        (String::new(), keychain.to_owned())
    } else {
        let mut characters = keychain.chars().peekable();
        let mut count_text = String::new();
        if characters
            .peek()
            .is_some_and(|key| ('1'..='9').contains(key))
        {
            while characters.peek().is_some_and(char::is_ascii_digit) {
                count_text.push(characters.next().expect("peeked count digit"));
            }
        }
        let remaining = characters.collect::<String>();
        if remaining.is_empty() {
            (String::new(), keychain.to_owned())
        } else {
            (count_text, remaining)
        }
    };

    let tokens = ferric_browser_core::binding_key_tokens(&remaining).ok_or_else(|| {
        "bindings.explain keychain must contain canonical binding keys".to_owned()
    })?;
    let count = if count_text.is_empty() {
        1
    } else {
        count_text.parse::<u32>().unwrap_or(9_999).clamp(1, 9_999)
    };
    Ok((count, !count_text.is_empty(), tokens))
}

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

        let (count, explicit_count, tokens) = parse_explain_keychain(keychain)?;

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
        let route_token = tokens
            .first()
            .and_then(|key| ferric_browser_core::LogicalKeyToken::parse(key).ok());
        let routing_decision = route_token.as_ref().and_then(|token| {
            super::mixed_policy_runtime::decision_for_key(self.rust(), mode, token.as_str())
        });
        let mixed_reason = routing_decision.map(|decision| match decision.reason {
            ferric_browser_core::MixedKeyDecisionReason::ExplicitMode => "explicit-mode",
            ferric_browser_core::MixedKeyDecisionReason::ReservedControl => "reserved-control",
            ferric_browser_core::MixedKeyDecisionReason::ActiveBrowserCount => "active-count",
            ferric_browser_core::MixedKeyDecisionReason::ActiveBrowserKeychain => "active-keychain",
            ferric_browser_core::MixedKeyDecisionReason::BrowserFirstPolicy => "browser-first",
            ferric_browser_core::MixedKeyDecisionReason::ExactPageKey => "exact-page-key",
            ferric_browser_core::MixedKeyDecisionReason::NoPageKeyMapping => "unmapped-key",
        });
        let policy = &self.rust().mixed_policy;
        let site_rule_config = serde_json::from_value::<Config>(self.rust().config.clone()).ok();
        let site_rule_for =
            |setting: &str, source: Option<ferric_browser_core::MixedPolicySource>| {
                if source != Some(ferric_browser_core::MixedPolicySource::Site) {
                    return None;
                }
                let config = site_rule_config.as_ref()?;
                matching_site_rules(config, &self.rust().current_url.to_string())
                    .into_iter()
                    .rev()
                    .find(|rule| rule.set.contains_key(setting))
                    .map(|rule| rule.id.clone())
            };
        let source_for =
            |setting: &str, source: Option<ferric_browser_core::MixedPolicySource>| match source {
                Some(ferric_browser_core::MixedPolicySource::Global) => Some("global".into()),
                Some(ferric_browser_core::MixedPolicySource::Profile) => Some("profile".into()),
                Some(ferric_browser_core::MixedPolicySource::Site) => {
                    site_rule_for(setting, source).map(|id| format!("site-rule:{id}"))
                }
                Some(ferric_browser_core::MixedPolicySource::Runtime) => {
                    let rust = self.rust();
                    [
                        ("temporary", &rust.temporary_overrides),
                        ("cli", &rust.cli_overrides),
                        ("runtime", &rust.runtime_overrides),
                    ]
                    .into_iter()
                    .find(|(_, overrides)| overrides.settings.contains_key(setting))
                    .map(|(layer, _)| layer.into())
                }
                None => None,
            };
        let suppresses_bindings = if mode == Mode::Normal
            && routing_decision
                .is_some_and(|decision| decision.owner == ferric_browser_core::InputOwner::Page)
        {
            let first = route_token
                .as_ref()
                .map(ferric_browser_core::LogicalKeyToken::as_str);
            definitions
                .iter()
                .filter(|binding| {
                    binding.mode == Mode::Normal
                        && binding.keys.first().map(String::as_str) == first
                })
                .map(|binding| serde_json::json!({
                    "keychain": binding.keys.concat(),
                    "command": binding.command,
                    "reason": "the first key is routed to the page before this browser binding can start"
                }))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let qml_special = route_token
            .as_ref()
            .map_or("none", |token| match token.as_str() {
                ":" => "command-entry",
                "/" => "search-forward",
                "?" => "search-backward",
                _ => "none",
            });

        Ok(serde_json::json!({
            "schema": 1,
            "keychain": keychain,
            "tokens": tokens,
            "requested_mode": object.get("mode").cloned().unwrap_or(Value::Null),
            "current_mode": ipc_mode_name(current_mode),
            "mode": mode_name,
            "count": {
                "explicit": explicit_count,
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
            "mixed_routing": {
                "owner": routing_decision.map(|decision| match decision.owner {
                    ferric_browser_core::InputOwner::Browser => "browser",
                    ferric_browser_core::InputOwner::Page => "page",
                }),
                "reason": mixed_reason,
                "shortcut_policy": policy.shortcut_policy.as_str(),
                "shortcut_policy_source": source_for(
                    "input.shortcut_policy",
                    policy.shortcut_policy_source
                ),
                "shortcut_policy_source_layer": policy.shortcut_policy_source.map(|source| source.to_string()),
                "shortcut_policy_site_rule": site_rule_for(
                    "input.shortcut_policy",
                    policy.shortcut_policy_source
                ),
                "page_keys_source": source_for("input.page_keys", policy.page_keys_source),
                "page_keys_source_layer": policy.page_keys_source.map(|source| source.to_string()),
                "page_keys_site_rule": site_rule_for("input.page_keys", policy.page_keys_source),
                "reserved": routing_decision.is_some_and(|decision|
                    decision.reason == ferric_browser_core::MixedKeyDecisionReason::ReservedControl
                ),
                "qml_special": qml_special,
                "browser_bindings_suppressed_by_page_mapping": suppresses_bindings
            },
            "reserved": {
                "escape": true,
                "chord": "Ctrl+Shift+Escape",
                "note": "Escape handling remains browser-reserved and is not configurable through the binding trie"
            }
        }))
    }
}

#[cfg(test)]
mod mixed_explain_keychain_tests {
    use super::{binding_command_status, binding_explanation_status, parse_explain_keychain};

    #[test]
    fn keychain_explanation_accepts_logical_tokens_and_count_prefixes() {
        assert_eq!(
            parse_explain_keychain("Ctrl+f").expect("named key"),
            (1, false, vec!["Ctrl+f".to_owned()])
        );
        assert_eq!(
            parse_explain_keychain("3").expect("mapped count-like key"),
            (1, false, vec!["3".to_owned()])
        );
        assert_eq!(
            parse_explain_keychain("3gg").expect("counted chain"),
            (3, true, vec!["g".to_owned(), "g".to_owned()])
        );
        assert_eq!(
            parse_explain_keychain("g,g").expect("comma-separated chain"),
            (1, false, vec!["g".to_owned(), "g".to_owned()])
        );
    }

    #[test]
    fn local_explanation_feedback_reports_routing_without_retaining_the_token() {
        let explanation = serde_json::json!({
            "keychain": "private-token",
            "resolution": "exact-prefix",
            "mixed_routing": {
                "owner": "page",
                "reason": "exact-page-key",
                "shortcut_policy_source": "site-rule:docs-shortcuts",
                "shortcut_policy_site_rule": "docs-shortcuts",
                "qml_special": "none"
            }
        });
        let status = binding_explanation_status(&explanation);
        assert!(status.contains("page (exact-page-key)"));
        assert!(status.contains("exact-prefix"));
        assert!(status.contains("docs-shortcuts"));
        assert!(!status.contains("private-token"));
        assert_eq!(
            binding_command_status("binding-list"),
            "Binding list requested"
        );
    }
}
