use super::{
    ChromePreferences, Config, ConfigurationLayers, CxxQtType, Duration, FeaturePreferences,
    FeedbackLane, FeedbackState, Instant, ParsedCommand, Path, PathBuf, Pin, PortalCapabilities,
    PortalProbeWorker, QString, Value, chrome_preferences, configured_bindings, desktop_portals,
    feature_preferences, hyprland, is_bounded_untrusted_text, load_theme_palette,
    matching_site_rules, publish_ipc_event, qobject, safe_ipc_url, sanitize_process_stderr,
    setting_metadata, settings_presentation, theme_presentation, try_wait_editor_process,
};

use super::browser_ui_page_windows::PageSettingsCompletion;
use super::configuration_completion::{ConfigurationCompletion, ConfigurationSaveOutcome};
use ferric_browser_application::{NativeOperationOutcome, PreparedConfigurationUpdate};
use ferric_browser_config::ConfigurationApplyBoundary;

const SETTINGS_RELOAD_FAILED: &str = "Settings changes couldn't be loaded. Your current settings are still active. Open :diagnostics for details.";

enum ConfigurationCommit<'a> {
    Reload(
        Option<
            Box<(
                ferric_browser_application::PreparedConfigurationUpdate,
                ferric_browser_application::ContextReloadEffect,
            )>,
        >,
    ),
    Runtime(
        bool,
        Option<&'a super::browser_ui_configuration_preparation::ConfigurationRequestOrigin>,
        &'a mut Option<PageSettingsCompletion>,
        Box<PreparedConfigurationUpdate>,
    ),
    Persisted(Box<ferric_browser_application::PersistedConfigurationUpdate>),
    CommittedPeer(Box<PreparedConfigurationUpdate>),
    Activation(Box<PreparedConfigurationUpdate>),
}

impl super::BrowserUiRust {
    pub(super) fn detach_configuration_publication(
        &mut self,
    ) -> Option<super::browser_ui_state::PendingConfigurationPublication> {
        let pending = self
            .configuration_io
            .pending_configuration_publication
            .take();
        if let Some(state) = self.state.as_mut() {
            state.detach_configuration_write_observer();
        }
        pending
    }
}

