//! Sparse shared label inputs for source-aware typed consumers.
use crate::{
    AuthoredHints, Config, ConfigError, ConfigurationSources, HintsConfig, PolicySourceIdentity,
    RuntimeOverrides, validate,
};

impl ConfigurationSources {
    /// Validates the complete base and authored Hint catalogue, then projects
    /// sparse label leaves in include application order. Catalogue selector
    /// tables retain include leaf merging; raw authored limits apply there.
    /// Standalone bases retain resolved limits for their unrelated selectors,
    /// and contribute only label leaves to the explicit in-memory seed.
    pub fn shared_label_sources(
        &self,
        base: &Config,
    ) -> Result<Vec<(PolicySourceIdentity, AuthoredHints)>, ConfigError> {
        validate(base)?;
        if self.documents().is_empty() {
            let hints = complete_labels(&base.hints);
            hints.validate().map_err(invalid)?;
            return Ok(vec![(PolicySourceIdentity::InMemory, hints)]);
        }
        let mut merged = toml::Value::Table(toml::map::Map::new());
        let mut sources = Vec::new();
        for (index, document) in self.documents().iter().enumerate() {
            if let Some(hints) = document.hints() {
                hints.validate().map_err(invalid)?;
                let value = toml::Value::try_from(hints).map_err(|error| {
                    ConfigError::validation(format_args!(
                        "could not encode authored Hint inputs: {error}"
                    ))
                })?;
                crate::runtime_config::merge(&mut merged, &value);
                let document_id = u32::try_from(index)
                    .map_err(|_| invalid("authored source catalogue is too large"))?;
                sources.push((
                    PolicySourceIdentity::Authored { document_id },
                    AuthoredHints {
                        chars: hints.chars.clone(),
                        min_chars: hints.min_chars,
                        auto_follow: hints.auto_follow.clone(),
                        ..AuthoredHints::default()
                    },
                ));
            }
        }
        let hints: HintsConfig = merged.try_into().map_err(|error| {
            ConfigError::validation(format_args!(
                "authored Hint catalogue has the wrong type: {error}"
            ))
        })?;
        if hints != base.hints {
            return Err(invalid(
                "authored Hint catalogue does not match the merged base",
            ));
        }
        Ok(sources)
    }
}

impl RuntimeOverrides {
    /// Projects only explicitly supplied shared label leaves. Defaults and
    /// unrelated settings do not become authored reset operations.
    pub fn shared_hint_labels(&self) -> Result<AuthoredHints, ConfigError> {
        let hints = AuthoredHints {
            chars: label_leaf(self, "hints.chars")?,
            min_chars: label_leaf(self, "hints.min_chars")?,
            auto_follow: label_leaf(self, "hints.auto_follow")?,
            ..AuthoredHints::default()
        };
        hints.validate().map_err(invalid)?;
        Ok(hints)
    }
}

fn label_leaf<T: serde::de::DeserializeOwned>(
    overrides: &RuntimeOverrides,
    key: &str,
) -> Result<Option<T>, ConfigError> {
    overrides
        .settings
        .get(key)
        .map(|value| {
            value.clone().try_into().map_err(|error| {
                ConfigError::validation(format_args!(
                    "shared label setting {key} has the wrong type: {error}"
                ))
            })
        })
        .transpose()
}

fn complete_labels(hints: &HintsConfig) -> AuthoredHints {
    AuthoredHints {
        chars: Some(hints.chars.clone()),
        min_chars: Some(hints.min_chars),
        auto_follow: Some(hints.auto_follow.clone()),
        ..AuthoredHints::default()
    }
}

fn invalid(message: impl std::fmt::Display) -> ConfigError {
    ConfigError::validation(format_args!("{message}"))
}
