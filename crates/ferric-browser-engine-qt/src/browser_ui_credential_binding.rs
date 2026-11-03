//! Private native receiver to live Core/profile ownership. None of these
//! functions is a Qt invokable or consumes a page-supplied ownership envelope.

use crate::bridge::qobject::{BrowserUi, NativeCredentialFacts};
use cxx_qt::CxxQtType;
use ferric_browser_application::{CredentialDocumentLease, EngineCredentialFacts};
use ferric_browser_core::TabId;

pub(crate) struct CredentialLeaseCapture(pub(super) Option<CredentialDocumentLease>);

pub(crate) fn credential_view_association_current(
    ui: &BrowserUi,
    tab: &str,
    index: i32,
    view_identity: &str,
) -> bool {
    let Some(tab) = TabId::from_display(tab) else {
        return false;
    };
    let rust = ui.rust();
    let Some(state) = rust.state.as_ref() else {
        return false;
    };
    rust.view_alive
        && !rust.profile_bootstrap_pending
        && !rust.page_window_preparing
        && !rust.ipc_shutdown_gate
        && state.shutdown() == ferric_browser_core::ShutdownState::Running
        && state.configuration().passkeys.phone == ferric_browser_config::PhonePasskeyPolicy::On
        && usize::try_from(index)
            .ok()
            .and_then(|index| rust.tab_ids.get(index))
            == Some(&tab)
        && state.tabs().get(&tab).is_some_and(|page| {
            Some(page.window) == rust.window
                && page.existence == ferric_browser_core::ExistenceState::Live
        })
        && !view_identity.is_empty()
        && rust
            .interaction
            .prompt_view_identities
            .get(&tab)
            .map(String::as_str)
            == Some(view_identity)
}

fn owned_tab(ui: &BrowserUi, value: &str) -> Option<TabId> {
    let tab = TabId::from_display(value)?;
    let rust = ui.rust();
    if !rust.view_alive || rust.profile_bootstrap_pending || rust.page_window_preparing {
        return None;
    }
    let state = rust.state.as_ref()?;
    if rust.ipc_shutdown_gate || state.shutdown() != ferric_browser_core::ShutdownState::Running {
        return None;
    }
    let page = state.tabs().get(&tab)?;
    let window = rust.window?;
    (rust.tab_ids.contains(&tab) && page.window == window && state.windows().contains_key(&window))
        .then_some(tab)
}

fn owned_active_tab(ui: &BrowserUi, value: &str) -> Option<TabId> {
    let tab = owned_tab(ui, value)?;
    let rust = ui.rust();
    (rust
        .state
        .as_ref()?
        .windows()
        .get(&rust.window?)?
        .active_tab
        == Some(tab))
    .then_some(tab)
}

pub(crate) fn credential_context_identity(ui: &BrowserUi, value: &str) -> String {
    // Install the boundary for every owned tab before navigation. Active-tab
    // and native foreground admission remain mandatory in capture().
    let Some(tab) = owned_tab(ui, value) else {
        return String::new();
    };
    let rust = ui.rust();
    if rust.credential_policy_epoch == 0 {
        return String::new();
    }
    let Some(state) = rust.state.as_ref() else {
        return String::new();
    };
    let Some(target) = state.capture_target(tab) else {
        return String::new();
    };
    let Some(session) = state.profile_instance_id() else {
        return String::new();
    };
    let window = &state.windows()[&state.tabs()[&tab].window];
    // A single unambiguous native identity fences ownership, selection,
    // context and both configured and installed effective-policy changes.
    serde_json::to_string(&(
        session.to_string(),
        window.id.to_string(),
        target.tab.to_string(),
        target.generation,
        target.document.to_string(),
        &window.context,
        window.active_tab == Some(tab),
        state.configuration_revision(),
        rust.config_revision,
        rust.policy_revision,
        rust.credential_policy_epoch,
        rust.blocking_policy_ready,
    ))
    .unwrap_or_default()
}

fn capture(
    ui: &BrowserUi,
    value: &str,
    facts: &NativeCredentialFacts,
) -> Option<CredentialDocumentLease> {
    let tab = owned_active_tab(ui, value)?;
    ui.rust()
        .state
        .as_ref()?
        .capture_credential_document(
            tab,
            &EngineCredentialFacts {
                security_origin: facts.origin.clone(),
                profile_generation: facts.profile_generation,
                page_generation: facts.page_generation,
                document_generation: facts.document_generation,
                host_generation: facts.host_generation,
                presentation_generation: facts.presentation_generation,
                // These are established by the private native receiver before
                // it constructs NativeCredentialFacts. No page boolean crosses
                // this seam as main-frame/security/policy authority.
                main_frame: true,
                secure_context: true,
                permissions_policy_allowed: true,
                foreground: facts.foreground,
            },
        )
        .ok()
}

pub(crate) fn capture_native_credential_lease(
    ui: &BrowserUi,
    tab: &str,
    facts: &NativeCredentialFacts,
) -> Box<CredentialLeaseCapture> {
    Box::new(CredentialLeaseCapture(capture(ui, tab, facts)))
}

impl CredentialLeaseCapture {
    pub(crate) fn is_valid(&self) -> bool {
        self.0.is_some()
    }

    pub(crate) fn is_current(
        &self,
        ui: &BrowserUi,
        tab: &str,
        facts: &NativeCredentialFacts,
    ) -> bool {
        self.0.is_some() && self.0 == capture(ui, tab, facts)
    }
}
