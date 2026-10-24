use super::{
    BrowserApplication, CommandSource, ContextsConfig, CxxQtType, DispatchTarget, Effect,
    EngineEffect, Event, IpcOpenTarget, IpcRoute, NavigationError, NavigationSource,
    PageOpenIntent, PendingContextRoute, Pin, QString, Target, ValidatedUrl, WindowId,
    captured_target_is_current, current_target, matching_context_routes, qobject, resolve_input,
    safe_ipc_url, validate_clipboard_navigation_input,
};

pub(super) fn context_route_target_is_current(
    application: Option<&BrowserApplication>,
    owner_window: Option<WindowId>,
    owner_token: &str,
    view_alive: bool,
    pending: &PendingContextRoute,
) -> bool {
    if !view_alive
        || owner_window != Some(pending.owner_window)
        || owner_token != pending.owner_token
    {
        return false;
    }
    let Some(application) = application else {
        return false;
    };
    application.windows().contains_key(&pending.target_window)
        && application
            .tabs()
            .get(&pending.target.tab)
            .and_then(|tab| application.profiles().get(&tab.profile))
            .is_some_and(|profile| !profile.privacy.is_transient())
        && application
            .tabs()
            .get(&pending.target.tab)
            .is_some_and(|tab| tab.window == pending.target_window)
        && captured_target_is_current(Some(application), pending.target)
}

pub(super) const fn context_route_host_intent_supported(
    profile_changed: bool,
    foreground: bool,
) -> bool {
    !profile_changed || foreground
}

pub(super) fn context_definition_by_route_name<'a>(
    config: &'a ContextsConfig,
    context_name: &str,
) -> Option<&'a ferric_browser_config::ContextDefinition> {
    config
        .contexts
        .iter()
        .find(|context| context.name == context_name)
}

pub(super) fn pending_context_route_for_target(
    config: &ContextsConfig,
    url: &ValidatedUrl,
    entry_point: &str,
    target: Target,
    target_window: WindowId,
    owner_window: WindowId,
    owner_token: String,
    foreground: bool,
) -> Option<PendingContextRoute> {
    let route = matching_context_routes(config, url.as_str(), entry_point)
        .into_iter()
        .next()?;
    let context = context_definition_by_route_name(config, &route.context)?;
    Some(PendingContextRoute {
        route_id: route.id.clone(),
        behavior: route.behavior.clone(),
        context_name: context.name.clone(),
        profile_name: context.profile.clone(),
        url: url.as_str().to_owned(),
        target,
        target_window,
        owner_window,
        owner_token,
        foreground,
        open_route: None,
    })
}

pub(super) fn take_current_context_route_record(
    pending: &mut Option<PendingContextRoute>,
    application: Option<&BrowserApplication>,
    owner_window: Option<WindowId>,
    owner_token: &str,
    view_alive: bool,
) -> Option<PendingContextRoute> {
    let is_current = pending.as_ref().is_some_and(|pending| {
        context_route_target_is_current(application, owner_window, owner_token, view_alive, pending)
    });
    if !is_current {
        pending.take();
        return None;
    }
    pending.take()
}

pub(super) fn capture_context_route_target(
    application: &BrowserApplication,
    owner_window: Option<WindowId>,
    owner_token: &str,
    view_alive: bool,
    route: &IpcRoute,
    target_window: WindowId,
) -> Option<(Target, WindowId, String, bool)> {
    if !view_alive || !application.windows().contains_key(&target_window) {
        return None;
    }
    let owner_window = owner_window?;
    let target_tab = match route.selector {
        DispatchTarget::Tab(tab) => tab,
        DispatchTarget::Active | DispatchTarget::LastFocused | DispatchTarget::Window(_) => {
            application.windows().get(&target_window)?.active_tab?
        }
    };
    let tab = application.tabs().get(&target_tab)?;
    if tab.window != target_window
        || application
            .profiles()
            .get(&tab.profile)
            .is_none_or(|profile| profile.privacy.is_transient())
    {
        return None;
    }
    let target = current_target(Some(application), Some(target_tab))?;
    let foreground = match super::browser_ui_ipc::page_open_intent(
        &application.configuration().tabs.presentation,
        route.open_target,
    ) {
        PageOpenIntent::NewPage { foreground } => foreground,
        PageOpenIntent::ExistingPresentation => route.open_target != IpcOpenTarget::BackgroundTab,
        PageOpenIntent::CurrentPage | PageOpenIntent::NewWindow { .. } => true,
    };
    Some((target, owner_window, owner_token.to_owned(), foreground))
}

