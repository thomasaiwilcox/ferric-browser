use super::browser_ui_page_windows::PageSettingsCompletion;
use super::browser_ui_state::{
    PendingProfileConfiguration, PendingSelectorAction, PendingSelectorValidation,
};
use super::selector_validation::{
    SelectorCallbackDecision, SelectorValidationItem, SelectorValidationResult,
    selector_callback_decision, selector_validation_items, selector_validation_items_for_check,
};
use super::{CxxQtType, Duration, Instant, Pin, QString, Value, qobject};
use ferric_browser_application::NativeOperationOutcome;
use ferric_browser_config::ProfilesConfig;

pub(super) const SELECTOR_VALIDATION_TIMEOUT: Duration = Duration::from_secs(5);

impl qobject::BrowserUi {
    pub(super) fn validate_or_queue_committed_peer_configuration(
        mut self: Pin<&mut Self>,
        prepared: ferric_browser_application::PreparedConfigurationUpdate,
    ) -> Result<Option<Value>, String> {
        if self.as_ref().rust().profile_bootstrap_pending || !prepared.is_committed_peer_update() {
            return Err(
                "Shared settings require a ready owner and their exact committed preparation"
                    .into(),
            );
        }
        let sources = prepared.policy_sources();
        let layers = sources.desired_layers();
        let items = selector_validation_items(layers);
        let fingerprint = selector_fingerprint(&items)?;
        let needs_validation = !items.is_empty()
            && fingerprint
                != self
                    .as_ref()
                    .rust()
                    .configuration_io
                    .active_selector_fingerprint;
        if needs_validation {
            self.as_mut().begin_selector_validation(
                fingerprint,
                items,
                PendingSelectorAction::CommittedPeer {
                    prepared: Box::new(prepared),
                },
                None,
                None,
            )?;
            return Ok(None);
        }
        let receipt = self.as_ref().rust().state.as_ref().and_then(|state| {
            self.as_ref()
                .rust()
                .configuration_io
                .selector_receipts
                .matching(state, layers)
        });
        self.as_mut()
            .commit_committed_peer_configuration(
                prepared,
                super::selector_validation_receipts::SelectorValidationPublication {
                    fingerprint,
                    receipt,
                },
            )
            .map(Some)
    }

    pub(super) fn validate_or_queue_runtime_config(
        mut self: Pin<&mut Self>,
        prepared: ferric_browser_application::PreparedConfigurationUpdate,
        status: String,
        persist: bool,
        origin: Option<&super::browser_ui_configuration_preparation::ConfigurationRequestOrigin>,
    ) -> Result<Option<Value>, String> {
        self.as_mut()
            .validate_or_queue_runtime_config_with_post_action(
                prepared, status, persist, origin, None,
            )
    }

    pub(super) fn validate_or_queue_runtime_config_with_post_action(
        mut self: Pin<&mut Self>,
        prepared: ferric_browser_application::PreparedConfigurationUpdate,
        status: String,
        persist: bool,
        origin: Option<&super::browser_ui_configuration_preparation::ConfigurationRequestOrigin>,
        post_action: Option<super::configuration_completion::ConfigurationCompletion>,
    ) -> Result<Option<Value>, String> {
        let layers = prepared.policy_sources().desired_layers().clone();
        let items = selector_validation_items(&layers);
        let fingerprint = selector_fingerprint(&items)?;
        let completion = origin
            .and_then(|origin| origin.handoff.as_ref())
            .map(super::browser_ui_page_windows::PageHandoff::begin_settings)
            .transpose()?;
        let receipt = self.as_ref().rust().state.as_ref().and_then(|state| {
            self.as_ref()
                .rust()
                .configuration_io
                .selector_receipts
                .matching(state, &layers)
        });
        // Equal selector strings can belong to a changed raw rule catalogue.
        // Site Policies must retain evidence for that exact saved graph so its
        // explicit activation can reuse the receipt without another pipeline.
        let requires_receipt = matches!(
            post_action.as_ref(),
            Some(super::configuration_completion::ConfigurationCompletion::SitePolicy { .. })
        );
        let needs_validation = !items.is_empty()
            && (requires_receipt && receipt.is_none()
                || fingerprint
                    != self
                        .as_ref()
                        .rust()
                        .configuration_io
                        .active_selector_fingerprint);
        if needs_validation {
            self.as_mut().begin_selector_validation(
                fingerprint,
                items,
                PendingSelectorAction::RuntimeConfig {
                    post_action,
                    prepared: Box::new(prepared),
                    status,
                    persist,
                },
                origin,
                completion,
            )?;
            return Ok(None);
        }
        let result = self.as_mut().commit_runtime_config_persisted(
            prepared,
            &status,
            persist,
            super::selector_validation_receipts::SelectorValidationPublication {
                fingerprint,
                receipt,
            },
            origin,
            completion,
            post_action,
        )?;
        Ok(Some(result))
    }

