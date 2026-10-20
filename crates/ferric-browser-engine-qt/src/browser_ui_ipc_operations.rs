//! Mutable IPC operation dispatch.

#[allow(clippy::wildcard_imports)]
use super::*;
use crate::OperationUpdate;

impl qobject::BrowserUi {
    pub(super) fn ipc_operation_cancel(
        mut self: Pin<&mut Self>,
        params: &Value,
    ) -> Result<Value, PublicError> {
        let object = params
            .as_object()
            .ok_or_else(|| "operations.cancel params must be an object".to_owned())?;
        let id = object
            .get("operation_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| "operations.cancel requires operation_id".to_owned())?;
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let record = this.operation(id).ok_or_else(|| {
            PublicError::new(
                ferric_browser_ipc::ErrorCode::NotFound,
                "The operation was not found.",
                "unknown or expired operation",
            )
        })?;
        if record.state.is_terminal() || !record.cancellable {
            return Err(PublicError::new(
                if record.state.is_terminal() {
                    ferric_browser_ipc::ErrorCode::InvalidArgument
                } else {
                    ferric_browser_ipc::ErrorCode::Unsupported
                },
                "The operation can no longer be cancelled.",
                "operation cancellation boundary",
            ));
        }
        this.set_operation(id.to_owned(), OperationUpdate::cancelled());
        if let Ok(values) = this.processes.userscript_cancellations.lock()
            && let Some(cancellation) = values.get(id)
        {
            cancellation.store(true, Ordering::Release);
        }
        if let Ok(values) = this.processes.spawn_cancellations.lock()
            && let Some(cancellation) = values.get(id)
        {
            cancellation.store(true, Ordering::Release);
        }
        let download_cancelled = this
            .pending_download
            .as_ref()
            .is_some_and(|request| request.operation_id == id);
        if download_cancelled {
            super::browser_ui_download_requests::finish_pending_download_operation(
                this,
                OperationUpdate::cancelled(),
            );
        }
        let selector_cancelled = this
            .configuration_io
            .pending_selector_validation
            .as_ref()
            .is_some_and(|pending| pending.operation_id.as_deref() == Some(id));
        if selector_cancelled {
            this.configuration_io.pending_selector_validation.take();
        }
        cancel_pending_selection_operation(this, id);
        drop(rust);
        if selector_cancelled {
            self.as_mut()
                .set_selector_validation_request(QString::default());
        }
        if download_cancelled {
            self.as_mut()
                .set_status_text(QString::from("Download request cancelled"));
        }
        Ok(serde_json::json!({"operation_id": id, "status": "cancelled"}))
    }
}
