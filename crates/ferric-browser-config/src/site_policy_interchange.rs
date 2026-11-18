//! Closed local exchange data. This is never a runtime configuration source.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    InteractionPolicyPatch, MAX_SITE_POLICY_IMPORT_OPERATIONS, MAX_SITE_POLICY_INTERCHANGE_BYTES,
    SiteRule, validate_site_rule,
};

/// Implemented format features. Native key delivery remains separately
/// qualified by the engine's build/backend capability manifest.
pub const SITE_POLICY_EXCHANGE_CAPABILITIES: &[&str] = &[
    "interaction-policy-v1",
    "semantic-anchors-v1",
    "collections-v1",
    "explicit-roots-v1",
    "predicates-v1",
    "hint-labels-v1",
    "page-keys-v1",
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SitePolicyExchangeOperation {
    UpsertInteraction {
        id: String,
        pattern: String,
        priority: i32,
        interaction: Box<InteractionPolicyPatch>,
    },
    UnsetInteraction {
        id: String,
        pattern: String,
    },
}

impl SitePolicyExchangeOperation {
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::UpsertInteraction { id, .. } | Self::UnsetInteraction { id, .. } => id,
        }
    }

    fn validate(&self) -> Result<(), String> {
        let (id, pattern, priority, interaction) = match self {
            Self::UpsertInteraction {
                id,
                pattern,
                priority,
                interaction,
            } => (id, pattern, *priority, Some(interaction.as_ref().clone())),
            Self::UnsetInteraction { id, pattern } => (id, pattern, 0, None),
        };
        validate_site_rule(&SiteRule {
            id: id.clone(),
            pattern: pattern.clone(),
            priority,
            interaction,
            set: BTreeMap::new(),
        })
        .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SitePolicyExchange {
    format: String,
    format_version: u32,
    #[serde(default)]
    required_capabilities: Vec<String>,
    rules: Vec<SitePolicyExchangeOperation>,
}

impl SitePolicyExchange {
    /// # Errors
    /// Rejects oversize/non-UTF-8 documents, unknown fields/operations, invalid
    /// policies and duplicate IDs. Unknown capability tokens remain diagnostic
    /// data; the draft service must block apply until they are resolved.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if u64::try_from(bytes.len()).map_err(|_| "exchange is too large")?
            > MAX_SITE_POLICY_INTERCHANGE_BYTES
        {
            return Err("site policy exchange exceeds 2 MiB".into());
        }
        let text = std::str::from_utf8(bytes).map_err(|_| "site policy exchange must be UTF-8")?;
        let mut value: Self = toml::from_str(text)
            .map_err(|error| format!("invalid site policy exchange: {error}"))?;
        value.validate()?;
        value.derive_requirements();
        Ok(value)
    }

    /// Creates an exchange with requirements derived from its actual content.
    /// # Errors
    /// Rejects invalid/duplicate operations or an empty/oversize batch.
    pub fn new(rules: Vec<SitePolicyExchangeOperation>) -> Result<Self, String> {
        Self::with_requirements(rules, Vec::new())
    }

    /// Preserves declared requirements when exporting a reviewed import draft.
    /// # Errors
    /// Rejects invalid operations, metadata or output bounds.
    pub fn with_requirements(
        rules: Vec<SitePolicyExchangeOperation>,
        required_capabilities: Vec<String>,
    ) -> Result<Self, String> {
        let mut value = Self {
            format: "ferric-site-interaction".into(),
            format_version: 1,
            required_capabilities,
            rules,
        };
        value.validate()?;
        value.derive_requirements();
        if value.required_capabilities.len() > 16 {
            return Err("exchange has too many required capabilities".into());
        }
        Ok(value)
    }

    /// # Errors
    /// Rejects invalid versions, identities, policies or capability metadata.
    pub fn validate(&self) -> Result<(), String> {
        if self.format != "ferric-site-interaction" || self.format_version != 1 {
            return Err("unsupported site policy exchange format/version".into());
        }
        if self.rules.is_empty() || self.rules.len() > MAX_SITE_POLICY_IMPORT_OPERATIONS {
            return Err("a site policy exchange requires 1..=256 operations".into());
        }
        let mut ids = BTreeSet::new();
        for rule in &self.rules {
            if !ids.insert(rule.id()) {
                return Err(format!("duplicate site rule ID in exchange: {}", rule.id()));
            }
            rule.validate()?;
        }
        if self.required_capabilities.len() > 16 || self.required_capabilities().len() > 16 {
            return Err("exchange has too many required capabilities".into());
        }
        let mut names = BTreeSet::new();
        for name in &self.required_capabilities {
            if name.is_empty()
                || name.len() > 64
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                || !names.insert(name)
            {
                return Err("exchange capabilities must be unique bounded lowercase tokens".into());
            }
        }
        Ok(())
    }

    /// # Errors
    /// Rejects invalid or over-limit output. Ordinary settings never enter it.
    pub fn encode(&self) -> Result<String, String> {
        self.validate()?;
        let mut value = self.clone();
        value.derive_requirements();
        let text = toml::to_string_pretty(&value).map_err(|error| error.to_string())?;
        if u64::try_from(text.len()).map_err(|_| "exchange is too large")?
            > MAX_SITE_POLICY_INTERCHANGE_BYTES
        {
            return Err("site policy exchange exceeds 2 MiB".into());
        }
        Ok(text)
    }

    #[must_use]
    pub fn rules(&self) -> &[SitePolicyExchangeOperation] {
        &self.rules
    }

    /// Includes declared requirements and features derived from actual content.
    #[must_use]
    pub fn required_capabilities(&self) -> Vec<String> {
        self.derived_requirements()
    }

    #[must_use]
    pub fn unsupported_capabilities(&self) -> Vec<String> {
        self.required_capabilities()
            .into_iter()
            .filter(|name| !SITE_POLICY_EXCHANGE_CAPABILITIES.contains(&name.as_str()))
            .collect()
    }

    fn derive_requirements(&mut self) {
        self.required_capabilities = self.derived_requirements();
    }

    fn derived_requirements(&self) -> Vec<String> {
        let mut names = self
            .required_capabilities
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        names.insert("interaction-policy-v1".into());
        for rule in &self.rules {
            if let SitePolicyExchangeOperation::UpsertInteraction { interaction, .. } = rule {
                for (needed, name) in patch_capabilities(interaction) {
                    if needed {
                        names.insert(name.into());
                    }
                }
            }
        }
        names.into_iter().collect()
    }
}

fn patch_capabilities(patch: &InteractionPolicyPatch) -> [(bool, &'static str); 6] {
    let anchors = patch.anchors.as_deref().unwrap_or_default();
    let collections = patch.collections.as_deref().unwrap_or_default();
    [
        (patch.anchors.is_some(), "semantic-anchors-v1"),
        (patch.collections.is_some(), "collections-v1"),
        (
            anchors.iter().any(|entry| entry.root.is_some())
                || collections
                    .iter()
                    .any(|entry| entry.root.is_some() || entry.primary_root_steps.is_some()),
            "explicit-roots-v1",
        ),
        (
            anchors.iter().any(|entry| entry.predicates.is_some())
                || collections.iter().any(|entry| {
                    entry.item_attributes.is_some()
                        || entry.primary_predicates.is_some()
                        || entry.identity_attributes.is_some()
                }),
            "predicates-v1",
        ),
        (patch.labels.is_some(), "hint-labels-v1"),
        (
            patch.hint_page_keys.is_some()
                || patch.remaps.is_some()
                || patch.page_receiver.is_some(),
            "page-keys-v1",
        ),
    ]
}
