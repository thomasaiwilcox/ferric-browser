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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct IpcRoute {
    pub(super) selector: DispatchTarget,
    pub(super) open_target: IpcOpenTarget,
    pub(super) profile: Option<String>,
    pub(super) context: Option<String>,
    pub(super) external_open: bool,
    pub(super) source: CommandSource,
}

/// Invalid local IPC command data.
///
/// JSON decoding is a presentation/protocol boundary: every failure here is
/// an invalid argument, independent of the wording used for diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct IpcCommandError(String);

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
        formatter.write_str(&self.0)
    }
}

impl Error for IpcCommandError {}

impl From<IpcCommandError> for PublicError {
    fn from(error: IpcCommandError) -> Self {
        error.into_public()
    }
}
