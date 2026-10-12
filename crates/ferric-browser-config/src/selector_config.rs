//! Typed selector policy entries accepted by the configuration layers.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub const MAX_SELECTORS_PER_FAMILY: usize = 16;
pub const MAX_SELECTOR_BYTES: usize = 1_024;
pub const MAX_SELECTOR_RULE_BYTES: usize = 16 * 1024;
pub const MAX_RESOLVED_SELECTOR_BYTES: usize = 32 * 1024;

/// The deliberately closed set of selector families supported by Ferric.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HintSelectorFamily {
    All,
    Links,
    Inputs,
    Buttons,
    Images,
    Media,
    Scrollables,
}

impl HintSelectorFamily {
    pub const ALL: [Self; 7] = [
        Self::All,
        Self::Links,
        Self::Inputs,
        Self::Buttons,
        Self::Images,
        Self::Media,
        Self::Scrollables,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Links => "links",
            Self::Inputs => "inputs",
            Self::Buttons => "buttons",
            Self::Images => "images",
            Self::Media => "media",
            Self::Scrollables => "scrollables",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|family| family.as_str() == value)
    }
}

/// Determines how a selector policy overlays its inherited family.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HintSelectorMode {
    Extend,
    Replace,
}

/// One family-local CSS selector policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HintSelectorPolicy {
    pub mode: HintSelectorMode,
    #[serde(default)]
    pub selectors: Vec<String>,
}

impl HintSelectorPolicy {
    /// Checks the count, encoded byte bounds, control characters, duplicates,
    /// and per-family serialized string budget.
    pub fn validate(&self) -> Result<(), String> {
        if self.selectors.len() > MAX_SELECTORS_PER_FAMILY {
            return Err(format!(
                "at most {MAX_SELECTORS_PER_FAMILY} selectors are supported per family"
            ));
        }
        let mut seen = BTreeSet::new();
        let mut total = 0_usize;
        for selector in &self.selectors {
            let bytes = selector.len();
            if selector.is_empty() || bytes > MAX_SELECTOR_BYTES {
                return Err(format!(
                    "selectors must contain 1..={MAX_SELECTOR_BYTES} UTF-8 bytes"
                ));
            }
            if selector.chars().any(char::is_control) {
                return Err("selectors cannot contain control characters".into());
            }
            if !seen.insert(selector) {
                return Err("identical selectors are not allowed in one family".into());
            }
            total = total.saturating_add(bytes);
        }
        if total > MAX_SELECTOR_RULE_BYTES {
            return Err("selector strings exceed the 16 KiB family rule limit".into());
        }
        Ok(())
    }

    #[must_use]
    pub fn selector_bytes(&self) -> usize {
        self.selectors.iter().fold(0_usize, |total, selector| {
            total.saturating_add(selector.len())
        })
    }

    /// Applies one layer while retaining stable insertion order and the
    /// inherited replace decision.
    pub fn overlay(inherited: Option<&Self>, incoming: Self) -> Result<Self, String> {
        incoming.validate()?;
        if incoming.mode == HintSelectorMode::Replace {
            return Ok(incoming);
        }

        let mode = inherited.map_or(HintSelectorMode::Extend, |policy| policy.mode);
        let mut selectors = inherited.map_or_else(Vec::new, |policy| policy.selectors.clone());
        for selector in incoming.selectors {
            if !selectors.contains(&selector) {
                selectors.push(selector);
            }
        }
        let merged = Self { mode, selectors };
        merged.validate()?;
        Ok(merged)
    }
}

#[must_use]
pub fn selector_setting_family(key: &str) -> Option<HintSelectorFamily> {
    let family = key.strip_prefix("hints.selectors.")?;
    if family.contains('.') {
        return None;
    }
    HintSelectorFamily::parse(family)
}

pub fn validate_selector_policy_map(
    policies: &BTreeMap<HintSelectorFamily, HintSelectorPolicy>,
) -> Result<(), String> {
    let mut total = 0_usize;
    for (family, policy) in policies {
        policy
            .validate()
            .map_err(|error| format!("hints.selectors.{}: {error}", family.as_str()))?;
        total = total.saturating_add(policy.selector_bytes());
    }
    if total > MAX_RESOLVED_SELECTOR_BYTES {
        return Err("resolved selector strings exceed the 32 KiB policy limit".into());
    }
    Ok(())
}
