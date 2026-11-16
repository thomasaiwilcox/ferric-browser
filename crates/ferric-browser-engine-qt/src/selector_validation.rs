//! Browser-owned selector syntax validation request construction.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use ferric_browser_config::{
    Config, ConfigurationSources, HintSelectorFamily, HintSelectorPolicy, ProfilesConfig,
    RuntimeOverrides, SiteRule,
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
    let mut seen = SelectorOccurrences::default();
    append_layers(&mut items, &mut seen, layers);
    items
}

/// Exact syntax-source occurrences before deduplication. Paths stay private to
/// the browser; equality never relies on diagnostic strings or a hash alone.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct SelectorValidationInventory {
    documents: Vec<SelectorDocumentInventory>,
    // Fixed base/profile/runtime/command-line/temporary order. Typed raw rules
    // retain indices, IDs and fields independently of diagnostic formatting.
    layers: [SelectorLayerInventory; 5],
    pub(super) items: Vec<SelectorValidationItem>,
}

#[derive(Clone, Debug, PartialEq)]
struct SelectorDocumentInventory {
    path: PathBuf,
    selectors: Option<BTreeMap<HintSelectorFamily, ferric_browser_config::AuthoredHintSelectors>>,
    rules: Option<Vec<SiteRule>>,
}
#[derive(Clone, Debug, PartialEq)]
struct SelectorLayerInventory {
    selectors: BTreeMap<HintSelectorFamily, HintSelectorPolicy>,
    rules: Vec<SiteRule>,
}
impl SelectorLayerInventory {
    fn overrides(input: &RuntimeOverrides) -> Self {
        Self {
            selectors: input
                .settings
                .iter()
                .filter_map(|(key, value)| {
                    Some((family_for_key(key)?, value.clone().try_into().ok()?))
                })
                .collect(),
            rules: input.site_rules.clone(),
        }
    }
}

pub(super) fn selector_validation_inventory(
    layers: &ConfigurationLayers,
) -> SelectorValidationInventory {
    let mut items = Vec::new();
    let mut seen = SelectorOccurrences {
        deduplicate: false,
        ..SelectorOccurrences::default()
    };
    append_layers(&mut items, &mut seen, layers);
    SelectorValidationInventory {
        documents: layers
            .base_authored
            .documents()
            .iter()
            .map(|document| SelectorDocumentInventory {
                path: document.path().to_owned(),
                selectors: document.hints().and_then(|hints| hints.selectors.clone()),
                rules: document.site_rules().map(<[SiteRule]>::to_vec),
            })
            .collect(),
        layers: [
            SelectorLayerInventory {
                selectors: layers.base.hints.selectors.clone(),
                rules: layers.base.site_rules.clone(),
            },
            SelectorLayerInventory::overrides(&layers.profile),
            SelectorLayerInventory::overrides(&layers.runtime),
            SelectorLayerInventory::overrides(&layers.command_line),
            SelectorLayerInventory::overrides(&layers.temporary),
        ],
        items,
    }
}

fn append_layers(
    items: &mut Vec<SelectorValidationItem>,
    seen: &mut SelectorOccurrences,
    layers: &ConfigurationLayers,
) {
    append_authored_sources(items, seen, &layers.base_authored);
    append_config(items, seen, &layers.base, "base");
    append_overrides(items, seen, &layers.profile, "profile");
    append_overrides(items, seen, &layers.runtime, "runtime");
    append_overrides(items, seen, &layers.command_line, "command-line");
    append_overrides(items, seen, &layers.temporary, "temporary");
}

/// Validation requests deduplicate literal CSS; inventories retain every source
/// occurrence so a duplicate cannot erase ownership of another layer or field.
struct SelectorOccurrences {
    deduplicate: bool,
    seen: BTreeSet<String>,
}
impl Default for SelectorOccurrences {
    fn default() -> Self {
        Self {
            deduplicate: true,
            seen: BTreeSet::new(),
        }
    }
}
impl SelectorOccurrences {
    fn insert(&mut self, selector: String) -> bool {
        !self.deduplicate || self.seen.insert(selector)
    }
}

