//! Route validation for the Qt IPC boundary.
//!
//! This module owns only the projection from an already-decoded [`IpcRoute`]
//! onto live browser state. It deliberately does not inspect JSON or execute
//! commands.

use ferric_browser_ipc::ErrorCode;

use super::{CxxQtType, DispatchTarget, IpcOpenTarget, IpcRoute, PublicError, WindowId, qobject};

impl qobject::BrowserUi {
    fn ipc_state_unavailable(&self) -> PublicError {
        let (code, message) = if self.rust().profile_bootstrap_pending {
            (
                ErrorCode::Busy,
                "The browser is still starting. Try again shortly.",
            )
        } else {
            (
                ErrorCode::Engine,
                "This browser window is unavailable. Check its status or restart it.",
            )
        };
        PublicError::new(code, message, "core state unavailable")
    }

    pub(super) fn ipc_route_window(&self, route: &IpcRoute) -> Result<WindowId, PublicError> {
        let state = self
            .rust()
            .state
            .as_ref()
            .ok_or_else(|| self.ipc_state_unavailable())?;
        match route.selector {
            DispatchTarget::Active => state.active_window(),
            DispatchTarget::LastFocused => state.last_focused_window().or(state.active_window()),
            DispatchTarget::Window(window) => Some(window),
            DispatchTarget::Tab(tab) => state.tabs().get(&tab).map(|tab| tab.window),
        }
        .and_then(|window| state.windows().get(&window).map(|_| window))
        .ok_or_else(|| {
            PublicError::new(
                ErrorCode::StaleTarget,
                "The requested browser window is no longer available. Refresh the window list.",
                "requested window unavailable",
            )
        })
    }

    pub(super) fn validate_ipc_route(&self, route: &IpcRoute) -> Result<(), String> {
        self.validate_ipc_route_public(route)
            .map_err(|error| error.to_string())
    }

    pub(super) fn validate_ipc_route_public(&self, route: &IpcRoute) -> Result<(), PublicError> {
        let state = self
            .rust()
            .state
            .as_ref()
            .ok_or_else(|| self.ipc_state_unavailable())?;
        let window = self.ipc_route_window(route)?;
        if let Some(context_name) = route.context.as_deref() {
            let context = self
                .rust()
                .contexts
                .as_ref()
                .and_then(|contexts| {
                    contexts
                        .contexts()
                        .iter()
                        .find(|context| context.name == context_name)
                })
                .ok_or_else(|| {
                    PublicError::new(
                        ErrorCode::NotFound,
                        "The requested context was not found. Refresh the context list.",
                        format!("context not found: {context_name}"),
                    )
                })?;
            if route.open_target == IpcOpenTarget::PrivateWindow {
                return Err(PublicError::new(
                    ErrorCode::Denied,
                    "Private windows cannot use a saved context.",
                    "private-window cannot use a durable context",
                ));
            }
            if !matches!(route.open_target, IpcOpenTarget::Window)
                && context.profile != self.rust().profile_name
            {
                return Err(PublicError::new(
                    ErrorCode::Denied,
                    "The requested context belongs to another profile. Open its profile first.",
                    "context belongs to a different profile",
                ));
            }
            if route
                .profile
                .as_deref()
                .is_some_and(|profile| profile != context.profile)
            {
                return Err(PublicError::new(
                    ErrorCode::InvalidArgument,
                    "The profile and context selectors refer to different profiles.",
                    "profile and context selectors disagree",
                ));
            }
        }
        if let Some(profile) = route.profile.as_deref()
            && !matches!(
                route.open_target,
                IpcOpenTarget::Window | IpcOpenTarget::PrivateWindow
            )
        {
            let requested = state
                .profiles()
                .values()
                .find(|candidate| candidate.label == profile)
                .ok_or_else(|| {
                    PublicError::new(
                        ErrorCode::NotFound,
                        "The requested profile was not found. Refresh the profile list.",
                        format!("profile not found: {profile}"),
                    )
                })?;
            if requested.id != state.windows()[&window].profile {
                return Err(PublicError::new(
                    ErrorCode::Denied,
                    "The requested profile is not open in this window. Open its profile first.",
                    "the requested profile is not an active GUI profile",
                ));
            }
        }
        Ok(())
    }
}
