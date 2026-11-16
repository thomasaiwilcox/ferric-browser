//! Site Policies uses the existing configuration validation and commit lanes.
use crate::{CxxQtType, Pin, QString, Value, qobject};
use ferric_browser_application::SitePolicyDraftApply;

mod builder;
mod catalogue;
mod editor;
mod explanation;
mod files;
mod preview;
pub(super) use files::PendingPolicyFile;
pub(super) use preview::PendingSitePolicyPreview;

pub(super) fn uses_read_turn(command: &crate::ParsedCommand) -> bool {
    matches!(
        command.name.as_str(),
        "site-policy"
            | "site-policy-preview"
            | "site-policy-list"
            | "site-policy-explain"
            | "site-policy-import"
            | "site-policy-export"
    )
}

pub(super) fn is_draft_management(command: &crate::ParsedCommand) -> bool {
    matches!(
        command.name.as_str(),
        "site-policy-edit" | "site-policy-discard"
    )
}

fn invalid_request(message: String) -> crate::PublicError {
    crate::PublicError::new(
        ferric_browser_ipc::ErrorCode::InvalidArgument,
        message.clone(),
        message,
    )
}

pub(super) struct PendingSitePolicyActivation {
    pub(super) observer: SitePolicyDraftApply,
    pub(super) operation_id: Option<String>,
}

pub(super) fn activation_pending(rust: &crate::BrowserUiRust, operation: &str) -> bool {
    rust.configuration_io
        .site_policy_activation
        .as_ref()
        .is_some_and(|pending| pending.operation_id.as_deref() == Some(operation))
}

impl qobject::BrowserUi {
    pub(super) fn execute_site_policy_command(
        mut self: Pin<&mut Self>,
        command: &crate::ParsedCommand,
        preparation: Option<
            &super::browser_ui_configuration_preparation::ConfigurationPreparationTurn,
        >,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let preparation =
            preparation.ok_or("Site Policies requests require deferred preparation")?;
        if !preparation.is_current(self.as_ref().get_ref()) {
            return Err("Site Policies request owner changed".into());
        }
        match ferric_browser_core::parse_site_policy_command(command)? {
            ferric_browser_core::SitePolicyCommand::Apply {
                draft,
                revision,
                activate,
            } => self
                .as_mut()
                .apply_site_policy_draft(&draft, revision, activate, preparation),
            ferric_browser_core::SitePolicyCommand::Edit(edit) => {
                self.edit_site_policy_request(&edit)
            }
            ferric_browser_core::SitePolicyCommand::Discard { draft } => {
                self.discard_site_policy_request(&draft)
            }
            _ => Err("unsupported Site Policies management request".into()),
        }
    }

    pub(super) fn execute_site_policy_management_read(
        mut self: Pin<&mut Self>,
        command: &crate::ParsedCommand,
        read: &super::browser_ui_configuration_read::ConfigurationReadTurn,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        read.validate(self.as_ref().get_ref())
            .map_err(|error| error.user_message().to_owned())?;
        let value = match ferric_browser_core::parse_site_policy_command(command)? {
            ferric_browser_core::SitePolicyCommand::Edit(edit) => {
                self.as_mut().edit_site_policy_request(&edit)?
            }
            ferric_browser_core::SitePolicyCommand::Discard { draft } => {
                self.as_mut().discard_site_policy_request(&draft)?
            }
            _ => return Err("expected a Site Policies draft management request".into()),
        };
        read.validate(self.as_ref().get_ref())
            .map_err(|error| error.user_message().to_owned())?;
        Ok(value)
    }

    fn discard_site_policy_request(mut self: Pin<&mut Self>, draft: &str) -> Result<Value, String> {
        self.as_mut().cancel_preview_for_draft(draft);
        self.as_ref()
            .rust()
            .state
            .as_ref()
            .ok_or("profile is unavailable")?
            .discard_site_policy_draft(draft)?;
        let value = serde_json::json!({"draft":draft,"discarded":true});
        self.as_mut()
            .set_site_policy_result(QString::from(value.to_string()));
        Ok(value)
    }

