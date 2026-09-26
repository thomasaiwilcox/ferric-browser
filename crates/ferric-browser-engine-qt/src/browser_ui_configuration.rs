use super::{
    ChromePreferences, Config, ConfigurationLayers, CxxQtType, Duration, FeaturePreferences,
    Instant, ParsedCommand, Path, PathBuf, Pin, PortalCapabilities, PortalProbeWorker, QString,
    RuntimeOverrides, Value, chrome_preferences, config_get_command_parameters,
    configured_bindings, desktop_portals, feature_preferences, hyprland, is_bounded_untrusted_text,
    load_theme_palette, matching_site_rules, publish_ipc_event, qobject, safe_ipc_url,
    sanitize_process_stderr, save_runtime_overrides_atomic, setting_metadata,
    settings_presentation, theme_presentation, try_wait_editor_process,
};

impl qobject::BrowserUi {
    pub(super) fn select_site_rule_settings(mut self: Pin<&mut Self>, url: &QString) -> bool {
        let config = match serde_json::from_value::<Config>(self.as_ref().rust().config.clone()) {
            Ok(config) => config,
            Err(error) => {
                self.as_mut()
                    .set_status_text(QString::from(format!("Site rules unavailable: {error}")));
                return false;
            }
        };
        let mut javascript = None;
        let mut images = None;
        let mut force_dark = None;
        let mut autoplay = None;
        let mut zoom = None;
        for rule in matching_site_rules(&config, &url.to_string()) {
            for (key, value) in &rule.set {
                match key.as_str() {
                    "content.javascript" => javascript = value.as_bool(),
                    "content.images" => images = value.as_bool(),
                    "content.force_dark" => force_dark = value.as_bool(),
                    "content.autoplay" => autoplay = value.as_str().map(ToOwned::to_owned),
                    "content.zoom" => {
                        zoom = value.as_float().or_else(|| {
                            value
                                .as_integer()
                                .and_then(|value| i32::try_from(value).ok())
                                .map(f64::from)
                        });
                    }
                    _ => {}
                }
            }
        }
        self.as_mut()
            .set_site_rule_javascript_set(javascript.is_some());
        self.as_mut()
            .set_site_rule_javascript_enabled(javascript.unwrap_or_default());
        self.as_mut().set_site_rule_images_set(images.is_some());
        self.as_mut()
            .set_site_rule_images_enabled(images.unwrap_or_default());
        self.as_mut()
            .set_site_rule_force_dark_set(force_dark.is_some());
        self.as_mut()
            .set_site_rule_force_dark_enabled(force_dark.unwrap_or_default());
        self.as_mut().set_site_rule_autoplay_set(autoplay.is_some());
        self.as_mut()
            .set_site_rule_autoplay(QString::from(autoplay.unwrap_or_default()));
        self.as_mut().set_site_rule_zoom_set(zoom.is_some());
        self.as_mut().set_site_rule_zoom(zoom.unwrap_or_default());
        true
    }

    pub(super) fn clear_focus_observation_state(mut self: Pin<&mut Self>, index: i32) {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.focus_observations
            .retain(|(candidate, _), _| *candidate != index);
        this.focus_suppressions
            .retain(|(candidate, _), _| *candidate != index);
    }

    pub(super) fn suppress_current_focus_observations(mut self: Pin<&mut Self>) {
        let active = self.as_ref().rust().active_tab_index;
        let suppressed = self
            .as_ref()
            .rust()
            .focus_observations
            .iter()
            .filter(|((index, _), _)| *index == active)
            .map(|(key, observation)| (key.clone(), observation.sequence))
            .collect::<Vec<_>>();
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        for (key, sequence) in suppressed {
            this.focus_suppressions.insert(key, sequence);
        }
    }

    pub(super) fn configured_site_entry_mode(&self, url: &str) -> Option<String> {
        if self
            .rust()
            .active_site_experiment
            .as_ref()
            .is_some_and(|experiment| {
                experiment.kind == "compiled-defaults" && safe_ipc_url(url) == experiment.url
            })
        {
            return None;
        }
        let config = serde_json::from_value::<Config>(self.rust().config.clone()).ok()?;
        let mut entry_mode = None;
        for rule in matching_site_rules(&config, url) {
            if let Some(mode) = rule
                .set
                .get("input.entry_mode")
                .and_then(toml::Value::as_str)
            {
                entry_mode = Some(mode.to_owned());
            }
        }
        entry_mode
    }

