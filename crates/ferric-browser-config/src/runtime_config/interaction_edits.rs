//! Exact section edits within one caller-selected generated/temporary layer.
//! These primitives do not select a source, persist, publish or activate rules.
//! The application must retain its revision/reservation and reload boundaries.

use super::RuntimeOverrides;
use crate::{
    ConfigError, InteractionPolicyPatch, MAX_SITE_RULES, SiteRule, validate_runtime_overrides,
};
use std::collections::BTreeMap;

impl RuntimeOverrides {
    /// Creates one exact-ID rule with a complete Interaction section and no
    /// ordinary settings. Other IDs may deliberately share its pattern.
    /// Validation failure leaves this entire layer unchanged.
    pub fn create_site_interaction(
        &mut self,
        id: &str,
        pattern: &str,
        priority: i32,
        interaction: InteractionPolicyPatch,
    ) -> Result<(), ConfigError> {
        if self.exact_site_rule_index(id, pattern)?.is_some() {
            return Err(ConfigError::validation(format_args!(
                "site rule {id} already exists"
            )));
        }
        if self.site_rules.len() >= MAX_SITE_RULES {
            return Err(ConfigError::Validation(
                "at most 256 runtime site rules are supported".into(),
            ));
        }
        let mut candidate = self.clone();
        candidate.site_rules.push(SiteRule {
            id: id.into(),
            pattern: pattern.into(),
            priority,
            set: BTreeMap::new(),
            interaction: Some(interaction),
        });
        validate_runtime_overrides(&candidate)?;
        *self = candidate;
        Ok(())
    }

    /// Replaces the complete section at an existing exact ID and expected
    /// pattern. Preserves ordinary settings, priority and unrelated rules.
    /// Returns false for an unchanged valid replacement, after validation.
    pub fn replace_site_interaction(
        &mut self,
        id: &str,
        expected_pattern: &str,
        interaction: InteractionPolicyPatch,
    ) -> Result<bool, ConfigError> {
        let index = self
            .exact_site_rule_index(id, expected_pattern)?
            .ok_or_else(|| {
                ConfigError::validation(format_args!("site rule {id} does not exist"))
            })?;
        let changed = self.site_rules[index].interaction.as_ref() != Some(&interaction);
        let mut candidate = self.clone();
        candidate.site_rules[index].interaction = Some(interaction);
        validate_runtime_overrides(&candidate)?;
        *self = candidate;
        Ok(changed)
    }

    /// Removes only Interaction from the exact rule, retaining its ordinary
    /// settings. Prunes the rule only when it contributes neither section.
    /// False reports absence; it does not validate the rest of an unchanged
    /// layer. The expected pattern is checked even if the section is absent.
    pub fn unset_site_interaction(
        &mut self,
        id: &str,
        expected_pattern: &str,
    ) -> Result<bool, ConfigError> {
        let Some(index) = self.exact_site_rule_index(id, expected_pattern)? else {
            return Ok(false);
        };
        if self.site_rules[index].interaction.is_none() {
            return Ok(false);
        }
        let mut candidate = self.clone();
        candidate.site_rules[index].interaction = None;
        if candidate.site_rules[index].set.is_empty() {
            candidate.site_rules.remove(index);
        }
        validate_runtime_overrides(&candidate)?;
        *self = candidate;
        Ok(true)
    }
}
