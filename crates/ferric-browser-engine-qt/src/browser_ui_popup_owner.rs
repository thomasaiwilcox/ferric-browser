//! Synchronous, typed admission for a native popup's independent runtime.
//! Qt must accept a new-page request while its signal is being delivered.
//! Reuse validated profile resources rather than doing disk bootstrap there.

use super::{
    BrowserApplication, CxxQtType, Event, ExistenceState, PendingProfileConfiguration, Pin,
    PrivacyKind, QString, RuntimeInput, TabTransfer, Target, bootstrap_runtime, qobject,
};

// Private, typed state. It is never deserialized from QML, IPC or a page.
pub(super) struct PreparedPageWindow {
    source_owner: QString,
    source_window: super::WindowId,
    source_target: Target,
    context: Option<String>,
    source_context: Option<String>,
    group: String,
    profile_instance: String,
    placeholder: Target,
    destination_window: super::WindowId,
    url: super::ValidatedUrl,
    committed: bool,
}

impl qobject::BrowserUi {
    // Prepare a new window from a Core page before its first engine view is
    // allocated. The source is retained until QML confirms the destination is
    // ready, so a factory failure leaves its navigation intact. Raw validated
    // navigation stays inside Rust; IPC-safe URL projections are not reused.
    pub(super) fn initialize_page_owner(
        self: Pin<&mut Self>,
        mut destination: Pin<&mut Self>,
        tab_id: &QString,
    ) -> bool {
        let result = self
            .as_ref()
            .get_ref()
            .prepare_page_owner(destination.as_mut(), &tab_id.to_string());
        match result {
            Ok(()) => true,
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "The page window couldn't be opened: {error}"
                )));
                false
            }
        }
    }

    fn prepare_page_owner(
        &self,
        mut destination: Pin<&mut Self>,
        tab_id: &str,
    ) -> Result<(), String> {
        let tab = super::TabId::from_display(tab_id).ok_or("the page identity is invalid")?;
        let source = self.rust();
        let state = source
            .state
            .as_ref()
            .ok_or("the source window is unavailable")?;
        let window = source.window.ok_or("the source window is unavailable")?;
        if state.configuration().tabs.presentation
            != ferric_browser_config::TabsPresentation::Windows
            || !source.tab_ids.contains(&tab)
            || source.tab_ids.len() < 2
            || source.interaction.prompt_view_identities.contains_key(&tab)
        {
            return Err("only an unpresented page in window mode can use this path".into());
        }
        let page = state
            .tabs()
            .get(&tab)
            .filter(|page| page.window == window && page.existence == ExistenceState::Live)
            .ok_or("the source page is no longer available")?;
        let pending_url = source
            .pending_engine_actions
            .iter()
            .rev()
            .find_map(|request| match request {
                super::engine_requests::NativeRequest::Engine(super::EngineEffect::Navigate {
                    target,
                    url,
                }) if target.tab == tab && state.capture_target(tab) == Some(*target) => {
                    Some(url.clone())
                }
                _ => None,
            });
        if page.loading == super::LoadingState::Provisional && pending_url.is_none() {
            return Err("the navigation request is still being prepared".into());
        }
        let url = pending_url
            .or_else(|| {
                page.url
                    .as_deref()
                    .and_then(|url| super::ValidatedUrl::parse(url).ok())
            })
            .unwrap_or_else(|| {
                super::ValidatedUrl::parse("about:blank").expect("built-in blank URL")
            });
        let source_target = state.capture_target(tab).ok_or("the page is stale")?;
        let source_context = state.windows()[&window].context.clone();
        let restored = source.restored_page_windows.get(&tab).cloned();
        let context = restored.as_ref().map_or_else(
            || source_context.clone(),
            |metadata| metadata.context.clone(),
        );
        let profile_instance = state
            .profile_instance_id()
            .ok_or("the profile is unavailable")?
            .to_string();
        let source_owner = source.window_token.clone();
        let group = if let Some(metadata) = restored {
            metadata.group
        } else if source.page_window_group.is_empty() {
            source.window_token.to_string()
        } else {
            source.page_window_group.clone()
        };
        destination
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_window_preparing = true;
        let placeholder = match self.inherit_owner_profile(destination.as_mut(), None) {
            Ok(target) => target,
            Err(error) => {
                destination
                    .as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .page_window_preparing = false;
                return Err(error);
            }
        };
        destination
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_window_group = group.clone();
        if context != source_context {
            let window = destination
                .as_ref()
                .rust()
                .window
                .ok_or("the destination window is unavailable")?;
            destination
                .as_mut()
                .reduce_event(Event::SetWindowContext {
                    window,
                    context: context.clone(),
                })
                .map_err(|error| error.to_string())?;
            destination.as_mut().sync_context_metadata();
        }
        let destination_window = destination
            .as_ref()
            .rust()
            .window
            .ok_or("the destination window is unavailable")?;
        destination
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .prepared_page_window = Some(PreparedPageWindow {
            source_owner,
            source_window: window,
            source_target,
            context,
            source_context,
            group,
            profile_instance,
            placeholder,
            destination_window,
            url,
            committed: false,
        });
        Ok(())
    }

    // Native profile/window checks precede this call. Both Core mutations run
    // without a Qt callback or worker poll between final validation and commit.
    pub(super) fn commit_page_owner(
        mut self: Pin<&mut Self>,
        mut destination: Pin<&mut Self>,
    ) -> bool {
        let transfer = {
            let source = self.as_ref();
            let source = source.rust();
            let child = destination.as_ref();
            let child = child.rust();
            let Some(prepared) = child.prepared_page_window.as_ref() else {
                return false;
            };
            let (Some(state), Some(child_state), Some(child_window)) =
                (source.state.as_ref(), child.state.as_ref(), child.window)
            else {
                return false;
            };
            if prepared.committed
                || state.shutdown() != ferric_browser_core::ShutdownState::Running
                || child_state.shutdown() != ferric_browser_core::ShutdownState::Running
                || source.ipc_shutdown_gate
                || child.ipc_shutdown_gate
                || child_window != prepared.destination_window
                || !child.page_window_preparing
                || child.profile_bootstrap_pending
                || child.ipc_owner.is_some()
                || !child.interaction.prompt_view_identities.is_empty()
                || !source.view_alive
                || source.profile_bootstrap_pending
                || source.window_token != prepared.source_owner
                || source.window != Some(prepared.source_window)
                || source.tab_ids.len() < 2
                || !source.tab_ids.contains(&prepared.source_target.tab)
                || source
                    .interaction
                    .prompt_view_identities
                    .contains_key(&prepared.source_target.tab)
                || state.capture_target(prepared.source_target.tab) != Some(prepared.source_target)
                || child_state.capture_target(prepared.placeholder.tab)
                    != Some(prepared.placeholder)
                || state
                    .profile_instance_id()
                    .map(|id| id.to_string())
                    .as_deref()
                    != Some(&prepared.profile_instance)
                || state.profile_instance_id() != child_state.profile_instance_id()
                || state.windows()[&prepared.source_window].context != prepared.source_context
                || child_state.windows()[&child_window].context != prepared.context
                || child.page_window_group != prepared.group
                || state.configuration().tabs.presentation
                    != ferric_browser_config::TabsPresentation::Windows
            {
                return false;
            }
            let page = &state.tabs()[&prepared.source_target.tab];
            if page.existence != ExistenceState::Live || page.window != prepared.source_window {
                return false;
            }
            TabTransfer {
                identity: Some(ferric_browser_core::TabTransferIdentity::capture(page)),
                url: Some(prepared.url.to_string()),
                title: page.title.clone(),
                loading: super::LoadingState::Idle,
                pinned: page.pinned,
                muted: page.muted,
                zoom_hundredths: ((page.zoom * 100.0).round() as u32).clamp(25, 500),
            }
        };
        let tab = destination
            .as_ref()
            .rust()
            .prepared_page_window
            .as_ref()
            .expect("validated")
            .source_target
            .tab;
        let window = destination.as_ref().rust().window.expect("validated");
        let imported = destination
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .state
            .as_mut()
            .expect("validated")
            .dispatch_runtime(RuntimeInput::Lifecycle(Event::TransferTabIn {
                window,
                transfer,
            }));
        let Ok(imported) = imported else { return false };
        // TransferTabOut has no further admission condition after the live,
        // running source was checked above. Neither dispatch calls into Qt.
        let released = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .state
            .as_mut()
            .expect("validated")
            .dispatch_runtime(RuntimeInput::Lifecycle(Event::TransferTabOut { tab }));
        let Ok(released) = released else { return false };
        {
            let mut rust = destination.as_mut().rust_mut();
            let child = rust.as_mut().get_mut();
            child
                .prepared_page_window
                .as_mut()
                .expect("validated")
                .committed = true;
            child.page_window_preparing = false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_actions
            .retain(|request| {
                request
                    .captured_target()
                    .is_none_or(|target| target.tab != tab)
            });
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .restored_page_windows
            .remove(&tab);
        self.as_mut().observe_runtime_effects(released);
        destination.as_mut().observe_runtime_effects(imported);
        self.as_mut().sync_tab_order_from_core();
        destination.as_mut().sync_tab_order_from_core();
        true
    }

    pub(super) fn activate_prepared_page(mut self: Pin<&mut Self>) -> QString {
        if self
            .as_ref()
            .rust()
            .prepared_page_window
            .as_ref()
            .is_none_or(|prepared| !prepared.committed)
        {
            return QString::from(
                "Page opening cancelled because this window changed. Open the page again.",
            );
        }
        let prepared = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .prepared_page_window
            .take()
            .expect("committed intent");
        let current = {
            let this = self.as_ref();
            let rust = this.rust();
            rust.view_alive
                && !rust.profile_bootstrap_pending
                && !rust.ipc_shutdown_gate
                && rust.window == Some(prepared.destination_window)
                && rust.state.as_ref().is_some_and(|state| {
                    state.shutdown() == ferric_browser_core::ShutdownState::Running
                        && state.capture_target(prepared.source_target.tab)
                            == Some(prepared.source_target)
                        && state
                            .profile_instance_id()
                            .map(|id| id.to_string())
                            .as_deref()
                            == Some(&prepared.profile_instance)
                        && state
                            .windows()
                            .get(&prepared.destination_window)
                            .is_some_and(|window| {
                                window.context == prepared.context
                                    && window.active_tab == Some(prepared.source_target.tab)
                            })
                })
        };
        if !current {
            return QString::from(
                "Page opening cancelled because this window changed. Open the page again.",
            );
        }
        if self.as_mut().navigate_url(&prepared.url).is_err() {
            return QString::from(
                "The page window is ready, but its navigation couldn't start. Open the page again.",
            );
        }
        QString::default()
    }

    pub(super) fn retire_page_presentation(mut self: Pin<&mut Self>) -> bool {
        if self.as_ref().rust().state.as_ref().is_none_or(|state| {
            state.configuration().tabs.presentation
                != ferric_browser_config::TabsPresentation::Windows
        }) {
            return false;
        }
        let tabs = self.as_ref().rust().tab_ids.clone();
        for tab in tabs {
            let closed = self.as_ref().get_ref().capture_closed_page(tab);
            if self
                .as_mut()
                .reduce_event(Event::TransferTabOut { tab })
                .is_err()
            {
                return false;
            }
            if let Some(closed) = closed {
                self.as_mut().remember_closed_page(tab, closed);
            }
            super::live_ipc_owners::forget_page_order(&[tab]);
        }
        self.as_mut().sync_tab_order_from_core();
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_window_closing = true;
        self.as_mut().refresh_ipc_owner();
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_actions
            .clear();
        true
    }

    pub(super) fn handoff_popup_owner(
        mut self: Pin<&mut Self>,
        mut destination: Pin<&mut Self>,
        token: &QString,
        join_page_group: bool,
    ) -> bool {
        if let Err(error) =
            self.as_mut()
                .try_handoff_popup_owner(destination.as_mut(), token, join_page_group)
        {
            self.set_status_text(QString::from(format!("Could not open the popup: {error}")));
            return false;
        }
        true
    }

    fn try_handoff_popup_owner(
        mut self: Pin<&mut Self>,
        mut destination: Pin<&mut Self>,
        token: &QString,
        join_page_group: bool,
    ) -> Result<(), String> {
        let token = token.to_string();
        let source = self.as_ref();
        let source = source.rust();
        if source.profile_bootstrap_pending
            || !source.view_alive
            || destination
                .as_ref()
                .rust()
                .state
                .as_ref()
                .and_then(BrowserApplication::profile_snapshot)
                .is_some()
        {
            return Err("the window profile is not ready".into());
        }
        let (_, target, requested_url, _) = source
            .navigation
            .popup_journey_targets
            .iter()
            .find(|(candidate, _, _, claimed)| candidate == &token && *claimed)
            .ok_or("the popup request is no longer available")?;
        let (target, requested_url) = (*target, requested_url.clone());
        let source_state = source
            .state
            .as_ref()
            .ok_or("the opener is no longer available")?;
        let tab_state = source_state
            .tabs()
            .get(&target.tab)
            .filter(|tab| tab.existence == ExistenceState::Live)
            .ok_or("the opener page is no longer available")?;
        if source_state.capture_target(target.tab) != Some(target) {
            return Err("the popup page has changed".into());
        }
        // The native bridge reads the destination from the actual Qt popup
        // request. Capture the logical group before profile initialization
        // emits Qt callbacks; explicit windows and dialogs stay independent.
        let page_group = (join_page_group
            && source_state.configuration().tabs.presentation
                == ferric_browser_config::TabsPresentation::Windows)
            .then(|| {
                if source.page_window_group.is_empty() {
                    source.window_token.to_string()
                } else {
                    source.page_window_group.clone()
                }
            });
        let transfer = TabTransfer {
            identity: None,
            url: tab_state.url.clone(),
            title: tab_state.title.clone(),
            loading: tab_state.loading,
            pinned: false,
            muted: tab_state.muted,
            zoom_hundredths: ((tab_state.zoom * 100.0).round() as u32).clamp(25, 500),
        };
        let popup_target = self
            .as_ref()
            .get_ref()
            .inherit_owner_profile(destination.as_mut(), Some(transfer))?;
        if let Some(group) = page_group {
            destination
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .page_window_group = group;
        }
        destination
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .navigation
            .popup_journey_targets
            .push((token.clone(), popup_target, requested_url.clone(), true));
        destination
            .as_mut()
            .update_current_url(QString::from(requested_url));
        self.as_mut()
            .reduce_event(Event::TransferTabOut { tab: target.tab })
            .map_err(|error| error.to_string())?;
        let _ = self.as_mut().take_journey_transition(target);
        let _ = self.as_mut().take_journey_parent(target);
        {
            let mut rust = self.as_mut().rust_mut();
            let source = rust.as_mut().get_mut();
            source
                .popup_tab_ids
                .retain(|candidate| *candidate != target.tab);
            source
                .navigation
                .popup_journey_targets
                .retain(|(candidate, _, _, _)| candidate != &token);
            source
                .navigation
                .pending_navigation_urls
                .remove(&target.tab);
        }
        Ok(())
    }
    pub(super) fn initialize_window_owner(
        mut self: Pin<&mut Self>,
        destination: Pin<&mut Self>,
        profile_name: &QString,
    ) -> bool {
        if self.as_ref().rust().profile_name != profile_name.to_string() {
            self.set_status_text(QString::from(
                "The source profile changed. Request the new window again.",
            ));
            return false;
        }
        match self
            .as_ref()
            .get_ref()
            .inherit_owner_profile(destination, None)
        {
            Ok(_) => true,
            Err(error) => {
                self.as_mut().set_status_text(QString::from(format!(
                    "The window couldn't be opened: {error}"
                )));
                false
            }
        }
    }

    // Both ordinary same-profile windows and popups retain validated profile
    // resources. Only popup admission transfers an existing logical tab.
    pub(super) fn inherit_owner_profile(
        &self,
        mut destination: Pin<&mut Self>,
        transfer: Option<TabTransfer>,
    ) -> Result<Target, String> {
        let source = self.rust();
        if source.profile_bootstrap_pending
            || !source.view_alive
            || destination.as_ref().rust().profile_bootstrap_pending
            || destination
                .as_ref()
                .rust()
                .state
                .as_ref()
                .and_then(BrowserApplication::profile_snapshot)
                .is_some()
        {
            return Err("the window profile is not ready".into());
        }
        let source_state = source
            .state
            .as_ref()
            .ok_or("the source window is unavailable")?;
        let source_window = source
            .window
            .and_then(|id| source_state.windows().get(&id))
            .ok_or("the source window is unavailable")?;
        let profile = source_state
            .profiles()
            .get(&source_window.profile)
            .ok_or("the source profile is unavailable")?;
        let (runtime, window, _) = bootstrap_runtime(profile.privacy, &profile.label)
            .ok_or("the window could not start")?;
        let mut state = source_state
            .inherit_profile(runtime)
            .map_err(|error| error.to_string())?;
        let mut effect = source
            .profile_open_effect
            .clone()
            .ok_or("the source profile is not ready")?;
        effect.configuration = state.configuration_snapshot();
        effect.profile = state
            .profile_snapshot()
            .ok_or("the source profile is not ready")?;
        effect.profile_overrides = source.profile_overrides.clone();
        effect.runtime_overrides = source.runtime_overrides.clone();
        if let Some(transfer) = transfer {
            state
                .dispatch_runtime(RuntimeInput::Lifecycle(Event::TransferTabIn {
                    window,
                    transfer,
                }))
                .map_err(|error| error.to_string())?;
        }
        if profile.privacy == PrivacyKind::Normal && source_window.context.is_some() {
            state
                .dispatch_runtime(RuntimeInput::Lifecycle(Event::SetWindowContext {
                    window,
                    context: source_window.context.clone(),
                }))
                .map_err(|error| error.to_string())?;
        }
        let tab = state
            .windows()
            .get(&window)
            .and_then(|window| window.active_tab)
            .ok_or("the window page could not start")?;
        let target = state
            .capture_target(tab)
            .ok_or("the window page could not start")?;
        let pending = PendingProfileConfiguration {
            context_catalog_revision: source.context_catalog_revision,
            private_profile: profile.privacy != PrivacyKind::Normal,
            profile_name: source.profile_name.clone(),
            storage_base: source.profile_storage_base.clone(),
            state,
            window,
            tab,
        };
        // Setters can synchronously notify QML. Own every source value before
        // the first notification; no source Rust borrow survives reentry.
        let config_json = source.config_json.clone();
        let config_base_json = source.config_base_json.clone();
        let cli_overrides_json = source.cli_overrides_json.clone();
        let profile_overrides_json = source.profile_overrides_json.clone();
        let config_path = source.config_path.clone();
        let contexts_json = source.contexts_json.clone();
        let temporary_overrides = source.temporary_overrides.clone();
        let private_start_state = source.private_start_state.clone();
        let session = (source_state.configuration().tabs.presentation
            == ferric_browser_config::TabsPresentation::Windows)
            .then(|| {
                (
                    source.session_id,
                    source.session_path.clone(),
                    source.recovered_session_ids.clone(),
                )
            });
        destination.as_mut().set_config_json(config_json);
        destination.as_mut().set_config_base_json(config_base_json);
        destination
            .as_mut()
            .set_cli_overrides_json(cli_overrides_json);
        destination
            .as_mut()
            .set_profile_overrides_json(profile_overrides_json);
        destination.as_mut().set_config_path(config_path);
        destination.as_mut().set_contexts_json(contexts_json);
        destination.as_mut().set_profile_bootstrap_pending(true);
        destination
            .as_mut()
            .finish_configure_profile(pending, Ok(effect));
        {
            let mut rust = destination.as_mut().rust_mut();
            let child = rust.as_mut().get_mut();
            child.temporary_overrides = temporary_overrides;
            child.private_start_state = private_start_state;
            if let Some((id, path, recovered)) = session {
                child.session_id = id;
                child.session_path = path;
                child.recovered_session_ids = recovered;
                child.recovery_paths.clear();
            }
        }
        destination.as_mut().sync_context_metadata();
        Ok(target)
    }
}
