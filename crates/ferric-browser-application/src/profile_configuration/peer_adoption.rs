//! Receive the exact authoritative shared layer without rewriting its file or
//! reclassifying an already committed change as a new settings proposal.
use crate::{
    BrowserApplication, PreparedConfigurationUpdate,
    configuration::{ConfigurationAdmission, ConfigurationOwner},
};
use ferric_browser_config::{ConfigError, ConfigurationApplyBoundary};
use std::sync::{Arc, atomic::Ordering};

impl BrowserApplication {
    /// Captures a committed shared runtime with this owner's own base, profile,
    /// CLI and temporary layers. Raw/catalogue checks are mandatory. Effective
    /// conflicts remain closed source-compilation outcomes, including contexts
    /// first opened after the originating write was admitted. This capture
    /// grants no CSS/live proof and must survive deferred validation intact.
    ///
    /// No reservation or worker is created and no file is written. A shared
    /// runtime can also have been committed only in memory in transient mode.
    ///
    /// # Errors
    /// Rejects unavailable source state and invalid raw/catalogue combinations.
    /// Publication still checks owner, local/shared revisions and actual sharing
    /// identity; equal restored values cannot revive an older capture.
    pub fn prepare_committed_peer_configuration(
        &self,
    ) -> Result<Option<PreparedConfigurationUpdate>, ConfigError> {
        if self.profile_open.is_some()
            || self.runtime.view().shutdown() != ferric_browser_core::ShutdownState::Running
            || self.configuration_reservation.load(Ordering::Acquire)
        {
            return Ok(None);
        }
        let Some(profile) = self.active_profile.as_ref() else {
            return Ok(None);
        };
        let sharing = Arc::clone(&profile.session.0.configuration);
        let (runtime, profile_revision) = {
            let shared = sharing.state.lock().map_err(|_| {
                ConfigError::validation(format_args!("Profile settings are unavailable"))
            })?;
            let Some(runtime) = shared.runtime.as_ref() else {
                return Ok(None);
            };
            if runtime == &self.configuration.layers().runtime {
                return Ok(None);
            }
            (runtime.clone(), shared.revision)
        };
        let owner = ConfigurationOwner::capture(self);
        let expected_revision = self.configuration.revision();
        let mut layers = self.configuration.layers().clone();
        layers.runtime = runtime;
        let mut state = self.configuration.clone();
        state.update_layers_at(layers, ConfigurationApplyBoundary::RuntimeUpdate)?;
        state.policy_sources().validate_raw_sources()?;
        Ok(Some(PreparedConfigurationUpdate {
            owner,
            expected_revision,
            profile_revision,
            state,
            preparation_identity: Arc::new(()),
            // An unadmitted read must not keep a retired root's in-memory
            // authority alive after its last actual profile owner closes.
            admission: ConfigurationAdmission::CommittedPeer(Arc::downgrade(&sharing)),
        }))
    }
}

#[cfg(test)]
mod tests;
