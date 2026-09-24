//! Read-only runtime guard predicates used by the Qt adapter.
//!
//! The adapter may ask whether an engine callback still refers to the current
//! browser document, but it never mutates state to make that true.

use ferric_browser_application::BrowserApplication;
use ferric_browser_core::{ExistenceState, RendererState, TabId, Target};
use std::time::Instant;

#[must_use]
pub(super) fn current_target(
    application: Option<&BrowserApplication>,
    tab: Option<TabId>,
) -> Option<Target> {
    application?.capture_target(tab?)
}

#[must_use]
pub(super) fn captured_target_is_current(
    application: Option<&BrowserApplication>,
    target: Target,
) -> bool {
    application
        .and_then(|application| application.capture_target(target.tab))
        .is_some_and(|current| current == target)
}

#[must_use]
pub(super) fn live_document_available(
    application: Option<&BrowserApplication>,
    tab: Option<TabId>,
) -> bool {
    let Some(target) = current_target(application, tab) else {
        return false;
    };
    application.is_some_and(|application| {
        application.tabs().get(&target.tab).is_some_and(|tab| {
            tab.document == target.document
                && tab.existence == ExistenceState::Live
                && tab.renderer == RendererState::Healthy
        })
    })
}

#[must_use]
pub(super) fn elapsed_ms(clock: Instant) -> u64 {
    u64::try_from(clock.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub(super) fn validate_clipboard_navigation_input(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("Clipboard read returned no text; navigation was not started".into());
    }
    if value.len() > 64 * 1024 {
        return Err("Clipboard text exceeds 64 KiB; navigation was not started".into());
    }
    if value.chars().any(char::is_control) {
        return Err(
            "Clipboard text contains control characters; navigation was not started".into(),
        );
    }
    Ok(value.to_owned())
}
