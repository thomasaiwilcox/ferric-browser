//! Retained selected-owner identity across synchronous Qt notifications.

use cxx_qt::CxxQtType;
use ferric_browser_application::ConfigurationRequestCapture;
use ferric_browser_core::{Target, WindowId};

use crate::{BrowserUiRust, QString, qobject};

/// Core ownership while UI properties are still being projected. Unlike the
/// completed policy capture, this does not require the new UI index/URL yet.
pub(super) struct CoreSelectionCapture {
    request: ConfigurationRequestCapture,
    window: WindowId,
    pub target: Target,
    runtime_url: Option<String>,
}

impl CoreSelectionCapture {
    pub(super) fn capture(ui: &qobject::BrowserUi) -> Option<Self> {
        Self::capture_rust(ui.rust())
    }

    fn capture_rust(rust: &BrowserUiRust) -> Option<Self> {
        let state = rust.state.as_ref()?;
        let window = rust.window?;
        let tab = state.windows().get(&window)?.active_tab?;
        let capture = Self {
            request: state.capture_configuration_request()?,
            window,
            target: state.capture_target(tab)?,
            runtime_url: state.tabs().get(&tab)?.url.clone(),
        };
        capture.is_current_rust(rust).then_some(capture)
    }

    pub(super) fn is_current(&self, ui: &qobject::BrowserUi) -> bool {
        self.is_current_rust(ui.rust())
    }

    fn is_current_rust(&self, rust: &BrowserUiRust) -> bool {
        self.context_is_current(rust)
            && rust
                .state
                .as_ref()
                .is_some_and(|state| state.configuration_request_is_current(&self.request))
    }

    fn context_is_current(&self, rust: &BrowserUiRust) -> bool {
        rust.view_alive
            && rust.window == Some(self.window)
            && rust.state.as_ref().is_some_and(|state| {
                state.configuration_request_owner_is_current(&self.request)
                    && state.capture_target(self.target.tab) == Some(self.target)
                    && state
                        .tabs()
                        .get(&self.target.tab)
                        .is_some_and(|tab| tab.url == self.runtime_url)
                    && state
                        .windows()
                        .get(&self.window)
                        .is_some_and(|window| window.active_tab == Some(self.target.tab))
            })
    }
}

/// Latest browser-owned URL fact, including a provisional address whose Core
/// model still names the prior document. This grants no receiver/action proof.
pub(super) struct UrlProjectionInput {
    scope: CoreSelectionCapture,
    pub url: QString,
}

impl UrlProjectionInput {
    pub(super) fn capture(ui: &qobject::BrowserUi, url: QString) -> Option<Self> {
        Some(Self {
            scope: CoreSelectionCapture::capture(ui)?,
            url,
        })
    }

    pub(super) fn is_current(&self, ui: &qobject::BrowserUi) -> bool {
        self.scope.is_current(ui)
    }

    pub(super) fn context_is_current(&self, rust: &BrowserUiRust) -> bool {
        self.scope.context_is_current(rust)
    }
}

impl BrowserUiRust {
    pub(super) fn url_projection_is_complete(&self) -> bool {
        self.url_projection_input
            .as_ref()
            .zip(self.completed_url_projection.as_ref())
            .is_some_and(|(latest, completed)| std::sync::Arc::ptr_eq(latest, completed))
    }

    pub(super) fn url_projection_is_current(&self) -> bool {
        self.url_projection_is_complete()
            && self.url_projection_input.as_ref().is_some_and(|input| {
                input.context_is_current(self) && input.url == self.current_url
            })
    }
}

pub(super) struct PolicyRefreshCapture {
    request: ConfigurationRequestCapture,
    window: WindowId,
    target: Target,
    index: i32,
    input_epoch: i32,
    url: String,
}

impl PolicyRefreshCapture {
    pub(super) fn capture(ui: &qobject::BrowserUi) -> Option<Self> {
        Self::capture_rust(ui.rust())
    }

    pub(super) fn capture_rust(rust: &BrowserUiRust) -> Option<Self> {
        let state = rust.state.as_ref()?;
        let window = rust.window?;
        let tab = state.windows().get(&window)?.active_tab?;
        let capture = Self {
            request: state.capture_configuration_request()?,
            window,
            target: state.capture_target(tab)?,
            index: rust.active_tab_index,
            input_epoch: rust.input_epoch,
            url: rust.current_url.to_string(),
        };
        capture.is_current_rust(rust).then_some(capture)
    }

