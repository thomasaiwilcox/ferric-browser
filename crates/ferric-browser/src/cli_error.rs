//! Keep typed local and remote failures intact until their final presentation.

use ferric_browser_ipc::{ErrorCode, ProtocolError, PublicError, Response, canonical_error_code};
use serde_json::{Value, json};

#[derive(Debug)]
pub(super) enum CliError {
    Local(PublicError),
    Protocol(ProtocolError),
}

impl CliError {
    pub(super) fn local(code: ErrorCode, message: &str, context: impl Into<String>) -> Self {
        Self::Local(PublicError::new(code, message, context))
    }

    pub(super) fn invalid(context: impl Into<String>) -> Self {
        Self::local(
            ErrorCode::InvalidArgument,
            "The command-line request is invalid.",
            context,
        )
    }

    pub(super) fn command_error(error: &ferric_browser_core::CommandError) -> Self {
        use ferric_browser_core::CommandError;
        let message = match error {
            CommandError::InvalidName(_) | CommandError::UnknownCommand(_) => {
                "Use a command listed by :help in the browser.".into()
            }
            CommandError::DuplicateAlias(_) | CommandError::AliasCycle(_) => {
                "The command alias is invalid. Check its definition.".into()
            }
            CommandError::Empty
            | CommandError::TooLong
            | CommandError::TooManyCommands
            | CommandError::TooManyArguments
            | CommandError::Nul
            | CommandError::ControlCharacter(_)
            | CommandError::UnterminatedQuote
            | CommandError::TrailingEscape
            | CommandError::EmptyCommand
            | CommandError::AliasDepth => error.to_string(),
        };
        Self::local(ErrorCode::InvalidArgument, &message, error.to_string())
    }

    pub(super) fn config(context: impl Into<String>) -> Self {
        Self::local(
            ErrorCode::Config,
            "Ferric Browser configuration is invalid.",
            context,
        )
    }

    pub(super) fn storage(context: impl Into<String>) -> Self {
        Self::local(
            ErrorCode::Storage,
            "Ferric Browser storage is unavailable.",
            context,
        )
    }

    pub(super) fn incompatible_data(custom_directory: bool) -> Self {
        let next_action = if custom_directory {
            "back up this folder, then choose a new, empty folder with --basedir, or use a compatible version of Ferric Browser"
        } else {
            "start with --temp-basedir to use a temporary profile, or back up the existing data and use a compatible version of Ferric Browser"
        };
        Self::local(
            ErrorCode::Storage,
            "Ferric Browser can't use this data folder because its format is unknown or incompatible.",
            "selected roots have a missing or incompatible schema marker",
        ).with_recovery("existing files were left unchanged; the browser did not start", next_action)
    }

    pub(super) fn invalid_socket_address(context: impl Into<String>) -> Self {
        Self::local(
            ErrorCode::Config,
            "Ferric Browser's local connection address is invalid.",
            context,
        ).with_recovery(
            "existing data was left in place; no connection was opened",
            "choose a shorter --basedir path, or shorten XDG_RUNTIME_DIR when using the default data folders",
        )
    }

    fn with_recovery(self, preserved: &str, next_action: &str) -> Self {
        let mut error = self.into_protocol();
        let details = error
            .details
            .as_mut()
            .and_then(Value::as_object_mut)
            .expect("CLI presentation supplies an object for details");
        details.insert("preserved".into(), json!(preserved));
        details.insert("next_action".into(), json!(next_action));
        Self::Protocol(error)
    }

    pub(super) fn no_instance(context: impl Into<String>) -> Self {
        Self::local(
            ErrorCode::NoInstance,
            "No running Ferric Browser instance was found.",
            context,
        )
    }

    pub(super) fn code(&self) -> ErrorCode {
        match self {
            Self::Local(error) => error.code(),
            Self::Protocol(error) => canonical_error_code(&error.code),
        }
    }

    pub(super) fn status(&self) -> i32 {
        exit_status(self.code())
    }

    pub(super) fn completion_unknown(self) -> Self {
        let mut error = self.into_protocol();
        let details = error
            .details
            .as_mut()
            .and_then(Value::as_object_mut)
            .expect("CLI presentation supplies an object for details");
        details.insert(
            "preserved".into(),
            json!("request completion is unconfirmed"),
        );
        details.insert(
            "next_action".into(),
            json!("check current state before retrying"),
        );
        Self::Protocol(error)
    }

