//! Preparation uses the existing configuration-validation slot. Its private
//! turn value is passed through synchronous executors, so reentrant QML entry
//! points cannot inherit permission to prepare on their own input stack.
use crate::browser_ui_page_windows::{PageContinuation, PageHandoff};
use crate::browser_ui_state::{PendingSelectorAction, PendingSelectorValidation};
use crate::{CxxQtType, ParsedCommand, Pin, QString, Target, Value, WindowId, qobject};
use cxx_qt::Threading;
use ferric_browser_application::ConfigurationRequestCapture;

pub(super) struct ConfigurationPreparationTurn {
    pub(super) origin: ConfigurationRequestOrigin,
    owner: ConfigurationRequestCapture,
    _private: (),
}

impl ConfigurationPreparationTurn {
    pub(super) fn validate_handoff(
        &self,
        ui: &qobject::BrowserUi,
        operation: Option<&str>,
        incoming: Option<&PageHandoff>,
    ) -> Result<(), String> {
        if !self.is_current(ui) || self.origin.operation_id.as_deref() != operation {
            return Err("The settings command changed before its page handoff".into());
        }
        if self
            .origin
            .handoff
            .as_ref()
            .is_some_and(|root| incoming.is_none_or(|incoming| !root.same_root(incoming)))
        {
            return Err("The settings command's original page handoff changed".into());
        }
        let check = |origin: Option<&ConfigurationRequestOrigin>,
                     completion: bool,
                     pending_operation: Option<&str>| {
            if let Some(origin) =
                origin.filter(|origin| origin.generation == self.origin.generation)
            {
                if pending_operation != operation || origin.operation_id.as_deref() != operation {
                    return Err(
                        "The pending settings receipt belongs to another operation".to_owned()
                    );
                }
                if completion
                    && origin.handoff.as_ref().is_none_or(|root| {
                        incoming.is_none_or(|incoming| !root.same_root(incoming))
                    })
                {
                    return Err(
                        "The pending settings receipt belongs to another page handoff".to_owned(),
                    );
                }
            }
            Ok(())
        };
        let io = &ui.rust().configuration_io;
        if let Some(pending) = &io.pending_selector_validation {
            check(
                pending.origin.as_ref(),
                pending.completion.is_some(),
                pending.operation_id.as_deref(),
            )?;
        }
        if let Some(pending) = &io.pending_configuration_publication {
            check(
                pending.origin.as_ref(),
                pending.completion.as_ref().is_some_and(
                    super::browser_ui_page_windows::AdmittedPageSettingsCompletion::has_handoff,
                ),
                pending.operation_id.as_deref(),
            )?;
        }
        Ok(())
    }

    pub(super) fn is_current(&self, ui: &qobject::BrowserUi) -> bool {
        let rust = ui.rust();
        rust.view_alive
            && !rust.profile_bootstrap_pending
            && !rust.ipc_shutdown_gate
            && self.origin.is_current(rust)
            && rust
                .state
                .as_ref()
                .is_some_and(|state| state.configuration_request_owner_is_current(&self.owner))
    }
}

/// Completion metadata follows existing validation/persistence phases. It
/// confers no preparation or publication permission.
#[derive(Clone)]
pub(super) struct ConfigurationRequestOrigin {
    pub(super) generation: u64,
    pub(super) operation_id: Option<String>,
    pub(super) setting_feedback: Option<crate::browser_ui_state::SettingFeedback>,
    pub(super) handoff: Option<PageHandoff>,
}

impl ConfigurationRequestOrigin {
    pub(super) fn is_current(&self, rust: &crate::BrowserUiRust) -> bool {
        rust.configuration_io.selector_origin_generation == self.generation
            && self.operation_id.as_ref().is_none_or(|id| {
                rust.operation(id)
                    .is_some_and(|operation| !operation.state.is_terminal())
            })
    }
}

pub(super) struct DeferredConfigurationPreparation {
    owner: ConfigurationRequestCapture,
    window: Option<WindowId>,
    target: Option<Target>,
    action: DeferredConfigurationAction,
    handoff: Option<PageHandoff>,
    ready: bool,
}

pub(super) enum DeferredConfigurationAction {
    Commands(PageContinuation),
    Mutation(ParsedCommand),
    SiteDoctor(QString),
}

