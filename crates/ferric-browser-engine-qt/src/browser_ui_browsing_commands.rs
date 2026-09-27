use super::{
    CommandRegistry, CxxQtType, Event, ExistenceState, JourneyEdgeKind, Mode, ParsedCommand, Pin,
    QString, ScrollTargetAction, SearchCase, TabId, ValidatedUrl, Value, current_target,
    is_bounded_untrusted_text, is_safe_history_url, parse_scroll_options,
    parse_scroll_target_options, parse_search_next_options, qobject, safe_ipc_url,
};

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ScrollTargetEffect {
    StartHints,
    QueueEngineAction(String),
}

pub(super) fn dispatch_scroll_target_command(
    command: &ParsedCommand,
    registry: &CommandRegistry,
    core_mode: Mode,
    hint_chrome_available: bool,
    target_tab: Option<TabId>,
    mut apply_effect: impl FnMut(ScrollTargetEffect),
) -> Result<Value, String> {
    registry
        .validate(command, core_mode)
        .map_err(|error| error.to_string())?;
    let action = parse_scroll_target_options(&command.arguments)?;
    let action_name = action.as_str();
    if action == ScrollTargetAction::Select {
        if !hint_chrome_available {
            return Err(
                "scroll-target requires full browser chrome and is unavailable in this window"
                    .into(),
            );
        }
        apply_effect(ScrollTargetEffect::StartHints);
        return Ok(serde_json::json!({
            "status": "accepted",
            "action": "select",
            "mode": "hint"
        }));
    }
    let tab_id = target_tab.map_or_else(String::new, |tab| tab.to_string());
    apply_effect(ScrollTargetEffect::QueueEngineAction(format!(
        "scroll-target\t{tab_id}\t{action_name}"
    )));
    Ok(serde_json::json!({
        "status": "accepted",
        "action": action_name,
        "pending": true
    }))
}

