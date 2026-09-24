//! Structured IPC error responses.
//!
//! The Qt adapter selects effects; this module owns the wire-level public
//! failure envelope so callers never classify failures by parsing prose.

use ferric_browser_ipc::{ErrorCode, ProtocolError, PublicError, Response};
use serde_json::Value;

/// A validated IPC query failure with its stable, public protocol category.
///
/// Query implementations select this category at the boundary. The dispatcher
/// then turns it into the wire error without parsing presentation text.
#[derive(Debug)]
pub(super) enum IpcQueryError {
    Invalid(String),
    NotFound(String),
    Busy(String),
    Unavailable(String),
}

impl IpcQueryError {
    pub(super) fn message(&self) -> &str {
        match self {
            Self::Invalid(message)
            | Self::NotFound(message)
            | Self::Busy(message)
            | Self::Unavailable(message) => message,
        }
    }

    pub(super) fn into_public_parts(self) -> (&'static str, String) {
        match self {
            Self::Invalid(message) => ("E_INVALID_PARAMS", message),
            Self::NotFound(message) => ("E_NOT_FOUND", message),
            Self::Busy(message) => ("E_BUSY", message),
            Self::Unavailable(message) => ("E_ENGINE", message),
        }
    }
}

pub(super) fn ipc_failure(id: &str, code: &str, message: impl Into<String>) -> Response {
    Response::failure(
        id,
        ProtocolError {
            code: code.into(),
            message: message.into(),
            details: None,
        },
    )
}

pub(super) fn ipc_action_failure(
    id: &str,
    action_id: &str,
    operation_id: &str,
    error: impl Into<PublicError>,
) -> Response {
    let error = error.into();
    let code = error.code();
    Response::failure(
        id,
        ProtocolError {
            code: code.as_str().into(),
            message: error.user_message().into(),
            details: Some(serde_json::json!({
                "action_id": action_id,
                "operation_id": operation_id,
                "category": action_failure_category(code),
                "retry": matches!(code, ErrorCode::StaleTarget | ErrorCode::Busy | ErrorCode::Timeout),
                "diagnostic_context": error.diagnostic_context(),
            })),
        },
    )
}

pub(super) fn ipc_action_failure_with_context(
    id: &str,
    action_id: &str,
    operation_id: &str,
    error: impl Into<PublicError>,
    command_context: Value,
) -> Response {
    let error = error.into();
    let code = error.code();
    Response::failure(
        id,
        ProtocolError {
            code: code.as_str().into(),
            message: error.user_message().into(),
            details: Some(serde_json::json!({
                "action_id": action_id,
                "operation_id": operation_id,
                "category": action_failure_category(code),
                "retry": matches!(code, ErrorCode::StaleTarget | ErrorCode::Busy | ErrorCode::Timeout),
                "command_context": command_context,
                "diagnostic_context": error.diagnostic_context(),
            })),
        },
    )
}

pub(super) fn ipc_command_failure_with_context(
    id: &str,
    error: impl Into<PublicError>,
    command_context: Value,
) -> Response {
    let error = error.into();
    Response::failure(
        id,
        ProtocolError {
            code: error.code().as_str().into(),
            message: error.user_message().into(),
            details: Some(serde_json::json!({
                "command_context": command_context,
                "diagnostic_context": error.diagnostic_context(),
            })),
        },
    )
}

pub(super) fn action_failure_category(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::InvalidArgument => "invalid-subject-or-parameters",
        ErrorCode::NotFound => "not-found",
        ErrorCode::StaleTarget => "stale-target",
        ErrorCode::ConfirmationRequired => "confirmation-required",
        ErrorCode::Unsupported => "missing-capability",
        ErrorCode::Denied => "denied-source-or-privacy",
        ErrorCode::Cancelled => "cancelled",
        _ => "executor-failure",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_failures_map_each_outcome_to_a_stable_public_code() {
        for (error, expected_code) in [
            (IpcQueryError::Invalid("input".into()), "E_INVALID_PARAMS"),
            (IpcQueryError::NotFound("missing".into()), "E_NOT_FOUND"),
            (IpcQueryError::Busy("loading".into()), "E_BUSY"),
            (IpcQueryError::Unavailable("offline".into()), "E_ENGINE"),
        ] {
            let (code, _) = error.into_public_parts();
            assert_eq!(code, expected_code);
        }
    }

    #[test]
    fn action_failures_keep_codes_and_categories_structured() {
        let response = ipc_action_failure(
            "request-1",
            "browser.tab.close",
            "operation-1",
            PublicError::new(ErrorCode::StaleTarget, "Safe message", "diagnostic"),
        );
        let value = serde_json::to_value(response).expect("response serializes");
        assert_eq!(value["error"]["code"], "E_STALE_TARGET");
        assert_eq!(value["error"]["details"]["category"], "stale-target");
        assert_eq!(value["error"]["details"]["retry"], true);
    }
}
