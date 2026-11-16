//! Active Mixed-key policy resolution and synchronous routing projection.

use cxx_qt::CxxQtType;
use ferric_browser_config::{Config, InputShortcutPolicy, RuntimeOverrides, matching_site_rules};
use ferric_browser_core::{
    BindingResolver, BrowserKeySequenceState, EffectiveMixedKeyPolicy, LogicalKeyToken,
    MixedKeyDecision, MixedKeyDecisionReason, MixedPolicyLayer, Mode, resolve_mixed_key_policy,
};

use crate::{BrowserUiRust, ConfigurationLayers, Pin, QString, Value, qobject};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum MixedPolicyState {
    Ready(EffectiveMixedKeyPolicy),
    Unavailable(String),
    Pending,
}

impl Default for MixedPolicyState {
    fn default() -> Self {
        Self::Unavailable("policy preparation has not completed".into())
    }
}

impl MixedPolicyState {
    pub(super) fn from_result(result: Result<EffectiveMixedKeyPolicy, String>) -> Self {
        match result {
            Ok(policy) => Self::Ready(policy),
            Err(error) => Self::Unavailable(error),
        }
    }

    pub(super) fn policy(&self) -> Option<&EffectiveMixedKeyPolicy> {
        match self {
            Self::Ready(policy) => Some(policy),
            Self::Unavailable(_) | Self::Pending => None,
        }
    }

    pub(super) fn error(&self) -> Option<&str> {
        match self {
            Self::Ready(_) => None,
            Self::Unavailable(error) => Some(error),
            Self::Pending => Some("selected context preparation is pending"),
        }
    }

    pub(super) fn status(&self) -> &'static str {
        match self {
            Self::Ready(_) => "ready",
            Self::Unavailable(_) | Self::Pending => "unavailable",
        }
    }
}

fn layer_from_overrides(overrides: &RuntimeOverrides) -> Result<Option<MixedPolicyLayer>, String> {
    let policy = overrides
        .settings
        .get("input.shortcut_policy")
        .map(|value| {
            value
                .clone()
                .try_into::<InputShortcutPolicy>()
                .map(|policy| policy.as_str().to_owned())
                .map_err(|_| "input.shortcut_policy override is invalid".to_owned())
        })
        .transpose()?;
    layer_from_settings(&overrides.settings, policy)
}

fn layer_from_settings(
    settings: &std::collections::BTreeMap<String, toml::Value>,
    policy: Option<String>,
) -> Result<Option<MixedPolicyLayer>, String> {
    let page_keys = settings
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
        // Site `set` uses the existing strict string grammar. Runtime layers
        // use the Config enum codec, which also accepts TOML unit-enum tables.
        let policy = rule
            .set
            .get("input.shortcut_policy")
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| "site input.shortcut_policy must be a string".to_owned())
            })
            .transpose()?;
        if let Some(layer) = layer_from_settings(&rule.set, policy)? {
            merge_layer(&mut site, Some(layer));
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
    let sequence = sequence_state(rust);
    if let Some(policy) = rust.active_mixed_policy().policy() {
        return Some(policy.decide(mode, &token, sequence));
    }
    // These owners do not depend on a resolved Mixed policy. In particular,
    // a count/keychain continuation must not open the command/search UI.
    let reason = if mode != Mode::Normal {
        MixedKeyDecisionReason::ExplicitMode
    } else if matches!(
        token.as_str(),
        ":" | "Escape" | "Ctrl+Escape" | "Ctrl+Shift+Escape"
    ) {
        MixedKeyDecisionReason::ReservedControl
    } else {
        match sequence {
            BrowserKeySequenceState::ActiveCount => MixedKeyDecisionReason::ActiveBrowserCount,
            BrowserKeySequenceState::ActiveKeychain => {
                MixedKeyDecisionReason::ActiveBrowserKeychain
            }
            BrowserKeySequenceState::Idle => return None,
        }
    };
    Some(MixedKeyDecision {
        owner: ferric_browser_core::InputOwner::Browser,
        reason,
        shortcut_policy_source: None,
        page_keys_source: None,
    })
}

impl BrowserUiRust {
    /// A retained comparison value cannot grant admission while selected
    /// context work is pending. This read never compiles or resets a sequence.
    pub(super) fn active_mixed_policy(&self) -> &MixedPolicyState {
        if self.interaction.interaction_policy_refresh_pending {
            &MixedPolicyState::Pending
        } else {
            &self.mixed_policy
        }
    }