    pub(super) fn runtime_config_candidate(
        &self,
        runtime_overrides: &RuntimeOverrides,
        temporary_overrides: &RuntimeOverrides,
    ) -> Result<Value, String> {
        self.runtime_config_candidate_from(
            &self.rust().base_config,
            &self.rust().profile_overrides,
            runtime_overrides,
            temporary_overrides,
        )
    }

    pub(super) fn runtime_config_candidate_from(
        &self,
        base_value: &Value,
        profile_overrides: &RuntimeOverrides,
        runtime_overrides: &RuntimeOverrides,
        temporary_overrides: &RuntimeOverrides,
    ) -> Result<Value, String> {
        let layers = ConfigurationLayers {
            base: serde_json::from_value(base_value.clone())
                .map_err(|error| format!("base configuration is invalid: {error}"))?,
            profile: profile_overrides.clone(),
            runtime: runtime_overrides.clone(),
            command_line: self.rust().cli_overrides.clone(),
            temporary: temporary_overrides.clone(),
        };
        serde_json::to_value(layers.resolve().map_err(|error| error.to_string())?)
            .map_err(|error| format!("could not serialize resolved configuration: {error}"))
    }

    pub(super) fn commit_runtime_config(
        mut self: Pin<&mut Self>,
        candidate: Value,
        layers: ConfigurationLayers,
        status: &str,
    ) -> Result<Value, String> {
        let previous_hints = serde_json::from_value::<Config>(self.as_ref().rust().config.clone())
            .ok()
            .map(|config| config.hints);
        let snapshot = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.state
                .as_mut()
                .ok_or_else(|| "application state is unavailable".to_owned())?
                .update_configuration(layers)
                .map_err(|error| error.to_string())?
        };
        let active_settings = snapshot.effective;
        let active_config = serde_json::to_value(&active_settings)
            .map_err(|error| format!("could not serialize active configuration: {error}"))?;
        let pending = snapshot.pending_changes;
        let pending_summary = pending
            .iter()
            .map(|change| {
                serde_json::json!({
                    "key": change.key,
                    "apply_time": change.apply_time,
                    "current": change.current,
                    "pending": change.pending
                })
            })
            .collect::<Vec<_>>();
        let config_json = serde_json::to_string(&active_config)
            .map_err(|error| format!("could not serialize effective configuration: {error}"))?;
        let bindings = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.config = active_config.clone();
            this.pending_config = (!pending.is_empty()).then_some(candidate.clone());
            this.learning_mode = active_config
                .get("discovery")
                .and_then(Value::as_object)
                .and_then(|discovery| discovery.get("learning_mode"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            configured_bindings(&this.registry, &this.config)
        };
        {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut().get_mut().bindings = bindings;
        }
        self.as_mut()
            .set_desktop_portal_mode(QString::from(desktop_portals::mode_name(
                &active_settings.desktop.portals,
            )));
        self.as_mut().update_chrome_preferences(&active_settings);
        self.as_mut().update_feature_preferences(&active_settings);
        self.as_mut().update_hint_preferences(&active_settings);
        self.as_mut().update_settings_presentation(&active_config);
        self.as_mut().set_config_json(QString::from(config_json));
        let learning_mode = self.as_ref().rust().learning_mode;
        self.as_mut().set_learning_mode(learning_mode);
        self.as_mut().update_theme_palette(&candidate);
        let label_settings_changed = previous_hints.is_some_and(|previous| {
            previous.chars != active_settings.hints.chars
                || previous.min_chars != active_settings.hints.min_chars
                || previous.auto_follow != active_settings.hints.auto_follow
        });
        if label_settings_changed && self.as_ref().rust().hint_session.is_some() {
            self.as_mut().cancel_hints();
        }
        let status = if pending.is_empty() {
            status.to_owned()
        } else {
            format!("{status}; {} lifecycle change(s) pending", pending.len())
        };
        self.as_mut().set_status_text(QString::from(status));
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let revision = this.ipc_sequence.saturating_add(1);
        publish_ipc_event(
            &mut this.ipc_sequence,
            "config.changed",
            serde_json::json!({
                "revision": revision,
                "pending": pending_summary,
            }),
        );
        Ok(active_config)
    }

