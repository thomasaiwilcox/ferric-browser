//! Typed, data-only configuration for `Ferric Browser`.
//!
//! This crate deliberately has no Qt, display-server, network, or command
//! execution dependencies. Files are parsed into a candidate configuration and
//! only committed by [`ConfigStore`] after validation succeeds.

#![allow(clippy::missing_errors_doc)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use ferric_browser_core::{MixedPolicyLayer, resolve_mixed_key_policy};
use serde::{Deserialize, Serialize};

mod appearance;
mod binding_config;
mod browser_config;
mod config_model;
mod configuration_diagnostic;
mod configuration_sources;
pub use configuration_diagnostic::MAX_CONFIGURATION_DIAGNOSTIC_BYTES;
pub use configuration_sources::{
    AuthoredHintSelectors, AuthoredHints, ConfigurationSourceDocument, ConfigurationSources,
};
mod download_directory;
mod interaction_capabilities;
mod interaction_codebook;
pub use interaction_codebook::CompiledInvocationHintCodebook;
mod interaction_effective;
mod interaction_keys;
mod interaction_name;
mod interaction_predicate_match;
pub use interaction_predicate_match::{
    CompiledInteractionAttributeTest, CompiledInteractionHrefMatch, CompiledInteractionPredicates,
    CompiledInteractionRoleMatch, InteractionAttributeEvidence, InteractionPredicateEvidenceError,
    InteractionPredicateFacts, InteractionPredicateWork, MAX_INTERACTION_HREF_EVIDENCE_BYTES,
    MAX_INTERACTION_PREDICATE_STEPS,
};
mod interaction_policy;
pub use interaction_capabilities::{
    NativeReceiverScopeRequirement, NativeRemapCapabilityRequirement, PageKeyDestinationClass,
};
pub use interaction_effective::*;
pub use interaction_keys::{CompiledInteractionKeys, CompiledInteractionRemap};
pub use interaction_name::{
    CompiledInteractionNameMatch, InteractionNameEvidence, InteractionNameEvidenceError,
    InteractionNameOperator, MAX_INTERACTION_NAME_EVIDENCE_BYTES,
    MAX_INTERACTION_NAME_EVIDENCE_CHARS,
};
mod interaction_predicates;
pub use interaction_policy::*;
mod interaction_roots;
mod interaction_selector;
mod interaction_selectors;
pub use interaction_selectors::InteractionSelectorSource;
mod interaction_validation;
mod site_policy_interchange;
mod site_policy_limits;
pub use site_policy_interchange::{
    SITE_POLICY_EXCHANGE_CAPABILITIES, SitePolicyExchange, SitePolicyExchangeOperation,
};
mod site_policy_rules;
mod site_policy_section;
mod site_policy_shared;
pub use download_directory::DownloadDirectory;
pub use interaction_predicates::{
    InteractionAttributeTest, InteractionHrefMatch, InteractionNameMatch, InteractionNumericRange,
    InteractionPredicates, InteractionTextCase, MAX_INTERACTION_ATTRIBUTE_TESTS,
    MAX_INTERACTION_MATCH_BYTES, validate_interaction_attribute_name,
};
pub use interaction_roots::{
    InteractionPageReceiver, InteractionRoot, InteractionRootStart, InteractionRootStep,
    InteractionRootStepKind, MAX_INTERACTION_RECEIVER_ORIGINS, MAX_INTERACTION_ROOT_STEPS,
    MAX_INTERACTION_SELECTOR_BYTES, is_canonical_interaction_origin,
    validate_interaction_primary_root_steps,
};
pub use interaction_selector::{
    InteractionSelector, InteractionSelectorAttributeOperator, InteractionSelectorError,
    InteractionSelectorRelation, InteractionSelectorStep, InteractionSelectorTest,
};
pub use interaction_validation::StaticInteractionValidation;
pub use site_policy_limits::{
    MAX_SITE_POLICY_IMPORT_OPERATIONS, MAX_SITE_POLICY_INTERCHANGE_BYTES,
};
pub use site_policy_rules::{
    AuthoredSitePolicyArray, ReplacedSitePolicyArray, ResolvedSitePolicyRules,
    RetiredSitePolicyRule, SelectedSitePolicyRule, SelectedSitePolicyRules, ShadowedSitePolicyRule,
    SitePolicySelectionTrace, SitePolicySourceRevisions, SitePolicySurvivorTrace,
    SurvivingSitePolicyRule, resolve_site_policy_rules, select_site_policy_rules,
};
pub use site_policy_section::{SitePolicySection, compose_site_policy_section};
mod routing;
mod runtime_config;
mod selector_config;
mod settings;
mod validation;

