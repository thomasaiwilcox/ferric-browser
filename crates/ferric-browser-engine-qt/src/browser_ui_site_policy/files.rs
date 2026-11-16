//! Owner-bound observers on the existing configuration file worker. Neither
//! importing nor exporting acquires a configuration publication reservation.
use crate::configuration_workers::ConfigWriteResult;
use crate::{CxxQtType, Pin, QString, Value, qobject};
use ferric_browser_application::{ConfigurationRequestCapture, OperationUpdate};
use ferric_browser_core::{FailureKind, SitePolicyCommand};
use ferric_browser_ipc::{ErrorCode, PublicError};
use serde_json::json;
use std::path::PathBuf;

enum Purpose {
    Import {
        draft: String,
        temporary: bool,
        replace_existing: bool,
    },
    Export {
        draft: Option<(String, u64)>,
    },
}
pub(crate) struct PendingPolicyFile {
    owner: ConfigurationRequestCapture,
    operation: String,
    purpose: Purpose,
    private: bool,
}

fn error(code: ErrorCode, message: impl Into<String>) -> PublicError {
    let message = message.into();
    PublicError::new(code, message.clone(), message)
}

impl qobject::BrowserUi {
    fn policy_export_protected_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let configured = self.rust().config_path.to_string();
        if !configured.is_empty() {
            paths.push(PathBuf::from(configured));
        }
        if let Some(state) = self.rust().state.as_ref() {
            let sources = state.configuration_policy_sources();
            for layers in [
                sources.desired_layers(),
                sources.reload_layers(),
                sources.startup_layers(),
            ] {
                paths.extend(
                    layers
                        .base_authored
                        .documents()
                        .iter()
                        .map(|document| document.path().to_owned()),
                );
            }
            if let Some(roots) = state.profile_snapshot().and_then(|profile| profile.roots) {
                paths.extend([
                    roots.state.join("runtime-overrides.toml"),
                    roots.config.join("contexts.toml"),
                    roots.config.join("profiles.toml"),
                ]);
            }
        }
        paths
    }
    pub(super) fn execute_site_policy_file(
        mut self: Pin<&mut Self>,
        request: SitePolicyCommand,
    ) -> Result<Value, PublicError> {
        let (path, purpose, document, overwrite, temporary_export) = match request {
            SitePolicyCommand::Import {
                path,
                temporary,
                replace_existing,
            } => (
                path,
                Purpose::Import {
                    draft: uuid::Uuid::new_v4().to_string(),
                    temporary,
                    replace_existing,
                },
                None,
                false,
                false,
            ),
            SitePolicyCommand::Export {
                path,
                subject,
                overwrite,
            } => {
                let browser = self.as_ref();
                let state = browser
                    .rust()
                    .state
                    .as_ref()
                    .ok_or("profile is unavailable")?;
                let (draft, temporary) = match &subject {
                    ferric_browser_core::SitePolicyExportSubject::Draft(id) => {
                        let view = state
                            .site_policy_draft(id)
                            .map_err(super::invalid_request)?;
                        (
                            Some((id.clone(), view.revision)),
                            view.source == "temporary",
                        )
                    }
                    ferric_browser_core::SitePolicyExportSubject::Rule(target) => (
                        None,
                        target.source == ferric_browser_core::SitePolicySource::Temporary,
                    ),
                };
                let document = state
                    .export_site_policy(&subject)
                    .map_err(super::invalid_request)?;
                (
                    path,
                    Purpose::Export { draft },
                    Some(document),
                    overwrite,
                    temporary,
                )
            }
            _ => {
                return Err(super::invalid_request(
                    "expected a site policy file request".into(),
                ));
            }
        };
        if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
            return Err(super::invalid_request(
                "policy file path is empty, too long or unsafe".into(),
            ));
        }
        let owner = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(ferric_browser_application::BrowserApplication::capture_configuration_request)
            .ok_or_else(|| error(ErrorCode::Busy, "profile is unavailable"))?;
        if self
            .as_ref()
            .rust()
            .configuration_io
            .site_policy_file
            .is_some()
            || self
                .as_ref()
                .rust()
                .configuration_io
                .config_write_worker
                .as_ref()
                .is_none_or(crate::configuration_workers::ConfigWriteWorker::is_pending)
        {
            return Err(error(
                ErrorCode::Busy,
                "a local file transfer is already pending",
            ));
        }
        let private = temporary_export
            || self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .and_then(ferric_browser_application::BrowserApplication::profile_snapshot)
                .is_some_and(|profile| {
                    !profile.durable || profile.privacy == ferric_browser_core::PrivacyKind::Private
                });
        let operation = format!("op-{}", uuid::Uuid::new_v4());
        let bytes = document
            .map(|document| document.encode())
            .transpose()
            .map_err(super::invalid_request)?;
        let protected = self.as_ref().policy_export_protected_paths();
        let reply = json!({"operation_id":operation,"status":if bytes.is_some(){"pending-export"}else{"pending-import"},
            "draft":match &purpose {Purpose::Import {draft,..}=>Some(draft),Purpose::Export {draft}=>draft.as_ref().map(|(id,_)|id)},
            "revision":match &purpose {Purpose::Import {..}=>Some(1),Purpose::Export {draft}=>draft.as_ref().map(|(_,revision)|*revision)},"activation_permitted":false,
            "persistence_warning":if private && bytes.is_some(){Some("This export persists outside the current session.")}else{None}});
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.start_operation(
                operation.clone(),
                OperationUpdate::running("site policy file transfer"),
            )?;
            if bytes.is_some()
                && !this
                    .state
                    .as_mut()
                    .is_some_and(|state| state.operations_mut().begin_commit(&operation))
            {
                return Err(error(
                    ErrorCode::Busy,
                    "export operation was retired before admission",
                ));
            }
            let worker = this
                .configuration_io
                .config_write_worker
                .as_mut()
                .ok_or("file worker is unavailable")?;
            let result = match bytes {
                Some(bytes) => worker.export_policy(
                    PathBuf::from(path),
                    bytes.into_bytes(),
                    overwrite,
                    protected,
                ),
                None => worker.import_policy(PathBuf::from(path)),
            };
            if let Err(message) = result {
                this.set_operation(
                    operation,
                    OperationUpdate::failed(FailureKind::Io, &message),
                );
                return Err(error(ErrorCode::Io, message));
            }
            this.configuration_io.site_policy_file = Some(PendingPolicyFile {
                owner,
                operation,
                purpose,
                private,
            });
        }
        self.as_mut()
            .set_site_policy_result(QString::from(reply.to_string()));
        self.as_mut().set_status_text(QString::from(
            if private && reply["status"] == "pending-export" {
                "Policy export queued. The file persists outside the current session."
            } else {
                "Site policy file transfer queued"
            },
        ));
        self.as_mut().runtime_work_available();
        Ok(reply)
    }

    pub(crate) fn finish_site_policy_file(
        mut self: Pin<&mut Self>,
        result: Result<ConfigWriteResult, String>,
    ) {
        let pending = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .configuration_io
            .site_policy_file
            .take();
        let Some(pending) = pending else {
            return;
        };
        if self
            .as_ref()
            .rust()
            .operation(&pending.operation)
            .is_none_or(|record| record.state.is_terminal())
        {
            return;
        }
        let current =
            self.as_ref().rust().view_alive
                && self.as_ref().rust().state.as_ref().is_some_and(|state| {
                    state.configuration_request_owner_is_current(&pending.owner)
                });
        if !current {
            self.as_mut().rust_mut().as_mut().get_mut().set_operation(pending.operation,OperationUpdate::failed(FailureKind::StaleTarget,
                "File transfer observer retired; no imported draft was created. An admitted export may still have completed; inspect its destination."));
            return;
        }
        let result = (|| -> Result<Value, (FailureKind, String)> {
            match (pending.purpose, result) {
                (
                    Purpose::Import {
                        draft,
                        temporary,
                        replace_existing,
                    },
                    Ok(ConfigWriteResult::PolicyImported(document)),
                ) => {
                    let state = self.as_ref();
                    let state = state
                        .rust()
                        .state
                        .as_ref()
                        .ok_or((FailureKind::StaleTarget, "profile is unavailable".into()))?;
                    if !state.configuration_request_is_current(&pending.owner) {
                        return Err((FailureKind::StaleTarget,"configuration changed while reading the import; retry and review fresh sources".into()));
                    }
                    let view = state
                        .import_site_policy_draft_with_id(
                            &draft,
                            &document,
                            temporary,
                            replace_existing,
                        )
                        .map_err(|error| (FailureKind::Config, error))?;
                    serde_json::to_value(view)
                        .map_err(|error| (FailureKind::Engine, error.to_string()))
                }
                (
                    Purpose::Export { draft },
                    Ok(ConfigWriteResult::PolicyExported {
                        path,
                        bytes,
                        durability_uncertain,
                    }),
                ) => Ok(json!({
                    "status":"exported","path":path,"bytes":bytes,"durability_uncertain":durability_uncertain,
                    "draft":draft.as_ref().map(|(id,_)|id),"revision":draft.as_ref().map(|(_,revision)|*revision),
                    "persistence_warning":if pending.private{Some("This file persists outside the current session.")}else{None}})),
                (_, Err(error)) => Err((FailureKind::Io, error)),
                (_, Ok(ConfigWriteResult::PolicyFailure { kind, message })) => Err((kind, message)),
                _ => Err((
                    FailureKind::Engine,
                    "file transfer result does not match its request".into(),
                )),
            }
        })();
        let (value, update) = match result {
            Ok(value) => {
                let warning =
                    value.get("durability_uncertain").and_then(Value::as_bool) == Some(true);
                let update = OperationUpdate::completed(if warning {
                    "Policy exported; directory durability is uncertain. Inspect the file before retrying."
                } else {
                    "Site policy file transfer completed; configuration was not activated"
                });
                (value, update)
            }
            Err((kind, message)) => (
                json!({"error":message}),
                OperationUpdate::failed(kind, message),
            ),
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .set_operation(pending.operation, update);
        self.as_mut()
            .set_site_policy_result(QString::from(value.to_string()));
        self.as_mut().set_status_text(QString::from(if value.get("error").is_some(){"Site policy file transfer failed; review its result"}
            else if value.get("durability_uncertain").and_then(Value::as_bool)==Some(true) {
                "Policy exported; directory durability is uncertain. Inspect the file before retrying."
            }
            else if pending.private && value.get("status").and_then(Value::as_str)==Some("exported") {
                "Policy exported. The file persists outside the current session."
            }else{"Site policy file transfer completed; review the result"}));
    }

    pub(crate) fn detach_site_policy_file(mut self: Pin<&mut Self>) {
        let pending = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .configuration_io
            .site_policy_file
            .take();
        if let Some(pending) = pending {
            self.rust_mut().as_mut().get_mut().set_operation(pending.operation,OperationUpdate::failed(FailureKind::StaleTarget,
                match pending.purpose {Purpose::Import {..}=>"Import owner closed; no draft will be created",
                    Purpose::Export {..}=>"Export observer detached; completion is unknown. Inspect the requested file before retrying."}));
        }
    }
}
