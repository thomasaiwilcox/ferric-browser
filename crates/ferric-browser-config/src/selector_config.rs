//! Typed selector policy entries accepted by the configuration layers.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{Config, matching_site_rules};
use crate::PolicyFieldOrigin;
use std::sync::Arc;

mod trace;
pub use trace::{
    HintSelectorCompositionTrace, HintSelectorContribution, HintSelectorContributionOutcome,
    HintSelectorDuplicate, HintSelectorFamilyTrace, HintSelectorFieldOrigins,
    HintSelectorInputField, HintSelectorSeedOrigins, HintSelectorValueOrigin,
};

#[derive(Clone, Copy)]
enum SelectorOverlayEvent {
    Replace {
        count: usize,
    },
    Extend {
        inherited: bool,
    },
    Append {
        input_index: usize,
    },
    Duplicate {
        input_index: usize,
        retained_index: usize,
    },
}

pub const MAX_SELECTORS_PER_FAMILY: usize = 16;
pub const MAX_RESOLVED_SELECTORS_PER_FAMILY: usize = 32;
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
        self.validate_with_limits(
            MAX_SELECTORS_PER_FAMILY,
            MAX_SELECTOR_RULE_BYTES,
            "16 KiB family rule",
        )
    }

    /// Checks a policy after it has been resolved across configuration layers.
    /// Each incoming layer still uses [`Self::validate`].
    pub fn validate_resolved(&self) -> Result<(), String> {
        self.validate_with_limits(
            MAX_RESOLVED_SELECTORS_PER_FAMILY,
            MAX_RESOLVED_SELECTOR_BYTES,
            "32 KiB resolved family",
        )
    }

    fn validate_with_limits(
        &self,
        maximum_count: usize,
        maximum_bytes: usize,
        budget_name: &str,
    ) -> Result<(), String> {
        if self.selectors.len() > maximum_count {
            return Err(format!(
                "at most {maximum_count} selectors are supported per family"
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
        if total > maximum_bytes {
            return Err(format!("selector strings exceed the {budget_name} limit"));
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
        Self::overlay_observed(inherited, incoming, |_| {})
    }

    fn overlay_observed(
        inherited: Option<&Self>,
        incoming: Self,
        mut observe: impl FnMut(SelectorOverlayEvent),
    ) -> Result<Self, String> {
        incoming.validate()?;
        if incoming.mode == HintSelectorMode::Replace {
            observe(SelectorOverlayEvent::Replace {
                count: incoming.selectors.len(),
            });
            return Ok(incoming);
        }

        observe(SelectorOverlayEvent::Extend {
            inherited: inherited.is_some(),
        });
        let mode = inherited.map_or(HintSelectorMode::Extend, |policy| policy.mode);
        let mut selectors = inherited.map_or_else(Vec::new, |policy| policy.selectors.clone());
        for (input_index, selector) in incoming.selectors.into_iter().enumerate() {
            if let Some(retained_index) = selectors.iter().position(|value| value == &selector) {
                observe(SelectorOverlayEvent::Duplicate {
                    input_index,
                    retained_index,
                });
            } else {
                selectors.push(selector);
                observe(SelectorOverlayEvent::Append { input_index });
            }
        }
        let merged = Self { mode, selectors };
        merged.validate_resolved()?;
        Ok(merged)
    }
}

/// Effective selectors for one family, with the configuration sources that
/// contributed policy entries. An absent policy means use the built-in list.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EffectiveHintSelectorFamily {
    pub policy: Option<HintSelectorPolicy>,
    pub provenance: Vec<String>,
}

/// Composes selector inputs in caller-supplied source order. URL matching and
/// source precedence remain the shared site-rule resolver's responsibility.
#[derive(Debug)]
pub struct HintSelectorPolicyBuilder {
    families: BTreeMap<HintSelectorFamily, EffectiveHintSelectorFamily>,
    // An overflowing union cannot shrink through extend. Retain only its last
    // bounded value and error until a later replace repairs that family.
    overflowed: BTreeMap<HintSelectorFamily, String>,
    trace: Option<HintSelectorCompositionTrace>,
}

impl HintSelectorPolicyBuilder {
    /// Seeds the already merged base once, retaining resolved selector limits.
    pub fn from_base(
        base: &BTreeMap<HintSelectorFamily, HintSelectorPolicy>,
        provenance: &str,
    ) -> Result<Self, String> {
        validate_resolved_selector_policy_map(base)?;
        let families = HintSelectorFamily::ALL
            .into_iter()
            .map(|family| {
                let policy = base.get(&family).cloned();
                let provenance = if policy.is_some() {
                    vec![provenance.to_owned()]
                } else {
                    Vec::new()
                };
                (family, EffectiveHintSelectorFamily { policy, provenance })
            })
            .collect();
        Ok(Self {
            families,
            overflowed: BTreeMap::new(),
            trace: None,
        })
    }

    /// Starts tracing at the include-merged base boundary. Each configured
    /// family needs exact mode/list origins supplied by its source catalogue.
    pub fn from_base_traced(
        base: &BTreeMap<HintSelectorFamily, HintSelectorPolicy>,
        provenance: &str,
        origins: &BTreeMap<HintSelectorFamily, HintSelectorSeedOrigins>,
    ) -> Result<Self, String> {
        if origins.len() != base.len() || base.keys().any(|family| !origins.contains_key(family)) {
            return Err("selector seed origins do not match configured base families".into());
        }
        let mut builder = Self::from_base(base, provenance)?;
        let mut trace = BTreeMap::new();
        for family in HintSelectorFamily::ALL {
            let fields = if let Some(policy) = base.get(&family) {
                let origins = &origins[&family];
                for origin in std::iter::once(&origins.mode).chain(origins.selectors.iter()) {
                    if origin.rule().is_some()
                        || !matches!(
                            origin.source(),
                            crate::PolicySourceIdentity::InMemory
                                | crate::PolicySourceIdentity::Authored { .. }
                        )
                    {
                        return Err(
                            "selector base seed must come from an authored or in-memory global"
                                .into(),
                        );
                    }
                }
                if !policy.selectors.is_empty() && origins.selectors.is_none() {
                    return Err("nonempty selector seed is missing its list origin".into());
                }
                HintSelectorFieldOrigins::seed(policy, origins)
            } else {
                HintSelectorFieldOrigins::default()
            };
            trace.insert(
                family,
                HintSelectorFamilyTrace {
                    seed: fields.clone(),
                    fields,
                    contributions: Vec::new(),
                },
            );
        }
        builder.trace = Some(trace);
        Ok(builder)
    }

    /// Applies only explicitly supplied family settings, using the existing
    /// authored input limits and stable extend/replace behavior. Effective
    /// overflow remains unavailable until finish or a later replacement.
    /// Raw validation failure leaves this builder unchanged.
    pub fn apply_settings(
        &mut self,
        settings: &BTreeMap<String, toml::Value>,
        provenance: &str,
    ) -> Result<(), String> {
        self.apply_settings_observed(settings, provenance, None)
    }

    /// Applies the same canonical overlay while retaining typed source facts.
    pub fn apply_settings_with_origin(
        &mut self,
        settings: &BTreeMap<String, toml::Value>,
        provenance: &str,
        origin: &Arc<PolicyFieldOrigin>,
    ) -> Result<(), String> {
        self.apply_settings_observed(settings, provenance, Some(origin))
    }

    fn apply_settings_observed(
        &mut self,
        settings: &BTreeMap<String, toml::Value>,
        provenance: &str,
        origin: Option<&Arc<PolicyFieldOrigin>>,
    ) -> Result<(), String> {
        if self.trace.is_some() && origin.is_none() {
            return Err("traced selector composition requires a typed input origin".into());
        }
        let mut updates = Vec::new();
        for family in HintSelectorFamily::ALL {
            let key = format!("hints.selectors.{}", family.as_str());
            let Some(value) = settings.get(&key) else {
                continue;
            };
            let incoming = value
                .clone()
                .try_into::<HintSelectorPolicy>()
                .map_err(|error| {
                    format!("{provenance} has an invalid policy for {key}: {error}")
                })?;
            incoming
                .validate()
                .map_err(|error| format!("{provenance}: {error}"))?;
            let inherited = self.families.get(&family);
            let previous = inherited.and_then(|entry| entry.policy.as_ref());
            let trace_input = self.trace.as_ref().map(|_| incoming.clone());
            let mut fields = self
                .trace
                .as_ref()
                .map(|trace| trace[&family].fields.clone());
            let mut duplicates = Vec::new();
            let selectors_explicit = self.trace.is_some() && value.get("selectors").is_some();
            let blocked = self.trace.is_some()
                && incoming.mode == HintSelectorMode::Extend
                && self.overflowed.contains_key(&family);
            let (policy, overflow) = if incoming.mode == HintSelectorMode::Extend
                && let Some(error) = self.overflowed.get(&family)
            {
                (previous.cloned(), Some(error.clone()))
            } else {
                let composed = if fields.is_some() {
                    HintSelectorPolicy::overlay_observed(previous, incoming, |event| {
                        if let (Some(fields), Some(origin)) = (fields.as_mut(), origin) {
                            fields.observe(event, origin, selectors_explicit, &mut duplicates);
                        }
                    })
                } else {
                    HintSelectorPolicy::overlay(previous, incoming)
                };
                match composed {
                    Ok(policy) => (Some(policy), None),
                    // Both inputs are individually valid. Only the composed
                    // effective limit can fail here; never expose the partial
                    // value unless a replacement later makes it valid again.
                    Err(error) => (
                        previous.cloned(),
                        Some(format!("{key}: {provenance}: {error}")),
                    ),
                }
            };
            let mut sources = inherited.map_or_else(Vec::new, |entry| entry.provenance.clone());
            sources.push(provenance.to_owned());
            let trace_update = trace_input.zip(origin).map(|(input, origin)| {
                let outcome =
                    HintSelectorContributionOutcome::observed(blocked, overflow.as_deref());
                let fields = overflow.is_none().then_some(fields).flatten();
                (
                    fields,
                    HintSelectorContribution {
                        source: Arc::clone(origin),
                        policy: input,
                        selectors_explicit,
                        outcome,
                        duplicates,
                    },
                )
            });
            updates.push((
                family,
                EffectiveHintSelectorFamily {
                    policy,
                    provenance: sources,
                },
                overflow,
                trace_update,
            ));
        }
        for (family, entry, overflow, trace_update) in updates {
            self.families.insert(family, entry);
            if let Some(error) = overflow {
                self.overflowed.insert(family, error);
            } else {
                self.overflowed.remove(&family);
            }
            if let (Some(trace), Some((fields, contribution))) = (self.trace.as_mut(), trace_update)
            {
                let entry = trace.get_mut(&family).expect("all families seeded");
                if let Some(fields) = fields {
                    entry.fields = fields;
                }
                entry.contributions.push(contribution);
            }
        }
        Ok(())
    }

    /// Exposes origins only with the same successful effective validation as
    /// `finish`. Overflow/conflict never returns partial winning fields.
    pub fn finish_with_trace(
        mut self,
    ) -> Result<
        (
            BTreeMap<HintSelectorFamily, EffectiveHintSelectorFamily>,
            HintSelectorCompositionTrace,
        ),
        String,
    > {
        let trace = self
            .trace
            .take()
            .ok_or("selector composition tracing was not enabled")?;
        let policies = self.finish()?;
        Ok((policies, trace))
    }

    /// Checks the aggregate effective budget before exposing any policy map.
    pub fn finish(
        mut self,
    ) -> Result<BTreeMap<HintSelectorFamily, EffectiveHintSelectorFamily>, String> {
        if let Some(error) = self.overflowed.values().next() {
            return Err(error.clone());
        }
        validate_resolved_selector_policy_map(
            &self
                .families
                .iter()
                .filter_map(|(family, entry)| {
                    entry
                        .policy
                        .as_ref()
                        .map(|policy| (*family, policy.clone()))
                })
                .collect(),
        )?;
        for entry in self.families.values_mut() {
            if entry.provenance.is_empty() {
                entry.provenance.push("built-in".into());
            }
        }
        Ok(self.families)
    }
}

/// Resolves configured selector entries through the existing site-rule order.
/// Built-in selectors remain the caller's fallback and are not duplicated here.
pub fn resolve_hint_selector_policies(
    config: &Config,
    url: &str,
) -> Result<BTreeMap<HintSelectorFamily, EffectiveHintSelectorFamily>, String> {
    let mut builder =
        HintSelectorPolicyBuilder::from_base(&config.hints.selectors, "effective-config")?;
    for rule in matching_site_rules(config, url) {
        builder.apply_settings(&rule.set, &format!("site-rule:{}", rule.id))?;
    }
    builder.finish()
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
    validate_selector_policy_map_with_limits(policies, false)
}

pub fn validate_resolved_selector_policy_map(
    policies: &BTreeMap<HintSelectorFamily, HintSelectorPolicy>,
) -> Result<(), String> {
    validate_selector_policy_map_with_limits(policies, true)
}

fn validate_selector_policy_map_with_limits(
    policies: &BTreeMap<HintSelectorFamily, HintSelectorPolicy>,
    resolved: bool,
) -> Result<(), String> {
    let mut total = 0_usize;
    for (family, policy) in policies {
        let validation = if resolved {
            policy.validate_resolved()
        } else {
            policy.validate()
        };
        validation.map_err(|error| format!("hints.selectors.{}: {error}", family.as_str()))?;
        total = total.saturating_add(policy.selector_bytes());
    }
    if total > MAX_RESOLVED_SELECTOR_BYTES {
        return Err("resolved selector strings exceed the 32 KiB policy limit".into());
    }
    Ok(())
}

#[cfg(test)]
mod builder_tests {
    use super::*;

    #[test]
    fn intermediate_overflow_stays_bounded_and_only_replace_can_repair_it() {
        for repair in [false, true] {
            let mut builder =
                HintSelectorPolicyBuilder::from_base(&BTreeMap::new(), "base").unwrap();
            for index in 0..100 {
                let input = BTreeMap::from([(
                    "hints.selectors.buttons".into(),
                    toml::Value::try_from(HintSelectorPolicy {
                        mode: HintSelectorMode::Extend,
                        selectors: vec![format!(".selector{index}")],
                    })
                    .unwrap(),
                )]);
                builder.apply_settings(&input, "site-rule").unwrap();
                assert!(
                    builder
                        .families
                        .values()
                        .all(|entry| entry
                            .policy
                            .as_ref()
                            .is_none_or(|policy| policy.selectors.len()
                                <= MAX_RESOLVED_SELECTORS_PER_FAMILY))
                );
            }
            if repair {
                let replacement = BTreeMap::from([(
                    "hints.selectors.buttons".into(),
                    toml::Value::try_from(HintSelectorPolicy {
                        mode: HintSelectorMode::Replace,
                        selectors: vec![".winner".into()],
                    })
                    .unwrap(),
                )]);
                builder.apply_settings(&replacement, "generated").unwrap();
                assert_eq!(
                    builder.finish().unwrap()[&HintSelectorFamily::Buttons]
                        .policy
                        .as_ref()
                        .unwrap()
                        .selectors,
                    [".winner"]
                );
            } else {
                assert!(builder.finish().is_err());
            }
        }
    }

    #[test]
    fn rejected_multi_family_overlay_does_not_publish_earlier_families() {
        let mut builder = HintSelectorPolicyBuilder::from_base(&BTreeMap::new(), "base").unwrap();
        let settings = BTreeMap::from([
            (
                "hints.selectors.all".into(),
                toml::Value::try_from(HintSelectorPolicy {
                    mode: HintSelectorMode::Replace,
                    selectors: vec![".valid".into()],
                })
                .unwrap(),
            ),
            (
                "hints.selectors.buttons".into(),
                toml::Value::try_from(HintSelectorPolicy {
                    mode: HintSelectorMode::Replace,
                    selectors: vec![String::new()],
                })
                .unwrap(),
            ),
        ]);
        assert!(builder.apply_settings(&settings, "generated").is_err());
        assert!(
            builder
                .finish()
                .unwrap()
                .values()
                .all(|entry| entry.policy.is_none() && entry.provenance == ["built-in"])
        );
    }

    #[test]
    fn final_selector_budget_includes_every_family_and_source() {
        fn full_selector(index: usize) -> String {
            let prefix = format!(".selector{index}");
            format!("{prefix}{}", "x".repeat(MAX_SELECTOR_BYTES - prefix.len()))
        }
        let full_policy = HintSelectorPolicy {
            mode: HintSelectorMode::Replace,
            selectors: (0..16).map(full_selector).collect(),
        };
        let base = BTreeMap::from([(HintSelectorFamily::Buttons, full_policy.clone())]);
        let settings = BTreeMap::from([(
            "hints.selectors.links".into(),
            toml::Value::try_from(full_policy).unwrap(),
        )]);
        for overflow in [false, true] {
            let mut builder = HintSelectorPolicyBuilder::from_base(&base, "base").unwrap();
            builder.apply_settings(&settings, "generated").unwrap();
            if overflow {
                let extra = BTreeMap::from([(
                    "hints.selectors.images".into(),
                    toml::Value::try_from(HintSelectorPolicy {
                        mode: HintSelectorMode::Extend,
                        selectors: vec![".extra".into()],
                    })
                    .unwrap(),
                )]);
                builder.apply_settings(&extra, "temporary").unwrap();
                assert!(builder.finish().is_err());
            } else {
                assert!(builder.finish().is_ok());
            }
        }
    }
}
