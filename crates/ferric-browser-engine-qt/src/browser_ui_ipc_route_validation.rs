//! Route validation for the Qt IPC boundary.
//!
//! This module owns only the projection from an already-decoded [`IpcRoute`]
//! onto live browser state. It deliberately does not inspect JSON or execute
//! commands.

use super::{CxxQtType, DispatchTarget, IpcOpenTarget, IpcRoute, qobject};

impl qobject::BrowserUi {
    pub(super) fn validate_ipc_route(&self, route: &IpcRoute) -> Result<(), String> {
        let state = self
            .rust()
            .state
            .as_ref()
            .ok_or_else(|| "core state unavailable".to_owned())?;
        let window = match route.selector {
            DispatchTarget::Active => state.active_window(),
            DispatchTarget::LastFocused => state.last_focused_window().or(state.active_window()),
            DispatchTarget::Window(window) => Some(window),
            DispatchTarget::Tab(tab) => state.tabs().get(&tab).map(|tab| tab.window),
        }
        .and_then(|window| state.windows().get(&window).map(|_| window))
        .ok_or_else(|| "requested window is not available".to_owned())?;
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
                .ok_or_else(|| format!("context not found: {context_name}"))?;
            if route.open_target == IpcOpenTarget::PrivateWindow {
                return Err("private-window cannot use a durable context".into());
            }
            if !matches!(route.open_target, IpcOpenTarget::Window)
                && context.profile != self.rust().profile_name
            {
                return Err("context belongs to a different profile".into());
            }
            if route
                .profile
                .as_deref()
                .is_some_and(|profile| profile != context.profile)
            {
                return Err("profile and context selectors disagree".into());
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
                .ok_or_else(|| format!("profile not found: {profile}"))?;
            if requested.id != state.windows()[&window].profile {
                return Err("the requested profile is not an active GUI profile".into());
            }
        }
        Ok(())
    }
}