    pub(super) fn begin_selector_validation(
        mut self: Pin<&mut Self>,
        fingerprint: String,
        items: Vec<SelectorValidationItem>,
        action: PendingSelectorAction,
        origin: Option<&super::browser_ui_configuration_preparation::ConfigurationRequestOrigin>,
        mut completion: Option<PageSettingsCompletion>,
    ) -> Result<u64, String> {
        let result = (|| {
            if self.as_ref().rust().profile_bootstrap_pending
                && !matches!(&action, PendingSelectorAction::Profile { pending, .. }
                    if self.as_ref().profile_configuration_request_is_current(&pending.request))
            {
                return Err(
                    "The profile is still opening. Validate settings after it is ready.".into(),
                );
            }
            let generation = self
                .as_ref()
                .rust()
                .configuration_io
                .selector_generation
                .checked_add(1)
                .ok_or("configuration request generation is exhausted")?;
            let validation = {
                let browser = self.as_ref();
                let rust = browser.rust();
                let peer_sources = match &action {
                    PendingSelectorAction::CommittedPeer { prepared }
                    | PendingSelectorAction::RuntimeConfig { prepared, .. } => {
                        Some(prepared.policy_sources())
                    }
                    _ => None,
                };
                let inputs = match &action {
                    PendingSelectorAction::CommittedPeer { .. }
                    | PendingSelectorAction::RuntimeConfig { .. } => rust
                        .state
                        .as_ref()
                        .zip(peer_sources.as_ref())
                        .map(|(state, sources)| (state, sources.desired_layers())),
                    PendingSelectorAction::Reload { publication } => rust
                        .state
                        .as_ref()
                        .map(|state| (state, &publication.layers)),
                    PendingSelectorAction::Profile { .. } => None,
                    PendingSelectorAction::Check { .. } | PendingSelectorAction::Prepare(_) => None,
                };
                if let PendingSelectorAction::Profile { pending, candidate } = &action {
                    rust.configuration_io.selector_receipts.capture_profile(
                        &pending.state,
                        candidate,
                        generation,
                        &items,
                    )
                } else {
                    inputs.and_then(|(state, layers)| {
                        rust.configuration_io
                            .selector_receipts
                            .capture(state, layers, generation, &items)
                    })
                }
            };
            let request = serde_json::json!({
                "generation": generation.to_string(),
                "items": items,
            });
            let request = serde_json::to_string(&request).unwrap_or_default();
            let (previous, same_request) = {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                if origin.is_some_and(|origin| {
                    this.configuration_io.selector_origin_generation != origin.generation
                }) {
                    return Err("A newer request replaced this settings preparation".into());
                }
                if origin
                    .and_then(|origin| origin.operation_id.as_deref())
                    .is_some_and(|id| {
                        this.operation(id)
                            .is_none_or(|operation| operation.state.is_terminal())
                    })
                {
                    return Err("The settings operation was cancelled before validation".into());
                }
                let previous = this.configuration_io.pending_selector_validation.take();
                let same_request = origin.is_some_and(|origin| {
                    previous
                        .as_ref()
                        .and_then(|pending| pending.origin.as_ref())
                        .is_some_and(|previous| previous.generation == origin.generation)
                });
                if !same_request
                    && let Some(operation_id) = previous
                        .as_ref()
                        .and_then(|pending| pending.operation_id.clone())
                {
                    this.set_operation(operation_id, crate::OperationUpdate::cancelled());
                }
                this.configuration_io.selector_generation = generation;
                this.configuration_io.selector_origin_generation =
                    origin.map_or(generation, |origin| origin.generation);
                this.configuration_io.pending_selector_validation =
                    Some(PendingSelectorValidation {
                        completion: completion.take(),
                        operation_id: origin.and_then(|origin| origin.operation_id.clone()),
                        setting_feedback: origin.and_then(|origin| origin.setting_feedback.clone()),
                        origin: origin.cloned(),
                        generation,
                        expected_revision: this.configuration_io.selector_config_revision,
                        owner: if matches!(&action, PendingSelectorAction::Profile { .. }) {
                            None
                        } else {
                            this.state
                                .as_ref()
                                .and_then(super::BrowserApplication::capture_configuration_request)
                        },
                        started_at: Some(Instant::now()),
                        fingerprint,
                        validation,
                        action,
                    });
                (previous, same_request)
            };
            let replaced_feedback = previous.and_then(|pending| {
                if let Some(completion) = pending.completion {
                    completion.finish(NativeOperationOutcome::Failed);
                }
                if same_request {
                    None
                } else {
                    pending.setting_feedback
                }
            });
            self.as_mut().finish_setting_feedback(
                replaced_feedback,
                Err("A newer request replaced this settings validation. Try again."),
            );
            self.as_mut()
                .set_selector_validation_request(QString::from(request));
            self.as_mut().set_status_text(QString::from(
                "Validating configured CSS selectors with WebEngine",
            ));
            Ok(generation)
        })();
        if result.is_err()
            && let Some(completion) = completion
        {
            completion.finish(NativeOperationOutcome::Failed);
        }
        result
    }

