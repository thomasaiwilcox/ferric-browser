//! An accepted GET for a never-committed page is session metadata, independent
//! of command continuations. A different document/profile cannot inherit it.
use crate::{BrowserUiRust, RuntimeEffect, TabId, Target, ValidatedUrl, WindowId};
use ferric_browser_application::ProfileInstanceId;

pub(super) struct AcceptedPageGet {
    target: Target,
    window: WindowId,
    profile: ProfileInstanceId,
    url: ValidatedUrl,
}

pub(super) fn observe(rust: &mut BrowserUiRust, effects: &[RuntimeEffect]) {
    let Some(state) = rust.state.as_ref() else {
        rust.accepted_page_gets.clear();
        return;
    };
    let profile = state.profile_instance_id();
    rust.accepted_page_gets.retain(|tab, intent| {
        profile == Some(intent.profile)
            && state.capture_target(*tab) == Some(intent.target)
            && state
                .tabs()
                .get(tab)
                .is_some_and(|page| page.window == intent.window && page.url.is_none())
    });
    let Some(profile) = profile else { return };
    for effect in effects {
        if let RuntimeEffect::Engine(crate::EngineEffect::Navigate { target, url }) = effect
            && state.capture_target(target.tab) == Some(*target)
            && let Some(page) = state
                .tabs()
                .get(&target.tab)
                .filter(|page| page.url.is_none())
        {
            rust.accepted_page_gets.insert(
                target.tab,
                AcceptedPageGet {
                    target: *target,
                    window: page.window,
                    profile,
                    url: url.clone(),
                },
            );
        }
    }
}

pub(super) fn session_url(rust: &BrowserUiRust, tab: TabId) -> Option<String> {
    if let Some(url) = crate::browser_ui_popup_owner::prepared_session_url(rust, tab) {
        return Some(url);
    }
    let state = rust.state.as_ref()?;
    let page = state.tabs().get(&tab)?;
    if let Some(url) = &page.url {
        return Some(url.clone());
    }
    let intent = rust.accepted_page_gets.get(&tab)?;
    (state.profile_instance_id() == Some(intent.profile)
        && page.window == intent.window
        && state.capture_target(tab) == Some(intent.target))
    .then(|| intent.url.to_string())
}

// Live transfer preserves the captured document, but changes its Core window.
// A stopped first load still needs its admitted GET for safe restoration.
pub(super) fn transfer(source: &mut BrowserUiRust, destination: &mut BrowserUiRust, tab: TabId) {
    let Some(mut intent) = source.accepted_page_gets.remove(&tab) else {
        return;
    };
    let Some(state) = destination.state.as_ref() else {
        return;
    };
    if source.window != Some(intent.window)
        || source
            .state
            .as_ref()
            .and_then(crate::BrowserApplication::profile_instance_id)
            != Some(intent.profile)
        || state.profile_instance_id() != Some(intent.profile)
        || state.capture_target(tab) != Some(intent.target)
    {
        return;
    }
    let Some(page) = state.tabs().get(&tab).filter(|page| page.url.is_none()) else {
        return;
    };
    intent.window = page.window;
    destination.accepted_page_gets.insert(tab, intent);
}