    /// Replace even a failed projection: a previous site's policy has no
    /// authority over the new context. Existing native gestures keep their
    /// independently frozen ownership in the native router.
    pub(super) fn replace_mixed_policy(&mut self, policy: MixedPolicyState) -> bool {
        if self.mixed_policy == policy {
            return false;
        }
        self.mixed_policy = policy;
        self.policy_revision = self.policy_revision.wrapping_add(1);
        if let Some(bindings) = self.bindings.as_mut() {
            bindings.reset();
        }
        self.macro_key_prefix = None;
        self.macro_key_started_ms = None;
        true
    }
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
            base_authored: self
                .rust()
                .state
                .as_ref()
                .map_or_else(std::sync::Arc::default, |state| {
                    state.configuration_layers().base_authored.clone()
                }),
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
    ) -> Result<EffectiveMixedKeyPolicy, String> {
        let effective = serde_json::from_value::<Config>(candidate.clone())
            .map_err(|error| format!("candidate configuration is invalid: {error}"))?;
        resolve_for_layers(
            &layers.base,
            &layers.profile,
            &layers.runtime,
            &layers.command_line,
            &layers.temporary,
            &effective,
            &self.rust().current_url.to_string(),
        )
    }

    pub(super) fn install_mixed_policy(mut self: Pin<&mut Self>, policy: MixedPolicyState) {
        let changed = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .replace_mixed_policy(policy);
        if changed {
            self.as_mut().set_binding_overlay(QString::default());
        }
        // Cold projection does not own command/reload feedback. Availability
        // and its reason are exposed by diagnostics; explicit operations add
        // failures to their own receipts. No writes may follow the overlay
        // notification, which can reenter with a newer selected context.
    }

    pub(super) fn clipboard_shortcut_browser_owned(self: Pin<&mut Self>, key: &QString) -> bool {
        let binding = self.as_ref();
        // Reserve a selected page command at ShortcutOverride, including denied
        // routes that must be consumed. Submission still happens only on down.
        // Raw and ordinary clipboard gestures retain their native owner.
        if binding.current_page_key_route(key).is_some() {
            return true;
        }
        let key = key.to_string();
        let this = binding.rust();
        match this.core_mode {
            Mode::Normal => {
                let decision = decision_for_key(this, Mode::Normal, &key);
                match decision.map(|value| value.reason) {
                    Some(MixedKeyDecisionReason::ExactPageKey) => false,
                    Some(
                        MixedKeyDecisionReason::ActiveBrowserCount
                        | MixedKeyDecisionReason::ActiveBrowserKeychain,
                    ) => true,
                    _ => {
                        this.macro_key_prefix.is_some()
                            || this
                                .bindings
                                .as_ref()
                                .is_some_and(|bindings| bindings.has_initial_key(&key))
                    }
                }
            }
            Mode::Insert | Mode::InsertLocked | Mode::PassThrough | Mode::PassThroughOnce => false,
            Mode::Caret => key != "Ctrl+c",
            _ => true,
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
            None if self.as_ref().rust().active_mixed_policy().error().is_some() => "unavailable",
            _ => "browser",
        };
        QString::from(route)
    }
}

#[cfg(test)]
mod tests {
    use ferric_browser_config::{InputShortcutPolicy, SiteRule};
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn loaded_enum_forms_match_canonical_layer_normalization() {
        let mut policies = Vec::new();
        for policy in ["\"mixed\"", "{mixed={}}"] {
            let text = format!(
                "schema_version={}\n[settings]\n\"input.shortcut_policy\"={policy}\n\"input.page_keys\"=['k']\n",
                RuntimeOverrides::default().schema_version
            );
            let runtime: RuntimeOverrides = toml::from_str(&text).unwrap();
            let mut layers = ConfigurationLayers::new(Config::default());
            layers.runtime = runtime;
            let effective = layers.resolve().unwrap();
            assert_eq!(effective.input.shortcut_policy, InputShortcutPolicy::Mixed);
            let resolved = resolve_for_layers(
                &layers.base,
                &layers.profile,
                &layers.runtime,
                &layers.command_line,
                &layers.temporary,
                &effective,
                "https://example.test/",
            )
            .unwrap();
            assert_eq!(
                resolved.shortcut_policy,
                ferric_browser_core::ShortcutPolicy::Mixed
            );
            assert!(
                resolved
                    .page_keys
                    .contains(&LogicalKeyToken::parse("k").unwrap())
            );
            policies.push(resolved);
        }
        assert_eq!(policies[0], policies[1]);
    }

    #[test]
    fn malformed_unit_enum_remains_rejected_by_config_and_engine() {
        let text = format!(
            "schema_version={}\n[settings]\n\"input.shortcut_policy\"={{mixed={{unexpected=true}}}}\n",
            RuntimeOverrides::default().schema_version
        );
        let runtime: RuntimeOverrides = toml::from_str(&text).unwrap();
        let mut layers = ConfigurationLayers::new(Config::default());
        layers.runtime = runtime;
        assert!(layers.resolve().is_err());
        assert!(layer_from_overrides(&layers.runtime).is_err());
    }

