//! Configuration and runtime-fact query projections.

#[allow(clippy::wildcard_imports)]
use super::*;
use ferric_browser_ipc::ErrorCode;

impl qobject::BrowserUi {
    pub(super) fn execute_config_get_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        turn: &super::browser_ui_configuration_read::ConfigurationReadTurn,
    ) -> Result<Value, PublicError> {
        turn.validate(self.as_ref().get_ref())?;
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| {
                let message = error.to_string();
                PublicError::new(ErrorCode::InvalidArgument, message.clone(), message)
            })?;
        let params = config_get_command_parameters(command).map_err(|message| {
            PublicError::new(ErrorCode::InvalidArgument, message.clone(), message)
        })?;
        let result = self
            .as_ref()
            .ipc_config_get(&params)
            .map_err(|error| match error {
                IpcQueryError::NotFound(message) => PublicError::new(
                    ErrorCode::NotFound,
                    format!(
                        "Unknown setting: {}. Use :settings to find available settings.",
                        params["key"].as_str().unwrap_or("?")
                    ),
                    message,
                ),
                IpcQueryError::Invalid(message) => {
                    PublicError::new(ErrorCode::InvalidArgument, message.clone(), message)
                }
                IpcQueryError::Busy(message) => PublicError::new(
                    ErrorCode::Busy,
                    "Settings are still loading. Try again.",
                    message,
                ),
                IpcQueryError::Unavailable(message) => PublicError::engine(message),
            })?;
        let key = result.get("key").and_then(Value::as_str).unwrap_or("?");
        let source = result
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let apply_time = result
            .get("apply_time")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let value = serde_json::to_string(result.get("value").unwrap_or(&Value::Null)).map_err(
            |error| PublicError::engine(format!("could not format configuration value: {error}")),
        )?;
        let pending = result
            .get("pending")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut status = format!("{key} = {value} (source: {source}, apply: {apply_time}");
        if pending {
            status.push_str(", pending");
        } else if result.get("pending_state").and_then(Value::as_str) == Some("unknown") {
            status.push_str(", pending state unknown");
        }
        status.push(')');
        turn.validate(self.as_ref().get_ref())?;
        self.as_mut().set_status_text(QString::from(status));
        Ok(result)
    }

    pub(super) fn ipc_config_get(&self, params: &Value) -> Result<Value, IpcQueryError> {
        let object = query_object(params, "config.get", &["key", "url", "explain"])
            .map_err(IpcQueryError::Invalid)?;
        let key = object
            .get("key")
            .and_then(Value::as_str)
            .filter(|key| !key.is_empty() && key.len() <= 256 && !key.chars().any(char::is_control))
            .ok_or_else(|| IpcQueryError::Invalid("config.get requires a nonempty key".into()))?;
        if let Some(url) = object.get("url")
            && !url.is_null()
            && url.as_str().is_none()
        {
            return Err(IpcQueryError::Invalid(
                "config.get url must be a string or null".into(),
            ));
        }
        let requested_url = object
            .get("url")
            .and_then(Value::as_str)
            .filter(|url| !url.is_empty());
        if requested_url.is_some_and(|url| {
            url.len() > 8192
                || url.chars().any(char::is_control)
                || !url.starts_with("http://") && !url.starts_with("https://")
        }) {
            return Err(IpcQueryError::Invalid(
                "Use an HTTP or HTTPS page address with --url.".into(),
            ));
        }
        if let Some(explain) = object.get("explain")
            && explain.as_bool().is_none()
        {
            return Err(IpcQueryError::Invalid(
                "config.get explain must be a boolean".into(),
            ));
        }
        if setting_metadata(key).is_none() {
            return Err(IpcQueryError::NotFound(format!(
                "configuration key not found: {key}"
            )));
        }
        if requested_url.is_some() && !setting_supports_site_scope(key) {
            return Err(IpcQueryError::Invalid(format!(
                "The setting {key} applies to the whole profile. Omit --url to inspect it."
            )));
        }
        // Retain internal origins for the legacy source summary; detailed
        // projection is controlled solely by the explicit explain flag.
        let request =
            ferric_browser_application::SettingInspectionRequest::new(key, requested_url, true)
                .map_err(|error| {
                    IpcQueryError::Invalid(if requested_url.is_some() {
                        "Use an HTTP or HTTPS page address with --url.".into()
                    } else {
                        error
                    })
                })?;
        let report = self.inspect_configuration_setting(&request)?;
        if ferric_browser_config::selector_setting_family(key).is_none()
            && report
                .active
                .as_ref()
                .is_ok_and(|value| value.value.is_none())
            && report
                .desired
                .as_ref()
                .is_ok_and(|value| value.value.is_none())
        {
            return Err(IpcQueryError::NotFound(format!(
                "configuration key not found: {key}"
            )));
        }
        crate::configuration_inspection_projection::project(
            &report,
            self.rust().ipc_sequence,
            object
                .get("explain")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .map_err(IpcQueryError::Unavailable)
    }
}
