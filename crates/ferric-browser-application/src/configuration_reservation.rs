//! A bounded publication reservation, released automatically on cancellation.
use crate::{BrowserApplication, ProfileSnapshot};
use ferric_browser_config::ConfigError;
use ferric_browser_core::ShutdownState;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Debug)]
pub struct ConfigurationReservation {
    gate: Arc<AtomicBool>,
    revision: u64,
    profile: Option<ProfileSnapshot>,
}
impl Drop for ConfigurationReservation {
    fn drop(&mut self) {
        self.gate.store(false, Ordering::Release);
    }
}
impl BrowserApplication {
    pub(crate) fn ensure_configuration_writable(&self) -> Result<(), ConfigError> {
        if self.configuration_reservation.load(Ordering::Acquire) {
            Err(ConfigError::Validation(
                "Settings are being reloaded. Try again in a moment.".into(),
            ))
        } else {
            Ok(())
        }
    }
    /// Keeps a validated asynchronous reload from racing another settings update.
    /// Dropping the reservation releases it without changing configuration.
    /// # Errors
    /// Rejects a busy or unavailable owner.
    pub fn reserve_configuration_reload(&self) -> Result<ConfigurationReservation, ConfigError> {
        if self.profile_open.is_some() || self.runtime.view().shutdown() != ShutdownState::Running {
            return Err(ConfigError::Validation(
                "This window is not ready to reload settings.".into(),
            ));
        }
        self.configuration_reservation
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| ConfigError::Validation("Settings are already being reloaded.".into()))?;
        Ok(ConfigurationReservation {
            gate: self.configuration_reservation.clone(),
            revision: self.configuration.revision(),
            profile: self.profile_snapshot(),
        })
    }
    /// Checks the original owner and releases the reservation immediately before
    /// synchronous publication. This emits no callbacks or runtime effects.
    /// # Errors
    /// Rejects a result whose application, profile or configuration changed.
    pub fn release_configuration_reload(
        &self,
        reservation: ConfigurationReservation,
    ) -> Result<(), ConfigError> {
        if !Arc::ptr_eq(&self.configuration_reservation, &reservation.gate)
            || self.configuration.revision() != reservation.revision
            || self.profile_snapshot() != reservation.profile
            || self.profile_open.is_some()
            || self.runtime.view().shutdown() != ShutdownState::Running
        {
            return Err(ConfigError::Validation(
                "The window changed during settings reload. Try again.".into(),
            ));
        }
        drop(reservation);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigurationLayers;
    use ferric_browser_config::{Config, RuntimeOverrides};
    #[test]
    fn reservation_blocks_every_configuration_entry_point_and_cancels_cleanly() {
        let mut app = BrowserApplication::new(Config::default());
        let layers = ConfigurationLayers::new(Config::default());
        let prepared = app.prepare_configuration_reload(layers.clone()).unwrap();
        let reservation = app.reserve_configuration_reload().unwrap();
        assert!(app.update_configuration(layers.clone()).is_err());
        assert!(app.activate_configuration(layers.clone()).is_err());
        assert!(app.replace_configuration(Config::default()).is_err());
        assert!(
            app.apply_configuration_layer(&RuntimeOverrides::default())
                .is_err()
        );
        assert!(app.prepare_configuration_reload(layers.clone()).is_err());
        assert!(
            app.publish_configuration_update(prepared, || Ok(()))
                .is_err()
        );
        assert!(app.reserve_configuration_reload().is_err());
        drop(reservation);
        assert!(app.update_configuration(layers).is_ok());
    }
    #[test]
    fn a_different_application_cannot_release_a_reservation() {
        let owner = BrowserApplication::new(Config::default());
        let other = BrowserApplication::new(Config::default());
        let reservation = owner.reserve_configuration_reload().unwrap();
        assert!(other.release_configuration_reload(reservation).is_err());
        assert!(owner.reserve_configuration_reload().is_ok());
    }
    #[test]
    fn valid_release_does_not_publish_a_revision() {
        let app = BrowserApplication::new(Config::default());
        let before = app.configuration_snapshot();
        let reservation = app.reserve_configuration_reload().unwrap();
        app.release_configuration_reload(reservation).unwrap();
        assert_eq!(app.configuration_snapshot(), before);
    }
}
