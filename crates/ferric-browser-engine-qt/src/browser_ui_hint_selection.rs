use super::browser_ui_scope_handoff::{hint_capabilities_json, known_hint_capability_id};
use super::hint_policy::{HintScopePurpose, LocalInteractionOperation, LocalInteractionOwner};
use super::{
    CommandSource, CxxQtType, DispatchTarget, Effect, EngineEffect, Event, HintTarget,
    HintTransport, IpcOpenTarget, IpcRoute, JourneyEdgeKind, Mode, ParsedCommand, Pin, QString,
    TabId, ValidatedUrl, Value, clean_link, current_target, external_hint_target,
    hint_intent_for_candidate, hint_kind_name, hint_uses_url_action, link_cleaning_policy,
    parse_hint_candidate, qobject, rapid_hint_keeps_mode, safe_ipc_url,
};

const HINT_CAPABILITY_ACTION_PREFIX: &str = "hint.capability.";

impl qobject::BrowserUi {
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
            if target == "choose" {
                if self.as_ref().rust().pending_hint_palette_permit.is_some() {
                    self.set_status_text(QString::from("Hint action palette is already open"));
                    return QString::from(r#"{"error":"palette-already-open"}"#);
                }
            } else if !self.as_mut().consume_hint_palette_permit() {
                self.set_status_text(QString::from(
                    "Hint action rejected: no local palette request",
                ));
                return QString::from(r#"{"error":"local-palette-permit-required"}"#);
            }
            let is_palette_entry = target == "choose";
            target.clone_into(
                &mut self
                    .as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .hint_rapid_target,
            );
            let response =
                self.as_mut()
                    .select_hint_internal(label, fresh_candidate_json, is_palette_entry);
            if is_palette_entry
                && serde_json::from_str::<Value>(&response.to_string())
                    .ok()
                    .and_then(|value| {
                        value
                            .get("action")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                    })
                    .as_deref()
                    == Some("choose")
                && !self.as_mut().mint_hint_palette_permit()
            {
                self.set_status_text(QString::from(
                    "Hint palette rejected: local authority unavailable",
                ));
                return QString::from(r#"{"error":"palette-permit-unavailable"}"#);
            }
            return response;
        }
        if let Some(capability_id) = action_id.strip_prefix(HINT_CAPABILITY_ACTION_PREFIX) {
            if action_id.len() > 128 || !known_hint_capability_id(capability_id) {
                self.set_status_text(QString::from("Hint capability rejected: invalid action ID"));
                return QString::from(r#"{"error":"invalid-capability-id"}"#);
            }
            if !self.as_mut().consume_hint_palette_permit() {
                self.set_status_text(QString::from(
                    "Hint capability rejected: no local palette request",
                ));
                return QString::from(r#"{"error":"local-palette-permit-required"}"#);
            }
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_hint_action = Some(action_id);
            return self.select_hint(label, fresh_candidate_json);
        }
        if !action_id.starts_with("userscript.") || action_id.len() > 256 {
            self.set_status_text(QString::from("Hint action rejected: invalid action ID"));
            return QString::from(r#"{"error":"invalid-action-id"}"#);
        }
        if !self.as_mut().consume_hint_palette_permit() {
            self.set_status_text(QString::from(
                "Hint action rejected: no local palette request",
            ));
            return QString::from(r#"{"error":"local-palette-permit-required"}"#);
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_hint_action = Some(action_id);
        self.select_hint(label, fresh_candidate_json)
    }

    pub(super) fn cancel_hint_palette(mut self: Pin<&mut Self>) {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.pending_hint_palette_permit = None;
        if this.hint_rapid_target == "choose" {
            "current".clone_into(&mut this.hint_rapid_target);
        }
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
        self.as_mut()
            .select_hint_internal(label, fresh_candidate_json, false)
    }

    fn select_hint_internal(
        mut self: Pin<&mut Self>,
        label: &QString,
        fresh_candidate_json: &QString,
        allow_local_palette: bool,
    ) -> QString {
        if self.as_ref().rust().hint_pointer_mode {
            self.set_status_text(QString::from(
                "Pointer Hint selection requires Enter release; no DOM click was sent",
            ));
            return QString::from(r#"{"error":"pointer-confirmation-required"}"#);
        }
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
        let configuration_revision = self.as_ref().rust().hint_configuration_revision;
        if self
            .as_ref()
            .rust()
            .hint_session
            .as_ref()
            .is_some_and(|session| session.configuration_revision != configuration_revision)
        {
            self.set_status_text(QString::from(
                "Hint selection rejected: configuration changed",
            ));
            return QString::from(r#"{"error":"stale-configuration"}"#);
        }
        let selector_snapshot = match super::browser_ui_navigation::effective_hint_selector_snapshot(
            &self.as_ref().rust().config_json.to_string(),
            &self.as_ref().rust().current_url.to_string(),
            configuration_revision,
            &self.as_ref().rust().hint_family.to_string(),
        ) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Hint selection rejected: selector policy unavailable: {error}"
                )));
                return QString::from(r#"{"error":"selector-policy-unavailable"}"#);
            }
        };
        let current_selector_revision = selector_snapshot
            .get("selector_revision")
            .and_then(Value::as_str)
            .and_then(|revision| u64::from_str_radix(revision, 16).ok());
        if self
            .as_ref()
            .rust()
            .hint_session
            .as_ref()
            .is_some_and(|session| {
                Some(session.selector_policy_revision) != current_selector_revision
            })
        {
            self.set_status_text(QString::from(
                "Hint selection rejected: selector policy changed",
            ));
            return QString::from(r#"{"error":"stale-selector-policy"}"#);
        }
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
                    && candidate.related_element_id == fresh.related_element_id
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
        if matches!(
            selected.kind,
            ferric_browser_core::HintKind::EditorFocus
                | ferric_browser_core::HintKind::LabelControl
        ) && !self.as_ref().rust().hint_proxy_targets_enabled
        {
            self.set_status_text(QString::from(
                "Hint selection rejected: validated proxy targets are disabled",
            ));
            return QString::from(r#"{"error":"proxy-targets-disabled"}"#);
        }
        let requested_capability = pending_action
            .as_deref()
            .and_then(|action| action.strip_prefix(HINT_CAPABILITY_ACTION_PREFIX));
        let is_facet = ferric_browser_core::hint_kind_is_region_or_surface(selected.kind);
        let choose = requested_capability.is_none()
            && pending_action.is_none()
            && (self.as_ref().rust().hint_rapid_target == "choose" || is_facet);
        let uses_url_action = hint_uses_url_action(
            selected.kind,
            self.as_ref().rust().hint_rapid,
            &self.as_ref().rust().hint_rapid_target,
            pending_action.is_some(),
        );
        let result = if let Some(capability_id) = requested_capability {
            let capability = ferric_browser_core::hint_capabilities(selected.kind)
                .iter()
                .copied()
                .find(|capability| capability.id() == capability_id);
            let Some(capability) = capability else {
                self.set_status_text(QString::from(
                    "Hint capability rejected: action is not valid for this target",
                ));
                return QString::from(r#"{"error":"unsupported-capability"}"#);
            };
            match capability {
                ferric_browser_core::HintCapability::SearchWithin
                    if matches!(
                        selected.kind,
                        ferric_browser_core::HintKind::Region
                            | ferric_browser_core::HintKind::ScrollableRegion
                    ) =>
                {
                    match self.as_mut().begin_hint_scope_handoff(
                        &selected,
                        &fresh,
                        HintScopePurpose::Search,
                    ) {
                        Ok(result) => result,
                        Err(error) => return error,
                    }
                }
                ferric_browser_core::HintCapability::HintWithin
                    if matches!(
                        selected.kind,
                        ferric_browser_core::HintKind::Region
                            | ferric_browser_core::HintKind::ScrollableRegion
                    ) =>
                {
                    match self.as_mut().begin_hint_scope_handoff(
                        &selected,
                        &fresh,
                        HintScopePurpose::Hint,
                    ) {
                        Ok(result) => result,
                        Err(error) => return error,
                    }
                }
                ferric_browser_core::HintCapability::GridWithin
                    if matches!(
                        selected.kind,
                        ferric_browser_core::HintKind::Region
                            | ferric_browser_core::HintKind::ScrollableRegion
                            | ferric_browser_core::HintKind::Surface
                            | ferric_browser_core::HintKind::FocusableSurface
                    ) =>
                {
                    match self.as_mut().begin_grid_scope_handoff(&selected, &fresh) {
                        Ok(result) => result,
                        Err(error) => return error,
                    }
                }
                ferric_browser_core::HintCapability::SetScrollTarget
                    if selected.kind == ferric_browser_core::HintKind::ScrollableRegion =>
                {
                    serde_json::json!({
                        "action":"scroll-target",
                        "kind":hint_kind_name(selected.kind),
                        "element_id":selected.element_id,
                        "frame_path":fresh.frame_path,
                    })
                }
                ferric_browser_core::HintCapability::Focus
                    if selected.kind == ferric_browser_core::HintKind::FocusableSurface =>
                {
                    serde_json::json!({
                        "action":"focus",
                        "kind":hint_kind_name(selected.kind),
                        "element_id":selected.element_id,
                        "frame_path":fresh.frame_path,
                        "x":fresh.geometry.x + (fresh.geometry.width / 2.0),
                        "y":fresh.geometry.y + (fresh.geometry.height / 2.0)
                    })
                }
                _ => {
                    self.set_status_text(QString::from(
                        "Hint capability is not available in this interaction mode",
                    ));
                    return QString::from(r#"{"error":"unsupported-capability"}"#);
                }
            }
        } else if choose {
            if !allow_local_palette {
                self.set_status_text(QString::from(
                    "Hint action rejected: palette requires a local UI entry",
                ));
                return QString::from(r#"{"error":"local-palette-required"}"#);
            }
            let mut result = serde_json::json!({
                "action":"choose",
                "kind":hint_kind_name(selected.kind),
                "label":label.to_string(),
                "has_url":selected.href.is_some(),
            });
            if is_facet {
                result["capabilities"] = hint_capabilities_json(selected.kind);
            }
            result
        } else {
            let transport = hint_intent_for_candidate(selected.kind, uses_url_action).transport();
            if transport == HintTransport::ValidatedUrlAction {
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
                                serde_json::json!({"error":"userscript-action-rejected"})
                                    .to_string(),
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
                            self.set_status_text(QString::from(
                                "Rapid hint target state is invalid",
                            ));
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
                    self.as_mut().set_status_text(QString::from(
                        "Hint link queued for a same-profile window",
                    ));
                    serde_json::json!({
                        "action":"window",
                        "action_id":"browser.link.open",
                        "kind":"link",
                        "target":"window",
                        "url":safe_ipc_url(url.as_str())
                    })
                } else if let Some(target_name) =
                    external_hint_target(&self.as_ref().rust().hint_rapid_target)
                        .map(ToOwned::to_owned)
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
                        "Hint userscript started: {script}"
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
            } else if transport == HintTransport::ScrollTargetPolicy {
                serde_json::json!({
                    "action":"scroll-target",
                    "kind":"scrollable",
                    "element_id":selected.element_id,
                    "frame_path":fresh.frame_path,
                })
            } else if transport == HintTransport::ExactDomFocus {
                serde_json::json!({
                    "action":"focus",
                    "kind": hint_kind_name(selected.kind),
                    "element_id": selected.element_id,
                    "frame_path": fresh.frame_path,
                    "x": fresh.geometry.x + (fresh.geometry.width / 2.0),
                    "y": fresh.geometry.y + (fresh.geometry.height / 2.0)
                })
            } else if transport == HintTransport::SemanticDomClick {
                serde_json::json!({
                    "action":"click",
                    "kind": hint_kind_name(selected.kind),
                    "element_id": selected.element_id,
                    "frame_path": fresh.frame_path,
                    "x": fresh.geometry.x + (fresh.geometry.width / 2.0),
                    "y": fresh.geometry.y + (fresh.geometry.height / 2.0)
                })
            } else {
                self.set_status_text(QString::from("Hint intent has no enabled transport"));
                return QString::from(r#"{"error":"hint-transport-unavailable"}"#);
            }
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
            Some(
                "focus"
                    | "scroll-target"
                    | "click"
                    | "hint-within"
                    | "target-search-within"
                    | "grid-within"
            )
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
}

impl qobject::BrowserUi {
    fn mint_hint_palette_permit(mut self: Pin<&mut Self>) -> bool {
        // This private issuer is called only by the direct QML hint.choose route.
        let permit = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(owner) = local_hint_owner(this) else {
                return false;
            };
            this.hint_local_interaction_authority.mint(
                CommandSource::Ui,
                owner,
                LocalInteractionOperation::HintPaletteSelection,
            )
        };
        let Some(permit) = permit else {
            return false;
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_hint_palette_permit = Some(permit);
        true
    }

    fn consume_hint_palette_permit(mut self: Pin<&mut Self>) -> bool {
        let permit = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(permit) = this.pending_hint_palette_permit.take() else {
                return false;
            };
            let Some(owner) = local_hint_owner(this) else {
                return false;
            };
            this.hint_local_interaction_authority.consume(
                permit,
                CommandSource::Ui,
                owner,
                LocalInteractionOperation::HintPaletteSelection,
            )
        };
        permit
    }
}

pub(super) fn local_hint_owner(rust: &super::BrowserUiRust) -> Option<LocalInteractionOwner> {
    let session = rust.hint_session.as_ref()?;
    Some(LocalInteractionOwner {
        window: rust.window?,
        session: session.id,
        target: session.target,
        configuration_revision: rust.hint_configuration_revision,
    })
}

#[cfg(test)]
mod tests {
    use super::super::browser_ui_scope_handoff::{
        hint_capabilities_json, known_hint_capability_id,
    };
    use ferric_browser_core::HintKind;

    #[test]
    fn region_surface_palette_actions_are_rust_derived_and_bounded() {
        let regions = hint_capabilities_json(HintKind::ScrollableRegion);
        let regions = regions.as_array().expect("capability array");
        assert_eq!(regions.len(), 4);
        assert_eq!(regions[0]["id"], "hint-within");
        assert_eq!(regions[0]["enabled"], true);
        assert_eq!(regions[1]["id"], "search-within");
        assert_eq!(regions[1]["enabled"], true);
        assert_eq!(regions[2]["id"], "grid-within");
        assert_eq!(regions[2]["enabled"], true);
        assert_eq!(regions[3]["id"], "set-scroll-target");
        assert_eq!(regions[3]["enabled"], true);

        let surface = hint_capabilities_json(HintKind::FocusableSurface);
        let surface = surface.as_array().expect("capability array");
        assert_eq!(surface.len(), 2);
        assert_eq!(surface[0]["id"], "grid-within");
        assert_eq!(surface[0]["enabled"], true);
        assert_eq!(surface[1]["id"], "focus");
        assert_eq!(surface[1]["enabled"], true);

        assert!(known_hint_capability_id("set-scroll-target"));
        assert!(!known_hint_capability_id("click"));
    }

    #[test]
    fn scoped_actions_are_available_only_for_region_and_surface_kinds() {
        let region = hint_capabilities_json(HintKind::Region);
        assert_eq!(region[0]["id"], "hint-within");
        assert_eq!(region[0]["enabled"], true);

        let surface = hint_capabilities_json(HintKind::Surface);
        assert!(
            surface
                .as_array()
                .expect("surface capabilities")
                .iter()
                .all(|capability| capability["id"] != "hint-within")
        );
    }
}
