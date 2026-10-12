//! Active Mixed-key policy resolution and synchronous routing projection.

use cxx_qt::CxxQtType;
use ferric_browser_config::{Config, RuntimeOverrides, matching_site_rules};
use ferric_browser_core::{
    BindingResolver, BrowserKeySequenceState, EffectiveMixedKeyPolicy, LogicalKeyToken,
    MixedKeyDecision, MixedKeyDecisionReason, MixedPolicyLayer, Mode, resolve_mixed_key_policy,
};

use crate::{BrowserUiRust, ConfigurationLayers, Pin, QString, Value, qobject};

fn layer_from_overrides(overrides: &RuntimeOverrides) -> Result<Option<MixedPolicyLayer>, String> {
    let policy = overrides
        .settings
        .get("input.shortcut_policy")
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "input.shortcut_policy override must be a string".to_owned())
        })
        .transpose()?;
    let page_keys = overrides
        .settings
        .get("input.page_keys")
        .map(|value| {
            value
                .as_array()
                .ok_or_else(|| "input.page_keys override must be a list".to_owned())?
                .iter()
                .map(|key| {
                    key.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "input.page_keys entries must be strings".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    if policy.is_none() && page_keys.is_none() {
        return Ok(None);
    }
    Ok(Some(MixedPolicyLayer {
        shortcut_policy: policy,
        page_keys,
    }))
}

fn merge_layer(target: &mut MixedPolicyLayer, overlay: Option<MixedPolicyLayer>) {
    if let Some(overlay) = overlay {
        if overlay.shortcut_policy.is_some() {
            target.shortcut_policy = overlay.shortcut_policy;
        }
        if overlay.page_keys.is_some() {
            target.page_keys = overlay.page_keys;
        }
    }
}

pub(super) fn resolve_for_layers(
    base: &Config,
    profile: &RuntimeOverrides,
    runtime: &RuntimeOverrides,
    command_line: &RuntimeOverrides,
    temporary: &RuntimeOverrides,
    effective: &Config,
    url: &str,
) -> Result<EffectiveMixedKeyPolicy, String> {
    let global = MixedPolicyLayer::new(
        Some(base.input.shortcut_policy.as_str()),
        Some(base.input.page_keys.clone()),
    );
    let profile = layer_from_overrides(profile)?;

    let mut site = MixedPolicyLayer::default();
    let mut has_site = false;
    for rule in matching_site_rules(effective, url) {
        if let Some(policy) = rule
            .set
            .get("input.shortcut_policy")
            .and_then(toml::Value::as_str)
        {
            site.shortcut_policy = Some(policy.to_owned());
            has_site = true;
        }
        if let Some(page_keys) = rule
            .set
            .get("input.page_keys")
            .and_then(toml::Value::as_array)
        {
            let page_keys = page_keys
                .iter()
                .map(|key| {
                    key.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "validated site page key was not a string".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            site.page_keys = Some(page_keys);
            has_site = true;
        }
    }

    // Runtime, CLI, and temporary global values follow matching site rules in
    // CONFIG-002. The config transaction has already validated each layer.
    let mut runtime_layer = MixedPolicyLayer::default();
    merge_layer(&mut runtime_layer, layer_from_overrides(runtime)?);
    merge_layer(&mut runtime_layer, layer_from_overrides(command_line)?);
    merge_layer(&mut runtime_layer, layer_from_overrides(temporary)?);
    let runtime = (runtime_layer.shortcut_policy.is_some() || runtime_layer.page_keys.is_some())
        .then_some(runtime_layer);
    let site = has_site.then_some(site);

    resolve_mixed_key_policy(
        Some(&global),
        profile.as_ref(),
        site.as_ref(),
        runtime.as_ref(),
    )
    .map_err(|error| error.to_string())
}

fn sequence_state(rust: &BrowserUiRust) -> BrowserKeySequenceState {
    if rust.macro_key_prefix.is_some()
        || rust
            .bindings
            .as_ref()
            .is_some_and(BindingResolver::has_pending_keychain)
    {
        BrowserKeySequenceState::ActiveKeychain
    } else if rust
        .bindings
        .as_ref()
        .is_some_and(BindingResolver::has_pending_count)
    {
        BrowserKeySequenceState::ActiveCount
    } else {
        BrowserKeySequenceState::Idle
    }
}

pub(super) fn decision_for_key(
    rust: &BrowserUiRust,
    mode: Mode,
    key: &str,
) -> Option<MixedKeyDecision> {
    let token = LogicalKeyToken::parse(key).ok()?;
    Some(rust.mixed_policy.decide(mode, &token, sequence_state(rust)))
}

impl qobject::BrowserUi {
    pub(super) fn runtime_config_candidate_from(
        &self,
        base_value: &Value,
        profile_overrides: &RuntimeOverrides,
        runtime_overrides: &RuntimeOverrides,
        temporary_overrides: &RuntimeOverrides,
    ) -> Result<Value, String> {
        let layers = ConfigurationLayers {
            base: serde_json::from_value(base_value.clone())
                .map_err(|error| format!("base configuration is invalid: {error}"))?,
            profile: profile_overrides.clone(),
            runtime: runtime_overrides.clone(),
            command_line: self.rust().cli_overrides.clone(),
            temporary: temporary_overrides.clone(),
        };
        serde_json::to_value(layers.resolve().map_err(|error| error.to_string())?)
            .map_err(|error| format!("could not serialize resolved configuration: {error}"))
    }

    pub(super) fn resolve_candidate_mixed_policy(
        &self,
        candidate: &Value,
        layers: &ConfigurationLayers,
    ) -> Result<(EffectiveMixedKeyPolicy, bool), String> {
        let effective = serde_json::from_value::<Config>(candidate.clone())
            .map_err(|error| format!("candidate configuration is invalid: {error}"))?;
        let policy = resolve_for_layers(
            &layers.base,
            &layers.profile,
            &layers.runtime,
            &layers.command_line,
            &layers.temporary,
            &effective,
            &self.rust().current_url.to_string(),
        )?;
        let changed = self.rust().mixed_policy != policy;
        Ok((policy, changed))
    }

    pub(super) fn refresh_mixed_policy_for_url(
        mut self: Pin<&mut Self>,
        effective: &Config,
        url: &str,
    ) -> bool {
        match self.as_mut().update_mixed_policy_for_url(effective, url) {
            Ok(_) => true,
            Err(error) => {
                self.as_mut().set_status_text(QString::from(format!(
                    "Mixed input policy unavailable: {error}"
                )));
                false
            }
        }
    }

    pub(super) fn mixed_key_route(self: Pin<&mut Self>, key: &QString) -> QString {
        let decision = decision_for_key(
            self.as_ref().rust(),
            self.as_ref().rust().core_mode,
            &key.to_string(),
        );
        let route = match decision.map(|decision| decision.reason) {
            Some(MixedKeyDecisionReason::ExactPageKey) => "page",
            Some(MixedKeyDecisionReason::ActiveBrowserCount) => "active-count",
            Some(MixedKeyDecisionReason::ActiveBrowserKeychain) => "active-keychain",
            Some(MixedKeyDecisionReason::ReservedControl) => "reserved",
            _ => "browser",
        };
        QString::from(route)
    }

    pub(super) fn update_mixed_policy_for_url(
        mut self: Pin<&mut Self>,
        effective: &Config,
        url: &str,
    ) -> Result<bool, String> {
        let rust_ref = self.as_ref();
        let rust = rust_ref.rust();
        let base = serde_json::from_value::<Config>(rust.base_config.clone())
            .map_err(|error| format!("base configuration is invalid: {error}"))?;
        let policy = resolve_for_layers(
            &base,
            &rust.profile_overrides,
            &rust.runtime_overrides,
            &rust.cli_overrides,
            &rust.temporary_overrides,
            effective,
            url,
        )?;
        let changed = self.as_ref().rust().mixed_policy != policy;
        if changed {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.mixed_policy = policy;
            if let Some(bindings) = this.bindings.as_mut() {
                bindings.reset();
            }
            this.macro_key_prefix = None;
            this.macro_key_started_ms = None;
            drop(rust);
            self.as_mut().set_binding_overlay(QString::default());
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use ferric_browser_config::{InputShortcutPolicy, SiteRule};
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn effective_policy_keeps_layer_and_site_precedence() {
        let base = Config::default();
        let mut profile = RuntimeOverrides::default();
        profile
            .set_literal("input.shortcut_policy", "mixed")
            .expect("profile policy");
        profile
            .set_literal("input.page_keys", "['g']")
            .expect("profile page keys");
        let mut effective = base.clone();
        effective.site_rules.push(SiteRule {
            id: "docs".into(),
            pattern: "https://docs.example.test/*".into(),
            priority: 0,
            set: BTreeMap::from([(
                "input.page_keys".into(),
                toml::Value::Array(vec![toml::Value::String("/".into())]),
            )]),
        });

        let policy = resolve_for_layers(
            &base,
            &profile,
            &RuntimeOverrides::default(),
            &RuntimeOverrides::default(),
            &RuntimeOverrides::default(),
            &effective,
            "https://docs.example.test/a",
        )
        .expect("effective mixed policy");
        assert_eq!(
            policy.shortcut_policy,
            ferric_browser_core::ShortcutPolicy::Mixed
        );
        assert_eq!(
            policy.shortcut_policy_source,
            Some(ferric_browser_core::MixedPolicySource::Profile)
        );
        assert_eq!(
            policy.page_keys_source,
            Some(ferric_browser_core::MixedPolicySource::Site)
        );
        assert!(policy.page_keys.iter().any(|key| key.as_str() == "/"));

        let mut runtime = RuntimeOverrides::default();
        runtime
            .set_literal("input.page_keys", "[]")
            .expect("runtime clear");
        let policy = resolve_for_layers(
            &base,
            &profile,
            &runtime,
            &RuntimeOverrides::default(),
            &RuntimeOverrides::default(),
            &effective,
            "https://docs.example.test/a",
        )
        .expect("runtime policy");
        assert_eq!(
            policy.page_keys_source,
            Some(ferric_browser_core::MixedPolicySource::Runtime)
        );
        assert!(policy.page_keys.is_empty());
        assert_eq!(
            base.input.shortcut_policy,
            InputShortcutPolicy::BrowserFirst
        );
    }

    #[test]
    fn synchronous_route_preserves_pending_browser_count_and_keychain() {
        let mut rust = BrowserUiRust::default();
        let global = MixedPolicyLayer::new(Some("mixed"), Some(vec!["i".into()]));
        rust.mixed_policy =
            resolve_mixed_key_policy(Some(&global), None, None, None).expect("valid page mapping");

        let idle = decision_for_key(&rust, Mode::Normal, "i").expect("logical key");
        assert_eq!(idle.reason, MixedKeyDecisionReason::ExactPageKey);

        {
            let bindings = rust.bindings.as_mut().expect("default bindings");
            let _ = bindings.feed("g", 10);
            assert!(bindings.has_pending_keychain());
        }
        let continuation = decision_for_key(&rust, Mode::Normal, "i").expect("logical key");
        assert_eq!(
            continuation.reason,
            MixedKeyDecisionReason::ActiveBrowserKeychain
        );
        assert_eq!(continuation.owner, ferric_browser_core::InputOwner::Browser);

        {
            let bindings = rust.bindings.as_mut().expect("default bindings");
            bindings.reset();
            let _ = bindings.feed("3", 20);
            assert!(bindings.has_pending_count());
        }
        let count_continuation = decision_for_key(&rust, Mode::Normal, "i").expect("logical key");
        assert_eq!(
            count_continuation.reason,
            MixedKeyDecisionReason::ActiveBrowserCount
        );
        assert_eq!(
            count_continuation.owner,
            ferric_browser_core::InputOwner::Browser
        );
    }
}