pub use appearance::{
    ContrastCheck, ThemeConfig, ThemeContrastReport, ThemePalette, ThemePaletteError, UiConfig,
    contrast_ratio, load_theme_palette, theme_contrast_report,
};
#[cfg(test)]
use appearance::{git_commit_at, load_theme_palette_at, parse_theme_palette};
pub use browser_config::*;
pub use config_model::*;
pub use routing::*;
use routing::{parse_context_pattern, parse_site_pattern};
pub use runtime_config::*;
pub use selector_config::*;
#[cfg(test)]
use settings::SETTING_METADATA;
pub use settings::*;
pub use validation::validate;
use validation::{
    runtime_site_rule_id, validate_contexts, validate_profile_definition, validate_profiles,
    validate_runtime_binding, validate_runtime_key, validate_runtime_overrides,
    validate_runtime_site_pattern, validate_slug,
};

/// Clean-break configuration schema. Earlier files are intentionally refused
/// instead of being partially interpreted by the pre-alpha runtime.
/// Configuration documents prior to v3 belong to the pre-application-layer
/// layout and are intentionally not interpreted by this clean break.
const CURRENT_SCHEMA_VERSION: u64 = 3;
pub const MAX_CONFIGURATION_INCLUDE_DEPTH: usize = 8;
/// Existing outer configuration and per-source site-rule bound.
pub const MAX_SITE_RULES: usize = 256;
pub const MAX_CONFIGURATION_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_SITE_RULE_SETTINGS: usize = 32;
const MAX_INCLUDE_DEPTH: usize = MAX_CONFIGURATION_INCLUDE_DEPTH;
const MAX_TOTAL_BYTES: u64 = MAX_CONFIGURATION_BYTES;
const MAX_RUNTIME_SETTINGS: usize = 256;
const MAX_RUNTIME_KEY_BYTES: usize = 256;
const MAX_RUNTIME_VALUE_BYTES: usize = 16 * 1024;
const RUNTIME_SCHEMA_VERSION: u64 = 3;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteRule {
    pub id: String,
    pub pattern: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub set: BTreeMap<String, toml::Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::interaction_policy::deserialize_present_option"
    )]
    pub interaction: Option<InteractionPolicyPatch>,
}

/// Validates one site rule independently of the surrounding configuration.
/// Runtime-generated rules use this same contract before they can be merged
/// into the effective configuration.
pub fn validate_site_rule(rule: &SiteRule) -> Result<(), ConfigError> {
    if let Some(interaction) = &rule.interaction {
        interaction
            .validate_authored()
            .map_err(|message| ConfigError::validation(format_args!("{message}")))?;
    }
    if validate_slug(&rule.id, "site rule id").is_err() {
        return Err(ConfigError::Validation(
            "site rule IDs must be unique, nonempty, and lowercase slugs".into(),
        ));
    }
    if rule.pattern.trim().is_empty()
        || rule.pattern.len() > 512
        || rule.pattern.chars().any(char::is_control)
        || parse_site_pattern(&rule.pattern).is_err()
    {
        return Err(ConfigError::validation(format_args!(
            "site rule {} has an invalid HTTP(S) pattern",
            rule.id
        )));
    }
    if rule.set.len() > MAX_SITE_RULE_SETTINGS {
        return Err(ConfigError::validation(format_args!(
            "site rule {} has too many settings",
            rule.id
        )));
    }
    let mut selector_rule_bytes = 0_usize;
    for (key, value) in &rule.set {
        let valid = match key.as_str() {
            "input.entry_mode" => value
                .as_str()
                .is_some_and(|value| matches!(value, "normal" | "insert" | "pass-through")),
            "input.shortcut_policy" => value
                .as_str()
                .is_some_and(|value| matches!(value, "browser-first" | "mixed")),
            "input.page_keys" => value.as_array().is_some_and(|values| {
                values.iter().all(toml::Value::is_str)
                    && validate_mixed_policy_layer(
                        None,
                        Some(
                            values
                                .iter()
                                .filter_map(toml::Value::as_str)
                                .map(str::to_owned)
                                .collect(),
                        ),
                    )
                    .is_ok()
            }),
            "hints.proxy_targets" => value
                .as_str()
                .is_some_and(|value| matches!(value, "off" | "validated")),
            "content.javascript" | "content.images" | "content.force_dark" => {
                value.as_bool().is_some()
            }
            "content.autoplay" => value
                .as_str()
                .is_some_and(|value| matches!(value, "engine-default" | "require-gesture")),
            "content.zoom" => {
                value
                    .as_float()
                    .is_some_and(|value| (0.25..=5.0).contains(&value))
                    || value
                        .as_integer()
                        .is_some_and(|value| (1..=5).contains(&value))
            }
            _ if selector_setting_family(key).is_some() => value
                .clone()
                .try_into::<HintSelectorPolicy>()
                .is_ok_and(|policy| policy.validate().is_ok()),
            _ => false,
        };
        if !setting_supports_site_scope(key) || !valid {
            return Err(ConfigError::validation(format_args!(
                "site rule {} has an invalid or unsupported setting {key}",
                rule.id
            )));
        }
        if selector_setting_family(key).is_some() {
            let policy = value
                .clone()
                .try_into::<HintSelectorPolicy>()
                .map_err(|error| {
                    ConfigError::validation(format_args!(
                        "site rule {} has an invalid selector policy for {key}: {error}",
                        rule.id
                    ))
                })?;
            selector_rule_bytes = selector_rule_bytes.saturating_add(policy.selector_bytes());
            if selector_rule_bytes > MAX_SELECTOR_RULE_BYTES {
                return Err(ConfigError::validation(format_args!(
                    "site rule {} exceeds the 16 KiB selector string limit",
                    rule.id
                )));
            }
        }
    }
    Ok(())
}

