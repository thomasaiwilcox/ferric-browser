//! Route validation for the Qt IPC boundary.
//!
//! This module owns only the projection from an already-decoded [`IpcRoute`]
//! onto live browser state. It deliberately does not inspect JSON or execute
//! commands.

use ferric_browser_ipc::ErrorCode;

use super::{
    BrowserUiRust, CxxQtType, DispatchTarget, IpcOpenTarget, IpcRoute, PublicError, WindowId,
    qobject,
};

fn ipc_state_unavailable(rust: &BrowserUiRust) -> PublicError {
    let (code, message) = if rust.profile_bootstrap_pending {
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

fn route_window(rust: &BrowserUiRust, route: &IpcRoute) -> Result<WindowId, PublicError> {
    let state = rust
        .state
        .as_ref()
        .ok_or_else(|| ipc_state_unavailable(rust))?;
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

fn validate_route(rust: &BrowserUiRust, route: &IpcRoute) -> Result<(), PublicError> {
    let window = route_window(rust, route)?;
    if let Some(context_name) = route.context.as_deref() {
        let context = rust
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
            && context.profile != rust.profile_name
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
        && (profile != rust.profile_name || rust.window != Some(window))
    {
        return Err(PublicError::new(
            ErrorCode::Denied,
            "The requested profile is not open in this window. Open its profile first.",
            "the requested profile is not an active GUI profile",
        ));
    }
    Ok(())
}

impl qobject::BrowserUi {
    pub(super) fn ipc_route_window(&self, route: &IpcRoute) -> Result<WindowId, PublicError> {
        route_window(self.rust(), route)
    }

    pub(super) fn validate_action_ipc_route(&self, route: &IpcRoute) -> Result<(), PublicError> {
        if route.context.is_some() {
            return Err(PublicError::new(
                ErrorCode::InvalidArgument,
                "An action can't be combined with a context selector.",
                "context routing is not valid for actions",
            ));
        }
        self.validate_ipc_route_public(route)
    }

    pub(super) fn validate_ipc_route_public(&self, route: &IpcRoute) -> Result<(), PublicError> {
        validate_route(self.rust(), route)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_core::{CommandSource, PrivacyKind};

    fn owner() -> (BrowserUiRust, IpcRoute) {
        let mut rust = BrowserUiRust::default();
        let (state, window, _) = super::super::BrowserApplication::bootstrap(
            super::super::Config::default(),
            PrivacyKind::Normal,
            "Work browser",
        )
        .expect("owner");
        rust.state = Some(state);
        rust.window = Some(window);
        rust.profile_name = "work".into();
        let route = IpcRoute {
            selector: DispatchTarget::Window(window),
            open_target: IpcOpenTarget::Tab,
            profile: Some("work".into()),
            context: None,
            external_open: false,
            source: CommandSource::Ipc,
        };
        (rust, route)
    }

    #[test]
    fn canonical_profile_name_is_valid_even_when_its_display_label_differs() {
        let (rust, route) = owner();
        assert_eq!(
            rust.state
                .as_ref()
                .unwrap()
                .profiles()
                .values()
                .next()
                .unwrap()
                .label,
            "Work browser"
        );
        assert!(validate_route(&rust, &route).is_ok());
        let mut display_label = route;
        display_label.profile = Some("Work browser".into());
        assert_eq!(
            validate_route(&rust, &display_label).unwrap_err().code(),
            ErrorCode::Denied
        );
    }

    #[test]
    fn another_profile_or_closed_window_cannot_use_the_owner() {
        let (rust, mut route) = owner();
        route.profile = Some("personal".into());
        assert_eq!(
            validate_route(&rust, &route).unwrap_err().code(),
            ErrorCode::Denied
        );
        route.profile = Some("work".into());
        route.selector = DispatchTarget::Window(
            WindowId::from_display("windowid-18446744073709551615").unwrap(),
        );
        assert_eq!(
            validate_route(&rust, &route).unwrap_err().code(),
            ErrorCode::StaleTarget
        );
    }
}
