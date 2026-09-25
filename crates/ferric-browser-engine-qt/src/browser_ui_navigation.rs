use super::{
    CommandSource, ContextsConfig, CxxQtType, DispatchTarget, Effect, EngineEffect, Event,
    HintKind, HintSession, HintTarget, IpcOpenTarget, IpcRoute, JourneyEdgeKind, Mode,
    NavigationError, NavigationSource, ParsedCommand, PendingContextRoute, Pin, QString, TabId,
    ValidatedUrl, Value, assign_labels, clean_link, current_target, external_hint_target,
    hint_json, hint_kind_name, link_cleaning_policy, matching_context_routes, parse_hint_candidate,
    parse_hint_payload, qobject, rapid_hint_keeps_mode, resolve_input, safe_ipc_url,
    validate_clipboard_navigation_input,
};

impl qobject::BrowserUi {
    pub(super) fn set_hint_mode(mut self: Pin<&mut Self>) {
        self.as_mut()
            .set_hint_options(false, false, "current", None);
    }

    pub(super) fn set_hint_mode_kind(mut self: Pin<&mut Self>, links_only: bool) {
        self.as_mut()
            .set_hint_options(links_only, false, "current", None);
    }

    pub(super) fn set_hint_options(
        mut self: Pin<&mut Self>,
        links_only: bool,
        rapid: bool,
        target: &str,
        script: Option<&str>,
    ) {
        self.as_mut().set_hint_links_only(links_only);
        self.as_mut().set_hint_rapid(rapid);
        target.clone_into(
            &mut self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .hint_rapid_target,
        );
        self.as_mut().rust_mut().as_mut().get_mut().hint_script = script.map(ToOwned::to_owned);
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .hint_rapid_tabs_created = 0;
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
        let candidates = match parse_hint_payload(&candidates_json.to_string()) {
            Ok(candidates) => candidates,
            Err(error) => {
                self.set_status_text(QString::from(format!("Hints rejected: {error}")));
                return QString::from(serde_json::json!({"error": error}).to_string());
            }
        };
        let mut ids = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut().get_mut().hint_ids.clone()
        };
        let labeled = match assign_labels(candidates) {
            Ok(labeled) => labeled,
            Err(error) => {
                self.set_status_text(QString::from(format!("Hints rejected: {error}")));
                return QString::from(serde_json::json!({"error": error.to_string()}).to_string());
            }
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
            "hints": hint_json(&labeled)
        });
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.hint_ids = ids;
            this.hint_session = Some(session);
            this.hint_values = QString::from(result["hints"].to_string());
        }
        self.as_mut().set_hint_visible(true);
        self.set_status_text(QString::from(format!(
            "{} hint(s) · type a label or click",
            labeled.len()
        )));
        QString::from(result.to_string())
    }

    pub(super) fn select_hint_action(
        mut self: Pin<&mut Self>,
        label: &QString,
        fresh_candidate_json: &QString,
        action_id: &QString,
    ) -> QString {
        let action_id = action_id.to_string();
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
        let result = if selected.kind == HintKind::Link {
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
                    _ => {
                        self.set_status_text(QString::from("Rapid hint target state is invalid"));
                        return QString::from(r#"{"error":"rapid-target-invalid-state"}"#);
                    }
                }
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
        self.as_mut().clear_hint_session_state();
        let keeps_hint_mode = rapid_hint_keeps_mode(
            self.as_ref().rust().hint_rapid,
            result.get("action").and_then(Value::as_str),
        );
        if !keeps_hint_mode {
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

    pub(super) fn queue_context_route(
        mut self: Pin<&mut Self>,
        url: &ValidatedUrl,
        entry_point: &str,
    ) -> bool {
        let pending = {
            let contexts_json = self.as_ref().rust().contexts_json.to_string();
            let Ok(config) = serde_json::from_str::<ContextsConfig>(&contexts_json) else {
                return false;
            };
            let Some(route) = matching_context_routes(&config, url.as_str(), entry_point)
                .into_iter()
                .next()
            else {
                return false;
            };
            let Some(context) = config
                .contexts
                .iter()
                .find(|context| context.name == route.context)
            else {
                return false;
            };
            PendingContextRoute {
                route_id: route.id.clone(),
                behavior: route.behavior.clone(),
                context_name: context.name.clone(),
                profile_name: context.profile.clone(),
                url: url.as_str().to_owned(),
            }
        };
        let route_id = QString::from(&pending.route_id);
        let behavior = QString::from(&pending.behavior);
        let context = QString::from(&pending.context_name);
        let profile = QString::from(&pending.profile_name);
        let url = QString::from(safe_ipc_url(&pending.url));
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_context_route = Some(pending);
        self.as_mut().set_context_route_id(route_id);
        self.as_mut().set_context_route_behavior(behavior);
        self.as_mut().set_context_route_context(context);
        self.as_mut().set_context_route_profile(profile);
        self.as_mut().set_context_route_url(url);
        true
    }

    pub(super) fn queue_context_route_for_input(
        mut self: Pin<&mut Self>,
        input: &str,
        entry_point: &str,
    ) -> bool {
        let Ok(result) = resolve_input(input, &self.as_ref().navigation_context()) else {
            return false;
        };
        if !matches!(
            result.source,
            NavigationSource::ExplicitUrl | NavigationSource::Host
        ) {
            return false;
        }
        self.as_mut().queue_context_route(&result.url, entry_point)
    }

    pub(super) fn clear_context_route(mut self: Pin<&mut Self>) -> Option<PendingContextRoute> {
        let pending = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_context_route
            .take();
        self.as_mut().set_context_route_id(QString::default());
        self.as_mut().set_context_route_behavior(QString::default());
        self.as_mut().set_context_route_context(QString::default());
        self.as_mut().set_context_route_profile(QString::default());
        self.as_mut().set_context_route_url(QString::default());
        pending
    }

    pub(super) fn accept_context_route(mut self: Pin<&mut Self>) -> bool {
        let Some(pending) = self.as_mut().clear_context_route() else {
            self.set_status_text(QString::from("No context route is pending"));
            return false;
        };
        let Ok(url) = ValidatedUrl::parse(pending.url.clone()) else {
            self.set_status_text(QString::from("Context route URL is invalid"));
            return false;
        };
        if pending.profile_name != self.as_ref().rust().profile_name {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some(format!(
                "new-window\tfalse\t{}\t{}\t{}",
                pending.profile_name,
                pending.context_name,
                url.as_str()
            ));
            self.set_status_text(QString::from(format!(
                "Context route accepted; opening {} in profile {}",
                pending.context_name, pending.profile_name
            )));
            return true;
        }
        let Some(window) = self.as_ref().rust().window else {
            self.set_status_text(QString::from("Context route window is unavailable"));
            return false;
        };
        let context_exists = self
            .as_ref()
            .rust()
            .contexts
            .as_ref()
            .is_some_and(|contexts| {
                contexts
                    .contexts()
                    .iter()
                    .any(|context| context.name == pending.context_name)
            });
        if !context_exists {
            self.set_status_text(QString::from("Context route target is unavailable"));
            return false;
        }
        if self
            .as_mut()
            .reduce_event(Event::SetWindowContext {
                window,
                context: Some(pending.context_name.clone()),
            })
            .is_err()
        {
            self.set_status_text(QString::from("Context route assignment was rejected"));
            return false;
        }
        if self.as_mut().navigate_url(&url).is_err() {
            self.set_status_text(QString::from("Context route navigation was rejected"));
            return false;
        }
        self.as_mut().sync_core_tabs();
        self.set_status_text(QString::from(format!(
            "Context route accepted: {}",
            pending.context_name
        )));
        true
    }

    pub(super) fn dismiss_context_route(mut self: Pin<&mut Self>) -> bool {
        let Some(pending) = self.as_mut().clear_context_route() else {
            self.set_status_text(QString::from("No context route is pending"));
            return false;
        };
        let Ok(url) = ValidatedUrl::parse(pending.url) else {
            self.set_status_text(QString::from("Context route URL is invalid"));
            return false;
        };
        match self.as_mut().navigate_url(&url) {
            Ok(()) => {
                self.set_status_text(QString::from("Navigation requested without context route"));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Navigation rejected: {error}")));
                false
            }
        }
    }

    pub(super) fn navigate_url(mut self: Pin<&mut Self>, url: &ValidatedUrl) -> Result<(), String> {
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        )
        .ok_or_else(|| "Tab is no longer live".to_owned())?;
        let effects = self
            .as_mut()
            .reduce_event(Event::StartNavigation {
                target,
                url: url.clone(),
            })
            .map_err(|error| error.clone())?;
        if !effects
            .iter()
            .any(|effect| matches!(effect, Effect::Engine(EngineEffect::Navigate { .. })))
        {
            return Err("Navigation ignored".into());
        }
        let resolved = QString::from(url.as_str());
        self.as_mut().set_initial_url(resolved.clone());
        self.as_mut().update_current_url(resolved);
        self.as_mut().set_load_state(QString::from("provisional"));
        Ok(())
    }

    pub(super) fn navigate(mut self: Pin<&mut Self>, input: &QString) {
        let requested = input.to_string();
        match resolve_input(&requested, &self.as_ref().navigation_context()) {
            Ok(result) => {
                if matches!(
                    result.source,
                    NavigationSource::ExplicitUrl | NavigationSource::Host
                ) && self
                    .as_mut()
                    .queue_context_route(&result.url, "explicit-open")
                {
                    self.set_status_text(QString::from("Context route confirmation required"));
                    return;
                }
                match self.as_mut().navigate_url(&result.url) {
                    Ok(()) => self.set_status_text(QString::from("Navigation requested")),
                    Err(error) if error == "Navigation ignored" => {
                        self.set_status_text(QString::from(error));
                    }
                    Err(error) => {
                        self.set_status_text(QString::from(format!(
                            "Navigation rejected: {error}"
                        )));
                    }
                }
            }
            Err(NavigationError::ExternalSchemeRequiresConfirmation { scheme, url }) => {
                self.as_mut().request_external_navigation(scheme, url);
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Navigation rejected: {error}")));
            }
        }
    }

    pub(super) fn navigate_initial(
        mut self: Pin<&mut Self>,
        input: &QString,
        entry_point: &QString,
        trusted_local_input: bool,
    ) {
        let requested = input.to_string();
        let entry_point = match entry_point.to_string().as_str() {
            "external-open" | "typed-initial-url" => entry_point.to_string(),
            _ => "typed-initial-url".into(),
        };
        let source = if trusted_local_input {
            CommandSource::Cli
        } else {
            CommandSource::Ui
        };
        match resolve_input(
            &requested,
            &self.as_ref().navigation_context_for_source(source),
        ) {
            Ok(result) => {
                if matches!(
                    result.source,
                    NavigationSource::ExplicitUrl | NavigationSource::Host
                ) && self.as_mut().queue_context_route(&result.url, &entry_point)
                {
                    self.set_status_text(QString::from("Context route confirmation required"));
                    return;
                }
                match self.as_mut().navigate_url(&result.url) {
                    Ok(()) => self.set_status_text(QString::from("Navigation requested")),
                    Err(error) => {
                        self.set_status_text(QString::from(format!("Navigation rejected: {error}")))
                    }
                }
            }
            Err(NavigationError::ExternalSchemeRequiresConfirmation { scheme, url }) => {
                self.as_mut().request_external_navigation(scheme, url);
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Navigation rejected: {error}")));
            }
        }
    }

    pub(super) fn navigate_without_context_route(mut self: Pin<&mut Self>, input: &QString) {
        let requested = input.to_string();
        match resolve_input(&requested, &self.as_ref().navigation_context()) {
            Ok(result) => match self.as_mut().navigate_url(&result.url) {
                Ok(()) => self.set_status_text(QString::from("Navigation requested")),
                Err(error) => {
                    self.set_status_text(QString::from(format!("Navigation rejected: {error}")))
                }
            },
            Err(NavigationError::ExternalSchemeRequiresConfirmation { scheme, url }) => {
                self.as_mut().request_external_navigation(scheme, url);
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Navigation rejected: {error}")));
            }
        }
    }

    pub(super) fn paste_open(mut self: Pin<&mut Self>, target: &QString) -> bool {
        self.as_mut().paste_open_channel(target, false)
    }

    pub(super) fn paste_open_primary(mut self: Pin<&mut Self>, target: &QString) -> bool {
        self.as_mut().paste_open_channel(target, true)
    }

    pub(super) fn paste_open_channel(
        mut self: Pin<&mut Self>,
        target: &QString,
        primary: bool,
    ) -> bool {
        let target = target.to_string();
        if !matches!(target.as_str(), "current" | "tab") {
            self.set_status_text(QString::from(
                "Clipboard navigation target must be current or tab",
            ));
            return false;
        }
        let raw = if primary {
            qobject::ferric_browser_read_primary_selection().to_string()
        } else {
            qobject::ferric_browser_read_clipboard().to_string()
        };
        let input = match validate_clipboard_navigation_input(&raw) {
            Ok(input) => input,
            Err(error) => {
                let status = if primary && raw.trim().is_empty() {
                    "Primary selection is unavailable or empty; navigation was not started"
                } else {
                    error.as_str()
                };
                self.set_status_text(QString::from(status));
                return false;
            }
        };
        let url = match resolve_input(&input, &self.as_ref().navigation_context()) {
            Ok(result) => result.url,
            Err(NavigationError::ExternalSchemeRequiresConfirmation { scheme, url }) => {
                return self.as_mut().request_external_navigation(scheme, url);
            }
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Clipboard navigation rejected: {error}"
                )));
                return false;
            }
        };
        if target == "tab" && self.as_mut().new_tab() < 0 {
            self.set_status_text(QString::from("Clipboard navigation could not create a tab"));
            return false;
        }
        match self.as_mut().navigate_url(&url) {
            Ok(()) => {
                self.set_status_text(QString::from("Clipboard navigation requested"));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Clipboard navigation rejected: {error}"
                )));
                false
            }
        }
    }
}
