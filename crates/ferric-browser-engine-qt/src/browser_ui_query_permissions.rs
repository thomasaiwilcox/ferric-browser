//! Permission, download, and context query projections.

#[allow(clippy::wildcard_imports)]
use super::*;

impl qobject::BrowserUi {
    pub(super) fn ipc_permissions_query(&self, params: &Value) -> Result<Value, IpcQueryError> {
        let object = params.as_object().ok_or_else(|| {
            IpcQueryError::Invalid("permissions.query params must be an object".into())
        })?;
        if object.keys().any(|key| key != "origin") {
            return Err(IpcQueryError::Invalid(
                "permissions.query contains an unknown field".into(),
            ));
        }
        let origin = object.get("origin").and_then(Value::as_str);
        if object
            .get("origin")
            .is_some_and(|value| !value.is_null() && origin.is_none_or(str::is_empty))
        {
            return Err(IpcQueryError::Invalid(
                "permissions.query origin must be a nonempty string or null".into(),
            ));
        }
        if self.rust().profile_persistence.lacks_durable_storage() {
            return Ok(serde_json::json!({
                "private": true,
                "ready": true,
                "rules": [],
                "sequence": self.rust().ipc_sequence
            }));
        }
        if self.rust().storage_library_dirty.get() || self.rust().storage_library.is_none() {
            return Err(IpcQueryError::Busy(
                "permission metadata is still loading; retry".into(),
            ));
        }
        let normalized_origin = origin
            .map(ferric_browser_storage::normalize_permission_origin)
            .transpose()
            .map_err(|error| IpcQueryError::Invalid(error.to_string()))?;
        let rules = self
            .rust()
            .storage_library
            .as_ref()
            .ok_or_else(|| IpcQueryError::Unavailable("permission metadata is unavailable".into()))?
            .permissions
            .iter()
            .filter(|rule| {
                normalized_origin
                    .as_deref()
                    .is_none_or(|origin| rule.origin == origin)
            })
            .cloned()
            .collect::<Vec<_>>();
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "private": false,
            "ready": true,
            "revision": self.rust().storage_library_revision,
            "rules": rules.into_iter().map(|rule| serde_json::json!({
                "origin": rule.origin,
                "permission": rule.permission,
                "decision": rule.decision,
                "expires_at": rule.expires_at,
                "updated_at": rule.updated_at
            })).collect::<Vec<_>>()
        }))
    }

    pub(super) fn ipc_downloads_query(&self, params: &Value) -> Result<Value, IpcQueryError> {
        let object = query_object(params, "downloads.query", &["include_private"])
            .map_err(IpcQueryError::Invalid)?;
        let include_private = query_bool_param(object, "downloads.query", "include_private", false)
            .map_err(IpcQueryError::Invalid)?;
        if include_private && !self.ipc_private_queries_enabled() {
            return Err(IpcQueryError::Invalid(
                "private download queries are disabled by configuration".into(),
            ));
        }
        if self.rust().profile_persistence.lacks_durable_storage() {
            return Ok(serde_json::json!({
                "downloads": [],
                "private": true,
                "ready": true,
                "sequence": self.rust().ipc_sequence
            }));
        }
        if self.rust().storage_library_dirty.get() || self.rust().storage_library.is_none() {
            return Err(IpcQueryError::Busy(
                "download metadata is still loading; retry".into(),
            ));
        }
        let Some(library) = self.rust().storage_library.as_ref() else {
            return Err(IpcQueryError::Unavailable(
                "download metadata is unavailable".into(),
            ));
        };
        let downloads = library.downloads.clone();
        let downloads = downloads
            .into_iter()
            .map(|download| {
                serde_json::json!({
                    "id": download.id,
                    "source_url": safe_ipc_url(&download.source_url),
                    "destination": download.destination,
                    "state": download.state.as_str(),
                    "bytes_received": download.bytes_received,
                    "created_at": download.created_at,
                    "completed_at": download.completed_at
                })
            })
            .collect::<Vec<_>>();
        Ok(serde_json::json!({
            "downloads": downloads,
            "private": false,
            "ready": true,
            "revision": self.rust().storage_library_revision,
            "sequence": self.rust().ipc_sequence
        }))
    }

    pub(super) fn ipc_private_queries_enabled(&self) -> bool {
        self.rust()
            .config
            .get("ipc")
            .and_then(Value::as_object)
            .and_then(|ipc| ipc.get("private_queries"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    pub(super) fn ipc_contexts_query(&self, params: &Value) -> Result<Value, String> {
        let object = query_object(params, "contexts.query", &["include_members"])?;
        let include_members = query_bool_param(object, "contexts.query", "include_members", false)?;
        let contexts = self
            .rust()
            .contexts
            .as_ref()
            .map(|registry| {
                registry
                    .contexts()
                    .iter()
                    .map(|context| {
                        let mut value = serde_json::json!({
                            "id": context.id.to_string(),
                            "name": context.name,
                            "label": context.label,
                            "profile": context.profile,
                            "sessions": context.sessions,
                            "workspace": context.workspace,
                            "accent": context.accent,
                            "default_target": context.default_target,
                            "created_at": context.created_at,
                            "updated_at": context.updated_at
                        });
                        if include_members {
                            value["members"] = serde_json::to_value(&context.members)
                                .unwrap_or_else(|_| Value::Array(Vec::new()));
                        }
                        value
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "contexts": contexts
        }))
    }
}