pub(super) fn selector_validation_items_for_check(
    config: &Config,
    profiles: Option<&ProfilesConfig>,
    authored: &ConfigurationSources,
) -> Vec<SelectorValidationItem> {
    let mut items = Vec::new();
    let mut seen = SelectorOccurrences::default();
    append_authored_sources(&mut items, &mut seen, authored);
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

fn append_authored_sources(
    output: &mut Vec<SelectorValidationItem>,
    seen: &mut SelectorOccurrences,
    authored: &ConfigurationSources,
) {
    for (index, document) in authored.documents().iter().enumerate() {
        let source = format!("base document {}", index + 1);
        if let Some(hints) = document.hints()
            && let Some(families) = &hints.selectors
        {
            for (family, policy) in families {
                append_selector_strings(
                    output,
                    seen,
                    *family,
                    policy.selectors.as_deref().unwrap_or_default(),
                    &source,
                );
            }
        }
        for rule in document.site_rules().unwrap_or_default() {
            append_site_rule(output, seen, rule, &source);
        }
    }
}

fn append_config(
    output: &mut Vec<SelectorValidationItem>,
    seen: &mut SelectorOccurrences,
    config: &Config,
    source: &str,
) {
    for (family, policy) in &config.hints.selectors {
        append_policy(output, seen, *family, policy, source);
    }
    for rule in &config.site_rules {
        append_site_rule(output, seen, rule, source);
    }
}

fn append_overrides(
    output: &mut Vec<SelectorValidationItem>,
    seen: &mut SelectorOccurrences,
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
        append_site_rule(output, seen, rule, source);
    }
}

fn append_site_rule(
    output: &mut Vec<SelectorValidationItem>,
    seen: &mut SelectorOccurrences,
    rule: &SiteRule,
    source: &str,
) {
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

    if let Some(policy) = &rule.interaction {
        for (index, field) in policy.selector_sources().into_iter().enumerate() {
            if seen.insert(field.selector.to_owned()) {
                output.push(SelectorValidationItem {
                    family: "interaction".into(),
                    index,
                    selector: field.selector.to_owned(),
                    rule: format!("{source} site rule {} interaction {}", rule.id, field.path),
                });
            }
        }
    }
}

fn append_policy(
    output: &mut Vec<SelectorValidationItem>,
    seen: &mut SelectorOccurrences,
    family: HintSelectorFamily,
    policy: &HintSelectorPolicy,
    rule: &str,
) {
    append_selector_strings(output, seen, family, &policy.selectors, rule);
}

fn append_selector_strings(
    output: &mut Vec<SelectorValidationItem>,
    seen: &mut SelectorOccurrences,
    family: HintSelectorFamily,
    selectors: &[String],
    rule: &str,
) {
    for (index, selector) in selectors.iter().enumerate() {
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
    fn config_check_validates_sources_hidden_by_parent_array_replacement() {
        let loaded = ferric_browser_config::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../ferric-browser-config/tests/fixtures/site-policy/includes/root.toml"
        ))
        .unwrap();
        assert!(loaded.config.site_rules.is_empty());
        assert!(
            loaded.config.hints.selectors[&HintSelectorFamily::Links]
                .selectors
                .is_empty()
        );
        let items = selector_validation_items_for_check(&loaded.config, None, &loaded.authored);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].selector, "a[");
        assert_eq!(items[0].family, "links");
        assert_eq!(items[0].rule, "base document 1");
        assert_eq!(items[1].selector, "button.hidden");
        assert_eq!(items[1].family, "interaction");
        assert!(items[1].rule.contains("base document 1 site rule hidden"));
        let mut layers = ConfigurationLayers::new(loaded.config);
        layers.base_authored = loaded.authored;
        assert_eq!(selector_validation_items(&layers), items);
    }

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

        let items = selector_validation_items_for_check(
            &Config::default(),
            Some(&profiles),
            &ConfigurationSources::default(),
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].rule, "profile work");
        assert_eq!(items[0].selector, ".profile-control");
    }

    #[test]
    fn interaction_selectors_include_disabled_and_shadowed_raw_rules() {
        let mut config: Config = toml::from_str(include_str!(
            "../../ferric-browser-config/tests/fixtures/site-policy/frame-shadow-grid.toml"
        ))
        .unwrap();
        config.site_rules[0].interaction.as_mut().unwrap().enabled = Some(false);
        ferric_browser_config::validate(&config).unwrap();
        let mut higher = config.site_rules[0].clone();
        higher.pattern = "https://other.example.test/*".into();
        higher
            .interaction
            .as_mut()
            .unwrap()
            .collections
            .as_mut()
            .unwrap()[0]
            .primary_selector = Some(".higher-primary".into());
        let mut layers = ConfigurationLayers::new(config);
        layers.temporary.site_rules.push(higher);
        let items = selector_validation_items(&layers);
        assert_eq!(items.len(), 6);
        assert!(items.iter().all(|item| item.family == "interaction"));
        for selector in [
            "iframe#fixture-frame",
            "catalog-shell",
            "#fixture-grid",
            "video-card",
            "a.primary",
            ".higher-primary",
        ] {
            assert!(
                items.iter().any(|item| item.selector == selector),
                "{selector}"
            );
        }
        assert!(items.iter().any(|item| item.selector == "a.primary" && item.rule.starts_with("base site rule")));
        assert!(items.iter().any(|item| item.selector == ".higher-primary"
            && item.rule.starts_with("temporary site rule")));
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
