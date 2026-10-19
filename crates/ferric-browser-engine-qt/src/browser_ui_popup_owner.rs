//! Synchronous, typed admission for a native popup's independent runtime.
//! Qt must accept a new-page request while its signal is being delivered.
//! Reuse validated profile resources rather than doing disk bootstrap there.

use super::{
    BrowserApplication, CxxQtType, Event, ExistenceState, PendingProfileConfiguration, Pin,
    PrivacyKind, QString, RuntimeInput, TabTransfer, Target, bootstrap_runtime, qobject,
};

impl qobject::BrowserUi {
    pub(super) fn handoff_popup_owner(
        mut self: Pin<&mut Self>,
        mut destination: Pin<&mut Self>,
        token: &QString,
    ) -> bool {
        if let Err(error) = self
            .as_mut()
            .try_handoff_popup_owner(destination.as_mut(), token)
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
        let transfer = TabTransfer {
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
    fn inherit_owner_profile(
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
        let (runtime, window, tab) = bootstrap_runtime(profile.privacy, &profile.label)
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
        destination
            .as_mut()
            .set_config_json(source.config_json.clone());
        destination
            .as_mut()
            .set_config_base_json(source.config_base_json.clone());
        destination
            .as_mut()
            .set_cli_overrides_json(source.cli_overrides_json.clone());
        destination
            .as_mut()
            .set_profile_overrides_json(source.profile_overrides_json.clone());
        destination
            .as_mut()
            .set_config_path(source.config_path.clone());
        destination
            .as_mut()
            .set_contexts_json(source.contexts_json.clone());
        destination.as_mut().set_profile_bootstrap_pending(true);
        destination
            .as_mut()
            .finish_configure_profile(pending, Ok(effect));
        {
            let mut rust = destination.as_mut().rust_mut();
            let child = rust.as_mut().get_mut();
            child.temporary_overrides = source.temporary_overrides.clone();
            child.private_start_state = source.private_start_state.clone();
        }
        destination.as_mut().sync_context_metadata();
        Ok(target)
    }
}