    pub(super) fn execute_configuration_read_command(
        mut self: Pin<&mut Self>,
        command: &crate::ParsedCommand,
        read: &super::browser_ui_configuration_read::ConfigurationReadTurn,
    ) -> Result<Value, crate::PublicError> {
        if command.name == "get" {
            return self.execute_config_get_command(command, read);
        }
        read.validate(self.as_ref().get_ref())?;
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| invalid_request(error.to_string()))?;
        let request =
            ferric_browser_core::parse_site_policy_command(command).map_err(invalid_request)?;
        let value = match request {
            ferric_browser_core::SitePolicyCommand::Open { url } => self
                .as_mut()
                .open_site_policy_workspace(url.as_deref())
                .map_err(crate::PublicError::engine),
            ferric_browser_core::SitePolicyCommand::Preview { draft, tab } => self
                .as_mut()
                .start_site_policy_preview(&draft, tab.as_deref())
                .map_err(crate::PublicError::engine),
            ferric_browser_core::SitePolicyCommand::List { url, all } => self
                .as_ref()
                .list_site_policies(url.as_deref(), all)
                .map_err(crate::PublicError::engine),
            ferric_browser_core::SitePolicyCommand::Explain { url, rule, subject } => self
                .as_ref()
                .explain_site_policy(url.as_deref(), rule.as_ref(), &subject),
            request @ (ferric_browser_core::SitePolicyCommand::Import { .. }
            | ferric_browser_core::SitePolicyCommand::Export { .. }) => {
                self.as_mut().execute_site_policy_file(request)
            }
            _ => Err(invalid_request("expected a Site Policies query".into())),
        }?;
        read.validate(self.as_ref().get_ref())?;
        self.as_mut()
            .set_site_policy_result(QString::from(value.to_string()));
        if matches!(
            command.name.as_str(),
            "site-policy-list" | "site-policy-explain"
        ) {
            self.as_mut()
                .set_status_text(QString::from("Site Policies inspection updated"));
        }
        Ok(value)
    }

    fn edit_site_policy_request(
        mut self: Pin<&mut Self>,
        edit: &ferric_browser_core::SitePolicyEdit,
    ) -> Result<Value, String> {
        let view = {
            let browser = self.as_ref();
            let state = browser
                .rust()
                .state
                .as_ref()
                .ok_or("profile is unavailable")?;
            match edit {
                ferric_browser_core::SitePolicyEdit::Rebase { draft, revision } => {
                    state.rebase_site_policy_draft(draft, *revision)?
                }
                ferric_browser_core::SitePolicyEdit::Rule { .. } => {
                    state.open_site_policy_draft(edit)?
                }
            }
        };
        let value = serde_json::to_value(view).map_err(|error| error.to_string())?;
        self.as_mut()
            .set_site_policy_result(QString::from(value.to_string()));
        Ok(value)
    }

    pub(super) fn site_policy_draft_snapshot(&self, id: &QString) -> QString {
        let result = self
            .rust()
            .state
            .as_ref()
            .ok_or_else(|| "profile is unavailable".to_owned())
            .and_then(|state| state.site_policy_draft(&id.to_string()))
            .and_then(|view| serde_json::to_value(view).map_err(|error| error.to_string()));
        QString::from(
            match result {
                Ok(value) => value,
                Err(error) => serde_json::json!({"error":error}),
            }
            .to_string(),
        )
    }

    pub(super) fn apply_site_policy_draft(
        mut self: Pin<&mut Self>,
        id: &str,
        revision: u64,
        activate: bool,
        preparation: &super::browser_ui_configuration_preparation::ConfigurationPreparationTurn,
    ) -> Result<Value, String> {
        if self
            .as_ref()
            .rust()
            .configuration_io
            .site_policy_activation
            .is_some()
        {
            return Err("Site policy activation is pending".into());
        }
        if !preparation.is_current(self.as_ref().get_ref()) {
            return Err("configuration preparation changed".into());
        }
        let (prepared, observer) = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .ok_or("profile is unavailable")?
            .begin_site_policy_draft_apply(id, revision, activate)?;
        let persist = observer.persist();
        let result = self
            .as_mut()
            .validate_or_queue_runtime_config_with_post_action(
                prepared.into_prepared(),
                "Site policy published; configuration activation required".into(),
                persist,
                Some(&preparation.origin),
                Some(
                    super::configuration_completion::ConfigurationCompletion::SitePolicy {
                        observer: Box::new(observer),
                        operation_id: preparation.origin.operation_id.clone(),
                    },
                ),
            )?;
        let status = result
            .as_ref()
            .and_then(|value| value.get("status"))
            .and_then(Value::as_str)
            .unwrap_or(if result.is_none() {
                "pending-validation"
            } else if activate {
                "activation-pending"
            } else {
                "published"
            });
        Ok(
            serde_json::json!({"draft":id,"revision":revision.to_string(),"status":status,"activate":activate}),
        )
    }

    pub(super) fn poll_site_policy_activation(mut self: Pin<&mut Self>) {
        let busy = {
            let browser = self.as_ref();
            let io = &browser.rust().configuration_io;
            io.pending_selector_validation.is_some()
                || io.pending_configuration_publication.is_some()
                || io.pending_context_reload.is_some()
        };
        if busy {
            return;
        }
        let activation = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .configuration_io
            .site_policy_activation
            .take();
        let Some(activation) = activation else {
            return;
        };
        if !self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .is_some_and(|state| activation.observer.owner_is_current(state))
        {
            return;
        }
        let prepared = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .ok_or_else(|| "profile is unavailable".to_owned())
            .and_then(|state| activation.observer.prepare_activation(state));
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(_) => {
                self.finish_site_policy_activation(activation, None);
                return;
            }
        };
        let sources = prepared.policy_sources();
        let layers = sources.desired_layers();
        let items = super::selector_validation::selector_validation_items(layers);
        let fingerprint = match serde_json::to_string(&items) {
            Ok(fingerprint) => fingerprint,
            Err(_) => {
                self.finish_site_policy_activation(activation, None);
                return;
            }
        };
        // The save phase already qualified this exact selector inventory. Reuse
        // its receipt; a lost context is an activation failure, never permission
        // to publish without qualification or silently retry another generation.
        let receipt = self.as_ref().rust().state.as_ref().and_then(|state| {
            self.as_ref()
                .rust()
                .configuration_io
                .selector_receipts
                .matching(state, layers)
        });
        if !items.is_empty() && receipt.is_none() {
            self.finish_site_policy_activation(activation, None);
            return;
        }
        let result = self.as_mut().commit_prepared_configuration_reload(
            prepared,
            super::selector_validation_receipts::SelectorValidationPublication {
                fingerprint,
                receipt,
            },
            "Site policy activated",
        );
        self.finish_site_policy_activation(activation, result.ok());
    }

    pub(super) fn finish_site_policy_activation(
        mut self: Pin<&mut Self>,
        activation: PendingSitePolicyActivation,
        result: Option<Value>,
    ) {
        if !self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .is_some_and(|state| activation.observer.owner_is_current(state))
        {
            return;
        }
        let active = result.is_some();
        activation
            .observer
            .finish_activation(self.as_ref().rust().state.as_ref(), active);
        if let Some(operation) = activation.operation_id {
            self.as_mut().rust_mut().as_mut().get_mut().set_operation(operation,
                crate::OperationUpdate::completed(if active { "site policy configuration activated" }
                    else { "policy published; configuration activation failed; review and reload configuration" }));
        }
        if !active {
            self.set_status_text(QString::from("Policy published; configuration activation failed. Review and reload configuration."));
        }
    }
}
