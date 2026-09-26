use super::{
    CommandSource, ContextsConfig, CxxQtType, Effect, EngineEffect, Event, NavigationError,
    NavigationSource, PendingContextRoute, Pin, QString, ValidatedUrl, current_target,
    matching_context_routes, qobject, resolve_input, safe_ipc_url,
    validate_clipboard_navigation_input,
};

impl qobject::BrowserUi {
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
