use super::{CxxQtType, ParsedCommand, Pin, Value, qobject};

impl qobject::BrowserUi {
    pub(super) fn execute_runtime_config_command(
        self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.queue_configuration_preparation(
            super::browser_ui_configuration_preparation::DeferredConfigurationAction::Mutation(
                command.clone(),
            ),
            None,
            &mut None,
        )
    }

    pub(super) fn prepare_runtime_config_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        preparation: Option<
            &super::browser_ui_configuration_preparation::ConfigurationPreparationTurn,
        >,
    ) -> Result<Value, String> {
        self.as_mut()
            .prepare_runtime_config_command_with_post_action(command, preparation, None)
    }

    pub(super) fn prepare_runtime_config_command_with_post_action(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        preparation: Option<
            &super::browser_ui_configuration_preparation::ConfigurationPreparationTurn,
        >,
        post_action: Option<super::site_doctor_completion::SiteDoctorCompletion>,
    ) -> Result<Value, String> {
        if preparation.is_some_and(|turn| !turn.is_current(self.as_ref().get_ref())) {
            return Err("The settings owner changed during preparation".into());
        }
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

        let (saved, mut layers) = {
            let browser = self.as_ref();
            let state = browser
                .rust()
                .state
                .as_ref()
                .ok_or_else(|| "application state is unavailable".to_owned())?;
            let saved = if !temporary && state.profile_snapshot().is_some() {
                Some(
                    state
                        .capture_saved_runtime_edits()
                        .map_err(|error| error.to_string())?,
                )
            } else {
                None
            };
            let layers = saved.as_ref().map_or_else(
                || state.configuration_layers().clone(),
                |capture| capture.layers().clone(),
            );
            (saved, layers)
        };
        let mut persistent = layers.runtime.clone();
        let mut temporary_layer = layers.temporary.clone();
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
        layers.runtime = persistent;
        layers.temporary = temporary_layer;
        let prepared = {
            let browser = self.as_ref();
            let state = browser
                .rust()
                .state
                .as_ref()
                .ok_or_else(|| "application state is unavailable".to_owned())?;
            if let Some(saved) = &saved {
                state.prepare_saved_runtime_edits(saved, layers.runtime)
            } else {
                state.prepare_configuration_update(layers)
            }
            .map_err(|error| error.to_string())?
        };
        let persistent_storage = !temporary
            && self.as_ref().rust().profile_persistence.is_durable()
            && self.as_ref().rust().storage_roots.is_some();
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
        let Some(result) = self
            .as_mut()
            .validate_or_queue_runtime_config_with_post_action(
                prepared,
                status,
                persistent_storage,
                preparation.map(|turn| &turn.origin),
                post_action
                    .map(super::configuration_completion::ConfigurationCompletion::SiteDoctor),
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
        if result.get("status").and_then(Value::as_str) == Some("pending-persistence") {
            return Ok(
                serde_json::json!({"status":"pending-persistence", "key": key, "scope":scope}),
            );
        }
        let value = key
            .split('.')
            .try_fold(&result, |value, part| value.as_object()?.get(part))
            .cloned()
            .unwrap_or(Value::Null);
        Ok(serde_json::json!({
            "status": "accepted",
            "persistence": self.as_ref().rust().runtime_config_persistence,
            "key": key,
            "pattern": pattern,
            "temporary": temporary,
            "changed": changed,
            "value": value
        }))
    }
}
