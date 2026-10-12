use super::{Config, CxxQtType, Pin, qobject};

impl qobject::BrowserUi {
    pub(super) fn update_hint_proxy_targets(
        mut self: Pin<&mut Self>,
        config: &Config,
        url: &str,
    ) -> bool {
        let enabled = matches!(
            ferric_browser_config::hint_proxy_targets_for_url(config, url),
            ferric_browser_config::HintProxyTargets::Validated
        );
        let changed = self.as_ref().rust().hint_proxy_targets_enabled != enabled;
        self.as_mut().set_hint_proxy_targets_enabled(enabled);
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .hint_proxy_targets_enabled = enabled;
        changed
    }
}
