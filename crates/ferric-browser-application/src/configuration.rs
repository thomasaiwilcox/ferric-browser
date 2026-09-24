use ferric_browser_config::{
    Config, ConfigError, PendingSettingChange, RuntimeOverrides, apply_immediate_config_changes,
    pending_config_changes, resolve_runtime_override_layers,
};

/// All precedence-bearing configuration inputs owned by the application.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigurationLayers {
    pub base: Config,
    pub profile: RuntimeOverrides,
    pub runtime: RuntimeOverrides,
    pub command_line: RuntimeOverrides,
    pub temporary: RuntimeOverrides,
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
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigurationSnapshot {
    pub effective: Config,
    pub revision: u64,
    pub pending_changes: Vec<PendingSettingChange>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ConfigurationState {
    layers: ConfigurationLayers,
    effective: Config,
    revision: u64,
    pending_changes: Vec<PendingSettingChange>,
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