/// Presentation only; this grants no preparation or publication authority.
pub(super) fn site_doctor_is_preparing(rust: &crate::BrowserUiRust, id: &str) -> bool {
    rust.view_alive
        && !rust.profile_bootstrap_pending
        && !rust.ipc_shutdown_gate
        && rust.configuration_io.pending_selector_validation.as_ref().is_some_and(|pending| {
            pending.generation == rust.configuration_io.selector_generation
                && pending.generation == rust.configuration_io.selector_origin_generation
                && pending.owner.as_ref().is_some_and(|owner| rust.state.as_ref().is_some_and(|state| state.configuration_request_is_current(owner)))
                && pending.operation_id.as_ref().is_none_or(|id| rust.operation(id).is_some_and(|operation| !operation.state.is_terminal()))
                && matches!(&pending.action, PendingSelectorAction::Prepare(preparation)
                    if matches!(&preparation.action, DeferredConfigurationAction::SiteDoctor(queued) if queued == &QString::from(id)))
        })
}

pub(super) fn finish_replaced_preparation(
    pending: PendingSelectorValidation,
) -> Option<crate::browser_ui_state::SettingFeedback> {
    if let Some(completion) = pending.completion {
        completion.finish(ferric_browser_application::NativeOperationOutcome::Failed);
    }
    if let PendingSelectorAction::Prepare(mut preparation) = pending.action
        && let Some(handoff) = preparation.handoff.take()
    {
        handoff.finish_close(ferric_browser_application::NativeOperationOutcome::Failed);
    }
    pending.setting_feedback
}

pub(super) fn is_configuration_mutation(command: &ParsedCommand) -> bool {
    matches!(
        command.name.as_str(),
        "set"
            | "unset"
            | "bind"
            | "unbind"
            | "site-policy-apply"
            | "site-policy-edit"
            | "site-policy-discard"
    )
}

/// Inspect typed routing only; literals and configuration stay untouched.
pub(super) fn needs_preparation(
    command: &ParsedCommand,
    registry: &crate::CommandRegistry,
) -> bool {
    if is_configuration_mutation(command) {
        return true;
    }
    if command.name == "action" {
        return crate::parse_action_invocation(command)
            .is_ok_and(|(mapped, _)| needs_preparation(&mapped, registry));
    }
    command.name == "command-execute"
        && command.arguments.first().is_some_and(|id| {
            registry.definitions().iter().any(|definition| {
                definition.action.to_string() == *id
                    && matches!(
                        definition.name.as_str(),
                        "set"
                            | "unset"
                            | "bind"
                            | "unbind"
                            | "site-policy-apply"
                            | "site-policy-edit"
                            | "site-policy-discard"
                    )
            })
        })
}

impl qobject::BrowserUi {
    // Attach completion only under the synchronous private turn that actually
    // transfers the operation. The candidate/writer stay in their original
    // application; no operation ID is recreated in a destination catalogue.
    pub(super) fn attach_configuration_handoff(
        mut self: Pin<&mut Self>,
        handoff: &PageHandoff,
        turn: &ConfigurationPreparationTurn,
    ) -> Result<(), String> {
        let attach =
            |origin: &mut Option<ConfigurationRequestOrigin>,
             completion: &mut Option<super::browser_ui_page_windows::PageSettingsCompletion>,
             operation: &mut Option<String>|
             -> Result<(), String> {
                if let Some(origin) = origin
                    .as_mut()
                    .filter(|origin| origin.generation == turn.origin.generation)
                {
                    if completion.is_none() {
                        *completion = Some(handoff.begin_settings()?);
                    }
                    origin.handoff = Some(handoff.clone());
                    origin.operation_id = None;
                    *operation = None;
                }
                Ok(())
            };
        let mut rust = self.as_mut().rust_mut();
        let io = &mut rust.as_mut().get_mut().configuration_io;
        if let Some(pending) = io.pending_selector_validation.as_mut() {
            attach(
                &mut pending.origin,
                &mut pending.completion,
                &mut pending.operation_id,
            )?;
        }
        if let Some(pending) = io.pending_configuration_publication.as_mut()
            && let Some(origin) = pending
                .origin
                .as_mut()
                .filter(|origin| origin.generation == turn.origin.generation)
        {
            pending
                .completion
                .as_ref()
                .ok_or("The admitted settings receipt is unavailable")?
                .attach_handoff(handoff)?;
            origin.handoff = Some(handoff.clone());
            origin.operation_id = None;
            pending.operation_id = None;
        }
        Ok(())
    }

