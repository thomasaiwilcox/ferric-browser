use ferric_browser_config::{
    Config, ConfigError, PendingSettingChange, RuntimeOverrides, apply_immediate_config_changes,
    pending_config_changes, resolve_runtime_override_layers,
};
use std::fmt;

/// All precedence-bearing configuration inputs owned by the application.
#[derive(Clone, PartialEq)]
pub struct ConfigurationLayers {
    pub base: Config,
    pub profile: RuntimeOverrides,
    pub runtime: RuntimeOverrides,
    pub command_line: RuntimeOverrides,
    pub temporary: RuntimeOverrides,
}

impl fmt::Debug for ConfigurationLayers {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfigurationLayers")
            .field("base", &"[redacted]")
            .field("profile", &"[redacted]")
            .field("runtime", &"[redacted]")
            .field("command_line", &"[redacted]")
            .field("temporary", &"[redacted]")
            .finish()
    }
}

impl ConfigurationLayers {
    #[must_use]
    pub fn new(base: Config) -> Self {
        Self {
            base,
            profile: RuntimeOverrides::default(),
            runtime: RuntimeOverrides::default(),
            command_line: RuntimeOverrides::default(),
            temporary: RuntimeOverrides::default(),
        }
    }

    /// Resolves all layers using the configuration crate's canonical
    /// precedence rules.
    ///
    /// # Errors
    ///
    /// Returns a schema or validation error when a layer is invalid.
    pub fn resolve(&self) -> Result<Config, ConfigError> {
        resolve_runtime_override_layers(
            &self.base,
            &self.profile,
            &self.runtime,
            &self.command_line,
            &self.temporary,
        )
    }
}

/// Immutable configuration projection for presentation adapters.
#[derive(Clone, PartialEq)]
pub struct ConfigurationSnapshot {
    pub effective: Config,
    pub revision: u64,
    pub pending_changes: Vec<PendingSettingChange>,
}

impl fmt::Debug for ConfigurationSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfigurationSnapshot")
            .field("effective", &"[redacted]")
            .field("revision", &self.revision)
            .field("pending_change_count", &self.pending_changes.len())
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct ConfigurationState {
    layers: ConfigurationLayers,
    effective: Config,
    revision: u64,
    pending_changes: Vec<PendingSettingChange>,
}

impl fmt::Debug for ConfigurationState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfigurationState")
            .field("revision", &self.revision)
            .field("pending_change_count", &self.pending_changes.len())
            .field("layers", &"[redacted]")
            .field("effective", &"[redacted]")
            .finish()
    }
}

impl ConfigurationState {
    pub(crate) fn new(effective: Config) -> Self {
        Self {
            layers: ConfigurationLayers::new(effective.clone()),
            effective,
            revision: 0,
            pending_changes: Vec::new(),
        }
    }

    pub(crate) const fn effective(&self) -> &Config {
        &self.effective
    }

    pub(crate) const fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn snapshot(&self) -> ConfigurationSnapshot {
        ConfigurationSnapshot {
            effective: self.effective.clone(),
            revision: self.revision,
            pending_changes: self.pending_changes.clone(),
        }
    }

    pub(crate) fn replace_effective(
        &mut self,
        effective: Config,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        ferric_browser_config::validate(&effective)?;
        self.layers = ConfigurationLayers::new(effective.clone());
        Ok(self.commit(effective, Vec::new()))
    }

    pub(crate) fn activate_layers(
        &mut self,
        layers: ConfigurationLayers,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        let effective = layers.resolve()?;
        self.layers = layers;
        Ok(self.commit(effective, Vec::new()))
    }

    pub(crate) fn update_layers(
        &mut self,
        layers: ConfigurationLayers,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        let candidate = layers.resolve()?;
        let current = toml::Value::try_from(&self.effective).map_err(|error| {
            ConfigError::Validation(format!("could not encode active configuration: {error}"))
        })?;
        let candidate_value = toml::Value::try_from(&candidate).map_err(|error| {
            ConfigError::Validation(format!("could not encode candidate configuration: {error}"))
        })?;
        let pending = pending_config_changes(&current, &candidate_value);
        let effective = apply_immediate_config_changes(&current, &candidate_value)
            .try_into::<Config>()
            .map_err(|error| {
                ConfigError::Validation(format!(
                    "could not decode active configuration after applying live changes: {error}"
                ))
            })?;
        ferric_browser_config::validate(&effective)?;
        self.layers = layers;
        Ok(self.commit(effective, pending))
    }

    pub(crate) fn replace_temporary(
        &mut self,
        temporary: RuntimeOverrides,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        let mut layers = self.layers.clone();
        layers.temporary = temporary;
        self.update_layers(layers)
    }

    /// Applies the sole currently allowlisted versioned policy through the
    /// normal typed override and whole-config validation path. The expected
    /// revision makes a reviewed preview stale after any intervening update.
    pub(crate) fn apply_versioned_javascript_policy(
        &mut self,
        enabled: bool,
        expected_revision: u64,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        if self.revision != expected_revision {
            return Err(ConfigError::Validation(
                "configuration changed after the versioned policy preview".into(),
            ));
        }
        let mut layers = self.layers.clone();
        layers
            .temporary
            .set_setting("content.javascript", toml::Value::Boolean(enabled))?;
        self.update_layers(layers)
    }

    fn commit(
        &mut self,
        effective: Config,
        pending_changes: Vec<PendingSettingChange>,
    ) -> ConfigurationSnapshot {
        self.effective = effective;
        self.pending_changes = pending_changes;
        self.revision = self.revision.saturating_add(1);
        self.snapshot()
    }
}

#[cfg(test)]
mod debug_tests {
    use super::*;

    #[test]
    fn configuration_application_debug_redacts_effective_values() {
        let sentinel = "private-configuration-sentinel-1a73";
        let mut config = Config::default();
        config.search_engines.insert(
            sentinel.into(),
            format!("https://example.test/?q={sentinel}"),
        );
        let layers = ConfigurationLayers::new(config.clone());
        let snapshot = ConfigurationSnapshot {
            effective: config.clone(),
            revision: 1,
            pending_changes: Vec::new(),
        };
        let state = ConfigurationState::new(config);

        for projection in [
            format!("{layers:?}"),
            format!("{snapshot:?}"),
            format!("{state:?}"),
        ] {
            assert!(!projection.contains(sentinel), "{projection}");
            assert!(!projection.contains("example.test"), "{projection}");
        }
    }
}