    pub(super) fn update_chrome_preferences(mut self: Pin<&mut Self>, config: &Config) {
        let preferences: ChromePreferences = chrome_preferences::project(config);
        self.as_mut()
            .set_chrome_font_family(QString::from(preferences.font_family));
        self.as_mut()
            .set_chrome_font_size_pt(preferences.font_size_pt);
        self.as_mut()
            .set_chrome_statusbar_mode(QString::from(preferences.statusbar_mode));
        self.as_mut()
            .set_chrome_tabs_mode(QString::from(preferences.tabs_mode));
        self.as_mut()
            .set_chrome_tab_position(QString::from(preferences.tab_position));
        self.as_mut()
            .set_chrome_reduced_motion(QString::from(preferences.reduced_motion));
    }

    pub(super) fn update_feature_preferences(mut self: Pin<&mut Self>, config: &Config) {
        let preferences: FeaturePreferences = feature_preferences::project(config);
        self.as_mut()
            .set_feature_switcher_max_results(preferences.switcher_max_results);
        self.as_mut()
            .set_feature_downloads_ask_destination(preferences.downloads_ask_destination);
        self.as_mut()
            .set_feature_desktop_notifications_enabled(preferences.desktop_notifications_enabled);
        self.as_mut()
            .set_feature_desktop_media_keys_enabled(preferences.desktop_media_keys_enabled);
        self.as_mut()
            .set_feature_push_service_enabled(preferences.push_service_enabled);
        self.as_mut()
            .set_feature_spellcheck_enabled(preferences.spellcheck_enabled);
        self.as_mut().set_feature_spellcheck_languages(
            preferences
                .spellcheck_languages
                .into_iter()
                .map(QString::from)
                .collect(),
        );
        self.as_mut().set_feature_blocking_list_ids(
            preferences
                .blocking_list_ids
                .into_iter()
                .map(QString::from)
                .collect(),
        );
        self.as_mut()
            .set_feature_blocking_update_interval_hours(preferences.blocking_update_interval_hours);
        self.as_mut()
            .set_feature_link_cleaning_update_source(QString::from(
                preferences.link_cleaning_update_source,
            ));
        self.as_mut()
            .set_feature_link_cleaning_update_sha256(QString::from(
                preferences.link_cleaning_update_sha256,
            ));
    }

    pub(super) fn update_hint_preferences(mut self: Pin<&mut Self>, config: &Config) {
        let policy = if self.as_ref().rust().hint_rapid {
            &config.hints.rapid_unmatched
        } else {
            &config.hints.unmatched
        };
        let policy = match policy {
            ferric_browser_config::HintUnmatchedPolicy::Hide => "hide",
            ferric_browser_config::HintUnmatchedPolicy::Dim => "dim",
            ferric_browser_config::HintUnmatchedPolicy::Show => "show",
        };
        self.as_mut()
            .set_hint_unmatched_policy(QString::from(policy));
        self.as_mut()
            .set_hint_marker_scale(config.hints.marker_scale);
    }

    pub(super) fn update_settings_presentation(mut self: Pin<&mut Self>, config: &Value) {
        self.as_mut()
            .set_settings_rows(settings_presentation::project_variant(config));
    }

