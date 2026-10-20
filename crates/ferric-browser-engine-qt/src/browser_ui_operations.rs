//! Native operation projection. Lifecycle and retention belong to the application.

use crate::{BrowserUiRust, publish_ipc_event};
use ferric_browser_application::{OperationRecord, OperationState, OperationUpdate};

pub(super) fn failure_kind(
    code: ferric_browser_ipc::ErrorCode,
) -> ferric_browser_core::FailureKind {
    use ferric_browser_core::FailureKind as Kind;
    use ferric_browser_ipc::ErrorCode;
    match code {
        ErrorCode::InvalidArgument | ErrorCode::Protocol => Kind::InvalidArgument,
        ErrorCode::Config => Kind::Config,
        ErrorCode::NotFound | ErrorCode::NoInstance => Kind::NotFound,
        ErrorCode::StaleTarget => Kind::StaleTarget,
        ErrorCode::Denied => Kind::Denied,
        ErrorCode::ConfirmationRequired => Kind::ConfirmationRequired,
        ErrorCode::Unsupported => Kind::Unsupported,
        ErrorCode::Busy => Kind::Busy,
        ErrorCode::Timeout => Kind::Timeout,
        ErrorCode::Io => Kind::Io,
        ErrorCode::Storage => Kind::Storage,
        ErrorCode::Engine => Kind::Engine,
        ErrorCode::Cancelled => Kind::Cancelled,
    }
}

impl BrowserUiRust {
    pub(super) fn operation(&self, id: &str) -> Option<&OperationRecord> {
        self.state.as_ref()?.operations().get(id)
    }

    pub(super) fn operation_records(&self) -> impl Iterator<Item = &OperationRecord> {
        self.state
            .iter()
            .flat_map(|application| application.operations().records())
    }

    pub(super) fn operation_revision(&self) -> u64 {
        self.state
            .as_ref()
            .map_or(0, |application| application.operations().revision())
    }

    pub(super) fn has_active_operations(&self) -> bool {
        self.state
            .as_ref()
            .is_some_and(|application| application.operations().has_active())
    }

    pub(super) fn admit_operation(
        &mut self,
        id: String,
    ) -> Result<(), ferric_browser_ipc::PublicError> {
        self.state
            .as_mut()
            .ok_or_else(|| {
                ferric_browser_ipc::PublicError::new(
                    ferric_browser_ipc::ErrorCode::NotFound,
                    "The browser owner is unavailable.",
                    "operation owner retired",
                )
            })?
            .operations_mut()
            .admit(id)
            .map_err(|kind| {
                ferric_browser_ipc::PublicError::new(
                    kind.into(),
                    "The operation could not be admitted. Wait for active work to finish.",
                    "operation admission rejected",
                )
            })
    }

    pub(super) fn start_operation(
        &mut self,
        id: String,
        update: OperationUpdate,
    ) -> Result<(), String> {
        let application = self
            .state
            .as_mut()
            .ok_or_else(|| "operation owner retired".to_owned())?;
        let catalog = application.operations_mut();
        if catalog.get(&id).is_none() {
            catalog
                .admit(id.clone())
                .map_err(|kind| format!("operation admission rejected: {kind:?}"))?;
        }
        if self.set_operation(id, update) {
            Ok(())
        } else {
            Err("operation is already terminal".into())
        }
    }

    pub(super) fn set_operation(&mut self, id: String, update: OperationUpdate) -> bool {
        let Some(application) = self.state.as_mut() else {
            return false;
        };
        let catalog = application.operations_mut();
        if !catalog.update(&id, update) {
            return false;
        }
        self.publish_operation_completions();
        true
    }

    /// Native handoffs finish through a matching native receipt. Once queued,
    /// they cannot be cancelled without also revoking the native request.
    pub(super) fn start_native_handoff(
        &mut self,
        requested_id: Option<&str>,
    ) -> Result<String, String> {
        let id = requested_id.map_or_else(|| format!("op-{}", uuid::Uuid::new_v4()), str::to_owned);
        self.start_operation(
            id.to_owned(),
            OperationUpdate::running("native handoff pending"),
        )?;
        if self
            .state
            .as_mut()
            .is_some_and(|application| application.operations_mut().begin_commit(&id))
        {
            Ok(id)
        } else {
            Err("native handoff owner is unavailable".into())
        }
    }

    pub(super) fn publish_operation_completions(&mut self) {
        let Some(application) = self.state.as_mut() else {
            return;
        };
        for completion in application.operations_mut().drain_completions() {
            publish_ipc_event(
                &mut self.ipc_sequence,
                "operation.completed",
                serde_json::json!({
                    "window_id": self.window.map(|window| window.to_string()),
                    "operation_id": completion.id, "status": completion.state.as_str(),
                    "operation_sequence": completion.sequence,
                    "error_code": completion.error.map(|kind| ferric_browser_ipc::ErrorCode::from(kind).as_str()),
                }),
            );
        }
    }

    pub(super) fn complete_synchronous_operation(&mut self, id: &str) {
        if self
            .operation(id)
            .is_some_and(|record| record.state == OperationState::Accepted)
        {
            self.set_operation(id.to_owned(), OperationUpdate::completed("request applied"));
        }
    }
}

pub(super) fn operation_value(record: &OperationRecord, include_stderr: bool) -> serde_json::Value {
    let mut value = serde_json::json!({
        "operation_id": record.id, "sequence": record.sequence,
        "status": record.state.as_str(), "detail": record.detail,
        "cancellable": record.cancellable,
        "error": record.failure.as_ref().map(|failure| serde_json::json!({
            "code": ferric_browser_ipc::ErrorCode::from(failure.kind).as_str(),
            "message": failure.message,
        })),
    });
    if include_stderr {
        value["stderr"] = record
            .stderr
            .clone()
            .map_or(serde_json::Value::Null, Into::into);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_handoffs_wait_for_receipts_with_and_without_an_envelope_id() {
        let (application, _, _) = ferric_browser_application::BrowserApplication::bootstrap(
            ferric_browser_config::Config::default(),
            ferric_browser_core::PrivacyKind::Normal,
            "handoff",
        )
        .unwrap();
        let mut rust = BrowserUiRust {
            state: Some(application),
            ..BrowserUiRust::default()
        };
        for requested in [Some("op-envelope"), None] {
            let id = rust.start_native_handoff(requested).unwrap();
            assert!(id.starts_with("op-"));
            rust.complete_synchronous_operation(&id);
            assert_eq!(rust.operation(&id).unwrap().state, OperationState::Running);
            assert!(!rust.operation(&id).unwrap().cancellable);
            assert_eq!(
                rust.state.as_mut().unwrap().operations_mut().cancel(&id),
                Err(ferric_browser_core::FailureKind::Unsupported)
            );
            assert!(rust.set_operation(
                id.clone(),
                OperationUpdate::failed(
                    ferric_browser_core::FailureKind::Engine,
                    "native rejection"
                )
            ));
            assert!(!rust.set_operation(id.clone(), OperationUpdate::completed("late receipt")));
            assert_eq!(rust.operation(&id).unwrap().state, OperationState::Failed);
        }
    }
}