impl qobject::BrowserUi {
    pub(super) fn queue_context_route(
        mut self: Pin<&mut Self>,
        url: &ValidatedUrl,
        entry_point: &str,
        route: Option<&IpcRoute>,
    ) -> bool {
        if self.as_ref().active_profile_is_transient() {
            return false;
        }
        let fallback_route = IpcRoute {
            selector: DispatchTarget::Active,
            open_target: IpcOpenTarget::Current,
            profile: None,
            context: None,
            external_open: false,
            source: CommandSource::Ui,
        };
        let route = route.unwrap_or(&fallback_route);
        let Some(target_window) = self.as_ref().ipc_route_window(route).ok() else {
            return false;
        };
        let Some((target, owner_window, owner_token, foreground)) = self
            .as_ref()
            .capture_context_route_target(route, target_window)
        else {
            return false;
        };
        let pending = {
            let contexts_json = self.as_ref().rust().contexts_json.to_string();
            let Ok(config) = serde_json::from_str::<ContextsConfig>(&contexts_json) else {
                return false;
            };
            let Some(pending) = pending_context_route_for_target(
                &config,
                url,
                entry_point,
                target,
                target_window,
                owner_window,
                owner_token,
                foreground,
            ) else {
                return false;
            };
            pending
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
        self.as_mut().set_context_route_behavior(behavior);
        self.as_mut().set_context_route_context(context);
        self.as_mut().set_context_route_profile(profile);
        self.as_mut().set_context_route_url(url);
        // Publish readiness after every field used by the prompt is present.
        self.as_mut().set_context_route_id(route_id);
        true
    }

    pub(super) fn queue_context_route_for_input(
        self: Pin<&mut Self>,
        input: &str,
        entry_point: &str,
    ) -> bool {
        let route = IpcRoute {
            selector: DispatchTarget::Active,
            open_target: IpcOpenTarget::Current,
            profile: None,
            context: None,
            external_open: false,
            source: CommandSource::Keyboard,
        };
        self.queue_context_route_for_input_with_route(input, entry_point, &route)
    }

    pub(super) fn queue_context_route_for_input_with_route(
        mut self: Pin<&mut Self>,
        input: &str,
        entry_point: &str,
        route: &IpcRoute,
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
        self.as_mut()
            .queue_context_route(&result.url, entry_point, Some(route))
    }

    fn capture_context_route_target(
        &self,
        route: &IpcRoute,
        target_window: WindowId,
    ) -> Option<(Target, WindowId, String, bool)> {
        let rust = self.rust();
        capture_context_route_target(
            rust.state.as_ref()?,
            rust.window,
            &rust.window_token.to_string(),
            rust.view_alive,
            route,
            target_window,
        )
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

    // Record the chosen outcome, so repeat does not reopen the same prompt or
    // lose a background tab / different-profile window destination.
    fn note_context_route_navigation(
        mut self: Pin<&mut Self>,
        pending: &PendingContextRoute,
        accepted: bool,
    ) {
        let Some(mut route) = pending.open_route.clone() else {
            return;
        };
        if accepted {
            if pending.profile_name != self.as_ref().rust().profile_name {
                route.open_target = super::IpcOpenTarget::Window;
            }
            route.context = Some(pending.context_name.clone());
            route.profile = Some(pending.profile_name.clone());
        } else {
            route.profile = Some(self.as_ref().rust().profile_name.clone());
            route.context = None;
        }
        self.as_mut().note_repeatable_invocation(
            &super::ParsedCommand {
                name: "open".into(),
                arguments: vec![pending.url.clone()],
            },
            Some(&route),
            1,
        );
    }

    fn take_current_context_route(mut self: Pin<&mut Self>) -> Option<PendingContextRoute> {
        let Some(pending) = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_context_route
            .take()
        else {
            self.set_status_text(QString::from("No context route is pending"));
            return None;
        };
        let rust = self.as_ref().rust();
        let mut current = Some(pending);
        let Some(pending) = take_current_context_route_record(
            &mut current,
            rust.state.as_ref(),
            rust.window,
            &rust.window_token.to_string(),
            rust.view_alive,
        ) else {
            self.as_mut().clear_context_route();
            self.set_status_text(QString::from(
                "Context route target changed; the request was discarded",
            ));
            return None;
        };
        self.as_mut().clear_context_route();
        Some(pending)
    }

    pub(super) fn accept_context_route(mut self: Pin<&mut Self>) -> bool {
        let Some(pending) = self.as_mut().take_current_context_route() else {
            return false;
        };
        let Ok(url) = ValidatedUrl::parse(pending.url.clone()) else {
            self.set_status_text(QString::from("Context route URL is invalid"));
            return false;
        };
        let profile_changed = pending.profile_name != self.as_ref().rust().profile_name;
        if !context_route_host_intent_supported(profile_changed, pending.foreground) {
            self.set_status_text(QString::from(
                "A background context open cannot create a nonactivated window",
            ));
            return false;
        }
        let planned_intent = self.as_ref().rust().state.as_ref().and_then(|state| {
            pending.open_route.as_ref().map(|route| {
                super::browser_ui_ipc::page_open_intent(
                    &state.configuration().tabs.presentation,
                    route.open_target,
                )
            })
        });
        let explicit_window = pending
            .open_route
            .as_ref()
            .is_some_and(|route| route.open_target == super::IpcOpenTarget::Window);
        if explicit_window || profile_changed {
            let presentation = self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .map(|state| &state.configuration().tabs.presentation);
            let window_privacy = if profile_changed {
                presentation.and_then(
                    |presentation| match super::browser_ui_ipc::page_open_intent(
                        presentation,
                        super::IpcOpenTarget::Window,
                    ) {
                        PageOpenIntent::NewWindow { privacy } => Some(privacy),
                        PageOpenIntent::ExistingPresentation => {
                            Some(super::PageOpenPrivacy::Normal)
                        }
                        _ => None,
                    },
                )
            } else {
                planned_intent.and_then(|intent| match intent {
                    PageOpenIntent::NewWindow { privacy } => Some(privacy),
                    PageOpenIntent::ExistingPresentation => Some(super::PageOpenPrivacy::Normal),
                    _ => None,
                })
            };
            let Some(window_privacy) = window_privacy else {
                self.set_status_text(QString::from("Context route host intent is unavailable"));
                return false;
            };
            if window_privacy != super::PageOpenPrivacy::Normal {
                self.set_status_text(QString::from("Context routing cannot open a private host"));
                return false;
            }
            let profile = pending.profile_name.clone();
            let action = super::browser_ui_ipc_preparation::window_open_action(
                false,
                &profile,
                Some(&pending.context_name),
                false,
                url.as_str(),
            );
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some(action);
            self.as_mut().note_context_route_navigation(&pending, true);
            self.set_status_text(QString::from(format!(
                "Context route accepted; opening {} in profile {}",
                pending.context_name, pending.profile_name
            )));
            return true;
        }
        let context_exists = self
            .as_ref()
            .rust()
            .contexts
            .as_ref()
            .is_some_and(|contexts| {
                contexts.contexts().iter().any(|context| {
                    context.name == pending.context_name && context.profile == pending.profile_name
                })
            });
        if !context_exists {
            self.set_status_text(QString::from("Context route target is unavailable"));
            return false;
        }
        if self
            .as_mut()
            .reduce_event(Event::SetWindowContext {
                window: pending.target_window,
                context: Some(pending.context_name.clone()),
            })
            .is_err()
        {
            self.set_status_text(QString::from("Context route assignment was rejected"));
            return false;
        }
        let navigation = if let Some(route) = pending.open_route.as_ref().filter(|route| {
            matches!(
                route.open_target,
                super::IpcOpenTarget::Tab | super::IpcOpenTarget::BackgroundTab
            )
        }) {
            let foreground = match planned_intent {
                Some(PageOpenIntent::NewPage { foreground }) => foreground,
                _ => route.open_target != super::IpcOpenTarget::BackgroundTab,
            };
            self.as_mut()
                .dispatch_planned_new_tab_open(url.as_str(), route, foreground, None)
                .map(|_| ())
        } else {
            self.as_mut().navigate_url_to_target(pending.target, &url)
        };
        if navigation.is_err() {
            self.set_status_text(QString::from("Context route navigation was rejected"));
            return false;
        }
        self.as_mut().sync_core_tabs();
        self.as_mut().note_context_route_navigation(&pending, true);
        self.set_status_text(QString::from(format!(
            "Context route accepted: {}",
            pending.context_name
        )));
        true
    }

    pub(super) fn dismiss_context_route(mut self: Pin<&mut Self>) -> bool {
        let Some(pending) = self.as_mut().take_current_context_route() else {
            return false;
        };
        let Ok(url) = ValidatedUrl::parse(pending.url.clone()) else {
            self.set_status_text(QString::from("Context route URL is invalid"));
            return false;
        };
        let planned_intent = self.as_ref().rust().state.as_ref().and_then(|state| {
            pending.open_route.as_ref().map(|route| {
                super::browser_ui_ipc::page_open_intent(
                    &state.configuration().tabs.presentation,
                    route.open_target,
                )
            })
        });
        if pending
            .open_route
            .as_ref()
            .is_some_and(|route| route.open_target == super::IpcOpenTarget::Window)
        {
            let profile = self.as_ref().rust().profile_name.clone();
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action =
                Some(super::browser_ui_ipc_preparation::window_open_action(
                    false,
                    &profile,
                    None,
                    true,
                    url.as_str(),
                ));
            self.as_mut().note_context_route_navigation(&pending, false);
            self.set_status_text(QString::from(
                "Opening a new window without context routing",
            ));
            return true;
        }
        let navigation = if let Some(route) = pending.open_route.as_ref().filter(|route| {
            matches!(
                route.open_target,
                super::IpcOpenTarget::Tab | super::IpcOpenTarget::BackgroundTab
            )
        }) {
            let foreground = match planned_intent {
                Some(PageOpenIntent::NewPage { foreground }) => foreground,
                _ => route.open_target != super::IpcOpenTarget::BackgroundTab,
            };
            self.as_mut()
                .dispatch_planned_new_tab_open(url.as_str(), route, foreground, None)
                .map(|_| ())
        } else {
            self.as_mut().navigate_url_to_target(pending.target, &url)
        };
        match navigation {
            Ok(()) => {
                self.as_mut().note_context_route_navigation(&pending, false);
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
        self.navigate_url_to_target(target, url)
    }

    fn navigate_url_to_target(
        mut self: Pin<&mut Self>,
        target: Target,
        url: &ValidatedUrl,
    ) -> Result<(), String> {
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
        self.as_mut().set_pending_engine_action(&effects);
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
                    .queue_context_route(&result.url, "explicit-open", None)
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
                ) && self
                    .as_mut()
                    .queue_context_route(&result.url, &entry_point, None)
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
