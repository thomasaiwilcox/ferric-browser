use super::{
    Config, CxxQtType, Event, ExistenceState, IpcRoute, ParsedCommand, Pin, QString,
    ResourceLifecycle, Uuid, Value, hyprland, is_bounded_untrusted_text, qobject,
    typed_ipc_command,
};

impl qobject::BrowserUi {
    pub(super) fn new_tab(mut self: Pin<&mut Self>) -> i32 {
        let Some(window) = self.as_ref().rust().window else {
            self.set_status_text(QString::from("No active window"));
            return -1;
        };
        if let Err(error) = self.as_mut().reduce_event(Event::OpenTab { window }) {
            self.set_status_text(QString::from(error));
            return -1;
        }
        let (tab, index, count) = {
            let rust = self.as_ref().get_ref().rust();
            let Some(state) = rust.state.as_ref() else {
                return -1;
            };
            let Some(tab) = state
                .windows()
                .get(&window)
                .and_then(|window| window.active_tab)
            else {
                return -1;
            };
            let index = rust.tab_ids.len();
            (tab, index, index.saturating_add(1))
        };
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.tab_ids.push(tab);
            this.tab = Some(tab);
        }
        let index = i32::try_from(index).unwrap_or(i32::MAX);
        self.as_mut()
            .set_tab_count(i32::try_from(count).unwrap_or(i32::MAX));
        self.as_mut().set_active_tab_properties(index, tab);
        self.set_status_text(QString::from("New tab"));
        index
    }

    pub(super) fn set_tab_pinned(mut self: Pin<&mut Self>, index: i32, pinned: bool) -> i32 {
        let Some(tab) = self.as_ref().get_ref().tab_for_index(index) else {
            self.set_status_text(QString::from("Unknown tab"));
            return -1;
        };
        match self
            .as_mut()
            .reduce_event(Event::SetTabPinned { tab, pinned })
        {
            Ok(_) => {
                self.as_mut().sync_tab_order_from_core();
                self.as_mut().set_status_text(QString::from(if pinned {
                    "Tab pinned"
                } else {
                    "Tab unpinned"
                }));
                self.as_ref()
                    .rust()
                    .tab_ids
                    .iter()
                    .position(|candidate| *candidate == tab)
                    .and_then(|position| i32::try_from(position).ok())
                    .unwrap_or(-1)
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Tab pinning rejected: {error}")));
                -1
            }
        }
    }

    pub(super) fn set_tab_muted(mut self: Pin<&mut Self>, index: i32, muted: bool) -> bool {
        let Some(tab) = self.as_ref().get_ref().tab_for_index(index) else {
            self.set_status_text(QString::from("Unknown tab"));
            return false;
        };
        match self
            .as_mut()
            .reduce_event(Event::SetTabMuted { tab, muted })
        {
            Ok(_) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some(format!("tab-mute\t{tab}\t{muted}"));
                self.set_status_text(QString::from(if muted {
                    "Tab muted"
                } else {
                    "Tab unmuted"
                }));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Tab mute rejected: {error}")));
                false
            }
        }
    }

    pub(super) fn move_tab(mut self: Pin<&mut Self>, index: i32, delta: i32) -> i32 {
        let Some(tab) = self.as_ref().get_ref().tab_for_index(index) else {
            self.set_status_text(QString::from("Unknown tab"));
            return -1;
        };
        let binding = self.as_ref();
        let Some(state) = binding.rust().state.as_ref() else {
            return -1;
        };
        let Some((window_id, pinned)) = state
            .tabs()
            .get(&tab)
            .map(|tab_state| (tab_state.window, tab_state.pinned))
        else {
            return -1;
        };
        let group = state
            .windows()
            .get(&window_id)
            .map(|window| {
                window
                    .tabs
                    .iter()
                    .filter(|candidate| {
                        state
                            .tabs()
                            .get(candidate)
                            .is_some_and(|tab| tab.pinned == pinned)
                    })
                    .copied()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let Some(current) = group.iter().position(|candidate| *candidate == tab) else {
            return -1;
        };
        let Some(next) = i64::try_from(current)
            .ok()
            .and_then(|current| current.checked_add(i64::from(delta)))
            .and_then(|next| usize::try_from(next).ok())
            .filter(|next| *next < group.len())
        else {
            return i32::try_from(current).unwrap_or(i32::MAX);
        };
        let target_index = state
            .windows()
            .get(&window_id)
            .and_then(|window| {
                window
                    .tabs
                    .iter()
                    .position(|candidate| *candidate == group[next])
            })
            .unwrap_or_else(|| usize::try_from(index).unwrap_or(0));
        if self
            .as_mut()
            .reduce_event(Event::MoveTab {
                tab,
                to_window: window_id,
                index: Some(target_index),
            })
            .is_err()
        {
            self.set_status_text(QString::from("Tab move rejected"));
            return -1;
        }
        self.as_mut().sync_tab_order_from_core();
        self.as_ref()
            .rust()
            .tab_ids
            .iter()
            .position(|candidate| *candidate == tab)
            .and_then(|position| i32::try_from(position).ok())
            .unwrap_or(-1)
    }

    pub(super) fn tab_has_active_operations(self: Pin<&mut Self>, id: &QString) -> bool {
        let id = id.to_string();
        let binding = self.as_ref();
        let rust = binding.rust();
        let Some(tab) = rust
            .tab_ids
            .iter()
            .chain(rust.popup_tab_ids.iter())
            .find(|candidate| candidate.to_string() == id)
            .copied()
        else {
            return true;
        };
        rust.pending_link_navigation
            .as_ref()
            .and_then(|pending| pending.target)
            .is_some_and(|target| target.tab == tab)
            || rust
                .pending_selection
                .as_ref()
                .is_some_and(|pending| pending.target.tab == tab)
            || rust
                .pending_caret
                .as_ref()
                .is_some_and(|pending| pending.target.tab == tab)
            || rust
                .pending_editor
                .as_ref()
                .is_some_and(|pending| pending.target.tab == tab)
            || rust
                .pending_spawn
                .as_ref()
                .is_some_and(|pending| pending.target.tab == tab)
            || rust
                .pending_action_target
                .as_ref()
                .is_some_and(|pending| pending.target.tab == tab)
            || rust
                .pending_userscript
                .as_ref()
                .is_some_and(|pending| pending.target.tab == tab)
            || rust
                .pending_download
                .as_ref()
                .is_some_and(|pending| pending.target.tab == tab)
            || rust.pending_navigation_urls.contains_key(&tab)
            || rust.pending_redirect_tabs.contains(&tab)
            || rust.pending_redirect_hops.contains_key(&tab)
            || rust
                .popup_journey_targets
                .iter()
                .any(|(_, target, _, _)| target.tab == tab)
            || rust
                .active_site_experiment
                .as_ref()
                .is_some_and(|experiment| {
                    experiment.tab == tab || experiment.temporary_tab == Some(tab)
                })
    }

    pub(super) fn commit_tab_suspend(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let id = id.to_string();
        if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
            self.set_status_text(QString::from("Tab suspension ID is invalid"));
            return false;
        }
        let index = self.as_mut().tab_index_for_id(&QString::from(id.as_str()));
        if index < 0 {
            self.set_status_text(QString::from("Tab suspension target is stale"));
            return false;
        }
        let Some(tab) = self.as_ref().tab_for_index(index) else {
            self.set_status_text(QString::from("Tab suspension target is stale"));
            return false;
        };
        if self
            .as_mut()
            .reduce_event(Event::SuspendTab { tab })
            .is_err()
        {
            self.set_status_text(QString::from(
                "Tab suspension rejected; only hidden live tabs can be frozen",
            ));
            return false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("tab-suspend\t{id}"));
        self.set_status_text(QString::from(
            "Tab suspension approved by the browser engine",
        ));
        true
    }

    pub(super) fn commit_tab_discard(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let id = id.to_string();
        if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
            self.set_status_text(QString::from("Tab discard ID is invalid"));
            return false;
        }
        let index = self.as_mut().tab_index_for_id(&QString::from(id.as_str()));
        if index < 0 {
            self.set_status_text(QString::from("Tab discard target is stale"));
            return false;
        }
        let Some(tab) = self.as_ref().tab_for_index(index) else {
            self.set_status_text(QString::from("Tab discard target is stale"));
            return false;
        };
        if self
            .as_mut()
            .reduce_event(Event::DiscardTab { tab })
            .is_err()
        {
            self.set_status_text(QString::from(
                "Tab discard rejected; only hidden live tabs can be discarded",
            ));
            return false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("tab-discard\t{id}"));
        self.set_status_text(QString::from("Tab discard approved by the browser engine"));
        true
    }

    pub(super) fn select_tab(mut self: Pin<&mut Self>, index: i32) -> bool {
        let Some(tab) = self.as_ref().get_ref().tab_for_index(index) else {
            self.set_status_text(QString::from("Unknown tab"));
            return false;
        };
        let Some(window) = self.as_ref().rust().window else {
            return false;
        };
        let was_non_active = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(|state| state.tabs().get(&tab))
            .is_some_and(|tab| tab.resources != ResourceLifecycle::Active);
        if self
            .as_mut()
            .reduce_event(Event::ActivateTab { window, tab })
            .is_err()
        {
            self.set_status_text(QString::from("Tab activation rejected"));
            return false;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut().get_mut().tab = Some(tab);
        }
        if was_non_active && self.as_mut().reduce_event(Event::ResumeTab { tab }).is_ok() {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some(format!("tab-resume\t{tab}"));
        }
        self.as_mut().set_active_tab_properties(index, tab);
        self.set_status_text(QString::from(if was_non_active {
            "Tab selected; resuming inactive page"
        } else {
            "Tab selected"
        }));
        true
    }

    pub(super) fn close_tab(mut self: Pin<&mut Self>, index: i32) -> bool {
        let Some(target) = self.as_ref().get_ref().target_for_index(index) else {
            self.set_status_text(QString::from("Tab is no longer live"));
            return false;
        };
        if let Err(error) = self
            .as_mut()
            .reduce_event(Event::CloseTab { tab: target.tab })
        {
            self.set_status_text(QString::from(format!("Tab close rejected: {error}")));
            return false;
        }
        // Complete the core close handshake in one Rust call. Splitting
        // CloseTab and ViewClosed across two consecutive QML invocations can
        // expose the intermediate Closing state to Qt property callbacks,
        // leaving the window without an active tab before the acknowledgement
        // reaches the reducer.
        self.as_mut()
            .finish_tab_close(index, target.tab, target.generation)
    }

    pub(super) fn execute_tab_close_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let mut id = None;
        let mut count = 1_u32;
        let mut count_seen = false;
        let mut index = 0;
        while index < command.arguments.len() {
            match command.arguments[index].as_str() {
                "--id" => {
                    if id.is_some() {
                        return Err("tab-close accepts --id at most once".into());
                    }
                    id = Some(
                        command
                            .arguments
                            .get(index + 1)
                            .ok_or_else(|| "tab-close --id requires a tab ID".to_owned())?
                            .clone(),
                    );
                    index += 2;
                }
                "--count" => {
                    if count_seen {
                        return Err("tab-close accepts --count at most once".into());
                    }
                    let value = command.arguments.get(index + 1).ok_or_else(|| {
                        "tab-close --count requires a value from 1 to 100".to_owned()
                    })?;
                    count = value
                        .parse::<u32>()
                        .ok()
                        .filter(|value| (1..=100).contains(value))
                        .ok_or_else(|| "tab-close count must be 1 to 100".to_owned())?;
                    count_seen = true;
                    index += 2;
                }
                value if !value.starts_with('-') && id.is_none() => {
                    id = Some(value.to_owned());
                    index += 1;
                }
                _ => return Err("tab-close accepts [--id ID] [--count N]".into()),
            }
        }
        if id.is_some() && count_seen {
            return Err("tab-close cannot combine --id and --count".into());
        }
        if let Some(id) = id {
            if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
                return Err("tab-close ID is invalid".into());
            }
            let tab_index = self.as_mut().tab_index_for_id(&QString::from(id.as_str()));
            if tab_index < 0 {
                return Err("tab-close target is stale or outside this window".into());
            }
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some(format!("tab-close\t{id}"));
            self.as_mut()
                .set_status_text(QString::from("Tab close requested"));
            return Ok(serde_json::json!({
                "status": "accepted",
                "action": "close",
                "tab_id": id,
                "pending": true
            }));
        }
        if self.as_ref().rust().tab_ids.is_empty() {
            return Err("tab-close has no live tab target".into());
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("tab-close-active\t{count}"));
        self.as_mut()
            .set_status_text(QString::from("Tab close requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "action": "close",
            "count": count,
            "pending": true
        }))
    }

    pub(super) fn execute_tab_action(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        operation_id: Option<&str>,
    ) -> Result<Value, String> {
        let (id, direction) = if command.name == "tab-move"
            && command.arguments.first().map(String::as_str) == Some("--context")
        {
            let Some(context) = command.arguments.get(1) else {
                return Err("tab-move --context requires a context name".into());
            };
            let active = self.as_ref().rust().active_tab_index;
            let Some(tab) = self.as_ref().tab_for_index(active) else {
                return Err("tab-move context has no active tab".into());
            };
            (tab.to_string(), vec!["--context".into(), context.clone()])
        } else if command.name == "tab-move"
            && command.arguments.len() == 3
            && command.arguments.get(1).map(String::as_str) == Some("--context")
        {
            let candidate = &command.arguments[0];
            let index = self
                .as_mut()
                .tab_index_for_id(&QString::from(candidate.as_str()));
            let index = if index >= 0 {
                index
            } else {
                let parsed = candidate
                    .parse::<usize>()
                    .ok()
                    .filter(|index| *index > 0)
                    .and_then(|index| index.checked_sub(1))
                    .and_then(|index| i32::try_from(index).ok())
                    .ok_or_else(|| {
                        "tab-move index must be a positive displayed index".to_owned()
                    })?;
                parsed
            };
            let Some(tab) = self.as_ref().tab_for_index(index) else {
                return Err("tab-move index is outside this window".into());
            };
            (
                tab.to_string(),
                vec!["--context".into(), command.arguments[2].clone()],
            )
        } else {
            let Some(id) = command.arguments.first() else {
                return Err(format!("{} requires a stable tab ID", command.name));
            };
            (id.clone(), command.arguments[1..].to_vec())
        };
        if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
            return Err("tab action ID is invalid".into());
        }
        let index = self.as_mut().tab_index_for_id(&QString::from(id.as_str()));
        if index < 0 {
            return Err("tab action target is stale or outside this window".into());
        }
        let tab = self
            .as_ref()
            .tab_for_index(index)
            .ok_or_else(|| "tab action target is stale".to_owned())?;
        let live = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(|state| state.tabs().get(&tab))
            .is_some_and(|tab| tab.existence == ExistenceState::Live);
        if !live {
            return Err("tab action target is stale".into());
        }
        match command.name.as_str() {
            "tab-focus" => {
                if !direction.is_empty() {
                    return Err("tab-focus accepts only a stable tab ID".into());
                }
                if !self.as_mut().select_tab(index) {
                    return Err("tab focus was rejected".into());
                }
                Ok(serde_json::json!({
                    "status": "accepted",
                    "action": "focus",
                    "tab_id": id
                }))
            }
            "tab-close" => {
                if !direction.is_empty() {
                    return Err("tab-close accepts only a stable tab ID".into());
                }
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some(format!("tab-close\t{id}"));
                self.as_mut()
                    .set_status_text(QString::from("Tab close requested"));
                Ok(serde_json::json!({
                    "status": "accepted",
                    "action": "close",
                    "tab_id": id,
                    "pending": true
                }))
            }
            "tab-suspend" => {
                if !direction.is_empty() {
                    return Err("tab-suspend accepts only a stable tab ID".into());
                }
                let resources = self
                    .as_ref()
                    .rust()
                    .state
                    .as_ref()
                    .and_then(|state| state.tabs().get(&tab))
                    .map(|tab| tab.resources)
                    .ok_or_else(|| "tab action target is stale".to_owned())?;
                if resources != ResourceLifecycle::Active {
                    return Err("tab is already suspended or is closing".into());
                }
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some(format!("tab-suspend-request\t{id}"));
                self.as_mut().set_status_text(QString::from(
                    "Tab suspension requested; waiting for engine eligibility",
                ));
                Ok(serde_json::json!({
                    "status": "accepted",
                    "action": "suspend",
                    "tab_id": id,
                    "pending": true
                }))
            }
            "tab-discard" => {
                if !direction.is_empty() {
                    return Err("tab-discard accepts only a stable tab ID".into());
                }
                let resources = self
                    .as_ref()
                    .rust()
                    .state
                    .as_ref()
                    .and_then(|state| state.tabs().get(&tab))
                    .map(|tab| tab.resources)
                    .ok_or_else(|| "tab action target is stale".to_owned())?;
                if resources != ResourceLifecycle::Active {
                    return Err("tab is already inactive or is closing".into());
                }
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some(format!("tab-discard-request\t{id}"));
                self.as_mut().set_status_text(QString::from(
                    "Tab discard requested; waiting for engine eligibility",
                ));
                Ok(serde_json::json!({
                    "status": "accepted",
                    "action": "discard",
                    "tab_id": id,
                    "pending": true
                }))
            }
            "tab-resume" => {
                if !direction.is_empty() {
                    return Err("tab-resume accepts only a stable tab ID".into());
                }
                let resources = self
                    .as_ref()
                    .rust()
                    .state
                    .as_ref()
                    .and_then(|state| state.tabs().get(&tab))
                    .map(|tab| tab.resources)
                    .ok_or_else(|| "tab action target is stale".to_owned())?;
                if resources == ResourceLifecycle::Active {
                    return Ok(serde_json::json!({
                        "status": "accepted",
                        "action": "resume",
                        "tab_id": id,
                        "pending": false
                    }));
                }
                self.as_mut()
                    .reduce_event(Event::ResumeTab { tab })
                    .map_err(|error| error.to_string())?;
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = Some(format!("tab-resume\t{id}"));
                self.as_mut()
                    .set_status_text(QString::from("Tab resume requested"));
                Ok(serde_json::json!({
                    "status": "accepted",
                    "action": "resume",
                    "tab_id": id,
                    "pending": true
                }))
            }
            "tab-mute" => {
                if direction.len() > 1 {
                    return Err("tab-mute accepts at most one state".into());
                }
                let currently_muted = self
                    .as_ref()
                    .rust()
                    .state
                    .as_ref()
                    .and_then(|state| state.tabs().get(&tab))
                    .is_some_and(|tab| tab.muted);
                let muted = match direction.first().map(String::as_str) {
                    None | Some("toggle") => !currently_muted,
                    Some("on") => true,
                    Some("off") => false,
                    Some(_) => return Err("tab-mute state must be on, off, or toggle".into()),
                };
                if !self.as_mut().set_tab_muted(index, muted) {
                    return Err("tab mute was rejected".into());
                }
                Ok(serde_json::json!({
                    "status": "accepted",
                    "action": "mute",
                    "tab_id": id,
                    "muted": muted,
                    "pending": true
                }))
            }
            "tab-move" => {
                if let [flag, context] = direction.as_slice()
                    && flag == "--context"
                {
                    if !is_bounded_untrusted_text(context) {
                        return Err("tab-move context is empty, oversized, or invalid".into());
                    }
                    if index != self.as_ref().rust().active_tab_index
                        && !self.as_mut().select_tab(index)
                    {
                        return Err("tab-move context could not select the requested tab".into());
                    }
                    let action = operation_id.map_or_else(
                        || format!("tab-move-context\t{context}\t{id}"),
                        |operation_id| format!("tab-move-context\t{context}\t{id}\t{operation_id}"),
                    );
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .pending_engine_action = Some(action);
                    self.as_mut()
                        .set_status_text(QString::from("Live tab context move requested"));
                    let mut result = serde_json::json!({
                        "status": "accepted",
                        "action": "move",
                        "tab_id": id,
                        "context": context,
                        "pending": true
                    });
                    if let Some(operation_id) = operation_id {
                        result["operation_id"] = serde_json::Value::String(operation_id.to_owned());
                    }
                    return Ok(result);
                }
                let [direction] = direction.as_slice() else {
                    return Err("tab-move requires left or right".into());
                };
                let delta = match direction.as_str() {
                    "left" => -1,
                    "right" => 1,
                    _ => return Err("tab-move direction must be left or right".into()),
                };
                let new_index = self.as_mut().move_tab(index, delta);
                if new_index < 0 {
                    return Err("tab move was rejected".into());
                }
                Ok(serde_json::json!({
                    "status": "accepted",
                    "action": "move",
                    "tab_id": id,
                    "direction": direction,
                    "index": new_index
                }))
            }
            "tab-pin" => {
                if direction.len() > 1 {
                    return Err("tab-pin accepts at most one state".into());
                }
                let currently_pinned = self
                    .as_ref()
                    .rust()
                    .state
                    .as_ref()
                    .and_then(|state| state.tabs().get(&tab))
                    .is_some_and(|tab| tab.pinned);
                let pinned = match direction.first().map(String::as_str) {
                    None | Some("toggle") => !currently_pinned,
                    Some("on") => true,
                    Some("off") => false,
                    Some(_) => return Err("tab-pin state must be on, off, or toggle".into()),
                };
                let new_index = self.as_mut().set_tab_pinned(index, pinned);
                if new_index < 0 {
                    return Err("tab pinning was rejected".into());
                }
                Ok(serde_json::json!({
                    "status": "accepted",
                    "action": "pin",
                    "tab_id": id,
                    "pinned": pinned,
                    "index": new_index
                }))
            }
            _ => Err("unsupported tab action".into()),
        }
    }

    pub(super) fn execute_window_action(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let [id] = command.arguments.as_slice() else {
            return Err(format!("{} requires a stable window ID", command.name));
        };
        if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
            return Err("window action ID is invalid".into());
        }
        let local_target = self.as_ref().rust().state.as_ref().and_then(|state| {
            state
                .windows()
                .values()
                .find(|window| window.id.to_string() == *id)
                .map(|window| (window.id, window.active_tab))
        });
        if let Some((window, active_tab)) = local_target {
            self.as_mut()
                .reduce_event(Event::FocusWindow { window })
                .map_err(|error| error.to_string())?;
            if let Some(tab) = active_tab {
                self.as_mut()
                    .reduce_event(Event::ActivateTab { window, tab })
                    .map_err(|error| error.to_string())?;
            }
            self.as_mut().sync_tab_order_from_core();
        } else if Uuid::parse_str(id).is_err() {
            return Err("window action target is stale".into());
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("window-focus\t{id}"));
        self.as_mut()
            .set_status_text(QString::from("Window focus requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "action": "focus",
            "window_id": id,
            "pending": true
        }))
    }

    pub(super) fn execute_window_move(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let [id, workspace] = command.arguments.as_slice() else {
            return Err(format!("{} requires WINDOW_ID and WORKSPACE", command.name));
        };
        if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
            return Err("window move ID is invalid".into());
        }
        if !hyprland::valid_workspace_selector(workspace) {
            return Err("window move workspace selector is invalid".into());
        }
        let config =
            serde_json::from_value::<Config>(self.rust().config.clone()).unwrap_or_default();
        let adapter = hyprland::HyprlandAdapter::from_config(&config.hyprland);
        if !adapter.workspace_routing_enabled() || adapter.status() != "ready" {
            return Err("compositor window movement unavailable".into());
        }
        let Some((window, active_tab)) = self.as_ref().rust().state.as_ref().and_then(|state| {
            state
                .windows()
                .values()
                .find(|window| window.id.to_string() == *id)
                .map(|window| (window.id, window.active_tab))
        }) else {
            return Err("window move target is stale".into());
        };
        self.as_mut()
            .reduce_event(Event::FocusWindow { window })
            .map_err(|error| error.to_string())?;
        if let Some(tab) = active_tab {
            self.as_mut()
                .reduce_event(Event::ActivateTab { window, tab })
                .map_err(|error| error.to_string())?;
        }
        self.as_mut().sync_tab_order_from_core();
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("window-move\t{id}\t{workspace}"));
        self.as_mut().set_status_text(QString::from(
            "Window activation requested before compositor move",
        ));
        Ok(serde_json::json!({
            "status": "accepted",
            "action": "move",
            "window_id": id,
            "workspace": workspace,
            "pending": true
        }))
    }

    pub(super) fn execute_command_action(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        route: Option<&IpcRoute>,
    ) -> Result<Value, String> {
        let (id, encoded_arguments) = match command.arguments.as_slice() {
            [id] => (id.as_str(), None),
            [id, encoded] if command.name == "command-execute" => {
                (id.as_str(), Some(encoded.as_str()))
            }
            _ => return Err(format!("{} requires a stable command ID", command.name)),
        };
        if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
            return Err("command action ID is invalid".into());
        }
        let definition = self
            .as_ref()
            .rust()
            .registry
            .definitions()
            .iter()
            .find(|definition| definition.action.to_string() == *id)
            .cloned()
            .ok_or_else(|| "command action target is stale".to_owned())?;
        match command.name.as_str() {
            "command-help" => {
                if encoded_arguments.is_some() {
                    return Err("command-help does not accept typed command arguments".into());
                }
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action =
                    Some(format!("show-binding-help\t{}", definition.name));
                self.as_mut()
                    .set_status_text(QString::from("Command help requested"));
                Ok(serde_json::json!({
                    "status": "accepted",
                    "action": "help",
                    "command_id": id
                }))
            }
            "command-execute" => {
                if matches!(
                    definition.name.as_str(),
                    "action" | "command-help" | "command-execute"
                ) {
                    return Err("command action cannot recursively invoke an action command".into());
                }
                let typed_arguments = encoded_arguments
                    .map(|encoded| {
                        serde_json::from_str::<Value>(encoded).map_err(|error| {
                            format!("captured command arguments are invalid JSON: {error}")
                        })
                    })
                    .transpose()?
                    .unwrap_or_else(|| serde_json::json!({}));
                if !typed_arguments.is_object() {
                    return Err("captured command arguments must be an object".into());
                }
                let (parsed, _) = typed_ipc_command(&serde_json::json!({
                    "command": definition.name,
                    "arguments": typed_arguments
                }))
                .map_err(|error| error.to_string())?;
                let mode = self.as_ref().rust().core_mode;
                self.as_ref()
                    .rust()
                    .registry
                    .validate(&parsed, mode)
                    .map_err(|error| error.to_string())?;
                if let Some(route) = route {
                    let result = self.as_mut().execute_ipc_command(parsed, route)?;
                    return Ok(serde_json::json!({
                        "status": "accepted",
                        "action": "execute",
                        "command_id": id,
                        "result": result
                    }));
                }
                if !self.as_mut().execute_parsed_commands(vec![parsed]) {
                    return Err(self.as_ref().rust().status_text.to_string());
                }
                Ok(serde_json::json!({
                    "status": "accepted",
                    "action": "execute",
                    "command_id": id
                }))
            }
            _ => Err("unsupported command action".into()),
        }
    }
}
