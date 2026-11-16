//! Native-only association capture across Qt reads. No QML invokable, JSON
//! eligibility claim or retained model URL can substitute for the native URL.
use crate::{CxxQtType, Pin, QString, qobject};
use ferric_browser_application::{SitePolicyMatchContext, SitePolicyObservation};
use ferric_browser_core::{ShutdownState, TabId, Target, ValidatedUrl, canonicalize};

pub(crate) struct SitePolicyAssociation {
    captured: Option<(Target, SitePolicyObservation)>,
}

fn owned_target(ui: &qobject::BrowserUi, tab: &str) -> Option<(i32, Target)> {
    let tab = TabId::from_display(tab)?;
    let rust = ui.rust();
    let state = rust.state.as_ref()?;
    let window = rust.window?;
    if !rust.view_alive
        || rust.profile_bootstrap_pending
        || rust.ipc_shutdown_gate
        || state.shutdown() != ShutdownState::Running
        || state.tabs().get(&tab)?.window != window
    {
        return None;
    }
    let index = i32::try_from(
        rust.tab_ids
            .iter()
            .position(|candidate| *candidate == tab)?,
    )
    .ok()?;
    Some((index, state.capture_target(tab)?))
}

pub(crate) fn begin_association(
    mut ui: Pin<&mut qobject::BrowserUi>,
    tab: &QString,
) -> Box<SitePolicyAssociation> {
    let captured =
        owned_target(ui.as_ref().get_ref(), &tab.to_string()).and_then(|(index, target)| {
            let observation = {
                let mut rust = ui.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.state
                    .as_mut()?
                    .begin_site_policy_association(this.window?, target)?
            };
            ui.as_mut().reject_observed_policy_context_for(index);
            Some((target, observation))
        });
    Box::new(SitePolicyAssociation { captured })
}

pub(crate) fn complete_association(
    mut ui: Pin<&mut qobject::BrowserUi>,
    captured: &SitePolicyAssociation,
    url: &QString,
) -> bool {
    let Some((target, observation)) = &captured.captured else {
        return false;
    };
    let Some((index, current)) = owned_target(ui.as_ref().get_ref(), &target.tab.to_string())
    else {
        return false;
    };
    if current != *target {
        return false;
    }
    // Check Qt's UTF-16 length before copying a potentially huge rejected data
    // URL. Core performs the authoritative encoded-byte and scheme validation.
    let observed = usize::try_from(url.len())
        .is_ok_and(|length| length <= ferric_browser_core::MAX_URL_BYTES)
        .then(|| canonicalize(&url.to_string()).ok())
        .flatten()
        .and_then(|url| ValidatedUrl::parse(url).ok());
    let matches = observed.is_some_and(|observed| {
        let binding = ui.as_ref();
        let rust = binding.rust();
        let model = rust
            .state
            .as_ref()
            .and_then(|state| state.tabs().get(&target.tab))
            .and_then(|tab| canonicalize(tab.url.as_deref().unwrap_or("about:blank")).ok())
            .and_then(|url| ValidatedUrl::parse(url).ok());
        model.is_some_and(|model| {
            SitePolicyMatchContext::for_url(&model) == SitePolicyMatchContext::for_url(&observed)
        })
    });
    let (changed, active) = {
        let mut rust = ui.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let Some(state) = this.state.as_mut() else {
            return false;
        };
        let active = this
            .window
            .and_then(|window| state.windows().get(&window))
            .is_some_and(|window| window.active_tab == Some(target.tab));
        let changed = if matches {
            state.accept_observed_interaction_context(observation)
        } else {
            state.reject_observed_interaction_context(observation)
        };
        (changed, active)
    };
    if matches && changed && active {
        ui.as_mut().request_interaction_policy_refresh();
    } else if !matches && changed {
        ui.as_mut().reject_observed_policy_context_for(index);
    }
    matches && changed
}
