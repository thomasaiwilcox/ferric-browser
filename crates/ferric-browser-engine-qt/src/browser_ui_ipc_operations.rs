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
        if this
            .pending_userscript
            .as_ref()
            .is_some_and(|pending| pending.operation_id == id)
        {
            this.pending_userscript = None;
            this.pending_selection = None;
        }
        if this
            .pending_selection
            .as_ref()
            .and_then(|pending| pending.operation_id.as_deref())
            == Some(id)
        {
            finish_pending_selection_operation(this, "cancelled");
        }
        if let Some(status) = this.operation_states.get_mut(id) {
            *status = "cancelled".into();
        }
        Ok(serde_json::json!({"operation_id": id, "status": "cancelled"}))
    }
}