    pub(super) fn queue_configuration_preparation(
        mut self: Pin<&mut Self>,
        action: DeferredConfigurationAction,
        operation: Option<&str>,
        handoff: &mut Option<PageHandoff>,
    ) -> Result<Value, String> {
        let (replaced, generation) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let state = this
                .state
                .as_ref()
                .ok_or("application state is unavailable")?;
            if !this.view_alive
                || this.profile_bootstrap_pending
                || this.ipc_shutdown_gate
                || this.pending_profile_configuration.is_some()
                || this.configuration_io.pending_context_reload.is_some()
                || this
                    .configuration_io
                    .pending_configuration_publication
                    .is_some()
                || state.configuration_write_pending()
                || this
                    .configuration_io
                    .pending_selector_validation
                    .as_ref()
                    .is_some_and(|pending| {
                        matches!(
                            pending.action,
                            PendingSelectorAction::Prepare(_)
                                | PendingSelectorAction::RuntimeConfig { .. }
                                | PendingSelectorAction::CommittedPeer { .. }
                                | PendingSelectorAction::Profile { .. }
                        )
                    })
            {
                return Err(
                    "Another configuration operation is still pending. Wait for it to finish."
                        .into(),
                );
            }
            let owner = state
                .capture_configuration_request()
                .ok_or("configuration owner is unavailable")?;
            let generation = this
                .configuration_io
                .selector_generation
                .checked_add(1)
                .ok_or("configuration request generation is exhausted")?;
            let target = this.tab.and_then(|tab| state.capture_target(tab));
            let previous = this.configuration_io.pending_selector_validation.take();
            this.configuration_io.selector_generation = generation;
            this.configuration_io.selector_origin_generation = generation;
            this.configuration_io.pending_selector_validation = Some(PendingSelectorValidation {
                completion: None,
                operation_id: operation.map(str::to_owned),
                setting_feedback: None,
                generation,
                expected_revision: this.configuration_io.selector_config_revision,
                owner: Some(owner.clone()),
                started_at: None,
                fingerprint: String::new(),
                validation: None,
                origin: None,
                action: PendingSelectorAction::Prepare(Box::new(
                    DeferredConfigurationPreparation {
                        owner,
                        window: this.window,
                        target,
                        action,
                        handoff: handoff.take(),
                        ready: false,
                    },
                )),
            });
            if let Some(id) = previous
                .as_ref()
                .and_then(|pending| pending.operation_id.as_ref())
            {
                this.set_operation(id.clone(), crate::OperationUpdate::cancelled());
            }
            if let Some(id) = operation {
                this.set_operation(
                    id.to_owned(),
                    crate::OperationUpdate::running("settings preparation"),
                );
            }
            (previous, generation)
        };
        let scheduled = self.as_ref().qt_thread().queue(move |mut ui| {
            let ready = {
                let mut rust = ui.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.configuration_io
                    .pending_selector_validation
                    .as_mut()
                    .is_some_and(|pending| {
                        if pending.generation != generation {
                            return false;
                        }
                        if let PendingSelectorAction::Prepare(preparation) = &mut pending.action {
                            preparation.ready = true;
                            true
                        } else {
                            false
                        }
                    })
            };
            if ready {
                ui.as_mut().runtime_work_available();
            }
        });
        if scheduled.is_err() {
            let failed = {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                if this
                    .configuration_io
                    .pending_selector_validation
                    .as_ref()
                    .is_some_and(|pending| pending.generation == generation)
                {
                    this.configuration_io.pending_selector_validation.take()
                } else {
                    None
                }
            };
            let error = "The settings window closed before preparation could be scheduled";
            if let Some(failed) = failed {
                if let PendingSelectorAction::Prepare(mut preparation) = failed.action
                    && let Some(handoff) = preparation.handoff.take()
                {
                    handoff.owner_lost();
                }
                if let Some(id) = failed.operation_id {
                    self.as_mut().rust_mut().as_mut().get_mut().set_operation(
                        id,
                        crate::OperationUpdate::failed(crate::FailureKind::Config, error),
                    );
                }
                self.as_mut()
                    .finish_setting_feedback(failed.setting_feedback, Err(error));
            }
            self.as_mut().finish_setting_feedback(
                replaced.and_then(finish_replaced_preparation),
                Err(error),
            );
            return Err(error.into());
        }
        self.as_mut().finish_setting_feedback(
            replaced.and_then(finish_replaced_preparation),
            Err("A newer request replaced this settings validation. Try again."),
        );
        self.as_mut()
            .set_selector_validation_request(QString::default());
        if self.as_ref().rust().configuration_io.selector_generation != generation {
            return Err("A newer request replaced this settings preparation".into());
        }
        self.as_mut()
            .set_status_text(QString::from("Preparing settings"));
        Ok(
            serde_json::json!({"status":"pending-preparation", "pending":true, "generation":generation.to_string()}),
        )
    }

    pub(super) fn poll_configuration_preparation(mut self: Pin<&mut Self>) {
        let pending = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if !this
                .configuration_io
                .pending_selector_validation
                .as_ref()
                .is_some_and(|pending| matches!(&pending.action, PendingSelectorAction::Prepare(preparation) if preparation.ready))
            {
                return;
            }
            this.configuration_io.pending_selector_validation.take()
        };
        let Some(pending) = pending else { return };
        let PendingSelectorAction::Prepare(mut preparation) = pending.action else {
            unreachable!()
        };
        let current = {
            let binding = self.as_ref();
            let rust = binding.rust();
            rust.view_alive
                && !rust.profile_bootstrap_pending
                && !rust.ipc_shutdown_gate
                && rust.window == preparation.window
                && rust.state.as_ref().is_some_and(|state| {
                    state.configuration_request_is_current(&preparation.owner)
                        && (matches!(
                            &preparation.action,
                            DeferredConfigurationAction::Mutation(_)
                        ) || rust.tab.and_then(|tab| state.capture_target(tab))
                            == preparation.target)
                })
                && pending.operation_id.as_ref().is_none_or(|id| {
                    rust.operation(id)
                        .is_some_and(|operation| !operation.state.is_terminal())
                })
        };
        let operation = if preparation.handoff.is_none() {
            pending.operation_id.as_deref()
        } else {
            None
        };
        let mut operation_failure_kind = crate::FailureKind::Config;
        let result = if current {
            let turn = ConfigurationPreparationTurn {
                origin: ConfigurationRequestOrigin {
                    generation: pending.generation,
                    operation_id: pending.operation_id.clone(),
                    setting_feedback: pending.setting_feedback.clone(),
                    handoff: preparation.handoff.clone(),
                },
                _private: (),
                owner: preparation.owner.clone(),
            };
            match preparation.action {
                DeferredConfigurationAction::Mutation(command) => {
                    if matches!(
                        command.name.as_str(),
                        "site-policy-apply" | "site-policy-edit" | "site-policy-discard"
                    ) {
                        self.as_mut()
                            .execute_site_policy_command(&command, Some(&turn))
                    } else if matches!(command.name.as_str(), "bind" | "unbind") {
                        self.as_mut()
                            .prepare_runtime_binding_command(&command, Some(&turn))
                    } else {
                        self.as_mut()
                            .prepare_runtime_config_command(&command, Some(&turn))
                    }
                }
                DeferredConfigurationAction::Commands(PageContinuation::Interactive {
                    commands,
                    count,
                }) => {
                    if self.as_mut().execute_counted_parsed_commands_in_turn(
                        commands,
                        count,
                        &mut preparation.handoff,
                        Some(&turn),
                    ) {
                        Ok(serde_json::json!({"status":"accepted"}))
                    } else {
                        Err(self.as_ref().rust().status_text.to_string())
                    }
                }
                DeferredConfigurationAction::Commands(PageContinuation::Ipc {
                    commands, ..
                }) => self
                    .as_mut()
                    .execute_routed_ipc_chain_in_turn(
                        commands,
                        operation,
                        &mut preparation.handoff,
                        Some(&turn),
                    )
                    .map_err(|error| {
                        operation_failure_kind =
                            crate::browser_ui_operations::failure_kind(error.code());
                        error.to_string()
                    }),
                DeferredConfigurationAction::Commands(PageContinuation::Recorded {
                    commands,
                    ..
                }) => self
                    .as_mut()
                    .execute_page_recorded_commands_in_turn(
                        commands,
                        operation,
                        &mut preparation.handoff,
                        Some(&turn),
                    )
                    .map_err(|error| error.to_string()),
                DeferredConfigurationAction::SiteDoctor(id) => {
                    if self
                        .as_mut()
                        .prepare_site_doctor_proposal(&id, true, Some(&turn))
                    {
                        Ok(serde_json::json!({"status":"accepted"}))
                    } else {
                        Err(self.as_ref().rust().status_text.to_string())
                    }
                }
            }
        } else {
            Err("The settings owner or document changed before preparation. Try again.".into())
        };
        match &result {
            Ok(_) => {
                self.as_mut().continue_setting_feedback_for_request(
                    pending.setting_feedback,
                    Some(pending.generation),
                );
                if let Some(id) = pending.operation_id
                    && self.as_ref().rust().operation(&id).is_some()
                    && !self
                        .as_mut()
                        .bind_configuration_operation_for_request(&id, Some(pending.generation))
                {
                    self.as_mut().rust_mut().as_mut().get_mut().set_operation(
                        id,
                        crate::OperationUpdate::completed("settings preparation accepted"),
                    );
                }
            }
            Err(error) => {
                self.as_mut()
                    .finish_setting_feedback(pending.setting_feedback, Err(error));
                if let Some(id) = pending.operation_id {
                    self.as_mut().rust_mut().as_mut().get_mut().set_operation(
                        id,
                        crate::OperationUpdate::failed(operation_failure_kind, error.clone()),
                    );
                }
                self.as_mut().set_status_text(QString::from(error));
            }
        }
        if let Some(handoff) = preparation.handoff {
            handoff.finish(current, result.is_ok(), self.as_ref().get_ref());
        }
    }
}