impl qobject::BrowserUi {
    pub(super) fn execute_tab_undo_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if !command.arguments.is_empty() {
            return Err("tab-undo does not accept arguments".into());
        }
        let id = self
            .as_ref()
            .rust()
            .closed_tabs
            .first()
            .map(|closed| closed.id.to_string())
            .ok_or_else(|| "no eligible closed tab is available".to_owned())?;
        if !self.as_mut().activate_switcher_result(
            &QString::from("closed"),
            &QString::from(&id),
            &QString::default(),
        ) {
            return Err("closed tab could not be reopened".into());
        }
        Ok(serde_json::json!({"status": "reopened", "closed_id": id}))
    }

    pub(super) fn execute_tab_clone_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if !command.arguments.is_empty() {
            return Err("tab-clone does not accept arguments".into());
        }
        if self.as_ref().rust().pending_engine_action.is_some() {
            return Err("another browser operation is already pending".into());
        }
        let source_url = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(|state| {
                self.as_ref()
                    .rust()
                    .tab
                    .and_then(|tab| state.tabs().get(&tab))
            })
            .and_then(|tab| tab.url.as_deref())
            .map_or_else(|| "about:blank".to_owned(), safe_ipc_url);
        if !is_safe_history_url(&source_url) {
            return Err("current tab has no safe URL descriptor to clone".into());
        }
        let url = ValidatedUrl::parse(source_url.clone())
            .map_err(|error| format!("current tab URL is not safe to clone: {error}"))?;
        let index = self.as_mut().new_tab();
        if index < 0 {
            return Err("clone tab creation was rejected".into());
        }
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        )
        .ok_or_else(|| "clone tab is no longer live".to_owned())?;
        let effects = self
            .as_mut()
            .reduce_event(Event::StartNavigation { target, url })
            .map_err(|error| error.to_string())?;
        self.as_mut()
            .mark_journey_transition(&effects, JourneyEdgeKind::Navigate, "tab-clone");
        self.as_mut().set_pending_engine_action(&effects);
        self.as_mut().sync_core_tabs();
        Ok(serde_json::json!({
            "status": "accepted",
            "action": "clone",
            "tab_id": target.tab.to_string(),
            "index": index,
            "url": source_url,
            "pending": true
        }))
    }

    pub(super) fn execute_reopen_in_window_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if !command.arguments.is_empty() {
            return Err("reopen-in-window does not accept arguments".into());
        }
        if self.as_ref().rust().pending_engine_action.is_some() {
            return Err("another browser operation is already pending".into());
        }
        let source_url = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(|state| {
                self.as_ref()
                    .rust()
                    .tab
                    .and_then(|tab| state.tabs().get(&tab))
            })
            .and_then(|tab| tab.url.as_deref())
            .map_or_else(|| "about:blank".to_owned(), safe_ipc_url);
        if !is_safe_history_url(&source_url) {
            return Err("current tab has no safe URL descriptor to reopen".into());
        }
        ValidatedUrl::parse(source_url.clone())
            .map_err(|error| format!("current tab URL is not safe to reopen: {error}"))?;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("reopen-window-confirm\t{source_url}"));
        self.as_mut().set_status_text(QString::from(
            "Reopen in same-profile window awaits confirmation; live state will be lost",
        ));
        Ok(serde_json::json!({
            "status": "accepted",
            "action": "reopen-in-window",
            "url": source_url,
            "warning": "Live page state, forms, media, and in-progress engine work are not preserved",
            "pending": true,
            "requires_confirmation": true
        }))
    }

    pub(super) fn execute_tab_detach_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        operation_id: Option<&str>,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if !command.arguments.is_empty() {
            return Err("tab-detach does not accept arguments".into());
        }
        let action = operation_id.map_or_else(
            || "tab-detach".to_owned(),
            |operation_id| format!("tab-detach\t{operation_id}"),
        );
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(action);
        self.as_mut()
            .set_status_text(QString::from("Live tab detach requested"));
        let mut result = serde_json::json!({
            "status": "accepted",
            "action": "tab-detach",
            "pending": true,
            "preserves_live_state": true
        });
        if let Some(operation_id) = operation_id {
            result["operation_id"] = serde_json::Value::String(operation_id.to_owned());
        }
        Ok(result)
    }

    pub(super) fn execute_tab_give_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        operation_id: Option<&str>,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let [window_id] = command.arguments.as_slice() else {
            return Err("tab-give requires exactly one WINDOW_ID".into());
        };
        if !is_bounded_untrusted_text(window_id) {
            return Err("tab-give requires a bounded target window ID".into());
        }
        if self.as_ref().rust().pending_engine_action.is_some() {
            return Err("another browser operation is already pending".into());
        }
        let action = operation_id.map_or_else(
            || format!("tab-give\t{window_id}"),
            |operation_id| format!("tab-give\t{window_id}\t{operation_id}"),
        );
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(action);
        self.as_mut()
            .set_status_text(QString::from("Live tab transfer requested"));
        let mut result = serde_json::json!({
            "status": "accepted",
            "action": "tab-give",
            "target_window": window_id,
            "pending": true,
            "warning": "The live view will be reparented into the target same-profile window"
        });
        if let Some(operation_id) = operation_id {
            result["operation_id"] = serde_json::Value::String(operation_id.to_owned());
        }
        Ok(result)
    }

    pub(super) fn execute_zoom_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let [requested] = command.arguments.as_slice() else {
            return Err("zoom requires in, out, reset, or a factor".into());
        };
        let (tab, current) = {
            let binding = self.as_ref();
            let rust = binding.rust();
            let tab = rust.tab.ok_or_else(|| "no active tab".to_owned())?;
            let current = rust
                .state
                .as_ref()
                .and_then(|state| state.tabs().get(&tab))
                .filter(|tab| tab.existence == ExistenceState::Live)
                .map(|tab| tab.zoom)
                .ok_or_else(|| "current tab is no longer live".to_owned())?;
            (tab, current)
        };
        let requested_factor = match requested.as_str() {
            "in" => current * 1.1,
            "out" => current / 1.1,
            "reset" => 1.0,
            value => value
                .parse::<f64>()
                .map_err(|_| "zoom factor must be in, out, reset, or a number".to_owned())?,
        };
        if !requested_factor.is_finite() {
            return Err("zoom factor must be finite".into());
        }
        let zoom = (requested_factor.clamp(0.25, 5.0) * 100.0).round() / 100.0;
        let zoom_hundredths = (zoom * 100.0).round() as u32;
        self.as_mut()
            .reduce_event(Event::SetTabZoom {
                tab,
                zoom_hundredths,
            })
            .map_err(|error| error.to_string())?;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("zoom\t{tab}\t{zoom:.2}"));
        self.as_mut().set_status_text(QString::from(format!(
            "Zoom set to {}%",
            (zoom * 100.0).round()
        )));
        Ok(serde_json::json!({
            "status": "accepted",
            "action": "zoom",
            "tab_id": tab.to_string(),
            "factor": zoom,
            "pending": true
        }))
    }

    pub(super) fn execute_search_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let mut backward = false;
        let mut case = SearchCase::Smart;
        let mut case_seen = false;
        let mut query = Vec::new();
        let mut index = 0;
        while index < command.arguments.len() {
            match command.arguments[index].as_str() {
                "--backward" if !backward => backward = true,
                "--backward" => return Err("search accepts --backward at most once".into()),
                "--case" => {
                    if case_seen {
                        return Err("search accepts --case at most once".into());
                    }
                    case_seen = true;
                    let value = command.arguments.get(index + 1).ok_or_else(|| {
                        "search --case requires smart, sensitive, or insensitive".to_owned()
                    })?;
                    case = match value.as_str() {
                        "smart" => SearchCase::Smart,
                        "sensitive" => SearchCase::Sensitive,
                        "insensitive" => SearchCase::Insensitive,
                        _ => {
                            return Err(
                                "search --case requires smart, sensitive, or insensitive".into()
                            );
                        }
                    };
                    index += 1;
                }
                "--" => {
                    query.extend(command.arguments[index + 1..].iter().cloned());
                    break;
                }
                value if value.starts_with("--") => {
                    return Err(format!("unknown search option: {value}"));
                }
                value => query.push(value.to_owned()),
            }
            index += 1;
        }
        let query = query.join(" ");
        if query.is_empty() || query.len() > 64 * 1024 || query.chars().any(char::is_control) {
            return Err("search requires bounded query text without control characters".into());
        }
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        )
        .ok_or_else(|| "search target is no longer live".to_owned())?;
        let effects = self
            .as_mut()
            .reduce_event(Event::StartSearch {
                target,
                query: query.clone(),
                backward,
                case,
            })
            .map_err(|error| error.to_string())?;
        self.as_mut().set_pending_engine_action(&effects);
        self.as_mut().set_search_text(QString::from(query.as_str()));
        self.as_mut().set_search_backward(backward);
        self.as_mut()
            .set_status_text(QString::from("Search requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "action": "search",
            "query": query,
            "backward": backward,
            "case": match case {
                SearchCase::Smart => "smart",
                SearchCase::Sensitive => "sensitive",
                SearchCase::Insensitive => "insensitive",
            },
            "pending": true
        }))
    }

    pub(super) fn execute_search_next_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let (backward, count) =
            parse_search_next_options(&command.arguments, self.as_ref().rust().search_backward)?;
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        )
        .ok_or_else(|| "search target is no longer live".to_owned())?;
        if self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(|state| state.tabs().get(&target.tab))
            .and_then(|tab| tab.search.as_ref())
            .is_none()
        {
            return Err("no retained in-page search".into());
        }
        for _ in 0..count {
            self.as_mut()
                .reduce_event(Event::SearchNext { target, backward })
                .map_err(|error| error.to_string())?;
        }
        self.as_mut().set_search_backward(backward);
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!(
            "find-next\t{count}\t{backward}\t{}",
            self.as_ref()
                .rust()
                .state
                .as_ref()
                .and_then(|state| state.tabs().get(&target.tab))
                .and_then(|tab| tab.search.as_ref())
                .map(|search| match search.case {
                    SearchCase::Smart => "smart",
                    SearchCase::Sensitive => "sensitive",
                    SearchCase::Insensitive => "insensitive",
                })
                .unwrap_or("smart")
        ));
        self.as_mut().set_status_text(QString::from(format!(
            "Search {} match{}",
            if backward { "backward" } else { "forward" },
            if count == 1 { "" } else { "es" }
        )));
        Ok(serde_json::json!({
            "status": "accepted",
            "action": "search-next",
            "tab_id": target.tab.to_string(),
            "backward": backward,
            "count": count,
            "pending": true
        }))
    }

    pub(super) fn execute_scroll_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let (kind, direction, half, count) = match command.name.as_str() {
            "scroll" => {
                let direction = command
                    .arguments
                    .first()
                    .ok_or_else(|| "scroll requires a direction".to_owned())?;
                if !matches!(direction.as_str(), "up" | "down" | "left" | "right") {
                    return Err("scroll direction must be up, down, left, or right".into());
                }
                let (half, count) = parse_scroll_options("scroll", &command.arguments[1..])?;
                ("scroll", direction.as_str(), half, count)
            }
            "scroll-page" => {
                let direction = command
                    .arguments
                    .first()
                    .ok_or_else(|| "scroll-page requires up or down".to_owned())?;
                if !matches!(direction.as_str(), "up" | "down") {
                    return Err("scroll-page direction must be up or down".into());
                }
                let (half, count) = parse_scroll_options("scroll-page", &command.arguments[1..])?;
                ("scroll-page", direction.as_str(), half, count)
            }
            "scroll-to" => {
                let edge = command
                    .arguments
                    .first()
                    .ok_or_else(|| "scroll-to requires top or bottom".to_owned())?;
                if !matches!(edge.as_str(), "top" | "bottom") || command.arguments.len() != 1 {
                    return Err("scroll-to requires exactly top or bottom".into());
                }
                ("scroll-to", edge.as_str(), false, 1)
            }
            _ => return Err("unsupported scroll command".into()),
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(match kind {
            "scroll" => format!("scroll\t{direction}\t{count}"),
            "scroll-page" => format!("scroll-page\t{direction}\t{half}\t{count}"),
            _ => format!("scroll-to\t{direction}"),
        });
        self.as_mut()
            .set_status_text(QString::from("Page scroll requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "action": kind,
            "direction": direction,
            "half": half,
            "count": count,
            "pending": true
        }))
    }

    pub(super) fn execute_scroll_target_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let registry = self.as_ref().rust().registry.clone();
        let core_mode = self.as_ref().rust().core_mode;
        let hint_chrome_available = self.as_ref().rust().hint_chrome_available;
        let target_tab = self
            .as_ref()
            .tab_for_index(self.as_ref().rust().active_tab_index);
        dispatch_scroll_target_command(
            command,
            &registry,
            core_mode,
            hint_chrome_available,
            target_tab,
            |effect| match effect {
                ScrollTargetEffect::StartHints => {
                    self.as_mut()
                        .set_hint_options("scrollables", false, "current", None, false, 1)
                }
                ScrollTargetEffect::QueueEngineAction(action) => {
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .pending_engine_action = Some(action);
                }
            },
        )
    }
}