    pub(super) fn queue_config_check_validation(
        mut self: Pin<&mut Self>,
        loaded: ferric_browser_config::LoadedConfig,
        profiles: Option<ProfilesConfig>,
    ) -> Result<bool, String> {
        let source_count = loaded.sources.len();
        let items = selector_validation_items_for_check(
            &loaded.config,
            profiles.as_ref(),
            &loaded.authored,
        );
        if items.is_empty() {
            self.as_mut().set_status_text(QString::from(format!(
                "Configuration valid (full-validation; {source_count} source files, no selectors)"
            )));
            return Ok(false);
        }
        let fingerprint = selector_fingerprint(&items)?;
        self.as_mut().begin_selector_validation(
            fingerprint,
            items,
            PendingSelectorAction::Check { source_count },
            None,
            None,
        )?;
        Ok(true)
    }

    pub(super) fn queue_profile_selector_validation(
        mut self: Pin<&mut Self>,
        mut pending: PendingProfileConfiguration,
        mut candidate: ferric_browser_application::ProfileOpenCandidate,
    ) {
        let request = std::sync::Arc::clone(&pending.request);
        if !self
            .as_ref()
            .profile_configuration_request_is_current(&request)
        {
            return;
        }
        if !self
            .as_ref()
            .profile_candidate_bindings_are_valid(&candidate)
        {
            candidate = match pending.state.prepare_profile_candidate_fallback(
                candidate,
                "Configured bindings could not compile; built-in safe configuration selected"
                    .into(),
            ) {
                Ok(candidate) => candidate,
                Err(error) => {
                    self.as_mut()
                        .finish_configure_profile(pending, Err(error.to_string()));
                    return;
                }
            };
            if !self
                .as_ref()
                .profile_candidate_bindings_are_valid(&candidate)
            {
                self.as_mut().finish_configure_profile(
                    pending,
                    Err("Built-in safe bindings could not compile".into()),
                );
                return;
            }
        }
        let items = selector_validation_items(candidate.layers());
        let fingerprint = selector_fingerprint(&items).unwrap_or_default();
        if items.is_empty() {
            self.as_mut()
                .admit_engine_checked_profile(pending, candidate, fingerprint);
            return;
        }
        if let Err(error) = self.as_mut().begin_selector_validation(
            fingerprint,
            items,
            PendingSelectorAction::Profile {
                pending: Box::new(pending),
                candidate: Box::new(candidate),
            },
            None,
            None,
        ) {
            if !self
                .as_ref()
                .profile_configuration_request_is_current(&request)
            {
                return;
            }
            self.as_mut().clear_policy_owner_preparation();
            self.as_mut().set_profile_bootstrap_pending(false);
            if self
                .as_ref()
                .profile_configuration_request_is_current(&request)
            {
                self.as_mut().set_status_text(QString::from(error));
            }
        }
    }

