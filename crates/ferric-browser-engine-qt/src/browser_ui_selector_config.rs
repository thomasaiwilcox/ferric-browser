use super::browser_ui_state::{
    PendingProfileConfiguration, PendingSelectorAction, PendingSelectorValidation,
};
use super::selector_validation::{
    SelectorCallbackDecision, SelectorValidationItem, SelectorValidationResult,
    selector_callback_decision, selector_validation_items, selector_validation_items_for_check,
};
use super::{
    Config, ConfigurationLayers, CxxQtType, Duration, Instant, PathBuf, Pin, ProfileOpenEffect,
    QString, RuntimeOverrides, Value, qobject,
};
use ferric_browser_config::ProfilesConfig;

const SELECTOR_VALIDATION_TIMEOUT: Duration = Duration::from_secs(5);

impl qobject::BrowserUi {
    pub(super) fn validate_or_queue_runtime_config(
        mut self: Pin<&mut Self>,
        candidate: Value,
        layers: ConfigurationLayers,
        status: String,
        persistent_overrides: RuntimeOverrides,
        persist_path: Option<PathBuf>,
    ) -> Result<Option<Value>, String> {
        let items = selector_validation_items(&layers);
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
                PendingSelectorAction::RuntimeConfig {
                    candidate,
                    layers: Box::new(layers),
                    status,
                    persistent_overrides,
                    persist_path,
                },
            );
            return Ok(None);
        }
        let result = self.as_mut().commit_runtime_config_persisted(
            candidate,
            layers,
            &status,
            persist_path
                .as_deref()
                .map(|path| (path, &persistent_overrides)),
            fingerprint,
        )?;
        Ok(Some(result))
    }

    pub(super) fn begin_selector_validation(
        mut self: Pin<&mut Self>,
        fingerprint: String,
        items: Vec<SelectorValidationItem>,
        action: PendingSelectorAction,
    ) -> u64 {
        let generation = self
            .as_ref()
            .rust()
            .configuration_io
            .selector_generation
            .wrapping_add(1)
            .max(1);
        let request = serde_json::json!({
            "generation": generation.to_string(),
            "items": items,
        });
        let request = serde_json::to_string(&request).unwrap_or_default();
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if let Some(previous) = this.configuration_io.pending_selector_validation.take()
                && let Some(operation_id) = previous.operation_id
            {
                this.set_operation(operation_id, crate::OperationUpdate::cancelled());
            }
            this.configuration_io.selector_generation = generation;
            this.configuration_io.pending_selector_validation = Some(PendingSelectorValidation {
                operation_id: None,
                generation,
                expected_revision: this.configuration_io.selector_config_revision,
                started_at: Instant::now(),
                fingerprint,
                action,
            });
        }
        self.as_mut()
            .set_selector_validation_request(QString::from(request));
        self.as_mut().set_status_text(QString::from(
            "Validating configured CSS selectors with WebEngine",
        ));
        generation
    }

    pub(super) fn queue_config_check_validation(
        mut self: Pin<&mut Self>,
        config: Config,
        profiles: Option<ProfilesConfig>,
        source_count: usize,
    ) -> Result<bool, String> {
        let items = selector_validation_items_for_check(&config, profiles.as_ref());
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
        );
        Ok(true)
    }

    pub(super) fn queue_profile_selector_validation(
        mut self: Pin<&mut Self>,
        pending: PendingProfileConfiguration,
        effect: ProfileOpenEffect,
        layers: ConfigurationLayers,
    ) {
        let items = selector_validation_items(&layers);
        if items.is_empty() {
            let fingerprint = selector_fingerprint(&items).unwrap_or_default();
            self.as_mut()
                .set_active_selector_fingerprint_value(fingerprint);
            self.as_mut().finish_configure_profile(pending, Ok(effect));
            return;
        }
        let fingerprint = selector_fingerprint(&items).unwrap_or_default();
        self.as_mut().begin_selector_validation(
            fingerprint,
            items,
            PendingSelectorAction::Profile {
                pending: Box::new(pending),
                effect: Box::new(effect),
            },
        );
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
        let pending = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let is_current = this
                .configuration_io
                .pending_selector_validation
                .as_ref()
                .is_some_and(|pending| pending.generation == generation);
            if !is_current {
                return false;
            }
            this.configuration_io.pending_selector_validation.take()
        };
        let Some(pending) = pending else {
            return false;
        };
        self.as_mut()
            .set_selector_validation_request(QString::default());
        let decision = selector_callback_decision(
            generation,
            pending.generation,
            pending.expected_revision,
            self.as_ref()
                .rust()
                .configuration_io
                .selector_config_revision,
            result.ok,
        );
        if !matches!(decision, SelectorCallbackDecision::Accept) {
            if let Some(operation_id) = pending.operation_id.as_ref() {
                self.as_mut().rust_mut().as_mut().get_mut().set_operation(
                    operation_id.clone(),
                    crate::OperationUpdate::failed(
                        crate::FailureKind::Config,
                        result.reason.clone(),
                    ),
                );
            }
        }
        match decision {
            SelectorCallbackDecision::Stale => self.as_mut().set_status_text(QString::from(
                "Selector settings changed. Validate the rule again.",
            )),
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
                Instant::now().duration_since(pending.started_at) >= SELECTOR_VALIDATION_TIMEOUT
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
            self.as_mut()
                .set_selector_validation_request(QString::default());
            let result = SelectorValidationResult {
                ok: false,
                family: String::new(),
                index: 0,
                rule: String::new(),
                reason: "WebEngine selector validation timed out".into(),
            };
            if let Some(operation_id) = pending.operation_id {
                self.as_mut().rust_mut().as_mut().get_mut().set_operation(
                    operation_id,
                    crate::OperationUpdate::failed(
                        crate::FailureKind::Timeout,
                        result.reason.clone(),
                    ),
                );
            }
            self.as_mut()
                .reject_selector_action(pending.action, &result);
        }
    }

    fn accept_selector_action(mut self: Pin<&mut Self>, pending: PendingSelectorValidation) {
        let operation_id = pending.operation_id;
        match pending.action {
            PendingSelectorAction::RuntimeConfig {
                candidate,
                layers,
                status,
                persistent_overrides,
                persist_path,
            } => {
                match self.as_mut().commit_runtime_config_persisted(
                    candidate,
                    *layers,
                    &status,
                    persist_path
                        .as_deref()
                        .map(|path| (path, &persistent_overrides)),
                    pending.fingerprint,
                ) {
                    Ok(_) => {
                        if let Some(operation_id) = operation_id {
                            if !self.as_mut().bind_configuration_operation(&operation_id) {
                                self.as_mut().rust_mut().as_mut().get_mut().set_operation(
                                    operation_id,
                                    crate::OperationUpdate::completed("settings published"),
                                );
                            }
                        }
                    }
                    Err(error) => {
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
            PendingSelectorAction::Reload { publication } => {
                let announce = publication.announce;
                if let Err(error) = self
                    .as_mut()
                    .queue_context_configuration_reload(*publication)
                {
                    self.report_config_reload_failure(&error, announce);
                }
            }
            PendingSelectorAction::Profile {
                pending: profile,
                effect,
            } => {
                self.as_mut()
                    .set_active_selector_fingerprint_value(pending.fingerprint);
                self.as_mut()
                    .finish_configure_profile(*profile, Ok(*effect));
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
            PendingSelectorAction::Profile { pending, effect } => {
                let mut pending = *pending;
                let mut effect = *effect;
                let safe = Config::default();
                match pending.state.replace_configuration(safe) {
                    Ok(snapshot) => {
                        effect.configuration = snapshot;
                        effect.configuration_error = Some(format!(
                            "{diagnostic}; built-in safe configuration remains active"
                        ));
                        self.as_mut()
                            .set_active_selector_fingerprint_value(String::new());
                        self.as_mut().finish_configure_profile(pending, Ok(effect));
                    }
                    Err(error) => {
                        self.as_mut().set_profile_bootstrap_pending(false);
                        self.as_mut().set_status_text(QString::from(format!(
                            "Selector validation failed ({diagnostic}); safe startup configuration could not be prepared: {error}"
                        )));
                    }
                }
            }
            PendingSelectorAction::Reload { publication } => {
                self.as_mut()
                    .report_config_reload_failure(&diagnostic, publication.announce);
            }
            PendingSelectorAction::RuntimeConfig { .. } | PendingSelectorAction::Check { .. } => {
                self.as_mut().set_status_text(QString::from(diagnostic));
            }
        }
    }

    pub(super) fn set_active_selector_fingerprint_value(
        mut self: Pin<&mut Self>,
        fingerprint: String,
    ) {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.configuration_io.active_selector_fingerprint = fingerprint;
        this.configuration_io.selector_config_revision = this
            .configuration_io
            .selector_config_revision
            .wrapping_add(1);
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
