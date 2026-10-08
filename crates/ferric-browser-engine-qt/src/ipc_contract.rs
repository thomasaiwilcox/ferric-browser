//! Types shared by IPC decoding, routing, and structured response handling.

use std::{error::Error, fmt};

use ferric_browser_core::{CommandSource, DispatchTarget};
use ferric_browser_ipc::{ErrorCode, PublicError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum IpcOpenTarget {
    Tab,
    BackgroundTab,
    Window,
    PrivateWindow,
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct IpcRoute {
    pub(super) selector: DispatchTarget,
    pub(super) open_target: IpcOpenTarget,
    pub(super) profile: Option<String>,
    pub(super) context: Option<String>,
    pub(super) external_open: bool,
    pub(super) source: CommandSource,
}

impl fmt::Debug for IpcRoute {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let selector = match self.selector {
            DispatchTarget::Active => "active",
            DispatchTarget::LastFocused => "last-focused",
            DispatchTarget::Window(_) => "window",
            DispatchTarget::Tab(_) => "tab",
        };
        formatter
            .debug_struct("IpcRoute")
            .field("selector", &selector)
            .field("open_target", &self.open_target)
            .field("has_profile", &self.profile.is_some())
            .field("has_context", &self.context.is_some())
            .field("external_open", &self.external_open)
            .field("source", &self.source)
            .finish()
    }
}

/// Invalid local IPC command data.
///
/// JSON decoding is a presentation/protocol boundary: every failure here is
/// an invalid argument, independent of the wording used for diagnostics.
#[derive(Clone, Eq, PartialEq)]
pub(super) struct IpcCommandError(String);

impl fmt::Debug for IpcCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("IpcCommandError([redacted])")
    }
}

impl IpcCommandError {
    pub(super) fn into_public(self) -> PublicError {
        PublicError::new(
            ErrorCode::InvalidArgument,
            "The command request is invalid.",
            self.0,
        )
    }
}

impl From<String> for IpcCommandError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

impl From<&str> for IpcCommandError {
    fn from(message: &str) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for IpcCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("IPC command data is invalid")
    }
}

impl Error for IpcCommandError {}

impl From<IpcCommandError> for PublicError {
    fn from(error: IpcCommandError) -> Self {
        error.into_public()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_command_error_diagnostics_redact_decoder_input() {
        let sentinel = "private-ipc-payload-sentinel";
        let error = IpcCommandError(format!("invalid value: {sentinel}"));

        assert!(!format!("{error:?}").contains(sentinel));
        assert!(!error.to_string().contains(sentinel));
        let public = error.into_public();
        assert_eq!(public.user_message(), "The command request is invalid.");
        assert!(public.diagnostic_context().contains(sentinel));
    }

    #[test]
    fn ipc_route_debug_redacts_profile_context_and_runtime_selector() {
        let sentinel = "private-ipc-route-sentinel";
        let route = IpcRoute {
            selector: DispatchTarget::Window(
                ferric_browser_core::WindowId::from_display("windowid-51711")
                    .expect("valid test window ID"),
            ),
            open_target: IpcOpenTarget::Tab,
            profile: Some(sentinel.into()),
            context: Some(sentinel.into()),
            external_open: false,
            source: CommandSource::Ipc,
        };

        let debug = format!("{route:?}");
        assert!(!debug.contains(sentinel));
        assert!(!debug.contains("51711"));
        assert!(!debug.contains("window-"));
        assert!(debug.contains("has_profile: true"));
        assert!(debug.contains("has_context: true"));
    }
}