    fn profile_candidate_bindings_are_valid(
        &self,
        candidate: &ferric_browser_application::ProfileOpenCandidate,
    ) -> bool {
        serde_json::to_value(&candidate.configuration().effective).is_ok_and(|config| {
            super::configured_bindings(&self.rust().registry, &config).is_some()
        })
    }

    fn admit_engine_checked_profile(
        mut self: Pin<&mut Self>,
        mut pending: PendingProfileConfiguration,
        candidate: ferric_browser_application::ProfileOpenCandidate,
        fingerprint: String,
    ) {
        if !self
            .as_ref()
            .profile_configuration_request_is_current(&pending.request)
        {
            return;
        }
        match pending.state.admit_profile_candidate(candidate) {
            Ok(()) => {
                pending.selector_fingerprint = Some(fingerprint);
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_profile_configuration = Some(pending);
                // The runtime poller may have gone idle while WebEngine
                // checked the selectors. Resume the admitted profile commit.
                self.as_mut().runtime_work_available();
            }
            Err(error) => self
                .as_mut()
                .finish_configure_profile(pending, Err(error.to_string())),
        }
    }

    fn selector_validation_owner_is_current(
        &self,
        action: &PendingSelectorAction,
        owner: Option<&ferric_browser_application::ConfigurationRequestCapture>,
    ) -> bool {
        if !self.rust().view_alive || self.rust().ipc_shutdown_gate {
            return false;
        }
        match action {
            PendingSelectorAction::Profile { pending, candidate } => {
                self.profile_configuration_request_is_current(&pending.request)
                    && pending.state.profile_candidate_is_current(candidate)
            }
            _ => owner.is_some_and(|owner| {
                self.rust()
                    .state
                    .as_ref()
                    .is_some_and(|state| state.configuration_request_is_current(owner))
            }),
        }
    }

