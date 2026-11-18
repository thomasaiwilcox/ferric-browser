//! Typed authored inputs retained before include merging. This catalogue is
//! validation/provenance data: its rules must never be matched as a second
//! effective configuration. In particular, array replacement still removes
//! included rules from the effective document.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    ConfigError, HintAutoFollow, HintProxyTargets, HintSelectorFamily, HintSelectorMode,
    HintSelectorPolicy, HintUnmatchedPolicy, SiteRule, validate_selector_policy_map,
    validate_site_rule,
};
use std::collections::{BTreeMap, BTreeSet};

/// Browser-owned source records in include application order (children first).
/// Global entries retain only registered settings and never fill defaults.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConfigurationSources {
    documents: Vec<ConfigurationSourceDocument>,
}

impl ConfigurationSources {
    #[must_use]
    pub fn documents(&self) -> &[ConfigurationSourceDocument] {
        &self.documents
    }

    pub(crate) fn push(&mut self, document: ConfigurationSourceDocument) {
        self.documents.push(document);
    }

    /// Checks that retained registered globals describe this exact base.
    /// Reconstructs include merging with the loader's merge operation and
    /// normalizes through `Config` deserialization (for example integer input
    /// accepted for a floating setting). This validates metadata only; it never
    /// installs the reconstructed value or matches its rules.
    ///
    /// # Errors
    /// Rejects a malformed catalogue or a caller pairing it with another base.
    pub fn validate_base_settings(&self, base: &crate::Config) -> Result<(), ConfigError> {
        if self.documents.is_empty() {
            return Ok(());
        }
        let mut merged = toml::Value::Table(toml::Table::new());
        for document in &self.documents {
            let mut globals = toml::Table::new();
            for (key, value) in document.settings() {
                insert_setting(&mut globals, key, value.clone())?;
            }
            crate::runtime_config::merge(&mut merged, &toml::Value::Table(globals));
        }
        let reconstructed: crate::Config = merged.try_into().map_err(|error| {
            ConfigError::validation(format_args!("invalid authored global catalogue: {error}"))
        })?;
        let expected = toml::Value::try_from(reconstructed).map_err(|error| {
            ConfigError::validation(format_args!("could not encode authored globals: {error}"))
        })?;
        let actual = toml::Value::try_from(base).map_err(|error| {
            ConfigError::validation(format_args!("could not encode base settings: {error}"))
        })?;
        for metadata in crate::setting_metadata_all() {
            let key = metadata.key.strip_suffix(".*").unwrap_or(metadata.key);
            if !matches!(key, "include" | "site_rules")
                && value_at(&expected, key) != value_at(&actual, key)
            {
                return Err(ConfigError::validation(format_args!(
                    "authored setting catalogue does not match the merged base at {key}"
                )));
            }
        }
        Ok(())
    }
}

fn value_at<'a>(value: &'a toml::Value, key: &str) -> Option<&'a toml::Value> {
    key.split('.')
        .try_fold(value, |value, part| value.get(part))
}

