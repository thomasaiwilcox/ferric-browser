use super::{
    CoreHintAutoFollow, CxxQtType, Event, HintInteraction, HintInteractionInput,
    HintInteractionOutcome, HintSession, HintTarget, Mode, Pin, QString, Value,
    assign_labels_with_options, current_target, hint_json, parse_hint_payload, qobject,
    refresh_labels,
};

impl qobject::BrowserUi {
    pub(super) fn note_hint_configuration_change(mut self: Pin<&mut Self>) {
        let hint_mode_was_active = self.as_ref().rust().core_mode == Mode::Hint;
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.hint_configuration_revision = this.hint_configuration_revision.saturating_add(1);
            this.hint_session = None;
            this.hint_interaction = None;
            this.hint_consumed.clear();
            this.pending_hint_action = None;
            this.pending_hint_palette_permit = None;
            this.hint_local_interaction_authority
                .clear_session_requests();
        }
        if hint_mode_was_active {
            self.as_mut().cancel_hints();
        } else {
            self.set_hint_visible(false);
        }
    }

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
            this.pending_hint_palette_permit = None;
            this.hint_local_interaction_authority
                .clear_session_requests();
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
            this.pending_hint_palette_permit = None;
            this.hint_local_interaction_authority
                .clear_session_requests();
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
        let configuration_revision = self.as_ref().rust().hint_configuration_revision;
        let session = HintSession::new_with_configuration_revision(
            &mut ids,
            HintTarget {
                tab: target.tab,
                generation: target.generation,
                document: target.document,
            },
            labeled.clone(),
            configuration_revision,
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
            this.pending_hint_palette_permit = None;
            this.hint_local_interaction_authority
                .clear_session_requests();
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