    pub(super) fn complete_selector_validation(
        mut self: Pin<&mut Self>,
        generation: &QString,
        result: &QString,
    ) -> bool {
        let generation = match generation.to_string().parse::<u64>() {
            Ok(value) => value,
            Err(_) => return false,
        };
        let result = match serde_json::from_str::<SelectorValidationResult>(&result.to_string()) {
            Ok(value) => value,
            Err(_) => SelectorValidationResult {
                ok: false,
                family: String::new(),
                index: 0,
                rule: String::new(),
                reason: "WebEngine returned an unreadable validation result".into(),
            },
        };
        // A queued callback can arrive after its deadline but before the
        // runtime timer polls it. Preserve the existing timeout rejection and
        // safe-profile fallback instead of consuming it as merely stale.
        let expired = self
            .as_ref()
            .rust()
            .configuration_io
            .pending_selector_validation
            .as_ref()
            .is_some_and(|pending| {
                pending.generation == generation
                    && !matches!(pending.action, PendingSelectorAction::Prepare(_))
                    && pending
                        .started_at
                        .is_some_and(|started| started.elapsed() >= SELECTOR_VALIDATION_TIMEOUT)
            });
        if expired {
            self.as_mut().expire_selector_validation();
            return true;
        }
        let pending = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let is_current = this
                .configuration_io
                .pending_selector_validation
                .as_ref()
                .is_some_and(|pending| {
                    pending.generation == generation
                        && !matches!(pending.action, PendingSelectorAction::Prepare(_))
                });
            if !is_current {
                return false;
            }
            this.configuration_io.pending_selector_validation.take()
        };
        let Some(mut pending) = pending else {
            return false;
        };
        self.as_mut()
            .set_selector_validation_request(QString::default());
        let owner_current = self
            .as_ref()
            .selector_validation_owner_is_current(&pending.action, pending.owner.as_ref());
        let operation_current = pending.operation_id.as_ref().is_none_or(|id| {
            self.as_ref()
                .rust()
                .operation(id)
                .is_some_and(|operation| !operation.state.is_terminal())
        });
        let request_current = {
            let browser = self.as_ref();
            let rust = browser.rust();
            rust.view_alive
                && !rust.ipc_shutdown_gate
                && pending.origin.as_ref().map_or(
                    rust.configuration_io.selector_origin_generation == pending.generation,
                    |origin| origin.is_current(rust),
                )
        };
        // Clearing the QML request above may reenter or itself cross the
        // deadline. A still-current profile must follow timeout fallback here
        // as well; a replaced owner/generation remains stale below.
        let timed_out = pending
            .started_at
            .is_none_or(|started| started.elapsed() >= SELECTOR_VALIDATION_TIMEOUT);
        let result = if timed_out {
            SelectorValidationResult {
                ok: false,
                family: String::new(),
                index: 0,
                rule: String::new(),
                reason: "WebEngine selector validation timed out".into(),
            }
        } else {
            result
        };
        let decision = if owner_current
            && operation_current
            && request_current
            && self.as_ref().rust().configuration_io.selector_generation == pending.generation
        {
            selector_callback_decision(
                generation,
                pending.generation,
                pending.expected_revision,
                self.as_ref()
                    .rust()
                    .configuration_io
                    .selector_config_revision,
                result.ok,
            )
        } else {
            SelectorCallbackDecision::Stale
        };
        if !matches!(decision, SelectorCallbackDecision::Accept) {
            if let Some(completion) = pending.completion.take() {
                completion.finish(if matches!(decision, SelectorCallbackDecision::Stale) {
                    NativeOperationOutcome::Stale
                } else {
                    NativeOperationOutcome::Failed
                });
            }
            let detail = if matches!(decision, SelectorCallbackDecision::Stale) {
                "Settings changed during validation. Try again.".to_owned()
            } else {
                selector_diagnostic(&result)
            };
            self.as_mut()
                .finish_setting_feedback(pending.setting_feedback.take(), Err(&detail));
            if let Some(operation_id) = pending.operation_id.as_ref() {
                self.as_mut().rust_mut().as_mut().get_mut().set_operation(
                    operation_id.clone(),
                    crate::OperationUpdate::failed(
                        if timed_out && matches!(decision, SelectorCallbackDecision::Reject) {
                            crate::FailureKind::Timeout
                        } else {
                            crate::FailureKind::Config
                        },
                        detail.clone(),
                    ),
                );
            }
        }
        match decision {
            SelectorCallbackDecision::Stale => {
                if let PendingSelectorAction::Profile { pending, .. } = pending.action {
                    if self
                        .as_ref()
                        .profile_configuration_request_is_current(&pending.request)
                    {
                        self.as_mut().finish_configure_profile(
                            *pending,
                            Err("Profile validation became stale before metadata admission".into()),
                        );
                    }
                } else {
                    self.as_mut().set_status_text(QString::from(
                        "Selector settings changed. Validate the rule again.",
                    ));
                }
            }
            SelectorCallbackDecision::Reject => self
                .as_mut()
                .reject_selector_action(pending.action, &result),
            SelectorCallbackDecision::Accept => self.as_mut().accept_selector_action(pending),
        }
        true
    }

    pub(super) fn expire_selector_validation(mut self: Pin<&mut Self>) {
        let expired = self
            .as_ref()
            .rust()
            .configuration_io
            .pending_selector_validation
            .as_ref()
            .is_some_and(|pending| {
                pending.started_at.is_some_and(|started| {
                    Instant::now().duration_since(started) >= SELECTOR_VALIDATION_TIMEOUT
                })
            });
        if !expired {
            return;
        }
        let pending = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .configuration_io
            .pending_selector_validation
            .take();
        if let Some(pending) = pending {
            if let Some(completion) = pending.completion {
                completion.finish(NativeOperationOutcome::Failed);
            }
            self.as_mut()
                .set_selector_validation_request(QString::default());
            let result = SelectorValidationResult {
                ok: false,
                family: String::new(),
                index: 0,
                rule: String::new(),
                reason: "WebEngine selector validation timed out".into(),
            };
            self.as_mut()
                .finish_setting_feedback(pending.setting_feedback, Err(&result.reason));
            if let Some(operation_id) = pending.operation_id {
                self.as_mut().rust_mut().as_mut().get_mut().set_operation(
                    operation_id,
                    crate::OperationUpdate::failed(
                        crate::FailureKind::Timeout,
                        result.reason.clone(),
                    ),
                );
            }
            let current = {
                let browser = self.as_ref();
                let rust = browser.rust();
                rust.configuration_io.selector_generation == pending.generation
                    && browser.selector_validation_owner_is_current(
                        &pending.action,
                        pending.owner.as_ref(),
                    )
            };
            if current {
                self.as_mut()
                    .reject_selector_action(pending.action, &result);
            }
        }
    }

    fn accept_selector_action(mut self: Pin<&mut Self>, pending: PendingSelectorValidation) {
        let receipt = self
            .as_ref()
            .rust()
            .configuration_io
            .selector_receipts
            .accepted(pending.validation, pending.generation);
        let operation_id = pending.operation_id;
        let setting_feedback = pending.setting_feedback;
        let origin = pending.origin;
        let request = origin.as_ref().map(|origin| origin.generation);
        match pending.action {
            PendingSelectorAction::Prepare(_) => {}
            PendingSelectorAction::CommittedPeer { prepared } => {
                let result = self.as_mut().commit_committed_peer_configuration(
                    *prepared,
                    super::selector_validation_receipts::SelectorValidationPublication {
                        fingerprint: pending.fingerprint,
                        receipt,
                    },
                );
                if let Some(completion) = pending.completion {
                    completion.finish(if result.is_ok() {
                        NativeOperationOutcome::Applied
                    } else {
                        NativeOperationOutcome::Failed
                    });
                }
                match result {
                    Ok(_) => {
                        self.as_mut()
                            .continue_setting_feedback_for_request(setting_feedback, request);
                        if let Some(id) = operation_id {
                            self.as_mut().rust_mut().as_mut().get_mut().set_operation(
                                id,
                                crate::OperationUpdate::completed("shared settings received"),
                            );
                        }
                    }
                    Err(error) => {
                        self.as_mut()
                            .finish_setting_feedback(setting_feedback, Err(&error));
                        if let Some(id) = operation_id {
                            self.as_mut().rust_mut().as_mut().get_mut().set_operation(
                                id,
                                crate::OperationUpdate::failed(
                                    crate::FailureKind::Config,
                                    error.clone(),
                                ),
                            );
                        }
                        self.as_mut().set_status_text(QString::from(format!(
                            "Shared settings reception postponed: {error}"
                        )));
                    }
                }
            }
            PendingSelectorAction::RuntimeConfig {
                post_action,
                prepared,
                status,
                persist,
            } => {
                match self.as_mut().commit_runtime_config_persisted(
                    *prepared,
                    &status,
                    persist,
                    super::selector_validation_receipts::SelectorValidationPublication {
                        fingerprint: pending.fingerprint,
                        receipt,
                    },
                    origin.as_ref(),
                    pending.completion,
                    post_action,
                ) {
                    Ok(_) => {
                        self.as_mut()
                            .continue_setting_feedback_for_request(setting_feedback, request);
                        if let Some(operation_id) = operation_id
                            && !self
                                .as_mut()
                                .bind_configuration_operation_for_request(&operation_id, request)
                        {
                            self.as_mut().rust_mut().as_mut().get_mut().set_operation(
                                operation_id,
                                crate::OperationUpdate::completed("settings published"),
                            );
                        }
                    }
                    Err(error) => {
                        self.as_mut()
                            .finish_setting_feedback(setting_feedback, Err(&error));
                        if let Some(operation_id) = operation_id {
                            self.as_mut().rust_mut().as_mut().get_mut().set_operation(
                                operation_id,
                                crate::OperationUpdate::failed(
                                    crate::FailureKind::Config,
                                    error.clone(),
                                ),
                            );
                        }
                        self.as_mut().set_status_text(QString::from(format!(
                            "Selector configuration commit rejected: {error}"
                        )));
                    }
                }
            }
            PendingSelectorAction::Reload { mut publication } => {
                publication.selectors.receipt = receipt;
                let announce = publication.announce;
                if let Err(error) = self
                    .as_mut()
                    .queue_context_configuration_reload(*publication)
                {
                    self.report_config_reload_failure(&error, announce);
                }
            }
            PendingSelectorAction::Profile {
                pending: mut profile,
                candidate,
            } => {
                if !self
                    .as_ref()
                    .profile_configuration_request_is_current(&profile.request)
                {
                    return;
                }
                profile.selector_receipt = receipt;
                self.as_mut().admit_engine_checked_profile(
                    *profile,
                    *candidate,
                    pending.fingerprint,
                );
            }
            PendingSelectorAction::Check { source_count } => {
                self.as_mut().set_status_text(QString::from(format!(
                    "Configuration valid (full-validation; {source_count} source files)"
                )));
            }
        }
    }

    fn reject_selector_action(
        mut self: Pin<&mut Self>,
        action: PendingSelectorAction,
        result: &SelectorValidationResult,
    ) {
        let diagnostic = selector_diagnostic(result);
        match action {
            PendingSelectorAction::Profile { pending, candidate } => {
                if !self
                    .as_ref()
                    .profile_configuration_request_is_current(&pending.request)
                {
                    return;
                }
                let mut pending = *pending;
                match pending.state.prepare_profile_candidate_fallback(
                    *candidate,
                    format!("{diagnostic}; built-in safe configuration selected"),
                ) {
                    Ok(candidate) => self
                        .as_mut()
                        .queue_profile_selector_validation(pending, candidate),
                    Err(error) => self
                        .as_mut()
                        .finish_configure_profile(pending, Err(error.to_string())),
                }
            }
            PendingSelectorAction::Reload { publication } => {
                self.as_mut()
                    .report_config_reload_failure(&diagnostic, publication.announce);
            }
            PendingSelectorAction::CommittedPeer { .. } => {
                self.as_mut().set_status_text(QString::from(format!(
                    "Shared settings reception postponed: {diagnostic}"
                )));
            }
            PendingSelectorAction::RuntimeConfig { post_action, .. } => {
                if let Some(post_action) = post_action {
                    self.as_mut().finish_configuration_completion(
                        post_action,
                        super::configuration_completion::ConfigurationSaveOutcome::Failed,
                    );
                }
                self.as_mut().set_status_text(QString::from(diagnostic));
            }
            PendingSelectorAction::Prepare(_) | PendingSelectorAction::Check { .. } => {
                self.as_mut().set_status_text(QString::from(diagnostic));
            }
        }
    }
}

fn selector_fingerprint(items: &[SelectorValidationItem]) -> Result<String, String> {
    serde_json::to_string(items)
        .map_err(|error| format!("could not encode selector policy fingerprint: {error}"))
}

fn selector_diagnostic(result: &SelectorValidationResult) -> String {
    let reason = result
        .reason
        .chars()
        .filter(|character| !character.is_control())
        .take(192)
        .collect::<String>();
    if result.family.is_empty() {
        return format!("WebEngine selector validation failed: {reason}");
    }
    let rule = if result.rule.is_empty() {
        "configuration"
    } else {
        &result.rule
    };
    format!(
        "Selector validation failed for {rule}/{}[{}]: {reason}",
        result.family, result.index
    )
}
