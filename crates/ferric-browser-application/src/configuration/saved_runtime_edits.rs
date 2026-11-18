//! Explicit edits of the authoritative saved Runtime, including a graph which
//! the local engine rejected and replaced with a safe active configuration.
use std::{
    fmt,
    sync::{Arc, Weak},
};

use ferric_browser_config::{ConfigError, ConfigurationApplyBoundary, RuntimeOverrides};

use super::{
    ConfigurationAdmission, ConfigurationLayers, ConfigurationRequestCapture,
    PreparedConfigurationUpdate,
};
use crate::{BrowserApplication, profile_configuration::SharedConfiguration};

/// A typed saved-source snapshot, not active configuration or a validation
/// receipt. Weak sharing identity cannot keep a retired root alive. Local and
/// shared revisions make both ordinary changes and equal-value ABA stale.
#[derive(Clone)]
pub struct SavedRuntimeEditCapture {
    pub(super) request: ConfigurationRequestCapture,
    sharing: Weak<SharedConfiguration>,
    shared_revision: u64,
    pub(super) layers: Arc<ConfigurationLayers>,
}

impl fmt::Debug for SavedRuntimeEditCapture {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SavedRuntimeEditCapture")
            .field("configuration_revision", &self.request.revision)
            .field("shared_revision", &self.shared_revision)
            .finish_non_exhaustive()
    }
}

impl SavedRuntimeEditCapture {
    /// This owner's other sources with the captured saved Runtime substituted.
    /// No consumer may treat these desired inputs as active configuration.
    #[must_use]
    pub fn layers(&self) -> &ConfigurationLayers {
        &self.layers
    }

    #[must_use]
    pub const fn configuration_revision(&self) -> u64 {
        self.request.revision
    }

    #[must_use]
    pub const fn shared_profile_revision(&self) -> u64 {
        self.shared_revision
    }
}

impl BrowserApplication {
    /// Captures the exact shared Runtime without adopting it or reading a file.
    /// Use this explicit source when repairing settings retained on disk after
    /// local engine validation selected a safe active fallback. Static/parser-
    /// invalid raw files are not recovered by this typed capture.
    ///
    /// # Errors
    /// Rejects busy/retired owners, a missing profile or unavailable shared data.
    pub fn capture_saved_runtime_edits(&self) -> Result<SavedRuntimeEditCapture, ConfigError> {
        self.ensure_configuration_writable()?;
        let request = self
            .capture_configuration_request()
            .ok_or_else(|| invalid("The configuration owner is unavailable"))?;
        let profile = self
            .active_profile
            .as_ref()
            .ok_or_else(|| invalid("Saved Runtime edits require an open profile"))?;
        let sharing = &profile.session.0.configuration;
        let (runtime, shared_revision) = {
            let shared = sharing
                .state
                .lock()
                .map_err(|_| invalid("Profile settings are unavailable"))?;
            let runtime = shared
                .runtime
                .as_ref()
                .ok_or_else(|| invalid("Saved Runtime settings are unavailable"))?;
            (runtime.clone(), shared.revision)
        };
        let mut layers = self.configuration.layers().clone();
        layers.runtime = runtime;
        Ok(SavedRuntimeEditCapture {
            request,
            sharing: Arc::downgrade(sharing),
            shared_revision,
            layers: Arc::new(layers),
        })
    }

    pub(super) fn check_saved_runtime_edit_capture(
        &self,
        capture: &SavedRuntimeEditCapture,
    ) -> Result<(), ConfigError> {
        self.ensure_configuration_writable()?;
        if !self.configuration_request_is_current(&capture.request) {
            return Err(invalid(
                "The saved Runtime capture belongs to a retired configuration",
            ));
        }
        let actual = self
            .active_profile
            .as_ref()
            .ok_or_else(|| invalid("The saved Runtime owner is unavailable"))?;
        let sharing = &actual.session.0.configuration;
        if !Weak::ptr_eq(&capture.sharing, &Arc::downgrade(sharing)) {
            return Err(invalid("The saved Runtime sharing owner changed"));
        }
        let shared = sharing
            .state
            .lock()
            .map_err(|_| invalid("Profile settings are unavailable"))?;
        if shared.revision != capture.shared_revision
            || shared.runtime.as_ref() != Some(&capture.layers.runtime)
        {
            return Err(invalid(
                "Shared profile settings changed since the saved Runtime capture",
            ));
        }
        Ok(())
    }

    /// Prepares a replacement Runtime over this exact saved-source capture.
    /// Other layers and activation boundaries remain this owner's captured
    /// inputs. The result is a fresh proposal: engine validation, shared-context
    /// preflight, reservation and the existing writer/publisher still apply.
    ///
    /// # Errors
    /// Rejects stale captures or invalid final configuration. This method does
    /// not write, publish, bypass validation or grant native input authority.
    pub fn prepare_saved_runtime_edits(
        &self,
        capture: &SavedRuntimeEditCapture,
        runtime: RuntimeOverrides,
    ) -> Result<PreparedConfigurationUpdate, ConfigError> {
        self.check_saved_runtime_edit_capture(capture)?;
        let mut layers = (*capture.layers).clone();
        layers.runtime = runtime;
        let mut state = self.configuration.clone();
        state.update_layers_at(layers, ConfigurationApplyBoundary::RuntimeUpdate)?;
        state.policy_sources().validate_raw_sources()?;
        Ok(PreparedConfigurationUpdate {
            owner: capture.request.owner.clone(),
            expected_revision: capture.request.revision,
            profile_revision: capture.shared_revision,
            state,
            preparation_identity: Arc::new(()),
            admission: ConfigurationAdmission::Fresh,
        })
    }
}

fn invalid(message: &str) -> ConfigError {
    ConfigError::Validation(message.into())
}