    #[test]
    fn loaded_enum_normalization_preserves_each_sparse_source() {
        for source in ["profile", "runtime", "cli", "temporary"] {
            let mut layers = ConfigurationLayers::new(Config::default());
            let loaded: RuntimeOverrides = toml::from_str(&format!(
                "schema_version={}\n[settings]\n\"input.shortcut_policy\"={{mixed={{}}}}\n\"input.page_keys\"=['k']\n",
                RuntimeOverrides::default().schema_version
            )).unwrap();
            match source {
                "profile" => layers.profile = loaded,
                "runtime" => layers.runtime = loaded,
                "cli" => layers.command_line = loaded,
                "temporary" => layers.temporary = loaded,
                _ => unreachable!(),
            }
            let effective = layers.resolve().unwrap();
            let resolve = |layers: &ConfigurationLayers| {
                resolve_for_layers(
                    &layers.base,
                    &layers.profile,
                    &layers.runtime,
                    &layers.command_line,
                    &layers.temporary,
                    &effective,
                    "https://example.test/",
                )
                .unwrap()
            };
            let policy = resolve(&layers);
            let expected = if source == "profile" {
                ferric_browser_core::MixedPolicySource::Profile
            } else {
                ferric_browser_core::MixedPolicySource::Runtime
            };
            assert_eq!(policy.shortcut_policy_source, Some(expected));
            assert_eq!(policy.page_keys_source, Some(expected));
            assert_eq!(policy.page_keys.len(), 1);
            // Absent higher leaves inherit; an explicit empty list clears.
            layers
                .temporary
                .settings
                .insert("input.page_keys".into(), toml::Value::Array(vec![]));
            assert!(resolve(&layers).page_keys.is_empty());
        }
    }

    #[test]
    fn unavailable_replaces_previous_site_policy_and_recovers_without_stale_sequences() {
        let mut rust = BrowserUiRust::default();
        assert_eq!(rust.mixed_policy.status(), "unavailable");
        assert!(rust.mixed_policy.policy().is_none());
        let global = MixedPolicyLayer::new(Some("mixed"), Some(vec!["k".into()]));
        let ready = MixedPolicyState::Ready(
            resolve_mixed_key_policy(Some(&global), None, None, None).unwrap(),
        );
        assert!(rust.replace_mixed_policy(ready.clone()));
        assert_eq!(
            decision_for_key(&rust, Mode::Normal, "k").unwrap().reason,
            MixedKeyDecisionReason::ExactPageKey
        );
        let bindings = rust.bindings.as_mut().unwrap();
        let _ = bindings.feed("g", 10);
        assert!(bindings.has_pending_keychain());
        let revision = rust.policy_revision;
        // Historical/fault-injected resolver failure: public Config admission
        // still rejects malformed data before it reaches the active owner.
        let unavailable =
            MixedPolicyState::from_result(Err("fault-injected resolution failure".into()));
        assert!(rust.replace_mixed_policy(unavailable.clone()));
        assert_eq!(rust.policy_revision, revision.wrapping_add(1));
        assert!(rust.mixed_policy.policy().is_none());
        assert!(
            rust.mixed_policy
                .error()
                .unwrap()
                .contains("fault-injected")
        );
        assert!(decision_for_key(&rust, Mode::Normal, "k").is_none());
        assert!(!rust.bindings.as_ref().unwrap().has_pending_keychain());
        assert!(!rust.replace_mixed_policy(unavailable));
        assert_eq!(rust.policy_revision, revision.wrapping_add(1));
        assert!(rust.replace_mixed_policy(ready));
        assert_eq!(rust.policy_revision, revision.wrapping_add(2));
        assert_eq!(
            decision_for_key(&rust, Mode::Normal, "k").unwrap().reason,
            MixedKeyDecisionReason::ExactPageKey
        );
    }

    #[test]
    fn malformed_site_leaves_are_not_silently_inherited() {
        for (key, value) in [
            (
                "input.shortcut_policy",
                toml::Value::Table(toml::Table::from_iter([(
                    "mixed".into(),
                    toml::Value::Table(toml::Table::new()),
                )])),
            ),
            ("input.page_keys", toml::Value::Boolean(false)),
        ] {
            let base = Config::default();
            let mut effective = base.clone();
            effective.site_rules.push(SiteRule {
                id: "historical-invalid".into(),
                pattern: "https://example.test/*".into(),
                priority: 0,
                interaction: None,
                set: BTreeMap::from([(key.into(), value)]),
            });
            let empty = RuntimeOverrides::default();
            assert!(
                resolve_for_layers(
                    &base,
                    &empty,
                    &empty,
                    &empty,
                    &empty,
                    &effective,
                    "https://example.test/"
                )
                .is_err()
            );
        }
    }

