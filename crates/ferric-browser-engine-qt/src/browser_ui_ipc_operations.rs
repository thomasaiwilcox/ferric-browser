//! Mutable IPC operation dispatch.

#[allow(clippy::wildcard_imports)]
use super::*;

impl qobject::BrowserUi {
    pub(super) fn ipc_operation_cancel(
        mut self: Pin<&mut Self>,
        params: &Value,
    ) -> Result<Value, String> {
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
        if !this.operation_states.contains_key(id) {
            return Err("operation was not found".into());
        }
        if this
            .operation_states
            .get(id)
            .is_some_and(|status| operation_is_terminal(status))
        {
            return Err("operation is no longer cancellable".into());
        }
        if let Ok(values) = this.userscript_cancellations.lock()
            && let Some(cancellation) = values.get(id)
        {
            cancellation.store(true, Ordering::Release);
        }
        if let Ok(values) = this.spawn_cancellations.lock()
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
                "cancelled",
            );
        }
        cancel_pending_selection_operation(this, id);
        if let Some(status) = this.operation_states.get_mut(id) {
            *status = "cancelled".into();
        }
        drop(rust);
        if download_cancelled {
            self.as_mut()
                .set_status_text(QString::from("Download request cancelled"));
        }
        Ok(serde_json::json!({"operation_id": id, "status": "cancelled"}))
    }
}
