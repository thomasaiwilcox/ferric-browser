use super::{ConfigurationLayers, CxxQtType, ParsedCommand, Pin, Value, qobject};

impl qobject::BrowserUi {
    pub(super) fn execute_runtime_config_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let mut temporary = false;
        let mut pattern = None;
        let mut positional = Vec::new();
        let mut arguments = command.arguments.iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--temp" if !temporary => temporary = true,
                "--pattern" if pattern.is_none() => {
                    let value = arguments
                        .next()
                        .ok_or_else(|| "--pattern requires a value".to_owned())?;
                    pattern = Some(value.as_str());
                }
                _ => positional.push(argument.as_str()),
            }
        }
        let (key, literal) = if command.name == "set" {
            let [setting] = positional.as_slice() else {
                return Err("set accepts [--temp] [--pattern PATTERN] KEY=VALUE".into());
            };
            let (key, literal) = setting
                .split_once('=')
                .ok_or_else(|| "set requires KEY=VALUE".to_owned())?;
            (key, Some(literal))
        } else {
            let [key] = positional.as_slice() else {
                return Err("unset accepts [--temp] [--pattern PATTERN] KEY".into());
            };
            (*key, None)
        };
        if key.is_empty() || key.len() > 256 || key.chars().any(char::is_control) {
            return Err("configuration key is empty, too long, or unsafe".into());
        }

        let mut persistent = self.as_ref().rust().runtime_overrides.clone();
        let mut temporary_layer = self.as_ref().rust().temporary_overrides.clone();
        let changed = if command.name == "set" {
            let literal = literal.ok_or_else(|| "set requires KEY=VALUE".to_owned())?;
            let layer = if temporary {
                &mut temporary_layer
            } else {
                &mut persistent
            };
            if let Some(pattern) = pattern {
                layer
                    .set_site_literal(pattern, key, literal)
                    .map_err(|error| error.to_string())?;
            } else {
                layer
                    .set_literal(key, literal)
                    .map_err(|error| error.to_string())?;
            }
            true
        } else if temporary {
            if let Some(pattern) = pattern {
                temporary_layer
                    .unset_site_setting(pattern, key)
                    .map_err(|error| error.to_string())?
            } else {
                temporary_layer
                    .unset_setting(key)
                    .map_err(|error| error.to_string())?
            }
        } else {
            if let Some(pattern) = pattern {
                persistent
                    .unset_site_setting(pattern, key)
                    .map_err(|error| error.to_string())?
            } else {
                persistent
                    .unset_setting(key)
                    .map_err(|error| error.to_string())?
            }
        };
        let candidate = self
            .as_ref()
            .runtime_config_candidate(&persistent, &temporary_layer)?;
        let persistent_storage = !temporary
            && self.as_ref().rust().profile_persistence.is_durable()
            && self.as_ref().rust().storage_roots.is_some();
        let persist_path = if persistent_storage {
            Some(
                self.as_ref()
                    .rust()
                    .storage_roots
                    .as_ref()
                    .map(|roots| roots.state.join("runtime-overrides.toml"))
                    .ok_or_else(|| "runtime storage is unavailable".to_owned())?,
            )
        } else {
            None
        };
        let layers = ConfigurationLayers {
            base: serde_json::from_value(self.as_ref().rust().base_config.clone())
                .map_err(|error| format!("base configuration is invalid: {error}"))?,
            profile: self.as_ref().rust().profile_overrides.clone(),
            runtime: persistent.clone(),
            command_line: self.as_ref().rust().cli_overrides.clone(),
            temporary: temporary_layer.clone(),
        };
        let scope = if temporary {
            "temporary"
        } else if persistent_storage {
            "persistent"
        } else {
            "memory-only"
        };
        let action = if command.name == "set" {
            "Set"
        } else {
            "Unset"
        };
        let detail = if changed {
            ""
        } else {
            " (no matching override)"
        };
        let status = pattern.map_or_else(
            || format!("{action} {key} ({scope}){detail}"),
            |pattern| format!("{action} {key} for {pattern} ({scope}){detail}"),
        );
        let Some(result) = self.as_mut().validate_or_queue_runtime_config(
            candidate,
            layers,
            status,
            persistent.clone(),
            persist_path,
        )?
        else {
            return Ok(serde_json::json!({
                "status": "pending-validation",
                "key": key,
                "pattern": pattern,
                "temporary": temporary,
                "changed": changed,
                "scope": scope
            }));
        };
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.runtime_overrides = persistent;
            this.temporary_overrides = temporary_layer;
        }
        let value = key
            .split('.')
            .try_fold(&result, |value, part| value.as_object()?.get(part))
            .cloned()
            .unwrap_or(Value::Null);
        Ok(serde_json::json!({
            "status": "accepted",
            "key": key,
            "pattern": pattern,
            "temporary": temporary,
            "changed": changed,
            "value": value
        }))
    }
}