    #[test]
    fn unavailable_preserves_browser_sequences_explicit_modes_and_recovery() {
        let mut rust = BrowserUiRust::default();
        assert!(decision_for_key(&rust, Mode::Normal, "k").is_none());
        for key in [":", "Escape", "Ctrl+Escape", "Ctrl+Shift+Escape"] {
            let decision = decision_for_key(&rust, Mode::Normal, key).unwrap();
            assert_eq!(decision.reason, MixedKeyDecisionReason::ReservedControl);
            assert_eq!(decision.owner, ferric_browser_core::InputOwner::Browser);
            assert_eq!(decision.shortcut_policy_source, None);
        }
        for mode in [
            Mode::Insert,
            Mode::InsertLocked,
            Mode::PassThrough,
            Mode::PassThroughOnce,
        ] {
            assert_eq!(
                decision_for_key(&rust, mode, "k").unwrap().reason,
                MixedKeyDecisionReason::ExplicitMode
            );
        }
        let _ = rust.bindings.as_mut().unwrap().feed("3", 10);
        assert_eq!(
            decision_for_key(&rust, Mode::Normal, "/").unwrap().reason,
            MixedKeyDecisionReason::ActiveBrowserCount
        );
        rust.bindings.as_mut().unwrap().reset();
        let _ = rust.bindings.as_mut().unwrap().feed("g", 20);
        assert_eq!(
            decision_for_key(&rust, Mode::Normal, "/").unwrap().reason,
            MixedKeyDecisionReason::ActiveBrowserKeychain
        );
    }

    #[test]
    fn pending_selected_context_closes_retained_policy_without_resetting_browser_sequence() {
        let mut rust = BrowserUiRust::default();
        let global = MixedPolicyLayer::new(Some("mixed"), Some(vec!["k".into()]));
        let ready = MixedPolicyState::Ready(
            resolve_mixed_key_policy(Some(&global), None, None, None).unwrap(),
        );
        rust.replace_mixed_policy(ready.clone());
        let revision = rust.policy_revision;
        rust.interaction.interaction_policy_refresh_pending = true;
        assert!(rust.active_mixed_policy().policy().is_none());
        assert!(decision_for_key(&rust, Mode::Normal, "k").is_none());
        let _ = rust.bindings.as_mut().unwrap().feed("3", 10);
        assert_eq!(
            decision_for_key(&rust, Mode::Normal, "/").unwrap().reason,
            MixedKeyDecisionReason::ActiveBrowserCount
        );
        rust.interaction.interaction_policy_refresh_pending = false;
        assert!(!rust.replace_mixed_policy(ready));
        assert_eq!(rust.policy_revision, revision);
        assert!(rust.bindings.as_ref().unwrap().has_pending_count());
    }

    #[test]
    fn unavailable_browser_guards_agree_with_core_without_granting_page_ownership() {
        let mut rust = BrowserUiRust::default();
        let global = MixedPolicyLayer::new(Some("mixed"), Some(vec!["k".into()]));
        let ready = resolve_mixed_key_policy(Some(&global), None, None, None).unwrap();
        for prefix in [None, Some("3"), Some("g")] {
            rust.bindings.as_mut().unwrap().reset();
            if let Some(prefix) = prefix {
                let _ = rust.bindings.as_mut().unwrap().feed(prefix, 10);
            }
            for mode in [
                Mode::Normal,
                Mode::Insert,
                Mode::Hint,
                Mode::PassThrough,
                Mode::PassThroughOnce,
            ] {
                for key in ["k", "/", ":", "Escape", "Ctrl+Escape", "Ctrl+Shift+Escape"] {
                    let expected = ready.decide(
                        mode,
                        &LogicalKeyToken::parse(key).unwrap(),
                        sequence_state(&rust),
                    );
                    let actual = decision_for_key(&rust, mode, key);
                    if matches!(
                        expected.reason,
                        MixedKeyDecisionReason::ExplicitMode
                            | MixedKeyDecisionReason::ReservedControl
                            | MixedKeyDecisionReason::ActiveBrowserCount
                            | MixedKeyDecisionReason::ActiveBrowserKeychain
                    ) {
                        let actual = actual.unwrap();
                        assert_eq!(actual.reason, expected.reason);
                        assert_eq!(actual.owner, ferric_browser_core::InputOwner::Browser);
                        assert_eq!(actual.page_keys_source, None);
                    } else {
                        assert!(actual.is_none());
                    }
                }
            }
        }
    }

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
            interaction: None,
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
        rust.mixed_policy = MixedPolicyState::Ready(
            resolve_mixed_key_policy(Some(&global), None, None, None).expect("valid page mapping"),
        );

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