    pub(super) fn update_theme_palette(mut self: Pin<&mut Self>, config_value: &Value) -> bool {
        let config = serde_json::from_value::<Config>(config_value.clone()).unwrap_or_default();
        let Ok(palette) = load_theme_palette(&config.theme) else {
            return false;
        };
        let palette_path = palette.path.clone();
        let mut watch_paths = Vec::new();
        if !palette_path.is_empty() {
            let path = PathBuf::from(&palette_path);
            watch_paths.push(path.clone());
            if let Some(parent) = path.parent() {
                watch_paths.push(parent.to_owned());
                if let Some(grandparent) = parent.parent() {
                    watch_paths.push(grandparent.to_owned());
                }
            }
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .theme_watch
                .set_sources(Path::new(""), &watch_paths);
        }
        let presentation = theme_presentation::project(&palette);
        self.as_mut()
            .set_theme_background_color(QString::from(presentation.background));
        self.as_mut()
            .set_theme_surface_color(QString::from(presentation.surface));
        self.as_mut()
            .set_theme_panel_color(QString::from(presentation.panel));
        self.as_mut()
            .set_theme_primary_text_color(QString::from(presentation.primary_text));
        self.as_mut()
            .set_theme_secondary_text_color(QString::from(presentation.secondary_text));
        self.as_mut()
            .set_theme_muted_text_color(QString::from(presentation.muted_text));
        self.as_mut()
            .set_theme_border_color(QString::from(presentation.border));
        self.as_mut()
            .set_theme_accent_color(QString::from(presentation.accent));
        self.as_mut()
            .set_theme_warning_color(QString::from(presentation.warning));
        self.as_mut()
            .set_theme_error_color(QString::from(presentation.error));
        self.as_mut()
            .set_theme_success_color(QString::from(presentation.success));
        self.as_mut()
            .set_theme_private_color(QString::from(presentation.private));
        self.as_mut()
            .set_theme_mode_insert_color(QString::from(presentation.mode_insert));
        self.as_mut()
            .set_theme_selection_color(QString::from(presentation.selection));
        self.as_mut()
            .set_theme_selection_text_color(QString::from(presentation.selection_text));
        self.as_mut()
            .set_theme_contrast_status(QString::from(presentation.contrast_status));
        self.as_mut()
            .set_theme_contrast_reason(QString::from(presentation.contrast_reason));
        self.as_mut().rust_mut().as_mut().get_mut().theme_palette = palette;
        true
    }

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
        if persistent_storage {
            let path = self
                .as_ref()
                .rust()
                .storage_roots
                .as_ref()
                .map(|roots| roots.state.join("runtime-overrides.toml"))
                .ok_or_else(|| "runtime storage is unavailable".to_owned())?;
            save_runtime_overrides_atomic(&path, &persistent)
                .map_err(|error| format!("could not save runtime overrides: {error}"))?;
        }
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
        let result = self
            .as_mut()
            .commit_runtime_config(candidate, layers, &status)?;
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

