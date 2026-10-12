//! Browser-owned selector syntax validation request construction.

use std::collections::BTreeSet;

use ferric_browser_config::{
    Config, HintSelectorFamily, HintSelectorPolicy, ProfilesConfig, RuntimeOverrides,
};
use serde::{Deserialize, Serialize};

use crate::ConfigurationLayers;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct SelectorValidationItem {
    pub(super) family: String,
    pub(super) index: usize,
    pub(super) selector: String,
    pub(super) rule: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct SelectorValidationResult {
    pub(super) ok: bool,
    #[serde(default)]
    pub(super) family: String,
    #[serde(default)]
    pub(super) index: usize,
    #[serde(default)]
    pub(super) rule: String,
    #[serde(default)]
    pub(super) reason: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SelectorCallbackDecision {
    Accept,
    Reject,
    Stale,
}

pub(super) const fn selector_callback_decision(
    callback_generation: u64,
    pending_generation: u64,
    expected_revision: u64,
    current_revision: u64,
    parser_succeeded: bool,
) -> SelectorCallbackDecision {
    if callback_generation != pending_generation || expected_revision != current_revision {
        SelectorCallbackDecision::Stale
    } else if parser_succeeded {
        SelectorCallbackDecision::Accept
    } else {
        SelectorCallbackDecision::Reject
    }
}

pub(super) fn selector_validation_items(
    layers: &ConfigurationLayers,
) -> Vec<SelectorValidationItem> {
    let mut items = Vec::new();
    let mut seen = BTreeSet::new();
    append_config(&mut items, &mut seen, &layers.base, "base");
    append_overrides(&mut items, &mut seen, &layers.profile, "profile");
    append_overrides(&mut items, &mut seen, &layers.runtime, "runtime");
    append_overrides(&mut items, &mut seen, &layers.command_line, "command-line");
    append_overrides(&mut items, &mut seen, &layers.temporary, "temporary");
    items
}

pub(super) fn selector_validation_items_for_check(
    config: &Config,
    profiles: Option<&ProfilesConfig>,
) -> Vec<SelectorValidationItem> {
    let mut items = Vec::new();
    let mut seen = BTreeSet::new();
    append_config(&mut items, &mut seen, config, "base");
    if let Some(profiles) = profiles {
        for profile in &profiles.profiles {
            let overrides = RuntimeOverrides {
                settings: profile.overrides.clone(),
                ..RuntimeOverrides::default()
            };
            append_overrides(
                &mut items,
                &mut seen,
                &overrides,
                &format!("profile {}", profile.name),
            );
        }
    }
    items
}

fn append_config(
    output: &mut Vec<SelectorValidationItem>,
    seen: &mut BTreeSet<String>,
    config: &Config,
    source: &str,
) {
    for (family, policy) in &config.hints.selectors {
        append_policy(output, seen, *family, policy, source);
    }
    for rule in &config.site_rules {
        for (key, value) in &rule.set {
            let Some(family) = family_for_key(key) else {
                continue;
            };
            if let Ok(policy) = value.clone().try_into::<HintSelectorPolicy>() {
                append_policy(
                    output,
                    seen,
                    family,
                    &policy,
                    &format!("{source} site rule {}", rule.id),
                );
            }
        }
    }
}

fn append_overrides(
    output: &mut Vec<SelectorValidationItem>,
    seen: &mut BTreeSet<String>,
    overrides: &RuntimeOverrides,
    source: &str,
) {
    for (key, value) in &overrides.settings {
        let Some(family) = family_for_key(key) else {
            continue;
        };
        if let Ok(policy) = value.clone().try_into::<HintSelectorPolicy>() {
            append_policy(output, seen, family, &policy, source);
        }
    }
    for rule in &overrides.site_rules {
        for (key, value) in &rule.set {
            let Some(family) = family_for_key(key) else {
                continue;
            };
            if let Ok(policy) = value.clone().try_into::<HintSelectorPolicy>() {
                append_policy(
                    output,
                    seen,
                    family,
                    &policy,
                    &format!("{source} site rule {}", rule.id),
                );
            }
        }
    }
}

fn append_policy(
    output: &mut Vec<SelectorValidationItem>,
    seen: &mut BTreeSet<String>,
    family: HintSelectorFamily,
    policy: &HintSelectorPolicy,
    rule: &str,
) {
    for (index, selector) in policy.selectors.iter().enumerate() {
        if seen.insert(selector.clone()) {
            output.push(SelectorValidationItem {
                family: family.as_str().to_owned(),
                index,
                selector: selector.clone(),
                rule: rule.to_owned(),
            });
        }
    }
}

fn family_for_key(key: &str) -> Option<HintSelectorFamily> {
    let name = key.strip_prefix("hints.selectors.")?;
    if name.contains('.') {
        return None;
    }
    HintSelectorFamily::parse(name)
}

#[cfg(test)]
mod tests {
    use ferric_browser_config::{HintSelectorMode, HintSelectorPolicy};

    use super::*;

    #[test]
    fn validation_items_cover_layers_and_deduplicate_css_syntax_checks() {
        let mut layers = ConfigurationLayers::new(Config::default());
        layers.base.hints.selectors.insert(
            HintSelectorFamily::Links,
            HintSelectorPolicy {
                mode: HintSelectorMode::Replace,
                selectors: vec!["a[href]".into()],
            },
        );
        layers
            .profile
            .set_literal(
                "hints.selectors.buttons",
                "{ mode = 'extend', selectors = ['button', 'a[href]'] }",
            )
            .unwrap();

        let items = selector_validation_items(&layers);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].selector, "a[href]");
        assert_eq!(items[0].rule, "base");
        assert_eq!(items[1].selector, "button");
        assert_eq!(items[1].family, "buttons");
    }

    #[test]
    fn config_check_includes_every_declared_profile_policy() {
        let policy = HintSelectorPolicy {
            mode: HintSelectorMode::Replace,
            selectors: vec![".profile-control".into()],
        };
        let profiles = ProfilesConfig {
            schema_version: 3,
            profiles: vec![ferric_browser_config::ProfileDefinition {
                name: "work".into(),
                label: "Work".into(),
                default: false,
                overrides: std::collections::BTreeMap::from([(
                    "hints.selectors.all".into(),
                    toml::Value::try_from(policy).expect("encode profile selector policy"),
                )]),
            }],
        };

        let items = selector_validation_items_for_check(&Config::default(), Some(&profiles));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].rule, "profile work");
        assert_eq!(items[0].selector, ".profile-control");
    }

    #[test]
    fn selector_callback_decision_rejects_stale_and_invalid_candidates() {
        assert_eq!(
            selector_callback_decision(7, 8, 3, 3, true),
            SelectorCallbackDecision::Stale
        );
        assert_eq!(
            selector_callback_decision(8, 8, 3, 4, true),
            SelectorCallbackDecision::Stale
        );
        assert_eq!(
            selector_callback_decision(8, 8, 3, 3, false),
            SelectorCallbackDecision::Reject
        );
        assert_eq!(
            selector_callback_decision(8, 8, 3, 3, true),
            SelectorCallbackDecision::Accept
        );
    }
}