fn validate_mixed_policy_layer(
    shortcut_policy: Option<&str>,
    page_keys: Option<Vec<String>>,
) -> Result<(), String> {
    let layer = MixedPolicyLayer::new(shortcut_policy, page_keys);
    resolve_mixed_key_policy(None, Some(&layer), None, None)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn set_private_permissions(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn set_private_file_mode(options: &mut fs::OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
}

fn io_error(path: &Path, error: &std::io::Error) -> ConfigError {
    ConfigError::Io {
        path: path.to_owned(),
        message: configuration_diagnostic::bounded_diagnostic(format_args!("{error}")),
    }
}
fn default_true() -> bool {
    true
}
fn default_context_target() -> String {
    "reuse-or-window".into()
}
fn default_context_behavior() -> String {
    "prompt".into()
}
fn default_statusbar() -> String {
    "in-mode".into()
}
fn default_tabs() -> String {
    "multiple".into()
}
fn default_tab_position() -> String {
    "top".into()
}
fn default_font_family() -> String {
    "monospace".into()
}
fn default_font_size() -> f64 {
    10.0
}
fn default_keychain_timeout() -> u64 {
    1_000
}
fn default_count_limit() -> u32 {
    9_999
}
fn default_overlay_delay() -> u64 {
    350
}
fn default_search_engine() -> String {
    "ddg".into()
}
fn default_start_pages() -> Vec<String> {
    vec!["rb://start".into()]
}
fn default_new_tab() -> String {
    "rb://start".into()
}
fn default_builtin() -> String {
    "builtin".into()
}
fn default_related_position() -> String {
    "after-opener".into()
}
fn default_undo_limit() -> u32 {
    100
}
fn default_checkpoint() -> u64 {
    5
}
fn default_retention() -> u32 {
    90
}
fn default_command_limit() -> u32 {
    1_000
}
fn default_zoom() -> f64 {
    1.0
}
fn default_hint_chars() -> String {
    "asdfghjkl".into()
}
fn default_hint_min_chars() -> u8 {
    1
}
fn default_hint_marker_scale() -> f64 {
    1.0
}
fn default_update_interval() -> u32 {
    24
}
fn default_lists() -> Vec<String> {
    vec!["easylist".into(), "easyprivacy".into()]
}
fn default_languages() -> Vec<String> {
    vec!["system".into()]
}
fn default_log_size() -> u32 {
    5
}
fn default_retained_files() -> u32 {
    3
}
fn default_search_engines() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("ddg".into(), "https://duckduckgo.com/?q={query}".into()),
        ("g".into(), "https://www.google.com/search?q={query}".into()),
    ])
}

fn valid_link_update_source(source: &str) -> bool {
    if !(10..=2048).contains(&source.len())
        || !source.is_ascii()
        || !source.bytes().all(|byte| byte.is_ascii_graphic())
        || !source.starts_with("https://")
        || source.contains(['?', '#', '@'])
    {
        return false;
    }
    let authority = source[8..].split('/').next().unwrap_or_default();
    let (host, port) = authority
        .rsplit_once(':')
        .map_or((authority, None), |(host, port)| (host, Some(port)));
    !host.is_empty()
        && host.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'-'
        })
        && !host.starts_with('.')
        && !host.ends_with('.')
        && !host.contains("..")
        && port.is_none_or(|value| {
            !value.is_empty()
                && value.len() <= 5
                && value.bytes().all(|byte| byte.is_ascii_digit())
                && value.parse::<u16>().is_ok_and(|port| port != 0)
        })
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (byte.is_ascii_lowercase() && byte <= b'f'))
}

#[cfg(test)]
mod tests;