#[cfg(test)]
mod projection_tests {
    use super::*;

    fn queued() -> crate::BrowserUiRust {
        let mut rust = crate::BrowserUiRust::default();
        let owner = rust
            .state
            .as_ref()
            .unwrap()
            .capture_configuration_request()
            .unwrap();
        rust.last_site_doctor_result =
            Some(serde_json::json!({"id":"fix-current", "state":"pending-confirmation"}));
        rust.configuration_io.selector_generation = 7;
        rust.configuration_io.selector_origin_generation = 7;
        rust.configuration_io.pending_selector_validation = Some(PendingSelectorValidation {
            completion: None,
            origin: None,
            operation_id: None,
            setting_feedback: None,
            generation: 7,
            expected_revision: 0,
            owner: Some(owner.clone()),
            started_at: None,
            fingerprint: String::new(),
            validation: None,
            action: PendingSelectorAction::Prepare(Box::new(DeferredConfigurationPreparation {
                owner,
                window: rust.window,
                target: None,
                action: DeferredConfigurationAction::SiteDoctor(QString::from("fix-current")),
                handoff: None,
                ready: false,
            })),
        });
        rust
    }

    #[test]
    fn queued_projection_is_read_only_and_cannot_outlive_its_request() {
        let mut rust = queued();
        assert_eq!(
            crate::site_doctor_completion::proposal_projection(&rust).unwrap()["state"],
            "preparing"
        );
        assert_eq!(
            rust.last_site_doctor_result.as_ref().unwrap()["state"],
            "pending-confirmation"
        );
        assert!(rust.configuration_io.network_policy_worker.is_none());
        assert!(!site_doctor_is_preparing(&rust, "another-fix"));
        rust.start_operation(
            "fix-operation".into(),
            crate::OperationUpdate::running("preparing"),
        )
        .unwrap();
        rust.configuration_io
            .pending_selector_validation
            .as_mut()
            .unwrap()
            .operation_id = Some("fix-operation".into());
        assert!(site_doctor_is_preparing(&rust, "fix-current"));
        rust.set_operation("fix-operation".into(), crate::OperationUpdate::cancelled());
        assert!(!site_doctor_is_preparing(&rust, "fix-current"));
    }

    #[test]
    fn queued_projection_rejects_retired_owner_revision_and_generation() {
        for change in ["owner", "revision", "generation", "removed"] {
            let mut rust = queued();
            match change {
                "owner" => {
                    rust.state = Some(crate::BrowserApplication::new(crate::Config::default()))
                }
                "revision" => {
                    let state = rust.state.as_mut().unwrap();
                    let mut layers = state.configuration_layers().clone();
                    layers
                        .temporary
                        .set_literal("content.zoom", "1.25")
                        .unwrap();
                    state.update_configuration(layers).unwrap();
                }
                "generation" => rust.configuration_io.selector_origin_generation += 1,
                _ => rust.configuration_io.pending_selector_validation = None,
            }
            assert_eq!(
                crate::site_doctor_completion::proposal_projection(&rust).unwrap()["state"],
                "pending-confirmation",
                "{change}"
            );
        }
    }
}