    pub(super) fn is_current(&self, ui: &qobject::BrowserUi) -> bool {
        self.is_current_rust(ui.rust())
    }

    pub(super) fn is_current_rust(&self, rust: &BrowserUiRust) -> bool {
        rust.view_alive
            && !rust.profile_bootstrap_pending
            && rust.window == Some(self.window)
            && rust.tab == Some(self.target.tab)
            && rust.active_tab_index == self.index
            && rust.input_epoch == self.input_epoch
            && rust.current_url.to_string() == self.url
            && usize::try_from(self.index)
                .ok()
                .and_then(|index| rust.tab_ids.get(index))
                == Some(&self.target.tab)
            && rust.state.as_ref().is_some_and(|state| {
                state.configuration_request_is_current(&self.request)
                    && state.capture_target(self.target.tab) == Some(self.target)
                    && state
                        .windows()
                        .get(&self.window)
                        .is_some_and(|window| window.active_tab == Some(self.target.tab))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_core::{Event, ValidatedUrl};
    use ferric_browser_runtime::RuntimeInput;
    use std::sync::Arc;

    fn address(rust: &mut BrowserUiRust, url: &str) {
        let tab = rust.tab.unwrap();
        let state = rust.state.as_mut().unwrap();
        state
            .dispatch_runtime(RuntimeInput::Lifecycle(Event::SameDocumentNavigation {
                target: state.capture_target(tab).unwrap(),
                url: ValidatedUrl::parse(url).unwrap(),
            }))
            .unwrap();
    }

    fn input(rust: &BrowserUiRust, url: &str) -> Arc<UrlProjectionInput> {
        Arc::new(UrlProjectionInput {
            scope: CoreSelectionCapture::capture_rust(rust).unwrap(),
            url: QString::from(url),
        })
    }

    #[test]
    fn provisional_url_fact_survives_settings_change_but_not_model_or_owner_change() {
        let mut rust = BrowserUiRust::default();
        address(&mut rust, "https://example.test/old");
        let trusted = input(&rust, "https://example.test/loading");
        rust.current_url = trusted.url.clone();
        rust.url_projection_input = Some(Arc::clone(&trusted));
        rust.completed_url_projection = Some(Arc::clone(&trusted));
        assert!(rust.url_projection_is_current());
        let tab = rust.tab.unwrap();
        assert_eq!(
            rust.state.as_ref().unwrap().tabs()[&tab].url.as_deref(),
            Some("https://example.test/old")
        );
        let state = rust.state.as_mut().unwrap();
        let mut layers = state.configuration_layers().clone();
        layers
            .temporary
            .set_literal("content.zoom", "1.25")
            .unwrap();
        state.update_configuration(layers).unwrap();
        assert!(!trusted.scope.is_current_rust(&rust));
        // URL facts are not old configuration/policy validation receipts.
        assert!(trusted.context_is_current(&rust));
        assert!(rust.url_projection_is_current());
        let target = rust.state.as_ref().unwrap().capture_target(tab).unwrap();
        address(&mut rust, "https://example.test/new");
        assert_eq!(
            rust.state.as_ref().unwrap().capture_target(tab),
            Some(target)
        );
        assert!(!trusted.context_is_current(&rust));
        assert!(!rust.url_projection_is_current());
        let other = BrowserUiRust::default();
        assert!(!trusted.context_is_current(&other));
    }

    #[test]
    fn equal_url_bytes_cannot_complete_a_new_publication() {
        let mut rust = BrowserUiRust::default();
        let previous = input(&rust, "https://example.test/same");
        rust.current_url = previous.url.clone();
        rust.url_projection_input = Some(Arc::clone(&previous));
        rust.completed_url_projection = Some(previous);
        assert!(rust.url_projection_is_current());
        let latest = input(&rust, "https://example.test/same");
        rust.url_projection_input = Some(Arc::clone(&latest));
        assert!(!rust.url_projection_is_complete());
        assert!(!rust.url_projection_is_current());
        rust.completed_url_projection = Some(latest);
        assert!(rust.url_projection_is_current());
    }
}
