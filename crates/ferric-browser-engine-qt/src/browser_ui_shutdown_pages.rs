//! Private document/view proof for an owned shutdown decision.
use crate::{CxxQtType, Target, WindowId, qobject};
use ferric_browser_application::ProfileInstanceId;

#[derive(PartialEq, Eq)]
pub(super) struct ShutdownPages {
    window: WindowId,
    token: String,
    profile: ProfileInstanceId,
    context: Option<String>,
    pages: Vec<(Target, Option<String>)>,
}
impl ShutdownPages {
    pub(super) fn capture(ui: &qobject::BrowserUi) -> Option<Self> {
        let rust = ui.rust();
        let state = rust.state.as_ref()?;
        if rust.profile_bootstrap_pending
            || state.shutdown() != ferric_browser_core::ShutdownState::Running
        {
            return None;
        }
        let window = rust.window?;
        let owner = state.windows().get(&window)?;
        let pages = owner
            .tabs
            .iter()
            .map(|tab| {
                Some((
                    state.capture_target(*tab)?,
                    rust.interaction.prompt_view_identities.get(tab).cloned(),
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            window,
            token: rust.window_token.to_string(),
            profile: state.profile_instance_id()?,
            context: owner.context.clone(),
            pages,
        })
    }
    pub(super) fn current(&self, ui: &qobject::BrowserUi) -> bool {
        Self::capture(ui).as_ref() == Some(self)
    }
}