impl qobject::BrowserUi {
    pub(super) fn retire_configuration_observers(mut self: Pin<&mut Self>) {
        self.as_mut().detach_site_policy_file();
        let retired = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.configuration_io.profile_request = std::sync::Arc::new(());
            if let Some(state) = this.state.as_mut() {
                state.retire_shared_policy_observer();
            }
            (
                this.configuration_io.site_policy_activation.take(),
                this.detach_configuration_publication(),
                this.configuration_io.pending_selector_validation.take(),
                this.pending_profile_configuration.take(),
            )
        };
        self.as_mut().set_view_alive(false);
        self.as_mut().set_site_policy_result(QString::default());
        self.as_mut()
            .set_selector_validation_request(QString::default());
        // Reporting a detached native receipt may wake the original issuer.
        // No Rust borrow or partially replaced application survives that wake.
        drop(retired);
    }

    pub(super) fn bind_configuration_operation(self: Pin<&mut Self>, operation_id: &str) -> bool {
        self.bind_configuration_operation_for_request(operation_id, None)
    }

    pub(super) fn bind_configuration_operation_for_request(
        mut self: Pin<&mut Self>,
        operation_id: &str,
        request: Option<u64>,
    ) -> bool {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if this
            .operation(operation_id)
            .is_none_or(|operation| operation.state.is_terminal())
        {
            return false;
        }
        let committing = this
            .configuration_io
            .pending_configuration_publication
            .as_ref()
            .is_some_and(|pending| {
                request.is_none_or(|request| {
                    pending
                        .origin
                        .as_ref()
                        .is_some_and(|origin| origin.generation == request)
                })
            });
        let pending = if let Some(pending) = this
            .configuration_io
            .pending_configuration_publication
            .as_mut()
            .filter(|pending| {
                request.is_none_or(|request| {
                    pending
                        .origin
                        .as_ref()
                        .is_some_and(|origin| origin.generation == request)
                })
            }) {
            pending.operation_id = Some(operation_id.to_owned());
            if let Some(origin) = pending.origin.as_mut() {
                origin.operation_id = pending.operation_id.clone();
            }
            true
        } else if let Some(pending) = this
            .configuration_io
            .pending_selector_validation
            .as_mut()
            .filter(|pending| {
                request.is_none_or(|request| {
                    pending
                        .origin
                        .as_ref()
                        .map_or(pending.generation, |origin| origin.generation)
                        == request
                })
            })
        {
            pending.operation_id = Some(operation_id.to_owned());
            if let Some(origin) = pending.origin.as_mut() {
                origin.operation_id = pending.operation_id.clone();
            }
            true
        } else {
            false
        };
        let pending =
            pending || super::browser_ui_site_policy::activation_pending(this, operation_id);
        if pending {
            this.set_operation(
                operation_id.to_owned(),
                crate::OperationUpdate::running("settings validation or persistence"),
            );
        }
        if committing && let Some(state) = this.state.as_mut() {
            state.operations_mut().begin_commit(operation_id);
        }
        pending
    }

    pub(super) fn poll_peer_configuration(mut self: Pin<&mut Self>) {
        {
            let browser = self.as_ref();
            let rust = browser.rust();
            if let Some(state) = rust.state.as_ref() {
                state.prepare_shared_policy_contexts();
            }
            if let Some(pending) = rust.pending_profile_configuration.as_ref() {
                pending.state.prepare_shared_policy_contexts();
            }
            if let Some(pending) = rust.configuration_io.pending_selector_validation.as_ref()
                && let super::browser_ui_state::PendingSelectorAction::Profile { pending, .. } =
                    &pending.action
            {
                pending.state.prepare_shared_policy_contexts();
            }
        }
        if self.as_ref().rust().profile_bootstrap_pending
            || self
                .as_ref()
                .rust()
                .configuration_io
                .pending_selector_validation
                .is_some()
            || self
                .as_ref()
                .rust()
                .configuration_io
                .pending_configuration_publication
                .is_some()
            || self
                .as_ref()
                .rust()
                .configuration_io
                .pending_context_reload
                .is_some()
        {
            return;
        }
        let prepared = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .map(super::BrowserApplication::prepare_committed_peer_configuration)
            .transpose();
        let prepared = match prepared {
            Ok(Some(Some(prepared))) => prepared,
            Ok(_) => return,
            Err(error) => {
                self.as_mut()
                    .report_config_reload_failure(&error.to_string(), false);
                return;
            }
        };
        let result = self
            .as_mut()
            .validate_or_queue_committed_peer_configuration(prepared);
        if let Err(error) = result {
            self.as_mut().set_status_text(QString::from(format!(
                "Shared settings reception postponed: {error}"
            )));
        }
    }

    pub(super) fn poll_configuration_publication(mut self: Pin<&mut Self>) {
        let result = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .state
            .as_mut()
            .and_then(super::BrowserApplication::poll_configuration_write);
        let Some(result) = result else {
            return;
        };
        let pending = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .configuration_io
            .pending_configuration_publication
            .take();
        let Some(pending) = pending else {
            return;
        };
        let operation_id = pending.operation_id.clone();
        let completion = pending.completion;
        let mut post_action = pending.post_action;
        let setting_feedback = pending.setting_feedback;
        let saved_outcome = result.as_ref().ok().map(|receipt| receipt.outcome.clone());
        let durability_uncertain = matches!(
            saved_outcome,
            Some(ferric_browser_config::RuntimeOverrideWriteOutcome::DurabilityUncertain(_))
        );
        if durability_uncertain && let Some(completion) = completion.as_ref() {
            completion.note_durability_uncertainty();
        }
        let result = result
            .map_err(|error| error.to_string())
            .and_then(|receipt| {
                self.as_mut().commit_validated_configuration(
                    pending.candidate,
                    pending.layers,
                    Some(&pending.status),
                    pending.selectors,
                    ConfigurationCommit::Persisted(Box::new(receipt)),
                    &mut post_action,
                )
            });
        // Replacement is irrevocable even if preparation/publication is now
        // stale. Keep that outcome distinct from a pre-replacement failure.
        if let (Err(error), Some(outcome)) = (&result, saved_outcome) {
            if let Some(post_action) = post_action.take() {
                self.as_mut().finish_configuration_completion(
                    post_action,
                    ConfigurationSaveOutcome::SavedInactive {
                        durability_uncertain: matches!(
                            outcome,
                            ferric_browser_config::RuntimeOverrideWriteOutcome::DurabilityUncertain(
                                _
                            )
                        ),
                    },
                );
            }
            if let Some(completion) = completion {
                completion.finish(NativeOperationOutcome::SettingsSavedReloadRequired {
                    durability_uncertain: matches!(
                        outcome,
                        ferric_browser_config::RuntimeOverrideWriteOutcome::DurabilityUncertain(_)
                    ),
                });
            }
            self.finish_saved_configuration_notice(operation_id, setting_feedback, &outcome, error);
            return;
        }
        if let Some(post_action) = post_action.take() {
            self.as_mut()
                .finish_configuration_completion(post_action, ConfigurationSaveOutcome::Failed);
        }
        if let Some(completion) = completion {
            completion.finish(if result.is_ok() {
                NativeOperationOutcome::Applied
            } else {
                NativeOperationOutcome::Failed
            });
        }
        if let Some(operation_id) = operation_id {
            let update = match &result {
                Ok(_)
                    if super::browser_ui_site_policy::activation_pending(
                        self.as_ref().rust(),
                        &operation_id,
                    ) =>
                {
                    crate::OperationUpdate::running("site policy configuration activation")
                }
                Ok(_) => crate::OperationUpdate::completed(if durability_uncertain {
                    "settings persisted and published with durability uncertainty"
                } else {
                    "settings persisted and published"
                }),
                Err(error) => {
                    crate::OperationUpdate::failed(crate::FailureKind::Config, error.clone())
                }
            };
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .set_operation(operation_id, update);
        }
        self.as_mut().finish_setting_feedback(
            setting_feedback,
            result.as_ref().map(|_| ()).map_err(String::as_str),
        );
        if let Err(error) = result {
            self.as_mut().report_config_reload_failure(&error, true);
        }
    }

    fn finish_saved_configuration_notice(
        mut self: Pin<&mut Self>,
        operation_id: Option<String>,
        feedback: Option<super::browser_ui_state::SettingFeedback>,
        outcome: &ferric_browser_config::RuntimeOverrideWriteOutcome,
        error: &str,
    ) {
        let message = settings_presentation::saved_configuration_status(outcome);
        if let Some(operation_id) = operation_id {
            let persistence = match outcome {
                ferric_browser_config::RuntimeOverrideWriteOutcome::Durable => "durable",
                ferric_browser_config::RuntimeOverrideWriteOutcome::DurabilityUncertain(_) => {
                    "durability-uncertain"
                }
            };
            self.as_mut().rust_mut().as_mut().get_mut().set_operation(
                operation_id,
                crate::OperationUpdate::completed(format!(
                    "saved-reload-required ({persistence}): {message} Reason: {error}"
                )),
            );
        }
        self.as_mut().set_status_text(QString::from(&message));
        if let Some(feedback) = feedback
            && self.as_ref().rust().setting_action_id == feedback.id
            && self.as_ref().rust().setting_action_state.to_string() == "pending"
        {
            self.publish_management_feedback(
                FeedbackLane::Setting,
                FeedbackState::Succeeded,
                &message,
                error,
            );
        }
    }

    pub(super) fn select_site_rule_settings(mut self: Pin<&mut Self>, url: &QString) -> bool {
        let Some(projection) = self.as_ref().rust().state.as_ref().map(|state| {
            super::site_rule_projection::resolve(state.configuration(), &url.to_string())
        }) else {
            self.as_mut().set_status_text(QString::from(
                "Site rules are unavailable while this window is closing.",
            ));
            return false;
        };
        let super::site_rule_projection::SiteRuleProjection {
            javascript,
            images,
            force_dark,
            autoplay,
            zoom,
        } = projection;
        self.as_mut()
            .set_site_rule_javascript_set(javascript.is_some());
        self.as_mut()
            .set_site_rule_javascript_enabled(javascript.unwrap_or_default());
        self.as_mut().set_site_rule_images_set(images.is_some());
        self.as_mut()
            .set_site_rule_images_enabled(images.unwrap_or_default());
        self.as_mut()
            .set_site_rule_force_dark_set(force_dark.is_some());
        self.as_mut()
            .set_site_rule_force_dark_enabled(force_dark.unwrap_or_default());
        self.as_mut().set_site_rule_autoplay_set(autoplay.is_some());
        self.as_mut()
            .set_site_rule_autoplay(QString::from(autoplay.unwrap_or_default()));
        self.as_mut().set_site_rule_zoom_set(zoom.is_some());
        self.as_mut().set_site_rule_zoom(zoom.unwrap_or_default());
        true
    }

    pub(super) fn clear_focus_observation_state(mut self: Pin<&mut Self>, index: i32) {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.interaction
            .focus_observations
            .retain(|(candidate, _), _| *candidate != index);
        this.interaction
            .focus_suppressions
            .retain(|(candidate, _), _| *candidate != index);
    }

    pub(super) fn suppress_current_focus_observations(mut self: Pin<&mut Self>) {
        let active = self.as_ref().rust().active_tab_index;
        let suppressed = self
            .as_ref()
            .rust()
            .interaction
            .focus_observations
            .iter()
            .filter(|((index, _), _)| *index == active)
            .map(|(key, observation)| (key.clone(), observation.sequence))
            .collect::<Vec<_>>();
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        for (key, sequence) in suppressed {
            this.interaction.focus_suppressions.insert(key, sequence);
        }
    }

    pub(super) fn configured_site_entry_mode(&self, url: &str) -> Option<String> {
        if self
            .rust()
            .active_site_experiment
            .as_ref()
            .is_some_and(|experiment| {
                experiment.kind == "compiled-defaults" && safe_ipc_url(url) == experiment.url
            })
        {
            return None;
        }
        let config = self.rust().state.as_ref()?.configuration();
        let mut entry_mode = None;
        for rule in matching_site_rules(config, url) {
            if let Some(mode) = rule
                .set
                .get("input.entry_mode")
                .and_then(toml::Value::as_str)
            {
                entry_mode = Some(mode.to_owned());
            }
        }
        entry_mode
    }

    pub(super) fn report_config_reload_failure(
        mut self: Pin<&mut Self>,
        error: &str,
        announce: bool,
    ) {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .profile_startup
            .settings_failed(error);
        self.as_mut().sync_profile_startup_notice();
        if announce {
            self.as_mut()
                .set_status_text(QString::from(SETTINGS_RELOAD_FAILED));
        }
    }

    pub(super) fn commit_prepared_configuration_reload(
        mut self: Pin<&mut Self>,
        prepared: PreparedConfigurationUpdate,
        selectors: super::selector_validation_receipts::SelectorValidationPublication,
        status: &str,
    ) -> Result<Value, String> {
        let layers = prepared.policy_sources().desired_layers().clone();
        let candidate = serde_json::to_value(layers.resolve().map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        self.as_mut().commit_validated_configuration(
            candidate,
            layers,
            Some(status),
            selectors,
            ConfigurationCommit::Activation(Box::new(prepared)),
            &mut None,
        )
    }

    pub(super) fn commit_reloaded_config(
        mut self: Pin<&mut Self>,
        candidate: Value,
        layers: ConfigurationLayers,
        status: Option<&str>,
        selectors: super::selector_validation_receipts::SelectorValidationPublication,
        contexts: Option<(
            ferric_browser_application::PreparedConfigurationUpdate,
            ferric_browser_application::ContextReloadEffect,
        )>,
    ) -> Result<Value, String> {
        let status = if status.is_none()
            && self.as_ref().rust().status_text.to_string() == SETTINGS_RELOAD_FAILED
        {
            Some("Settings reloaded")
        } else {
            status
        };
        let committed = self.as_mut().commit_validated_configuration(
            candidate,
            layers,
            status,
            selectors,
            ConfigurationCommit::Reload(contexts.map(Box::new)),
            &mut None,
        )?;
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.profile_startup.settings_recovered();
            if let Some(effect) = this.profile_open_effect.as_mut() {
                effect.configuration_error = None;
            }
        }
        self.as_mut().sync_profile_startup_notice();
        Ok(committed)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn commit_runtime_config_persisted(
        mut self: Pin<&mut Self>,
        prepared: PreparedConfigurationUpdate,
        status: &str,
        persist: bool,
        selectors: super::selector_validation_receipts::SelectorValidationPublication,
        origin: Option<&super::browser_ui_configuration_preparation::ConfigurationRequestOrigin>,
        mut completion: Option<PageSettingsCompletion>,
        mut post_action: Option<ConfigurationCompletion>,
    ) -> Result<Value, String> {
        let result = (|| {
            let layers = prepared.policy_sources().desired_layers().clone();
            let candidate =
                serde_json::to_value(layers.resolve().map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            self.as_mut().commit_validated_configuration(
                candidate,
                layers,
                Some(status),
                selectors,
                ConfigurationCommit::Runtime(persist, origin, &mut completion, Box::new(prepared)),
                &mut post_action,
            )
        })();
        if let Some(post_action) = post_action {
            self.as_mut()
                .finish_configuration_completion(post_action, ConfigurationSaveOutcome::Failed);
        }
        if let Some(completion) = completion {
            completion.finish(if result.is_ok() {
                NativeOperationOutcome::Applied
            } else {
                NativeOperationOutcome::Failed
            });
        }
        result
    }

    pub(super) fn commit_committed_peer_configuration(
        mut self: Pin<&mut Self>,
        prepared: PreparedConfigurationUpdate,
        selectors: super::selector_validation_receipts::SelectorValidationPublication,
    ) -> Result<Value, String> {
        if !prepared.is_committed_peer_update() {
            return Err("Shared settings adoption requires its exact committed preparation".into());
        }
        let layers = prepared.policy_sources().desired_layers().clone();
        let candidate = serde_json::to_value(layers.resolve().map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
        self.as_mut().commit_validated_configuration(
            candidate,
            layers,
            Some("Settings synchronized from another window"),
            selectors,
            ConfigurationCommit::CommittedPeer(Box::new(prepared)),
            &mut None,
        )
    }

    fn commit_validated_configuration(
        mut self: Pin<&mut Self>,
        candidate: Value,
        layers: ConfigurationLayers,
        status: Option<&str>,
        selectors: super::selector_validation_receipts::SelectorValidationPublication,
        commit: ConfigurationCommit<'_>,
        post_action: &mut Option<ConfigurationCompletion>,
    ) -> Result<Value, String> {
        // Once admitted, the application's receipt governs publication.
        // Proposal replacement retires an observer, never the saved write.
        if post_action
            .as_ref()
            .filter(|completion| !completion.is_admitted())
            .is_some_and(|completion| !completion.is_current(self.as_ref().rust()))
        {
            return Err("The configuration proposal or owner changed".into());
        }
        let committed_peer = matches!(&commit, ConfigurationCommit::CommittedPeer(_));
        let admitted = matches!(
            &commit,
            ConfigurationCommit::CommittedPeer(_)
                | ConfigurationCommit::Persisted(_)
                | ConfigurationCommit::Reload(Some(_))
        );
        let origin = match &commit {
            ConfigurationCommit::Runtime(_, origin, _, _) => origin.cloned(),
            _ => None,
        };
        if origin
            .as_ref()
            .is_some_and(|origin| !origin.is_current(self.as_ref().rust()))
        {
            return Err("A newer request replaced this settings preparation".into());
        }
        let (boundary, persistence, contexts, persisted, owned_preparation, mut completion) =
            match commit {
                ConfigurationCommit::Reload(contexts) => {
                    let (prepared, effect) = contexts.map(|pair| *pair).unzip();
                    (
                        ConfigurationApplyBoundary::Reload,
                        false,
                        effect,
                        None,
                        prepared,
                        None,
                    )
                }
                ConfigurationCommit::Activation(prepared) => (
                    ConfigurationApplyBoundary::Reload,
                    false,
                    None,
                    None,
                    Some(*prepared),
                    None,
                ),
                ConfigurationCommit::Runtime(persistence, _, completion, prepared) => (
                    ConfigurationApplyBoundary::RuntimeUpdate,
                    persistence,
                    None,
                    None,
                    Some(*prepared),
                    Some(completion),
                ),
                ConfigurationCommit::Persisted(receipt) => (
                    ConfigurationApplyBoundary::RuntimeUpdate,
                    false,
                    None,
                    Some(*receipt),
                    None,
                    None,
                ),
                ConfigurationCommit::CommittedPeer(prepared) => (
                    ConfigurationApplyBoundary::RuntimeUpdate,
                    false,
                    None,
                    None,
                    Some(*prepared),
                    None,
                ),
            };
        let policy_layers = layers.clone();
        let previous_config = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .map(|state| state.configuration().clone());
        let browser = self.as_ref();
        let state = browser
            .rust()
            .state
            .as_ref()
            .ok_or_else(|| "application state is unavailable".to_owned())?;
        let (prepared, write_outcome) = if let Some(receipt) = persisted {
            if let Some(error) = receipt.publication_error {
                return Err(error.to_string());
            }
            (receipt.prepared, Some(receipt.outcome))
        } else if let Some(prepared) = owned_preparation {
            (prepared, None)
        } else {
            let prepared = match boundary {
                ConfigurationApplyBoundary::RuntimeUpdate => {
                    state.prepare_configuration_update(layers)
                }
                ConfigurationApplyBoundary::Reload => state.prepare_configuration_reload(layers),
            }
            .map_err(|error| error.to_string())?;
            (prepared, None)
        };
        let current_url = self.as_ref().rust().current_url.to_string();
        let previous_selectors = state
            .configuration_policy_sources()
            .compile_hint_selectors(&current_url)
            .ok();
        let snapshot = prepared.snapshot();
        let (discovery_selectors, proxy_targets, discovery_error) = if committed_peer {
            let sources = prepared.policy_sources();
            let selectors = sources
                .compile_hint_selectors(&current_url)
                .map_err(|error| error.to_string());
            let proxy = sources
                .compile_hint_proxy_targets(&current_url)
                .map_err(|error| error.to_string());
            let diagnostic = selectors
                .as_ref()
                .err()
                .or_else(|| proxy.as_ref().err())
                .cloned();
            (selectors.ok(), proxy.ok(), diagnostic)
        } else {
            let discovery = self.as_ref().validate_candidate_hint_discovery(&prepared)?;
            (
                Some(discovery.selectors),
                Some(discovery.proxy_targets),
                None,
            )
        };
        let selector_policy_changed = discovery_selectors
            .as_ref()
            .is_none_or(|selectors| previous_selectors.as_ref() != Some(selectors));
        let active_settings = snapshot.effective;
        let requested_settings = policy_layers
            .resolve()
            .map_err(|error| format!("could not resolve requested configuration: {error}"))?;
        let active_config = serde_json::to_value(&active_settings)
            .map_err(|error| format!("could not serialize active configuration: {error}"))?;
        let mixed_resolution = self
            .as_ref()
            .resolve_candidate_mixed_policy(&active_config, &policy_layers);
        // A fresh invalid proposal must fail before admission. Already admitted
        // publication cannot be rolled back by a later context/projection error.
        let candidate_mixed_policy = if admitted {
            super::mixed_policy_runtime::MixedPolicyState::from_result(mixed_resolution)
        } else {
            super::mixed_policy_runtime::MixedPolicyState::Ready(mixed_resolution?)
        };
        let mixed_error = candidate_mixed_policy.error().map(str::to_owned);
        if persistence {
            if origin
                .as_ref()
                .is_some_and(|origin| !origin.is_current(self.as_ref().rust()))
            {
                return Err(
                    "The settings request was replaced or cancelled before write admission".into(),
                );
            }
            let (admitted_completion, observer) =
                super::browser_ui_page_windows::AdmittedPageSettingsCompletion::observe(
                    completion.as_mut().and_then(|completion| completion.take()),
                );
            let admission = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .state
                .as_mut()
                .ok_or_else(|| "application state is unavailable".to_owned())
                .and_then(|state| {
                    state
                        .request_configuration_write_with_observer(prepared, Some(observer))
                        .map_err(|error| error.to_string())
                });
            if let Err(error) = admission {
                admitted_completion.rejected();
                return Err(error);
            }
            if let Some(post_action) = post_action.as_mut() {
                post_action.admitted();
            }
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .configuration_io
                .pending_configuration_publication =
                Some(super::browser_ui_state::PendingConfigurationPublication {
                    post_action: post_action.take(),
                    completion: Some(admitted_completion),
                    operation_id: origin
                        .as_ref()
                        .and_then(|origin| origin.operation_id.clone()),
                    setting_feedback: origin
                        .as_ref()
                        .and_then(|origin| origin.setting_feedback.clone()),
                    origin: origin.clone(),
                    candidate,
                    layers: policy_layers,
                    status: status.unwrap_or("Settings saved").to_owned(),
                    selectors,
                });
            if let Some(origin) = origin.as_ref()
                && let Some(operation) = origin.operation_id.as_ref()
            {
                self.as_mut()
                    .bind_configuration_operation_for_request(operation, Some(origin.generation));
            }
            self.as_mut()
                .set_status_text(QString::from("Saving settings…"));
            self.as_mut().runtime_work_available();
            return Ok(serde_json::json!({"status":"pending-persistence"}));
        }
        let pending = snapshot.pending_changes;
        let pending_summary = pending
            .iter()
            .map(|change| {
                serde_json::json!({
                    "key": change.key,
                    "apply_time": change.apply_time,
                    "current": change.current,
                    "pending": change.pending
                })
            })
            .collect::<Vec<_>>();
        let config_json = serde_json::to_string(&active_config)
            .map_err(|error| format!("could not serialize effective configuration: {error}"))?;
        let base_config = serde_json::to_value(&policy_layers.base)
            .map_err(|error| format!("could not serialize base configuration: {error}"))?;
        let bindings = configured_bindings(&self.as_ref().rust().registry, &active_config);
        let contexts_json = contexts
            .as_ref()
            .map(|effect| serde_json::to_string(effect.configuration()))
            .transpose()
            .map_err(|error| format!("could not serialize context configuration: {error}"))?;
        let mut context_runtime_effects = Vec::new();
        self.as_mut().request_interaction_policy_refresh();
        let published_owner;
        let policy_changed;
        {
            let mut rust = self.as_mut().rust_mut();
            if post_action
                .as_ref()
                .filter(|completion| !completion.is_admitted())
                .is_some_and(|completion| !completion.is_current(&rust))
            {
                return Err("The configuration proposal changed before publication".into());
            }
            if origin
                .as_ref()
                .is_some_and(|origin| !origin.is_current(&rust))
            {
                return Err("A newer request replaced this settings preparation".into());
            }
            let state = rust
                .as_mut()
                .get_mut()
                .state
                .as_mut()
                .ok_or_else(|| "application state is unavailable".to_owned())?;
            if let Some(contexts) = contexts {
                let (_, effects) = state
                    .publish_context_configuration(prepared, contexts)
                    .map_err(|error| format!("configuration publication rejected: {error}"))?;
                context_runtime_effects = effects;
            } else {
                state
                    .publish_configuration_update(prepared, || Ok(()))
                    .map_err(|error| format!("configuration publication rejected: {error}"))?;
            }
            published_owner = state.capture_configuration_request();
        }
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.config = active_config.clone();
            crate::page_window_operations::update_policy(this);
            this.configuration_io.active_selector_fingerprint = selectors.fingerprint;
            if let Some(state) = this.state.as_ref() {
                this.configuration_io
                    .selector_receipts
                    .published(state, selectors.receipt);
            }
            this.configuration_io.selector_config_revision = this
                .configuration_io
                .selector_config_revision
                .wrapping_add(1);
            this.base_config = base_config;
            this.profile_overrides = policy_layers.profile.clone();
            this.runtime_overrides = policy_layers.runtime.clone();
            this.cli_overrides = policy_layers.command_line.clone();
            this.temporary_overrides = policy_layers.temporary.clone();
            policy_changed = this.replace_mixed_policy(candidate_mixed_policy);
            this.configuration_io.pending_config =
                (!pending.is_empty()).then_some(candidate.clone());
            this.learning_mode = active_config
                .get("discovery")
                .and_then(Value::as_object)
                .and_then(|discovery| discovery.get("learning_mode"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut().get_mut().replace_input_bindings(bindings);
        }
        let binding_context = self.as_ref().binding_context();
        self.as_mut().input_bindings_changed(binding_context);
        self.as_mut().note_native_prompt_configuration_change();
        if policy_changed {
            self.as_mut().set_binding_overlay(QString::default());
        }
        self.as_mut()
            .set_desktop_portal_mode(QString::from(desktop_portals::mode_name(
                &active_settings.desktop.portals,
            )));
        self.as_mut().update_chrome_preferences(&active_settings);
        self.as_mut()
            .update_feature_preferences_for_state(&active_settings, &requested_settings);
        self.as_mut().update_hint_preferences(&active_settings);
        let proxy_targets_changed = if let Some(policy) = proxy_targets {
            self.as_mut().apply_hint_proxy_targets(policy)
        } else {
            self.as_mut().close_hint_proxy_targets()
        };
        // The editors show committed choices, including values waiting for
        // reload or restart. Their apply-time labels describe the boundary.
        self.as_mut().update_settings_presentation(&candidate);
        self.as_mut().set_config_json(QString::from(config_json));
        // Compare source-aware policy values: a no-op watched reload does not
        // revoke authority, while source-only changes can matter even when
        // flattened Config values happen to be equal.
        self.as_mut().note_hint_policy_update(
            previous_config.as_ref(),
            &active_settings,
            proxy_targets_changed || selector_policy_changed,
        );
        let learning_mode = self.as_ref().rust().learning_mode;
        self.as_mut().set_learning_mode(learning_mode);
        self.as_mut().update_theme_palette(&candidate);
        if let Some(contexts_json) = contexts_json {
            let _ = self
                .as_mut()
                .observe_runtime_effects(context_runtime_effects);
            self.as_mut()
                .set_contexts_json(QString::from(contexts_json));
            let records = self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .and_then(super::BrowserApplication::contexts)
                .map(super::ContextSnapshot::from_records);
            self.as_mut().rust_mut().as_mut().get_mut().contexts = records;
            self.as_mut().sync_context_choices();
            self.as_mut().sync_context_metadata();
            self.as_mut().publish_context_catalog();
        }
        let persistence_name = match &write_outcome {
            None => "memory-only",
            Some(ferric_browser_config::RuntimeOverrideWriteOutcome::Durable) => "durable",
            Some(ferric_browser_config::RuntimeOverrideWriteOutcome::DurabilityUncertain(_)) => {
                "durability-uncertain"
            }
        };
        if let Some(status) = status {
            let status = settings_presentation::configuration_status(status, &pending);
            let mut status = match &write_outcome {
                Some(ferric_browser_config::RuntimeOverrideWriteOutcome::DurabilityUncertain(
                    error,
                )) => {
                    format!("{status}; applied, but saving may not survive a system crash: {error}")
                }
                _ => status,
            };
            if let Some(error) = discovery_error {
                status.push_str("; current site controls unavailable: ");
                status.push_str(&error);
            }
            if let Some(error) = mixed_error {
                status.push_str("; Mixed input policy unavailable: ");
                status.push_str(&error);
            }
            self.as_mut().set_status_text(QString::from(status));
        }
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.runtime_config_persistence = persistence_name;
        let revision = this.ipc_sequence.saturating_add(1);
        publish_ipc_event(
            &mut this.ipc_sequence,
            "config.changed",
            serde_json::json!({
                "revision": revision,
                "pending": pending_summary,
                "persistence": persistence_name,
            }),
        );
        drop(rust);
        crate::page_group_runtime::publish(self.as_ref().get_ref());
        if let Some(post_action) = post_action.take() {
            let durability_uncertain = matches!(
                write_outcome,
                Some(ferric_browser_config::RuntimeOverrideWriteOutcome::DurabilityUncertain(_))
            );
            self.as_mut().finish_configuration_completion(
                post_action,
                match published_owner {
                    Some(owner) => ConfigurationSaveOutcome::Published {
                        owner: Box::new(owner),
                        durability_uncertain,
                    },
                    None => ConfigurationSaveOutcome::SavedInactive {
                        durability_uncertain,
                    },
                },
            );
        }
        Ok(active_config)
    }

    pub(super) fn update_chrome_preferences(mut self: Pin<&mut Self>, config: &Config) {
        let preferences: ChromePreferences = chrome_preferences::project(config);
        self.as_mut()
            .set_chrome_font_family(QString::from(preferences.font_family));
        self.as_mut()
            .set_chrome_font_size_pt(preferences.font_size_pt);
        self.as_mut()
            .set_chrome_statusbar_mode(QString::from(preferences.statusbar_mode));
        self.as_mut()
            .set_chrome_tabs_mode(QString::from(preferences.tabs_mode));
        self.as_mut()
            .set_chrome_tab_position(QString::from(preferences.tab_position));
        self.as_mut()
            .set_chrome_reduced_motion(QString::from(preferences.reduced_motion));
    }

    pub(super) fn update_feature_preferences(self: Pin<&mut Self>, config: &Config) {
        self.update_feature_preferences_for_state(config, config);
    }

    pub(super) fn update_feature_preferences_for_state(
        mut self: Pin<&mut Self>,
        effective: &Config,
        requested: &Config,
    ) {
        let preferences: FeaturePreferences = feature_preferences::project(effective, requested);
        self.as_mut()
            .set_feature_switcher_max_results(preferences.switcher_max_results);
        self.as_mut()
            .set_feature_downloads_ask_destination(preferences.downloads_ask_destination);
        self.as_mut()
            .set_feature_desktop_notifications_enabled(preferences.desktop_notifications_enabled);
        self.as_mut()
            .set_feature_desktop_media_keys_enabled(preferences.desktop_media_keys_enabled);
        self.as_mut()
            .set_feature_push_service_enabled(preferences.push_service_enabled);
        self.as_mut()
            .set_feature_spellcheck_enabled(preferences.spellcheck_enabled);
        self.as_mut().set_feature_spellcheck_languages(
            preferences
                .spellcheck_languages
                .into_iter()
                .map(QString::from)
                .collect(),
        );
        self.as_mut().set_feature_blocking_list_ids(
            preferences
                .blocking_list_ids
                .into_iter()
                .map(QString::from)
                .collect(),
        );
        self.as_mut()
            .set_feature_blocking_update_interval_hours(preferences.blocking_update_interval_hours);
        self.as_mut()
            .set_feature_link_cleaning_update_source(QString::from(
                preferences.link_cleaning_update_source,
            ));
        self.as_mut()
            .set_feature_link_cleaning_update_sha256(QString::from(
                preferences.link_cleaning_update_sha256,
            ));
        self.as_mut()
            .set_feature_tabs_presentation_requested(QString::from(
                preferences.tabs_presentation_requested,
            ));
        self.as_mut()
            .set_feature_tabs_presentation_effective(QString::from(
                preferences.tabs_presentation_effective,
            ));
        self.as_mut()
            .set_feature_tabs_presentation_status(QString::from(
                preferences.tabs_presentation_status,
            ));
        self.as_mut()
            .set_feature_tabs_presentation_reason(QString::from(
                preferences.tabs_presentation_reason,
            ));
        self.as_mut()
            .set_feature_phone_passkeys_requested(QString::from(
                preferences.phone_passkeys_requested,
            ));
        self.as_mut()
            .set_feature_phone_passkeys_status(QString::from(preferences.phone_passkeys_status));
        self.as_mut()
            .set_feature_phone_passkeys_reason(QString::from(preferences.phone_passkeys_reason));
        self.as_mut()
            .set_feature_page_grouping_requested(QString::from(
                preferences.page_grouping_requested,
            ));
        self.as_mut()
            .set_feature_page_grouping_status(QString::from(preferences.page_grouping_status));
        self.as_mut()
            .set_feature_page_grouping_reason(QString::from(preferences.page_grouping_reason));
    }

    pub(super) fn update_hint_preferences(self: Pin<&mut Self>, config: &Config) {
        let policy = if self.as_ref().rust().hint_rapid {
            &config.hints.rapid_unmatched
        } else {
            &config.hints.unmatched
        };
        self.update_hint_display_preferences(policy, config.hints.marker_scale);
    }

    pub(super) fn update_hint_display_preferences(
        mut self: Pin<&mut Self>,
        policy: &ferric_browser_config::HintUnmatchedPolicy,
        marker_scale: f64,
    ) {
        let policy = match policy {
            ferric_browser_config::HintUnmatchedPolicy::Hide => "hide",
            ferric_browser_config::HintUnmatchedPolicy::Dim => "dim",
            ferric_browser_config::HintUnmatchedPolicy::Show => "show",
        };
        self.as_mut()
            .set_hint_unmatched_policy(QString::from(policy));
        self.set_hint_marker_scale(marker_scale);
    }

    pub(super) fn update_settings_presentation(mut self: Pin<&mut Self>, config: &Value) {
        self.as_mut()
            .set_settings_rows(settings_presentation::project_variant(config));
    }

    pub(super) fn update_theme_palette(mut self: Pin<&mut Self>, config_value: &Value) -> bool {
        let config = serde_json::from_value::<Config>(config_value.clone()).unwrap_or_default();
        let Ok(palette) = load_theme_palette(&config.theme) else {
            return false;
        };
        let palette_path = palette.path.clone();
        let mut watch_paths = Vec::new();
        if !palette_path.is_empty() {
            let path = PathBuf::from(&palette_path);
            watch_paths.push(path.clone());
            if let Some(parent) = path.parent() {
                watch_paths.push(parent.to_owned());
                if let Some(grandparent) = parent.parent() {
                    watch_paths.push(grandparent.to_owned());
                }
            }
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .configuration_io
                .theme_watch
                .set_sources(Path::new(""), &watch_paths);
        }
        let presentation = theme_presentation::project(&palette);
        self.as_mut()
            .set_theme_background_color(QString::from(presentation.background));
        self.as_mut()
            .set_theme_surface_color(QString::from(presentation.surface));
        self.as_mut()
            .set_theme_panel_color(QString::from(presentation.panel));
        self.as_mut()
            .set_theme_primary_text_color(QString::from(presentation.primary_text));
        self.as_mut()
            .set_theme_secondary_text_color(QString::from(presentation.secondary_text));
        self.as_mut()
            .set_theme_muted_text_color(QString::from(presentation.muted_text));
        self.as_mut()
            .set_theme_border_color(QString::from(presentation.border));
        self.as_mut()
            .set_theme_accent_color(QString::from(presentation.accent));
        self.as_mut()
            .set_theme_warning_color(QString::from(presentation.warning));
        self.as_mut()
            .set_theme_error_color(QString::from(presentation.error));
        self.as_mut()
            .set_theme_success_color(QString::from(presentation.success));
        self.as_mut()
            .set_theme_private_color(QString::from(presentation.private));
        self.as_mut()
            .set_theme_mode_insert_color(QString::from(presentation.mode_insert));
        self.as_mut()
            .set_theme_selection_color(QString::from(presentation.selection));
        self.as_mut()
            .set_theme_selection_text_color(QString::from(presentation.selection_text));
        self.as_mut()
            .set_theme_contrast_status(QString::from(presentation.contrast_status));
        self.as_mut()
            .set_theme_contrast_reason(QString::from(presentation.contrast_reason));
        self.as_mut().rust_mut().as_mut().get_mut().theme_palette = palette;
        true
    }

    pub(super) fn execute_help_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let topic = match command.arguments.as_slice() {
            [] => None,
            [topic] if is_bounded_untrusted_text(topic) => Some(topic.as_str()),
            _ => return Err("help accepts at most one command or setting topic".into()),
        };
        let action = topic.map_or_else(
            || "show-binding-help".to_owned(),
            |topic| {
                if setting_metadata(topic).is_some() {
                    format!("show-settings\t{topic}")
                } else {
                    format!("show-binding-help\t{topic}")
                }
            },
        );
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(action);
        self.as_mut()
            .set_status_text(QString::from("Help requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "surface": "help",
            "topic": topic
        }))
    }

    pub(super) fn execute_version_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if !command.arguments.is_empty() {
            return Err("version does not accept arguments".into());
        }
        let result = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "protocol_major": ferric_browser_ipc::PROTOCOL_MAJOR,
            "protocol_minor": ferric_browser_ipc::PROTOCOL_MINOR
        });
        self.as_mut().set_status_text(QString::from(format!(
            "Ferric Browser {} (IPC {}.{})",
            result["version"].as_str().unwrap_or("unknown"),
            result["protocol_major"],
            result["protocol_minor"]
        )));
        Ok(result)
    }

    pub(super) fn execute_diagnostics_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if !command.arguments.is_empty() {
            return Err("diagnostics does not accept arguments".into());
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some("show-diagnostics".into());
        self.as_mut()
            .set_status_text(QString::from("Diagnostics requested"));
        Ok(serde_json::json!({
            "status": "accepted",
            "surface": "diagnostics"
        }))
    }

    pub(super) fn queue_config_write(
        mut self: Pin<&mut Self>,
        path: PathBuf,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .configuration_io
            .config_write_worker
            .as_mut()
            .ok_or_else(|| "configuration writer is unavailable".to_owned())?
            .request(path, bytes)
    }

    pub(super) fn execute_config_export_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let [path] = command.arguments.as_slice() else {
            return Err("config-export accepts exactly one destination path".into());
        };
        if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
            return Err("config-export path is empty, too long, or unsafe".into());
        }
        let config = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .ok_or("effective configuration is unavailable")?
            .configuration()
            .clone();
        let bytes = toml::to_string_pretty(&config)
            .map_err(|error| format!("could not serialize effective configuration: {error}"))?;
        let path = PathBuf::from(path);
        let byte_count = bytes.len();
        self.as_mut()
            .queue_config_write(path.clone(), bytes.into_bytes())
            .map_err(|error| format!("could not queue config export: {error}"))?;
        self.as_mut().set_status_text(QString::from(format!(
            "Configuration export queued for {}",
            path.display()
        )));
        Ok(serde_json::json!({
            "status": "queued",
            "path": path,
            "bytes": byte_count,
            "review": true
        }))
    }

    pub(super) fn execute_config_check_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        if !command.arguments.is_empty() {
            return Err("config-check does not accept arguments".into());
        }
        let path_text = self.as_ref().rust().config_path.to_string();
        if path_text.is_empty() {
            self.as_mut()
                .set_status_text(QString::from("Built-in configuration is valid"));
            return Ok(serde_json::json!({
                "status": "valid",
                "source": "built-in",
                "sources": 0
            }));
        }
        let path = PathBuf::from(&path_text);
        let request = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .configuration_io
            .config_reload_worker
            .as_mut()
            .ok_or_else(|| "configuration reader is unavailable".to_owned())?
            .request_check(path.clone());
        request.map_err(|error| format!("could not queue configuration check: {error}"))?;
        self.as_mut()
            .set_status_text(QString::from("Configuration check queued"));
        Ok(serde_json::json!({
            "status": "queued",
            "source": path,
            "operation": "check"
        }))
    }

    pub(super) fn execute_config_reload_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        if !command.arguments.is_empty() {
            return Err("config-reload does not accept arguments".into());
        }
        let path = self.as_ref().rust().config_path.to_string();
        if path.is_empty() {
            return Err("no file-backed configuration is active".into());
        }
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.configuration_io.config_reload_worker.is_none() {
                return Err("Settings reload is unavailable in this window.".into());
            }
            this.configuration_io.config_reload_requested_by_user = true;
            this.configuration_io.config_watch.mark_pending(
                Instant::now()
                    .checked_sub(Duration::from_millis(200))
                    .unwrap_or_else(Instant::now),
            );
        }
        self.as_mut()
            .set_status_text(QString::from("Settings reload requested…"));
        // A binding may request this while a previous worker result is ready.
        // Its preparation/publication belongs to the real runtime turn.
        self.as_mut().runtime_work_available();
        Ok(serde_json::json!({
            "status": "queued",
            "source": path
        }))
    }

    pub(super) fn execute_config_write_defaults_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let [path] = command.arguments.as_slice() else {
            return Err("config-write-defaults requires exactly one destination path".into());
        };
        if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
            return Err("config-write-defaults path is empty, too long, or unsafe".into());
        }
        let bytes = toml::to_string_pretty(&Config::default())
            .map_err(|error| format!("could not serialize default configuration: {error}"))?;
        let path = PathBuf::from(path);
        let byte_count = bytes.len();
        self.as_mut()
            .queue_config_write(path.clone(), bytes.into_bytes())
            .map_err(|error| format!("could not queue default configuration: {error}"))?;
        self.as_mut().set_status_text(QString::from(format!(
            "Default configuration write queued for {}",
            path.display()
        )));
        Ok(serde_json::json!({
            "status": "queued",
            "path": path,
            "overwrite": false,
            "bytes": byte_count
        }))
    }

    pub(super) fn execute_theme_reload_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        if !command.arguments.is_empty() {
            return Err("theme-reload does not accept arguments".into());
        }
        let config = self.as_ref().rust().config.clone();
        if !self.as_mut().update_theme_palette(&config) {
            return Err("theme palette reload failed".into());
        }
        let hyprland_config = serde_json::from_value::<ferric_browser_config::HyprlandConfig>(
            config.get("hyprland").cloned().unwrap_or(Value::Null),
        )
        .unwrap_or_default();
        self.as_mut().set_hyprland_status(QString::from(
            hyprland::HyprlandAdapter::from_config(&hyprland_config).status(),
        ));
        self.as_mut()
            .set_status_text(QString::from("Theme palette reloaded"));
        let palette = self.as_ref().rust().theme_palette.clone();
        let provider_status = serde_json::json!({
            "source": palette.source,
            "path": palette.path,
            "layout": palette.provider_layout,
            "version": palette.provider_version,
            "commit": palette.provider_commit,
        });
        Ok(serde_json::json!({
            "status": "reloaded",
            "palette": serde_json::to_value(&palette).unwrap_or(Value::Null),
            "provider_status": provider_status
        }))
    }

    pub(super) fn poll_config_edit(mut self: Pin<&mut Self>) {
        let pending = self.as_ref().rust().processes.pending_config_edit.clone();
        let Some(pending) = pending else {
            return;
        };
        let status = if let Some(status) = pending.exit_status {
            Some(status)
        } else {
            match try_wait_editor_process(&pending.process) {
                Ok(Some(status)) => {
                    let mut rust = self.as_mut().rust_mut();
                    if let Some(current) = rust
                        .as_mut()
                        .get_mut()
                        .processes
                        .pending_config_edit
                        .as_mut()
                        .filter(|current| current.token == pending.token)
                    {
                        current.exit_status = Some(status);
                    }
                    Some(status)
                }
                Ok(None) => None,
                Err(error) => {
                    let removed = {
                        let mut rust = self.as_mut().rust_mut();
                        let this = rust.as_mut().get_mut();
                        if this
                            .processes
                            .pending_config_edit
                            .as_ref()
                            .is_some_and(|current| current.token == pending.token)
                        {
                            this.processes.pending_config_edit.take()
                        } else {
                            None
                        }
                    };
                    if removed.is_some() {
                        self.as_mut().set_status_text(QString::from(format!(
                            "Configuration editor status unavailable: {error}"
                        )));
                    }
                    return;
                }
            }
        };
        let Some(status) = status else {
            return;
        };
        let stderr = {
            let result = match pending.stderr.lock() {
                Ok(mut result) => result.take(),
                Err(_) => Some(Err("configuration editor stderr was unavailable".into())),
            };
            let Some(result) = result else {
                return;
            };
            match result {
                Ok(bytes) => {
                    let text = sanitize_process_stderr(&bytes);
                    (!text.is_empty()).then_some(text)
                }
                Err(error) => Some(error),
            }
        };
        {
            let removed = {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                if this
                    .processes
                    .pending_config_edit
                    .as_ref()
                    .is_some_and(|current| current.token == pending.token)
                {
                    this.processes.pending_config_edit.take()
                } else {
                    None
                }
            };
            if removed.is_some() {
                let stderr_detail = stderr
                    .as_deref()
                    .map_or_else(String::new, |text| format!(" (stderr: {text})"));
                if status.success() {
                    let mut rust = self.as_mut().rust_mut();
                    rust.as_mut()
                        .get_mut()
                        .configuration_io
                        .config_watch
                        .mark_pending(
                            Instant::now()
                                .checked_sub(Duration::from_millis(200))
                                .unwrap_or_else(Instant::now),
                        );
                    self.as_mut().set_status_text(QString::from(format!(
                        "Configuration editor finished; reloading {}{}",
                        pending.path.display(),
                        stderr_detail
                    )));
                } else {
                    self.as_mut().set_status_text(QString::from(format!(
                        "Configuration editor exited with status {status}{stderr_detail}"
                    )));
                }
            }
        }
    }

    pub(super) fn probe_desktop_portals(mut self: Pin<&mut Self>) -> bool {
        let request_result = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.portal_probe_worker.is_none() {
                this.portal_probe_worker = PortalProbeWorker::spawn().ok();
            }
            this.portal_probe_worker.as_mut().map_or_else(
                || Err("portal probe worker could not start".into()),
                PortalProbeWorker::request,
            )
        };
        if let Err(error) = request_result {
            let _ = error;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .portal_capabilities = PortalCapabilities::unavailable();
            return false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .portal_capabilities = PortalCapabilities::pending();
        true
    }

    pub(super) fn portal_capability_status(self: Pin<&mut Self>, capability: &QString) -> QString {
        let status = self
            .as_ref()
            .rust()
            .portal_capabilities
            .capability_status(&capability.to_string());
        QString::from(status)
    }
}
