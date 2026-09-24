//! Profile, session, window, and tab query projections.

#[allow(clippy::wildcard_imports)]
use super::*;

impl qobject::BrowserUi {
    pub(super) fn ipc_tabs_query(&self, params: &Value) -> Result<Value, String> {
        let object = query_object(
            params,
            "tabs.query",
            &["include_private", "active_only", "window"],
        )?;
        let include_private = query_bool_param(object, "tabs.query", "include_private", false)?;
        let active_only = query_bool_param(object, "tabs.query", "active_only", false)?;
        if include_private && !self.ipc_private_queries_enabled() {
            return Err("private tab queries are disabled by configuration".into());
        }
        let Some(state) = self.rust().state.as_ref() else {
            return Err("core state unavailable".into());
        };
        let contexts = self.rust().contexts.as_ref();
        let requested_window = match query_optional_string(object, "tabs.query", "window")? {
            None => None,
            Some(value) => match value {
                "active" => state.active_window(),
                "last-focused" => state.last_focused_window().or(state.active_window()),
                _ => return Err("window query accepts active or last-focused".into()),
            },
        };
        let tabs = state
            .tabs()
            .values()
            .filter(|tab| {
                include_private
                    || state
                        .profiles()
                        .get(&tab.profile)
                        .is_some_and(|profile| !profile.privacy.is_transient())
            })
            .filter(|tab| {
                requested_window.is_none_or(|window| tab.window == window)
            })
            .filter(|tab| {
                !active_only
                    || state
                        .windows()
                        .get(&tab.window)
                        .and_then(|window| window.active_tab)
                        == Some(tab.id)
            })
            .map(|tab| {
                let profile = state.profiles().get(&tab.profile);
                let context = state
                    .windows()
                    .get(&tab.window)
                    .and_then(|window| window.context.as_deref())
                    .and_then(|name| contexts?.contexts().iter().find(|context| context.name == name));
                serde_json::json!({
                    "id": tab.id.to_string(),
                    "window_id": tab.window.to_string(),
                    "profile_id": tab.profile.to_string(),
                    "profile_name": profile.map(|profile| profile.label.clone()),
                    "context_id": context.map(|context| context.id.to_string()),
                    "context_name": context.map(|context| context.name.clone()),
                    "workspace": context.and_then(|context| context.workspace.clone()),
                    "output": Value::Null,
                    "url": tab.url.as_deref().map_or_else(|| "about:blank".into(), safe_ipc_url),
                    "title": tab.title.replace(['\n', '\r'], " "),
                    "selected": state.windows().get(&tab.window).and_then(|window| window.active_tab) == Some(tab.id),
                    "pinned": tab.pinned,
                    "muted": tab.muted,
                    "loading": format!("{:?}", tab.loading).to_ascii_lowercase(),
                    "renderer": format!("{:?}", tab.renderer).to_ascii_lowercase(),
                    "document_revision": state.revision()
                })
            })
            .collect::<Vec<_>>();
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "tabs": tabs
        }))
    }

    pub(super) fn ipc_windows_query(&self, params: &Value) -> Result<Value, String> {
        let object = query_object(params, "windows.query", &["window"])?;
        let Some(state) = self.rust().state.as_ref() else {
            return Err("core state unavailable".into());
        };
        let contexts = self.rust().contexts.as_ref();
        let requested_window = match query_optional_string(object, "windows.query", "window")? {
            None => None,
            Some(value) => match value {
                "active" => state.active_window(),
                "last-focused" => state.last_focused_window().or(state.active_window()),
                _ => return Err("window query accepts active or last-focused".into()),
            },
        };
        let windows = state
            .windows()
            .values()
            .filter(|window| requested_window.is_none_or(|requested| requested == window.id))
            .map(|window| {
                let context = window.context.as_deref().and_then(|name| {
                    contexts?
                        .contexts()
                        .iter()
                        .find(|context| context.name == name)
                });
                serde_json::json!({
                    "id": window.id.to_string(),
                    "profile_id": window.profile.to_string(),
                    "context_id": context.map(|context| context.id.to_string()),
                    "context_name": context.map(|context| context.name.clone()),
                    "workspace": context.and_then(|context| context.workspace.clone()),
                    "tabs": window.tabs.iter().map(ToString::to_string).collect::<Vec<_>>(),
                    "active_tab": window.active_tab.map(|tab| tab.to_string()),
                    "mode": window.modes.last().map(|mode| format!("{mode:?}").to_ascii_lowercase())
                })
            })
            .collect::<Vec<_>>();
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "windows": windows,
            "live_registry": self.rust().live_window_registry.iter().map(|entry| serde_json::json!({
                "id": entry.id,
                "owner_token": entry.owner_token,
                "profile": entry.profile,
                "private": entry.private,
                "ephemeral": entry.ephemeral,
                "tab_count": entry.tab_count,
            })).collect::<Vec<_>>(),
        }))
    }

    pub(super) fn ipc_window_focus(
        mut self: Pin<&mut Self>,
        params: &Value,
    ) -> Result<Value, String> {
        query_empty_object_or_null(params, "window.focus")?;
        let window_id = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(|state| state.last_focused_window().or(state.active_window()))
            .ok_or_else(|| "no live browser window is available to focus".to_owned())?;
        let command = ParsedCommand {
            name: "window-focus".into(),
            arguments: vec![window_id.to_string()],
        };
        let mut result = self.as_mut().execute_window_action(&command)?;
        let operation_id = format!("op-{}", Uuid::new_v4());
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .operation_states
            .insert(operation_id.clone(), "running".into());
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("window-focus\t{window_id}\t{operation_id}"));
        result["operation_id"] = Value::String(operation_id);
        result["status"] = Value::String("accepted".into());
        Ok(serde_json::json!({
            "status": result.get("status").cloned().unwrap_or_else(|| Value::String("accepted".into())),
            "action": "focus",
            "window_id": window_id.to_string(),
            "target": "last-focused",
            "pending": true,
            "operation_id": result["operation_id"].clone()
        }))
    }

    pub(super) fn ipc_profiles_query(&self, params: &Value) -> Result<Value, String> {
        query_empty_object_or_null(params, "profiles.query")?;
        let Some(state) = self.rust().state.as_ref() else {
            return Err("core state unavailable".into());
        };
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "profiles": state.profiles().values().map(|profile| serde_json::json!({
                "id": profile.id.to_string(),
                "name": profile.label,
                "private": profile.privacy == PrivacyKind::Private,
                "ephemeral": profile.privacy.is_ephemeral(),
                "privacy": profile.privacy.name()
            })).collect::<Vec<_>>()
        }))
    }
}