    pub(super) fn execute_config_get_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let params = config_get_command_parameters(command)?;
        let result = self
            .as_ref()
            .ipc_config_get(&params)
            .map_err(|error| error.message().to_owned())?;
        let key = result.get("key").and_then(Value::as_str).unwrap_or("?");
        let source = result
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let apply_time = result
            .get("apply_time")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let value = serde_json::to_string(result.get("value").unwrap_or(&Value::Null))
            .map_err(|error| format!("could not format configuration value: {error}"))?;
        let pending = result
            .get("pending")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut status = format!("{key} = {value} (source: {source}, apply: {apply_time}");
        if pending {
            status.push_str(", pending");
        }
        status.push(')');
        self.as_mut().set_status_text(QString::from(status));
        Ok(result)
    }

    pub(super) fn execute_help_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let topic = match command.arguments.as_slice() {
            [] => None,
            [topic] if is_bounded_untrusted_text(topic) => Some(topic.as_str()),
            _ => return Err("help accepts at most one command or setting topic".into()),
        };
        let action = topic.map_or_else(
            || "show-binding-help".to_owned(),
            |topic| {
                if setting_metadata(topic).is_some() {
                    format!("show-settings\t{topic}")
                } else {
                    format!("show-binding-help\t{topic}")
                }
            },
        );
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(action);
        self.as_mut()
            .set_status_text(QString::from("Help requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "surface": "help",
            "topic": topic
        }))
    }

    pub(super) fn execute_version_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if !command.arguments.is_empty() {
            return Err("version does not accept arguments".into());
        }
        let result = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "protocol_major": ferric_browser_ipc::PROTOCOL_MAJOR,
            "protocol_minor": ferric_browser_ipc::PROTOCOL_MINOR
        });
        self.as_mut().set_status_text(QString::from(format!(
            "Ferric Browser {} (IPC {}.{})",
            result["version"].as_str().unwrap_or("unknown"),
            result["protocol_major"],
            result["protocol_minor"]
        )));
        Ok(result)
    }

    pub(super) fn execute_diagnostics_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if !command.arguments.is_empty() {
            return Err("diagnostics does not accept arguments".into());
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some("show-diagnostics".into());
        self.as_mut()
            .set_status_text(QString::from("Diagnostics requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "surface": "diagnostics"
        }))
    }

    pub(super) fn queue_config_write(
        mut self: Pin<&mut Self>,
        path: PathBuf,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .config_write_worker
            .as_mut()
            .ok_or_else(|| "configuration writer is unavailable".to_owned())?
            .request(path, bytes)
    }

    pub(super) fn execute_config_export_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let [path] = command.arguments.as_slice() else {
            return Err("config-export accepts exactly one destination path".into());
        };
        if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
            return Err("config-export path is empty, too long, or unsafe".into());
        }
        let config: Config = serde_json::from_value(self.as_ref().rust().config.clone())
            .map_err(|error| format!("effective configuration cannot be exported: {error}"))?;
        let bytes = toml::to_string_pretty(&config)
            .map_err(|error| format!("could not serialize effective configuration: {error}"))?;
        let path = PathBuf::from(path);
        let byte_count = bytes.len();
        self.as_mut()
            .queue_config_write(path.clone(), bytes.into_bytes())
            .map_err(|error| format!("could not queue config export: {error}"))?;
        self.as_mut().set_status_text(QString::from(format!(
            "Configuration export queued for {}",
            path.display()
        )));
        Ok(serde_json::json!({
            "status": "queued",
            "path": path,
            "bytes": byte_count,
            "review": true
        }))
    }

    pub(super) fn execute_config_check_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        if !command.arguments.is_empty() {
            return Err("config-check does not accept arguments".into());
        }
        let path_text = self.as_ref().rust().config_path.to_string();
        if path_text.is_empty() {
            self.as_mut()
                .set_status_text(QString::from("Built-in configuration is valid"));
            return Ok(serde_json::json!({
                "status": "valid",
                "source": "built-in",
                "sources": 0
            }));
        }
        let path = PathBuf::from(&path_text);
        let request = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .config_reload_worker
            .as_mut()
            .ok_or_else(|| "configuration reader is unavailable".to_owned())?
            .request_check(path.clone());
        request.map_err(|error| format!("could not queue configuration check: {error}"))?;
        self.as_mut()
            .set_status_text(QString::from("Configuration check queued"));
        Ok(serde_json::json!({
            "status": "queued",
            "source": path,
            "operation": "check"
        }))
    }

    pub(super) fn execute_config_reload_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        if !command.arguments.is_empty() {
            return Err("config-reload does not accept arguments".into());
        }
        let path = self.as_ref().rust().config_path.to_string();
        if path.is_empty() {
            return Err("no file-backed configuration is active".into());
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut().get_mut().config_watch.mark_pending(
                Instant::now()
                    .checked_sub(Duration::from_millis(200))
                    .unwrap_or_else(Instant::now),
            );
        }
        self.as_mut().poll_config();
        let status = self.as_ref().rust().status_text.to_string();
        if status.starts_with("Configuration reload rejected") {
            return Err(status);
        }
        Ok(serde_json::json!({
            "status": "queued",
            "source": path
        }))
    }

    pub(super) fn execute_config_write_defaults_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let [path] = command.arguments.as_slice() else {
            return Err("config-write-defaults requires exactly one destination path".into());
        };
        if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
            return Err("config-write-defaults path is empty, too long, or unsafe".into());
        }
        let bytes = toml::to_string_pretty(&Config::default())
            .map_err(|error| format!("could not serialize default configuration: {error}"))?;
        let path = PathBuf::from(path);
        let byte_count = bytes.len();
        self.as_mut()
            .queue_config_write(path.clone(), bytes.into_bytes())
            .map_err(|error| format!("could not queue default configuration: {error}"))?;
        self.as_mut().set_status_text(QString::from(format!(
            "Default configuration write queued for {}",
            path.display()
        )));
        Ok(serde_json::json!({
            "status": "queued",
            "path": path,
            "overwrite": false,
            "bytes": byte_count
        }))
    }

    pub(super) fn execute_theme_reload_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        if !command.arguments.is_empty() {
            return Err("theme-reload does not accept arguments".into());
        }
        let config = self.as_ref().rust().config.clone();
        if !self.as_mut().update_theme_palette(&config) {
            return Err("theme palette reload failed".into());
        }
        let hyprland_config = serde_json::from_value::<ferric_browser_config::HyprlandConfig>(
            config.get("hyprland").cloned().unwrap_or(Value::Null),
        )
        .unwrap_or_default();
        self.as_mut().set_hyprland_status(QString::from(
            hyprland::HyprlandAdapter::from_config(&hyprland_config).status(),
        ));
        self.as_mut()
            .set_status_text(QString::from("Theme palette reloaded"));
        let palette = self.as_ref().rust().theme_palette.clone();
        let provider_status = serde_json::json!({
            "source": palette.source,
            "path": palette.path,
            "layout": palette.provider_layout,
            "version": palette.provider_version,
            "commit": palette.provider_commit,
        });
        Ok(serde_json::json!({
            "status": "reloaded",
            "palette": serde_json::to_value(&palette).unwrap_or(Value::Null),
            "provider_status": provider_status
        }))
    }

    pub(super) fn poll_config_edit(mut self: Pin<&mut Self>) {
        let pending = self.as_ref().rust().pending_config_edit.clone();
        let Some(pending) = pending else {
            return;
        };
        let status = if let Some(status) = pending.exit_status {
            Some(status)
        } else {
            match try_wait_editor_process(&pending.process) {
                Ok(Some(status)) => {
                    let mut rust = self.as_mut().rust_mut();
                    if let Some(current) = rust
                        .as_mut()
                        .get_mut()
                        .pending_config_edit
                        .as_mut()
                        .filter(|current| current.token == pending.token)
                    {
                        current.exit_status = Some(status);
                    }
                    Some(status)
                }
                Ok(None) => None,
                Err(error) => {
                    let removed = {
                        let mut rust = self.as_mut().rust_mut();
                        let this = rust.as_mut().get_mut();
                        if this
                            .pending_config_edit
                            .as_ref()
                            .is_some_and(|current| current.token == pending.token)
                        {
                            this.pending_config_edit.take()
                        } else {
                            None
                        }
                    };
                    if removed.is_some() {
                        self.as_mut().set_status_text(QString::from(format!(
                            "Configuration editor status unavailable: {error}"
                        )));
                    }
                    return;
                }
            }
        };
        let Some(status) = status else {
            return;
        };
        let stderr = {
            let result = match pending.stderr.lock() {
                Ok(mut result) => result.take(),
                Err(_) => Some(Err("configuration editor stderr was unavailable".into())),
            };
            let Some(result) = result else {
                return;
            };
            match result {
                Ok(bytes) => {
                    let text = sanitize_process_stderr(&bytes);
                    (!text.is_empty()).then_some(text)
                }
                Err(error) => Some(error),
            }
        };
        {
            let removed = {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                if this
                    .pending_config_edit
                    .as_ref()
                    .is_some_and(|current| current.token == pending.token)
                {
                    this.pending_config_edit.take()
                } else {
                    None
                }
            };
            if removed.is_some() {
                let stderr_detail = stderr
                    .as_deref()
                    .map_or_else(String::new, |text| format!(" (stderr: {text})"));
                if status.success() {
                    let mut rust = self.as_mut().rust_mut();
                    rust.as_mut().get_mut().config_watch.mark_pending(
                        Instant::now()
                            .checked_sub(Duration::from_millis(200))
                            .unwrap_or_else(Instant::now),
                    );
                    self.as_mut().set_status_text(QString::from(format!(
                        "Configuration editor finished; reloading {}{}",
                        pending.path.display(),
                        stderr_detail
                    )));
                } else {
                    self.as_mut().set_status_text(QString::from(format!(
                        "Configuration editor exited with status {status}{stderr_detail}"
                    )));
                }
            }
        }
    }

    pub(super) fn probe_desktop_portals(mut self: Pin<&mut Self>) -> bool {
        let request_result = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.portal_probe_worker.is_none() {
                this.portal_probe_worker = PortalProbeWorker::spawn().ok();
            }
            this.portal_probe_worker.as_mut().map_or_else(
                || Err("portal probe worker could not start".into()),
                PortalProbeWorker::request,
            )
        };
        if let Err(error) = request_result {
            let _ = error;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .portal_capabilities = PortalCapabilities::unavailable();
            return false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .portal_capabilities = PortalCapabilities::pending();
        true
    }

    pub(super) fn portal_capability_status(self: Pin<&mut Self>, capability: &QString) -> QString {
        let status = self
            .as_ref()
            .rust()
            .portal_capabilities
            .capability_status(&capability.to_string());
        QString::from(status)
    }
}
