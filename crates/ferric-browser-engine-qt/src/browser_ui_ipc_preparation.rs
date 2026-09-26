use super::{
    CxxQtType, DispatchTarget, Event, IpcOpenTarget, IpcRoute, ParsedCommand,
    PendingLinkNavigation, Pin, QString, ValidatedUrl, Value, clean_link, clean_open_input,
    current_target, link_cleaning_policy, link_result_value, parse_switcher_command, qobject,
    resolve_input,
};

impl qobject::BrowserUi {
    pub(super) fn execute_clean_open_ipc(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        route: &IpcRoute,
        operation_id: Option<&str>,
    ) -> Result<Value, String> {
        let Some(input) = clean_open_input(command)? else {
            return Err("not a clean-link open command".into());
        };
        let navigation = resolve_input(
            &input,
            &self.as_ref().navigation_context_for_source(route.source),
        )
        .map_err(|error| error.to_string())?;
        let rules = link_cleaning_policy::active_rules(self.as_ref().rust().storage_roots.as_ref());
        let result =
            clean_link(navigation.url.as_str(), &rules).map_err(|error| error.to_string())?;
        if !result.changed {
            return self.execute_ipc_command_with_operation(
                ParsedCommand {
                    name: "open".into(),
                    arguments: vec![navigation.url.to_string()],
                },
                route,
                operation_id,
            );
        }
        let cleaned = ValidatedUrl::parse(result.cleaned.clone())
            .map_err(|error| format!("cleaned URL is invalid: {error}"))?;
        let mut value = link_result_value("open", &result, &rules);
        value["requires_confirmation"] = Value::Bool(true);
        let pending = match route.open_target {
            IpcOpenTarget::Window | IpcOpenTarget::PrivateWindow => {
                let private = route.open_target == IpcOpenTarget::PrivateWindow;
                if private && route.profile.is_some() {
                    return Err("private-window cannot select a durable profile".into());
                }
                let context_profile = route.context.as_deref().and_then(|context_name| {
                    self.as_ref()
                        .rust()
                        .contexts
                        .as_ref()?
                        .contexts()
                        .iter()
                        .find(|context| context.name == context_name)
                        .map(|context| context.profile.clone())
                });
                let profile = route
                    .profile
                    .clone()
                    .or(context_profile)
                    .unwrap_or_else(|| {
                        if private {
                            "private".into()
                        } else {
                            "secondary".into()
                        }
                    });
                let new_window_action = route.context.as_deref().map_or_else(
                    || format!("new-window\t{}\t{}\t{}", private, profile, cleaned.as_str()),
                    |context| {
                        format!(
                            "new-window\t{}\t{}\t{}\t{}",
                            private,
                            profile,
                            context,
                            cleaned.as_str()
                        )
                    },
                );
                PendingLinkNavigation {
                    target: None,
                    new_window_action: Some(new_window_action),
                    journey_parent: None,
                    url: cleaned,
                }
            }
            IpcOpenTarget::BackgroundTab => {
                let (window, original_window, original_tab, journey_parent) = {
                    let binding = self.as_ref();
                    let state = binding
                        .rust()
                        .state
                        .as_ref()
                        .ok_or_else(|| "core state unavailable".to_owned())?;
                    let window = match route.selector {
                        DispatchTarget::Active => state.active_window(),
                        DispatchTarget::LastFocused => {
                            state.last_focused_window().or(state.active_window())
                        }
                        DispatchTarget::Window(window) => Some(window),
                        DispatchTarget::Tab(tab) => state.tabs().get(&tab).map(|tab| tab.window),
                    }
                    .ok_or_else(|| "requested window is not available".to_owned())?;
                    let source_tab = match route.selector {
                        DispatchTarget::Tab(tab) => Some(tab),
                        _ => state
                            .windows()
                            .get(&window)
                            .and_then(|window| window.active_tab),
                    };
                    let journey_parent =
                        source_tab.and_then(|tab| state.journey().current_node(tab));
                    (
                        window,
                        state.active_window(),
                        state.windows()[&window].active_tab,
                        journey_parent,
                    )
                };
                self.as_mut()
                    .reduce_event(Event::OpenTab { window })
                    .map_err(|error| error.clone())?;
                let tab = self
                    .as_ref()
                    .rust()
                    .state
                    .as_ref()
                    .and_then(|state| state.windows().get(&window))
                    .and_then(|window| window.active_tab)
                    .ok_or_else(|| "new background tab was not created".to_owned())?;
                if let Some(original_tab) = original_tab {
                    self.as_mut()
                        .reduce_event(Event::ActivateTab {
                            window,
                            tab: original_tab,
                        })
                        .map_err(|error| error.clone())?;
                } else if let Some(original_window) = original_window
                    && original_window != window
                {
                    self.as_mut()
                        .reduce_event(Event::FocusWindow {
                            window: original_window,
                        })
                        .map_err(|error| error.clone())?;
                }
                value["tab_id"] = Value::String(tab.to_string());
                let target = current_target(self.as_ref().rust().state.as_ref(), Some(tab))
                    .ok_or_else(|| "new background tab target is not live".to_owned())?;
                PendingLinkNavigation {
                    target: Some(target),
                    new_window_action: None,
                    journey_parent,
                    url: cleaned,
                }
            }
            IpcOpenTarget::Tab => {
                let binding = self.as_ref();
                let state = binding
                    .rust()
                    .state
                    .as_ref()
                    .ok_or_else(|| "core state unavailable".to_owned())?;
                let window = match route.selector {
                    DispatchTarget::Active => state.active_window(),
                    DispatchTarget::LastFocused => {
                        state.last_focused_window().or(state.active_window())
                    }
                    DispatchTarget::Window(window) => Some(window),
                    DispatchTarget::Tab(tab) => state.tabs().get(&tab).map(|tab| tab.window),
                }
                .ok_or_else(|| "requested window is not available".to_owned())?;
                let target = state
                    .windows()
                    .get(&window)
                    .and_then(|window| window.active_tab)
                    .and_then(|tab| current_target(Some(state), Some(tab)))
                    .ok_or_else(|| "requested tab is not live".to_owned())?;
                PendingLinkNavigation {
                    target: Some(target),
                    new_window_action: None,
                    journey_parent: None,
                    url: cleaned,
                }
            }
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = None;
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_link_navigation = Some(pending);
        self.as_mut().publish_link_preview("open", &result, true);
        self.as_mut().set_link_preview_visible(true);
        // Clean background opens also restore the original active tab. Keep
        // the newly-created pending tab in the full projected tab order.
        self.as_mut().sync_tab_order_from_core();
        self.set_status_text(QString::from("Clean-link navigation awaits confirmation"));
        Ok(value)
    }

    pub(super) fn assign_ipc_context(
        mut self: Pin<&mut Self>,
        route: &IpcRoute,
    ) -> Result<(), String> {
        let Some(context_name) = route.context.as_deref() else {
            return Ok(());
        };
        let window = {
            let binding = self.as_ref();
            let state = binding
                .rust()
                .state
                .as_ref()
                .ok_or_else(|| "core state unavailable".to_owned())?;
            match route.selector {
                DispatchTarget::Active => state.active_window(),
                DispatchTarget::LastFocused => {
                    state.last_focused_window().or(state.active_window())
                }
                DispatchTarget::Window(window) => Some(window),
                DispatchTarget::Tab(tab) => state.tabs().get(&tab).map(|tab| tab.window),
            }
            .ok_or_else(|| "requested window is not available".to_owned())?
        };
        self.as_mut()
            .reduce_event(Event::SetWindowContext {
                window,
                context: Some(context_name.to_owned()),
            })
            .map_err(|error| error.clone())?;
        self.as_mut().sync_core_tabs();
        Ok(())
    }

    pub(super) fn execute_switcher_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let (scope, query) = parse_switcher_command(&command.arguments)?;
        self.as_mut()
            .set_switcher_request_scope(QString::from(&scope));
        self.as_mut()
            .set_switcher_request_query(QString::from(&query));
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some("show-switcher".into());
        self.as_mut()
            .set_status_text(QString::from("Universal switcher requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "scope": scope,
            "query": query
        }))
    }
}
