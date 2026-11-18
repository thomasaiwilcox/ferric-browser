use super::{CxxQtType, Pin, qobject};
use ferric_browser_application::PreparedConfigurationUpdate;
use ferric_browser_config::{EffectiveHintSelectorFamily, HintProxyTargets, HintSelectorFamily};
use std::collections::BTreeMap;

pub(super) struct HintDiscoveryConfiguration {
    pub(super) selectors: BTreeMap<HintSelectorFamily, EffectiveHintSelectorFamily>,
    pub(super) proxy_targets: HintProxyTargets,
}

impl qobject::BrowserUi {
    /// Validate the exact prepared discovery inputs before disk work or
    /// publication. The returned proxy value is installed only after commit.
    pub(super) fn validate_candidate_hint_discovery(
        &self,
        prepared: &PreparedConfigurationUpdate,
    ) -> Result<HintDiscoveryConfiguration, String> {
        let sources = prepared.policy_sources();
        let url = self.rust().current_url.to_string();
        let selectors = sources
            .compile_hint_selectors(&url)
            .map_err(|error| error.to_string())?;
        let proxy_targets = sources
            .compile_hint_proxy_targets(&url)
            .map_err(|error| error.to_string())?;
        Ok(HintDiscoveryConfiguration {
            selectors,
            proxy_targets,
        })
    }

    pub(super) fn apply_hint_proxy_targets(
        mut self: Pin<&mut Self>,
        policy: HintProxyTargets,
    ) -> bool {
        let enabled = matches!(policy, HintProxyTargets::Validated);
        let changed = self.as_ref().rust().hint_proxy_targets_enabled != enabled;
        self.as_mut().set_hint_proxy_targets_enabled(enabled);
        changed
    }

    /// Close the transport when its canonical source result is unavailable.
    /// This is an eligibility flag, not an invented effective Off policy; the
    /// shared source snapshot retains the independent compilation diagnostic.
    pub(super) fn close_hint_proxy_targets(mut self: Pin<&mut Self>) -> bool {
        let changed = self.as_ref().rust().hint_proxy_targets_enabled;
        self.as_mut().set_hint_proxy_targets_enabled(false);
        changed
    }
}