fn insert_setting(
    table: &mut toml::Table,
    key: &str,
    value: toml::Value,
) -> Result<(), ConfigError> {
    if let Some((head, tail)) = key.split_once('.') {
        let child = table
            .entry(head)
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        let table = child.as_table_mut().ok_or_else(|| {
            ConfigError::validation(format_args!(
                "authored setting catalogue has conflicting setting paths"
            ))
        })?;
        insert_setting(table, tail, value)
    } else {
        table.insert(key.into(), value);
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfigurationSourceDocument {
    path: PathBuf,
    /// `None` means absent; `Some([])` explicitly replaces an inherited array.
    site_rules: Option<Vec<SiteRule>>,
    hints: Option<AuthoredHints>,
    /// Sparse raw values at registry setting boundaries. This is inspection
    /// metadata, never another configuration to merge or match.
    settings: BTreeMap<String, toml::Value>,
}

impl ConfigurationSourceDocument {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn site_rules(&self) -> Option<&[SiteRule]> {
        self.site_rules.as_deref()
    }

    #[must_use]
    pub const fn hints(&self) -> Option<&AuthoredHints> {
        self.hints.as_ref()
    }

    /// Explicit registered global values before include merging. A table
    /// remains sparse; an empty list is retained as an authored replacement.
    /// Include directives are consumed by the loader, and site rules retain
    /// their separately validated typed catalogue.
    #[must_use]
    pub const fn settings(&self) -> &BTreeMap<String, toml::Value> {
        &self.settings
    }

    pub(crate) fn parse(path: &Path, value: &toml::Value) -> Result<Self, ConfigError> {
        if let Some(bindings) = value.get("bindings") {
            crate::binding_config::validate_raw(bindings)
                .map_err(|error| invalid(path, &error.to_string()))?;
        }
        let site_rules: Option<Vec<SiteRule>> = parse_field(path, value, "site_rules")?;
        if let Some(rules) = &site_rules {
            if rules.len() > crate::MAX_SITE_RULES {
                return Err(invalid(path, "at most 256 site rules are supported"));
            }
            let mut ids = BTreeSet::new();
            for rule in rules {
                if !ids.insert(&rule.id) {
                    return Err(invalid(
                        path,
                        "site rule IDs must be unique within a source",
                    ));
                }
                validate_site_rule(rule).map_err(|error| invalid(path, &error.to_string()))?;
            }
        }
        let hints: Option<AuthoredHints> = parse_field(path, value, "hints")?;
        if let Some(hints) = &hints {
            hints.validate().map_err(|error| invalid(path, &error))?;
        }
        Ok(Self {
            path: path.to_owned(),
            site_rules,
            hints,
            settings: crate::setting_metadata_all()
                .iter()
                .filter_map(|metadata| {
                    let key = metadata.key.strip_suffix(".*").unwrap_or(metadata.key);
                    if matches!(key, "include" | "site_rules") {
                        return None;
                    }
                    let raw = key
                        .split('.')
                        .try_fold(value, |value, part| value.get(part))?;
                    Some((key.to_owned(), raw.clone()))
                })
                .collect(),
        })
    }
}

/// Sparse shared Hint values. Absence must survive includes so future policy
/// resolution can identify the authored leaf rather than a synthesized default.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredHints {
    pub chars: Option<String>,
    pub min_chars: Option<u8>,
    pub auto_follow: Option<HintAutoFollow>,
    pub unmatched: Option<HintUnmatchedPolicy>,
    pub rapid_unmatched: Option<HintUnmatchedPolicy>,
    pub marker_scale: Option<f64>,
    pub proxy_targets: Option<HintProxyTargets>,
    pub selectors: Option<BTreeMap<HintSelectorFamily, AuthoredHintSelectors>>,
}

/// Include tables merge by leaf, so a source may supply selector strings while
/// another supplies mode. A missing mode must not become an authored `extend`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredHintSelectors {
    pub mode: Option<HintSelectorMode>,
    pub selectors: Option<Vec<String>>,
}

impl AuthoredHints {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if let Some(chars) = &self.chars {
            let mut seen = BTreeSet::new();
            if !(2..=32).contains(&chars.len())
                || !chars.is_ascii()
                || chars
                    .bytes()
                    .any(|byte| !byte.is_ascii_graphic() || byte == b'/' || !seen.insert(byte))
            {
                return Err(
                    "hints.chars must contain 2..=32 unique printable ASCII characters excluding /"
                        .into(),
                );
            }
        }
        if self
            .min_chars
            .is_some_and(|value| !(1..=8).contains(&value))
        {
            return Err("hints.min_chars must be 1..=8".into());
        }
        if self
            .marker_scale
            .is_some_and(|value| !value.is_finite() || !(0.75..=2.0).contains(&value))
        {
            return Err("hints.marker_scale must be 0.75..=2.0".into());
        }
        if let Some(families) = &self.selectors {
            let validation_policies = families
                .iter()
                .map(|(family, policy)| {
                    (
                        *family,
                        HintSelectorPolicy {
                            mode: policy.mode.unwrap_or(HintSelectorMode::Extend),
                            selectors: policy.selectors.clone().unwrap_or_default(),
                        },
                    )
                })
                .collect();
            validate_selector_policy_map(&validation_policies)?;
        }
        Ok(())
    }
}

fn parse_field<T: serde::de::DeserializeOwned>(
    path: &Path,
    value: &toml::Value,
    key: &str,
) -> Result<Option<T>, ConfigError> {
    value
        .get(key)
        .map(|value| {
            value
                .clone()
                .try_into()
                .map_err(|error| ConfigError::from_toml(path, error))
        })
        .transpose()
}

fn invalid(path: &Path, message: &str) -> ConfigError {
    ConfigError::validation(format_args!(
        "{}: {message}",
        crate::configuration_diagnostic::bounded_path(path)
    ))
}
