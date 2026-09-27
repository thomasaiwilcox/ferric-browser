use super::{
    CommandSource, CoreHintAutoFollow, CxxQtType, DispatchTarget, Effect, EngineEffect, Event,
    HintInteraction, HintInteractionInput, HintInteractionOutcome, HintKind, HintSession,
    HintTarget, IpcOpenTarget, IpcRoute, JourneyEdgeKind, Mode, ParsedCommand, Pin, QString, TabId,
    ValidatedUrl, Value, assign_labels_with_options, clean_link, current_target,
    external_hint_target, hint_json, hint_kind_name, hint_uses_url_action, link_cleaning_policy,
    parse_hint_candidate, parse_hint_payload, qobject, rapid_hint_keeps_mode, refresh_labels,
    safe_ipc_url,
};

impl qobject::BrowserUi {
    pub(super) fn set_hint_mode(mut self: Pin<&mut Self>) {
        self.as_mut()
            .set_hint_options("all", false, "current", None, false, 1);
    }

    pub(super) fn set_hint_mode_kind(mut self: Pin<&mut Self>, links_only: bool) {
        self.as_mut().set_hint_options(
            if links_only { "links" } else { "all" },
            false,
            "current",
            None,
            false,
            1,
        );
    }

    pub(super) fn set_hint_options(
        mut self: Pin<&mut Self>,
        family: &str,
        rapid: bool,
        target: &str,
        script: Option<&str>,
        first: bool,
        index: usize,
    ) {
        self.as_mut().set_hint_links_only(family == "links");
        self.as_mut().set_hint_family(QString::from(family));
        self.as_mut().set_hint_rapid(rapid);
        if let Ok(config) =
            serde_json::from_str::<super::Config>(&self.as_ref().rust().config_json.to_string())
        {
            self.as_mut().update_hint_preferences(&config);
        }
        target.clone_into(
            &mut self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .hint_rapid_target,
        );
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.hint_script = script.map(ToOwned::to_owned);
            this.hint_first = first;
            this.hint_index = index;
            this.hint_rapid_tabs_created = 0;
            this.hint_consumed.clear();
            this.hint_interaction = None;
        }
        let window = self.as_ref().rust().window;
        if let Some(window) = window
            && self
                .as_mut()
                .reduce_event(Event::PushMode {
                    window,
                    mode: Mode::Hint,
                })
                .is_err()
        {
            self.set_status_text(QString::from("Hint mode rejected"));
            return;
        }
        self.as_mut().set_core_mode(Mode::Hint);
        self.as_mut().set_mode(QString::from("hint"));
        self.set_status_text(QString::from("Select a page element"));
    }

    pub(super) fn clear_hint_session_state(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.hint_session = None;
            this.hint_interaction = None;
            this.hint_consumed.clear();
            this.hint_values = QString::default();
            this.pending_hint_action = None;
        }
        self.set_hint_visible(false);
    }

    pub(super) fn begin_hint_session(
        mut self: Pin<&mut Self>,
        candidates_json: &QString,
    ) -> QString {
        if self.as_ref().rust().core_mode != Mode::Hint {
            self.as_mut().set_hint_mode();
        }
        let Some(target) = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        ) else {
            self.set_status_text(QString::from("Hints unavailable: tab is no longer live"));
            return QString::from(r#"{"error":"stale-target"}"#);
        };
        let mut candidates = match parse_hint_payload(&candidates_json.to_string()) {
            Ok(candidates) => candidates,
            Err(error) => {
                self.set_status_text(QString::from(format!("Hints rejected: {error}")));
                return QString::from(serde_json::json!({"error": error}).to_string());
            }
        };
        candidates.retain(|candidate| {
            !self
                .as_ref()
                .rust()
                .hint_consumed
                .contains(&(candidate.frame_path.clone(), candidate.element_id))
        });
        let config =
            serde_json::from_str::<super::Config>(&self.as_ref().rust().config_json.to_string())
                .unwrap_or_default();
        let alphabet = config.hints.chars.clone();
        let minimum_width = usize::from(config.hints.min_chars);
        let mut ids = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut().get_mut().hint_ids.clone()
        };
        let binding = self.as_ref();
        let previous = binding
            .rust()
            .hint_session
            .as_ref()
            .filter(|session| {
                session.target.tab == target.tab && session.target.document == target.document
            })
            .map_or(&[][..], |session| session.hints.as_slice());
        let labeled = match if previous.is_empty() {
            assign_labels_with_options(candidates, &alphabet, minimum_width)
        } else {
            refresh_labels(candidates, previous, &alphabet, minimum_width)
        } {
            Ok(labeled) => labeled,
            Err(error) => {
                self.set_status_text(QString::from(format!("Hints rejected: {error}")));
                return QString::from(serde_json::json!({"error": error.to_string()}).to_string());
            }
        };
        let hint_first = self.as_ref().rust().hint_first;
        let hint_index = self.as_ref().rust().hint_index;
        if hint_first && hint_index > labeled.len() {
            self.set_status_text(QString::from(format!(
                "Hint index {hint_index} is unavailable; only {} candidate(s)",
                labeled.len()
            )));
            return QString::from(
                serde_json::json!({"error":"index-unavailable", "available":labeled.len()})
                    .to_string(),
            );
        }
        let auto_follow = match config.hints.auto_follow {
            ferric_browser_config::HintAutoFollow::Always => CoreHintAutoFollow::Always,
            ferric_browser_config::HintAutoFollow::UniqueMatch => CoreHintAutoFollow::UniqueMatch,
            ferric_browser_config::HintAutoFollow::FullMatch => CoreHintAutoFollow::FullMatch,
            ferric_browser_config::HintAutoFollow::Never => CoreHintAutoFollow::Never,
        };
        let interaction = HintInteraction::new(labeled.clone(), alphabet, auto_follow);
        let first_label = if hint_first {
            labeled.get(hint_index - 1).map(|hint| hint.label.clone())
        } else {
            interaction.initial_activation().map(ToOwned::to_owned)
        };
        let session = HintSession::new(
            &mut ids,
            HintTarget {
                tab: target.tab,
                generation: target.generation,
                document: target.document,
            },
            labeled.clone(),
        );
        let result = serde_json::json!({
            "session_id": session.id.to_string(),
            "hints": hint_json(&labeled),
            "state": hint_interaction_json(&interaction),
            "first_label": first_label,
        });
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.hint_ids = ids;
            this.hint_session = Some(session);
            this.hint_interaction = Some(interaction);
            this.hint_values = QString::from(result["hints"].to_string());
        }
        self.as_mut().set_hint_visible(!hint_first);
        self.set_status_text(QString::from(format!(
            "{} hint(s) · type a label or click",
            labeled.len()
        )));
        QString::from(result.to_string())
    }

    pub(super) fn update_hint_interaction(
        mut self: Pin<&mut Self>,
        action: &QString,
        text: &QString,
    ) -> QString {
        let action = action.to_string();
        let text = text.to_string();
        let input = match action.as_str() {
            "character" => text.chars().next().map(HintInteractionInput::Character),
            "text-mode" => Some(HintInteractionInput::EnterTextMode),
            "backspace" => Some(HintInteractionInput::Backspace),
            "clear" => Some(HintInteractionInput::Clear),
            "next" => Some(HintInteractionInput::Next),
            "previous" => Some(HintInteractionInput::Previous),
            "activate" => Some(HintInteractionInput::Activate),
            "rotate" => Some(HintInteractionInput::RotateCollision),
            "select" => Some(HintInteractionInput::SelectLabel(text.clone())),
            "cancel" => Some(HintInteractionInput::Cancel),
            _ => None,
        };
        let Some(input) = input else {
            return QString::from(r#"{"error":"invalid-interaction"}"#);
        };
        let (outcome, state) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(interaction) = this.hint_interaction.as_mut() else {
                return QString::from(r#"{"error":"no-session"}"#);
            };
            let outcome = interaction.handle(input);
            (outcome, hint_interaction_json(interaction))
        };
        let update_status = matches!(
            &outcome,
            HintInteractionOutcome::Updated | HintInteractionOutcome::RotateCollision
        );
        let mut response = serde_json::json!({"state": state});
        match outcome {
            HintInteractionOutcome::Updated => {
                response["outcome"] = Value::String("updated".into());
            }
            HintInteractionOutcome::Activate(label) => {
                response["outcome"] = Value::String("activate".into());
                response["label"] = Value::String(label);
            }
            HintInteractionOutcome::InvalidCharacter(character) => {
                response["outcome"] = Value::String("invalid".into());
                response["character"] = Value::String(character.to_string());
                self.as_mut()
                    .set_status_text(QString::from("Key is not in the hint alphabet"));
            }
            HintInteractionOutcome::NoMatches => {
                response["outcome"] = Value::String("no-matches".into());
                self.as_mut()
                    .set_status_text(QString::from("No hints match that input"));
            }
            HintInteractionOutcome::RotateCollision => {
                response["outcome"] = Value::String("rotate".into());
            }
            HintInteractionOutcome::Cancel => {
                response["outcome"] = Value::String("cancel".into());
            }
        }
        if update_status {
            let state = &response["state"];
            let typed = if state.get("mode").and_then(Value::as_str) == Some("text") {
                format!(
                    "/{}",
                    state
                        .get("query")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                )
            } else {
                state
                    .get("prefix")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            };
            let remaining = state.get("remaining").and_then(Value::as_u64).unwrap_or(0);
            let total = state.get("total").and_then(Value::as_u64).unwrap_or(0);
            let target = self.as_ref().rust().hint_rapid_target.clone();
            self.as_mut().set_status_text(QString::from(format!(
                "{target} · {typed} · {remaining}/{total}"
            )));
        }
        QString::from(response.to_string())
    }

    pub(super) fn select_hint_action(
        mut self: Pin<&mut Self>,
        label: &QString,
        fresh_candidate_json: &QString,
        action_id: &QString,
    ) -> QString {
        let action_id = action_id.to_string();
        if let Some(target) = match action_id.as_str() {
            "hint.current" => Some("current"),
            "hint.tab" => Some("tab"),
            "hint.tab-bg" => Some("tab-bg"),
            "hint.window" => Some("window"),
            "hint.yank" => Some("yank"),
            "hint.clean-yank" => Some("clean-yank"),
            "hint.download" => Some("download"),
            "hint.ephemeral" => Some("ephemeral"),
            "hint.choose" => Some("choose"),
            _ => None,
        } {
            target.clone_into(
                &mut self
                    .as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .hint_rapid_target,
            );
            return self.select_hint(label, fresh_candidate_json);
        }
        if !action_id.starts_with("userscript.") || action_id.len() > 256 {
            self.set_status_text(QString::from("Hint action rejected: invalid action ID"));
            return QString::from(r#"{"error":"invalid-action-id"}"#);
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_hint_action = Some(action_id);
        self.select_hint(label, fresh_candidate_json)
    }

    fn open_hint_in_background_tab(
        mut self: Pin<&mut Self>,
        source_tab: TabId,
        url: &ValidatedUrl,
    ) -> Result<Value, String> {
        let response = self.as_mut().execute_ipc_command(
            ParsedCommand {
                name: "open".into(),
                arguments: vec![url.to_string()],
            },
            &IpcRoute {
                selector: DispatchTarget::Tab(source_tab),
                open_target: IpcOpenTarget::BackgroundTab,
                profile: None,
                context: None,
                external_open: false,
                source: CommandSource::Ipc,
            },
        )?;
        self.as_mut()
            .set_status_text(QString::from("Hint link opened in background"));
        let mut result = serde_json::json!({
            "action":"tab-bg",
            "action_id":"browser.link.open",
            "kind":"link"
        });
        if let Some(tab_id) = response.get("tab_id") {
            result["tab_id"] = tab_id.clone();
        }
        Ok(result)
    }

    fn open_hint_in_foreground_tab(
        mut self: Pin<&mut Self>,
        source_tab: TabId,
        url: &ValidatedUrl,
    ) -> Result<Value, String> {
        let (window, journey_parent) = {
            let binding = self.as_ref();
            let state = binding
                .rust()
                .state
                .as_ref()
                .ok_or_else(|| "core state unavailable".to_owned())?;
            let window = state
                .tabs()
                .get(&source_tab)
                .map(|tab| tab.window)
                .ok_or_else(|| "hint source tab is no longer live".to_owned())?;
            (window, state.journey().current_node(source_tab))
        };
        // Hint selection runs while the core is deliberately in Hint mode.
        // Do not redispatch the normal-mode-only `tab-open` command here;
        // this is the already-authorized completion of the active hint action.
        let effects = self
            .as_mut()
            .reduce_event(Event::OpenTabWithNavigation {
                window,
                url: url.clone(),
                background: false,
            })
            .map_err(|error| error.clone())?;
        let tab = effects
            .iter()
            .find_map(|effect| match effect {
                Effect::Engine(EngineEffect::Navigate { target, .. }) => Some(target.tab),
                _ => None,
            })
            .ok_or_else(|| "foreground hint tab did not produce a navigation target".to_owned())?;
        if let Some(parent) = journey_parent {
            self.as_mut().mark_journey_parent(&effects, parent);
        }
        self.as_mut().sync_tab_order_from_core();
        self.as_mut().set_pending_engine_action(&effects);
        self.as_mut()
            .set_status_text(QString::from("Hint link opened in a foreground tab"));
        Ok(serde_json::json!({
            "action":"tab",
            "action_id":"browser.link.open",
            "kind":"link",
            "tab_id":tab.to_string()
        }))
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn select_hint(
        mut self: Pin<&mut Self>,
        label: &QString,
        fresh_candidate_json: &QString,
    ) -> QString {
        let fresh_value = match serde_json::from_str::<Value>(&fresh_candidate_json.to_string()) {
            Ok(value) => value,
            Err(error) => {
                self.set_status_text(QString::from(format!("Hint selection rejected: {error}")));
                return QString::from(r#"{"error":"invalid-fresh-candidate"}"#);
            }
        };
        let pending_action = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_hint_action
            .take();
        let visible = fresh_value
            .get("visible")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let fresh = match parse_hint_candidate(&fresh_value) {
            Ok(candidate) => candidate,
            Err(error) => {
                self.set_status_text(QString::from(format!("Hint selection rejected: {error}")));
                return QString::from(serde_json::json!({"error": error}).to_string());
            }
        };
        let Some(target) = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        ) else {
            self.set_status_text(QString::from("Hint selection rejected: stale tab"));
            return QString::from(r#"{"error":"stale-target"}"#);
        };
        let selected = {
            let binding = self.as_ref();
            let Some(session) = binding.rust().hint_session.as_ref() else {
                self.set_status_text(QString::from("Hint session has expired"));
                return QString::from(r#"{"error":"no-session"}"#);
            };
            session
                .select(
                    &label.to_string(),
                    HintTarget {
                        tab: target.tab,
                        generation: target.generation,
                        document: target.document,
                    },
                    &fresh.frame_path,
                    visible,
                    fresh.geometry,
                )
                .cloned()
        };
        let selected = match selected {
            Ok(candidate)
                if candidate.element_id == fresh.element_id
                    && candidate.kind == fresh.kind
                    && candidate.frame_path == fresh.frame_path
                    && candidate.text == fresh.text
                    && candidate.href == fresh.href =>
            {
                candidate
            }
            Ok(_) => {
                self.set_status_text(QString::from("Hint selection rejected: target changed"));
                return QString::from(r#"{"error":"target-changed"}"#);
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Hint selection rejected: {error}")));
                return QString::from(serde_json::json!({"error": error.to_string()}).to_string());
            }
        };
        let choose = self.as_ref().rust().hint_rapid_target == "choose" && pending_action.is_none();
        let uses_url_action = hint_uses_url_action(
            selected.kind,
            self.as_ref().rust().hint_rapid,
            &self.as_ref().rust().hint_rapid_target,
            pending_action.is_some(),
        );
        let result = if choose {
            serde_json::json!({
                "action":"choose",
                "kind":hint_kind_name(selected.kind),
                "label":label.to_string(),
                "has_url":selected.href.is_some(),
            })
        } else if uses_url_action {
            let Some(href) = selected.href.as_deref() else {
                self.set_status_text(QString::from("Hint link has no destination"));
                return QString::from(r#"{"error":"link-without-destination"}"#);
            };
            let url = match ValidatedUrl::parse(href) {
                Ok(url) => url,
                Err(error) => {
                    self.set_status_text(QString::from(format!("Hint link rejected: {error}")));
                    return QString::from(r#"{"error":"invalid-link"}"#);
                }
            };
            if let Some(action_id) = pending_action {
                let response = match self
                    .as_mut()
                    .execute_registered_userscript_action_for_hint(&action_id, url.as_str())
                {
                    Ok(response) => response,
                    Err(error) => {
                        self.set_status_text(QString::from(format!(
                            "Hint userscript action rejected: {error}"
                        )));
                        return QString::from(
                            serde_json::json!({"error":"userscript-action-rejected"}).to_string(),
                        );
                    }
                };
                self.as_mut().set_status_text(QString::from(format!(
                    "Hint userscript action started: {action_id}"
                )));
                let mut result = serde_json::json!({
                    "action":"userscript",
                    "action_id":action_id,
                    "kind":"link"
                });
                if let Some(operation_id) = response.get("operation_id") {
                    result["operation_id"] = operation_id.clone();
                }
                result
            } else if self.as_ref().rust().hint_rapid {
                match self.as_ref().rust().hint_rapid_target.as_str() {
                    "yank" => {
                        let copied = safe_ipc_url(url.as_str());
                        let mut rust = self.as_mut().rust_mut();
                        let this = rust.as_mut().get_mut();
                        this.clipboard_request = QString::from(&copied);
                        this.clipboard_request_sensitive = false;
                        this.clipboard_request_primary = false;
                        self.as_mut()
                            .set_status_text(QString::from("Hint link copied"));
                        serde_json::json!({
                            "action":"yank",
                            "action_id":"browser.link.copy",
                            "kind":"link"
                        })
                    }
                    "clean-yank" => {
                        let rules = link_cleaning_policy::active_rules(
                            self.as_ref().rust().storage_roots.as_ref(),
                        );
                        let cleaned = match clean_link(url.as_str(), &rules) {
                            Ok(result) => result,
                            Err(error) => {
                                self.set_status_text(QString::from(format!(
                                    "Clean hint link rejected: {error}"
                                )));
                                return QString::from(r#"{"error":"clean-link-rejected"}"#);
                            }
                        };
                        let copied = safe_ipc_url(&cleaned.cleaned);
                        let mut rust = self.as_mut().rust_mut();
                        let this = rust.as_mut().get_mut();
                        this.clipboard_request = QString::from(&copied);
                        this.clipboard_request_sensitive = false;
                        this.clipboard_request_primary = false;
                        self.as_mut()
                            .set_status_text(QString::from(if cleaned.changed {
                                "Clean hint link copied"
                            } else {
                                "Hint link copied; no cleaning changes applied"
                            }));
                        serde_json::json!({
                            "action":"clean-yank",
                            "action_id":"browser.link.clean-copy",
                            "kind":"link",
                            "changed":cleaned.changed,
                            "url":copied
                        })
                    }
                    "tab-bg" => {
                        if self.as_ref().rust().hint_rapid_tabs_created >= 20 {
                            self.set_status_text(QString::from(
                                "Rapid hint tab limit reached; confirm before continuing",
                            ));
                            return QString::from(r#"{"error":"rapid-tab-limit"}"#);
                        }
                        let result =
                            match self.as_mut().open_hint_in_background_tab(target.tab, &url) {
                                Ok(result) => result,
                                Err(error) => {
                                    self.set_status_text(QString::from(format!(
                                        "Rapid background hint rejected: {error}"
                                    )));
                                    return QString::from(
                                        r#"{"error":"background-navigation-rejected"}"#,
                                    );
                                }
                            };
                        self.as_mut()
                            .rust_mut()
                            .as_mut()
                            .get_mut()
                            .hint_rapid_tabs_created += 1;
                        result
                    }
                    "userscript" => {
                        let Some(script) = self.as_ref().rust().hint_script.clone() else {
                            self.set_status_text(QString::from(
                                "Hint userscript target has no configured script",
                            ));
                            return QString::from(r#"{"error":"userscript-not-selected"}"#);
                        };
                        let response = match self.as_mut().execute_userscript_for_hint(
                            &script,
                            target,
                            url.as_str(),
                        ) {
                            Ok(response) => response,
                            Err(error) => {
                                self.set_status_text(QString::from(format!(
                                    "Hint userscript rejected: {error}"
                                )));
                                return QString::from(r#"{"error":"userscript-rejected"}"#);
                            }
                        };
                        self.as_mut().set_status_text(QString::from(format!(
                            "Rapid hint userscript started: {script}"
                        )));
                        let mut result = serde_json::json!({
                            "action":"userscript",
                            "action_id":format!("userscript.{script}.run"),
                            "kind":"link",
                            "userscript":script
                        });
                        if let Some(operation_id) = response.get("operation_id") {
                            result["operation_id"] = operation_id.clone();
                        }
                        result
                    }
                    "download" => {
                        let mut result = self.as_mut().queue_download_request(
                            target,
                            url.as_str(),
                            "Rapid hint download queued",
                        );
                        result["action"] = Value::String("download".into());
                        result["action_id"] = Value::String("browser.link.download".into());
                        result["kind"] = Value::String("link".into());
                        result
                    }
                    _ => {
                        self.set_status_text(QString::from("Rapid hint target state is invalid"));
                        return QString::from(r#"{"error":"rapid-target-invalid-state"}"#);
                    }
                }
            } else if self.as_ref().rust().hint_rapid_target == "yank" {
                let copied = safe_ipc_url(url.as_str());
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.clipboard_request = QString::from(&copied);
                this.clipboard_request_sensitive = false;
                this.clipboard_request_primary = false;
                serde_json::json!({
                    "action":"yank",
                    "action_id":"browser.link.copy",
                    "kind":hint_kind_name(selected.kind)
                })
            } else if self.as_ref().rust().hint_rapid_target == "clean-yank" {
                let rules =
                    link_cleaning_policy::active_rules(self.as_ref().rust().storage_roots.as_ref());
                let cleaned = match clean_link(url.as_str(), &rules) {
                    Ok(result) => result,
                    Err(error) => {
                        self.set_status_text(QString::from(format!(
                            "Clean hint link rejected: {error}"
                        )));
                        return QString::from(r#"{"error":"clean-link-rejected"}"#);
                    }
                };
                let copied = safe_ipc_url(&cleaned.cleaned);
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.clipboard_request = QString::from(&copied);
                this.clipboard_request_sensitive = false;
                this.clipboard_request_primary = false;
                serde_json::json!({
                    "action":"clean-yank",
                    "action_id":"browser.link.clean-copy",
                    "kind":hint_kind_name(selected.kind),
                    "changed":cleaned.changed,
                    "url":copied
                })
            } else if self.as_ref().rust().hint_rapid_target == "tab-bg" {
                match self.as_mut().open_hint_in_background_tab(target.tab, &url) {
                    Ok(result) => result,
                    Err(error) => {
                        self.set_status_text(QString::from(format!(
                            "Background hint rejected: {error}"
                        )));
                        return QString::from(r#"{"error":"background-navigation-rejected"}"#);
                    }
                }
            } else if self.as_ref().rust().hint_rapid_target == "tab" {
                match self.as_mut().open_hint_in_foreground_tab(target.tab, &url) {
                    Ok(result) => result,
                    Err(error) => {
                        self.set_status_text(QString::from(format!(
                            "Hint tab navigation rejected: {error}"
                        )));
                        return QString::from(r#"{"error":"tab-navigation-rejected"}"#);
                    }
                }
            } else if self.as_ref().rust().hint_rapid_target == "window" {
                let profile = self.as_ref().rust().profile_name.clone();
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some(format!(
                    "new-window\tfalse\t{}\t{}",
                    profile,
                    safe_ipc_url(url.as_str())
                ));
                self.as_mut()
                    .set_status_text(QString::from("Hint link queued for a same-profile window"));
                serde_json::json!({
                    "action":"window",
                    "action_id":"browser.link.open",
                    "kind":"link",
                    "target":"window",
                    "url":safe_ipc_url(url.as_str())
                })
            } else if let Some(target_name) =
                external_hint_target(&self.as_ref().rust().hint_rapid_target).map(ToOwned::to_owned)
            {
                let response = match self.as_mut().execute_send_command(&ParsedCommand {
                    name: "send".into(),
                    arguments: vec![target_name.clone(), url.to_string()],
                }) {
                    Ok(response) => response,
                    Err(error) => {
                        self.set_status_text(QString::from(format!(
                            "Hint external target rejected: {error}"
                        )));
                        return QString::from(
                            serde_json::json!({"error":"external-target-rejected"}).to_string(),
                        );
                    }
                };
                self.as_mut()
                    .set_status_text(QString::from(format!("Hint link sent to {target_name}")));
                let mut result = serde_json::json!({
                    "action":"external-send",
                    "action_id":"browser.link.send",
                    "kind":"link",
                    "target":target_name
                });
                if let Some(operation_id) = response.get("operation_id") {
                    result["operation_id"] = operation_id.clone();
                }
                result
            } else if self.as_ref().rust().hint_rapid_target == "ephemeral" {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action =
                    Some(format!("ephemeral-window\t{}", safe_ipc_url(url.as_str())));
                self.as_mut().set_status_text(QString::from(
                    "Hint link queued for a fresh ephemeral window",
                ));
                serde_json::json!({
                    "action":"ephemeral",
                    "action_id":"browser.link.open-ephemeral",
                    "kind":"link",
                    "url":safe_ipc_url(url.as_str())
                })
            } else if self.as_ref().rust().hint_rapid_target == "userscript" {
                let Some(script) = self.as_ref().rust().hint_script.clone() else {
                    self.set_status_text(QString::from(
                        "Hint userscript target has no configured script",
                    ));
                    return QString::from(r#"{"error":"userscript-not-selected"}"#);
                };
                let response =
                    match self
                        .as_mut()
                        .execute_userscript_for_hint(&script, target, url.as_str())
                    {
                        Ok(response) => response,
                        Err(error) => {
                            self.set_status_text(QString::from(format!(
                                "Hint userscript rejected: {error}"
                            )));
                            return QString::from(r#"{"error":"userscript-rejected"}"#);
                        }
                    };
                self.as_mut()
                    .set_status_text(QString::from(format!("Hint userscript started: {script}")));
                let mut result = serde_json::json!({
                    "action":"userscript",
                    "action_id":format!("userscript.{script}.run"),
                    "kind":"link",
                    "userscript":script
                });
                if let Some(operation_id) = response.get("operation_id") {
                    result["operation_id"] = operation_id.clone();
                }
                result
            } else if self.as_ref().rust().hint_rapid_target == "download" {
                let result = self.as_mut().queue_download_request(
                    target,
                    url.as_str(),
                    "Rapid hint download queued",
                );
                let mut result = result;
                result["action"] = Value::String("download".into());
                result["action_id"] = Value::String("browser.link.download".into());
                result["kind"] = Value::String("link".into());
                result
            } else {
                let effects = match self
                    .as_mut()
                    .reduce_event(Event::StartNavigation { target, url })
                {
                    Ok(effects) => effects,
                    Err(error) => {
                        self.set_status_text(QString::from(format!(
                            "Hint navigation rejected: {error}"
                        )));
                        return QString::from(r#"{"error":"navigation-rejected"}"#);
                    }
                };
                self.as_mut()
                    .mark_journey_transition(&effects, JourneyEdgeKind::Hint, "hint");
                self.as_mut().set_pending_engine_action(&effects);
                serde_json::json!({
                    "action":"navigate",
                    "action_id":"browser.link.open",
                    "kind":"link"
                })
            }
        } else if selected.kind == HintKind::Scrollable {
            serde_json::json!({
                "action":"scroll-target",
                "kind":"scrollable",
                "element_id":selected.element_id,
                "frame_path":fresh.frame_path,
            })
        } else if matches!(
            selected.kind,
            HintKind::Input | HintKind::Select | HintKind::Textarea | HintKind::ContentEditable
        ) {
            serde_json::json!({
                "action":"focus",
                "kind": hint_kind_name(selected.kind),
                "element_id": selected.element_id,
                "frame_path": fresh.frame_path,
                "x": fresh.geometry.x + (fresh.geometry.width / 2.0),
                "y": fresh.geometry.y + (fresh.geometry.height / 2.0)
            })
        } else {
            serde_json::json!({
                "action":"click",
                "kind": hint_kind_name(selected.kind),
                "element_id": selected.element_id,
                "frame_path": fresh.frame_path,
                "x": fresh.geometry.x + (fresh.geometry.width / 2.0),
                "y": fresh.geometry.y + (fresh.geometry.height / 2.0)
            })
        };
        let keeps_hint_mode = rapid_hint_keeps_mode(
            self.as_ref().rust().hint_rapid,
            result.get("action").and_then(Value::as_str),
        );
        // Page-backed activations still need the retained Hint record after
        // this Rust callback returns. QML closes the session after the
        // asynchronous page script has consumed that record.
        let awaits_page_activation = matches!(
            result.get("action").and_then(Value::as_str),
            Some("focus" | "scroll-target" | "click")
        );
        let palette_open = result.get("action").and_then(Value::as_str) == Some("choose");
        if keeps_hint_mode {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.hint_consumed
                .insert((selected.frame_path.clone(), selected.element_id));
            this.hint_interaction = None;
            this.pending_hint_action = None;
        } else if !palette_open && !awaits_page_activation {
            self.as_mut().clear_hint_session_state();
            let window = self.as_ref().rust().window;
            if let Some(window) = window {
                let _ = self.as_mut().reduce_event(Event::PopMode { window });
            }
            self.as_mut().set_core_mode(Mode::Normal);
            self.as_mut().set_mode(QString::from("normal"));
        }
        QString::from(result.to_string())
    }

    pub(super) fn confirm_rapid_hint_tabs(mut self: Pin<&mut Self>) -> bool {
        let binding = self.as_ref();
        let rust = binding.rust();
        if rust.core_mode != Mode::Hint
            || !rust.hint_rapid
            || rust.hint_rapid_target != "tab-bg"
            || rust.hint_rapid_tabs_created < 20
        {
            self.set_status_text(QString::from("No rapid hint tab confirmation is pending"));
            return false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .hint_rapid_tabs_created = 0;
        self.set_status_text(QString::from(
            "Rapid hint tab limit confirmed for another 20 tabs",
        ));
        true
    }

    pub(super) fn cancel_hints(mut self: Pin<&mut Self>) {
        self.as_mut().clear_hint_session_state();
        if self.as_ref().rust().core_mode == Mode::Hint {
            let window = self.as_ref().rust().window;
            if let Some(window) = window {
                let _ = self.as_mut().reduce_event(Event::PopMode { window });
            }
            self.as_mut().set_core_mode(Mode::Normal);
            self.as_mut().set_mode(QString::from("normal"));
        }
        self.set_status_text(QString::from("Hints cancelled"));
    }
}
fn hint_interaction_json(interaction: &HintInteraction) -> Value {
    let snapshot = interaction.snapshot();
    serde_json::json!({
        "mode": match snapshot.mode {
            ferric_browser_core::HintInputMode::Label => "label",
            ferric_browser_core::HintInputMode::Text => "text",
        },
        "prefix": snapshot.prefix,
        "query": snapshot.query,
        "matching_labels": snapshot.matching_labels,
        "active_label": snapshot.active_label,
        "remaining": snapshot.remaining,
        "total": snapshot.total,
    })
}
