//! Association between an application configuration owner and its Core domain.
//! Captured only at activation/inheritance; focus and registry changes cannot
//! silently attach existing configuration to another profile.

use crate::BrowserApplication;
use ferric_browser_core::{PrivacyKind, ProfileId};
use ferric_browser_runtime::RuntimeView;
use std::collections::BTreeSet;

pub(crate) fn capture_core_profile_scope(
    runtime: RuntimeView<'_>,
    privacy: PrivacyKind,
) -> Option<ProfileId> {
    // A registry entry alone supplies no association. Require a real window
    // domain, independent of which window or tab currently has focus.
    let profile = runtime.windows().values().next()?.profile;
    if runtime.profiles().get(&profile)?.privacy != privacy {
        return None;
    }
    let mut seen = BTreeSet::new();
    for window in runtime.windows().values() {
        if window.profile != profile {
            return None;
        }
        for id in &window.tabs {
            let tab = runtime.tabs().get(id)?;
            if tab.profile != profile || tab.window != window.id || !seen.insert(*id) {
                return None;
            }
        }
    }
    if seen.len() != runtime.tabs().len() {
        return None;
    }
    Some(profile)
}

impl BrowserApplication {
    /// Contextual consumers use the retained association, never the first
    /// remaining profile or a privacy/name match. Global settings do not use
    /// this condition. The hot cache read performs no source work or allocation.
    pub(crate) fn interaction_core_profile(&self) -> Option<ProfileId> {
        let active = self.active_profile.as_ref()?;
        let profile = self.core_profile_scope?;
        (self.runtime.view().profiles().get(&profile)?.privacy == active.privacy())
            .then_some(profile)
    }
}

#[cfg(test)]
mod tests;
