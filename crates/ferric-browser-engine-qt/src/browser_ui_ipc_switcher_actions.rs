//! Mutable switcher activation dispatch.

#[allow(clippy::wildcard_imports)]
use super::*;

impl qobject::BrowserUi {
    pub(super) fn ipc_switcher_activate(
        mut self: Pin<&mut Self>,
        params: &Value,
    ) -> Result<Value, String> {
        let object = params
            .as_object()
            .ok_or_else(|| "switcher.activate params must be an object".to_owned())?;
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "kind" | "id" | "action" | "generation"))
        {
            return Err("switcher.activate contains an unknown field".into());
        }
        let kind = object
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| "switcher.activate requires a string kind".to_owned())?;
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| "switcher.activate requires a string id".to_owned())?;
        if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
            return Err("switcher.activate id is invalid".into());
        }
        let action = object
            .get("action")
            .map(|value| {
                value
                    .as_str()
                    .filter(|action| !action.is_empty() && action.len() <= 64)
                    .ok_or_else(|| "switcher.activate action must be a bounded string".to_owned())
            })
            .transpose()?
            .unwrap_or_else(|| switcher_default_action(kind).unwrap_or_default());
        let generation = object
            .get("generation")
            .map(|value| {
                value.as_u64().ok_or_else(|| {
                    "switcher.activate generation must be an unsigned integer".to_owned()
                })
            })
            .transpose()?;
        if generation.is_some() && kind != "tab" {
            return Err("switcher.activate generation is only valid for tab results".into());
        }
        if !switcher_action_allowed(kind, action) {
            return Err(format!(
                "switcher.activate action {action:?} is not allowed for result kind {kind:?}"
            ));
        }
        let binding = self.as_ref();
        let Some(state) = binding.rust().state.as_ref() else {
            return Err("core state unavailable".into());
        };
        let private = match kind {
            "tab" => state
                .tabs()
                .values()
                .find(|tab| tab.id.to_string() == id)
                .and_then(|tab| state.profiles().get(&tab.profile))
                .map(|profile| profile.privacy.is_transient())
                .ok_or_else(|| "switcher target is stale or unsupported".to_owned())?,
            "window" => state
                .windows()
                .values()
                .find(|window| window.id.to_string() == id)
                .and_then(|window| state.profiles().get(&window.profile))
                .map(|profile| profile.privacy.is_transient())
                .ok_or_else(|| "switcher target is stale or unsupported".to_owned())?,
            "closed" => binding
                .rust()
                .closed_tabs
                .iter()
                .find(|closed| closed.id.to_string() == id)
                .map(|closed| closed.private)
                .ok_or_else(|| "switcher target is stale or unsupported".to_owned())?,
            "context" | "history" | "bookmark" | "quickmark" | "session" | "download"
            | "command" | "action" => false,
            _ => return Err(format!("unknown switcher result kind: {kind}")),
        };
        if private {
            return Err("private or ephemeral switcher activation is disabled".into());
        }
        if !self
            .as_mut()
            .activate_switcher_action_internal(kind, id, action, generation)
        {
            return Err("switcher target is stale or unsupported".into());
        }
        Ok(serde_json::json!({
            "status": "accepted",
            "kind": kind,
            "id": id,
            "action": action,
            "sequence": self.rust().ipc_sequence
        }))
    }
}