    /// Already accepted requests are not rolled back when a later request fails.
    /// Counts describe admission, not completion of asynchronous operations.
    pub(super) fn after_accepted(self, kind: &str, accepted: usize) -> Self {
        if accepted == 0 {
            return self;
        }
        let mut error = self.into_protocol();
        let details = error
            .details
            .as_mut()
            .and_then(Value::as_object_mut)
            .expect("CLI presentation supplies an object for details");
        details.insert("accepted_requests".into(), json!(accepted));
        details.insert("request_kind".into(), json!(kind));
        details.insert(
            "preserved".into(),
            json!("earlier requests were accepted; their effects may still be in progress"),
        );
        details.insert(
            "next_action".into(),
            json!("check the browser state before retrying the remaining requests"),
        );
        Self::Protocol(error)
    }

    pub(super) fn into_protocol(self) -> ProtocolError {
        let mut error = match self {
            Self::Protocol(error) => error,
            Self::Local(error) => {
                // Raw local context stays in the typed cause; it must not become
                // public JSON merely because the user selected that format.
                Response::failure(
                    "cli",
                    ProtocolError {
                        code: error.code().as_str().into(),
                        message: error.user_message().into(),
                        details: None,
                    },
                )
                .error
                .expect("failure responses always contain an error")
            }
        };
        let code = canonical_error_code(&error.code);
        error.code = code.as_str().into();
        let (preserved, next_action) = code.guidance();
        let mut details = match error.details.take() {
            Some(Value::Object(details)) => details,
            Some(value) => serde_json::Map::from_iter([("source".into(), value)]),
            None => serde_json::Map::new(),
        };
        // Older peers may omit guidance. Keep valid server metadata, especially
        // its correlation ID; never create a second reference for the same error.
        for (key, fallback) in [("preserved", preserved), ("next_action", next_action)] {
            if details
                .get(key)
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                details.insert(key.into(), json!(fallback));
            }
        }
        if details
            .get("correlation_id")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            details.insert(
                "correlation_id".into(),
                json!(format!("err-{}", uuid::Uuid::new_v4())),
            );
        }
        error.details = Some(Value::Object(details));
        error
    }
}

impl From<String> for CliError {
    fn from(context: String) -> Self {
        Self::Local(PublicError::engine(context))
    }
}

impl From<&str> for CliError {
    fn from(context: &str) -> Self {
        Self::from(context.to_owned())
    }
}

pub(super) const fn exit_status(code: ErrorCode) -> i32 {
    match code {
        ErrorCode::InvalidArgument | ErrorCode::Config => 2,
        ErrorCode::NotFound | ErrorCode::NoInstance | ErrorCode::StaleTarget => 3,
        ErrorCode::Denied => 4,
        ErrorCode::Unsupported => 5,
        ErrorCode::Busy | ErrorCode::Timeout => 6,
        ErrorCode::Protocol => 7,
        ErrorCode::ConfirmationRequired
        | ErrorCode::Io
        | ErrorCode::Storage
        | ErrorCode::Engine
        | ErrorCode::Cancelled => 1,
    }
}

pub(super) fn structured(error: &ProtocolError) -> Value {
    let mut value = json!(error);
    let details = error
        .details
        .as_ref()
        .expect("CLI presentation supplies metadata");
    value["status"] = json!(exit_status(canonical_error_code(&error.code)));
    for key in ["correlation_id", "preserved", "next_action"] {
        value[key] = details[key].clone();
    }
    json!({"error": value})
}

pub(super) fn text(error: &ProtocolError) -> String {
    let details = error
        .details
        .as_ref()
        .expect("CLI presentation supplies metadata");
    // Older running peers put guidance and the same opaque reference in the
    // message. Preserve that complete message without duplicating its guidance.
    if details["correlation_id"]
        .as_str()
        .is_some_and(|reference| error.message.contains(reference))
    {
        return format!("ferric-browser: {}: {}", error.code, error.message);
    }
    let output = format!(
        "ferric-browser: {}: {}\nState: {}.\nNext: {}.",
        error.code,
        error.message,
        details["preserved"].as_str().unwrap_or_default(),
        details["next_action"].as_str().unwrap_or_default()
    );
    if matches!(
        canonical_error_code(&error.code),
        ErrorCode::Engine
            | ErrorCode::Io
            | ErrorCode::Storage
            | ErrorCode::Protocol
            | ErrorCode::Timeout
    ) {
        format!(
            "{output}\nReference: {}",
            details["correlation_id"].as_str().unwrap_or_default()
        )
    } else {
        output
    }
}
