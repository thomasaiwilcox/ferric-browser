//! Profile-wide persistent settings; window overrides remain locally owned.

use crate::{BrowserApplication, ConfigurationLayers};
use ferric_browser_config::{ConfigError, RuntimeOverrides};

#[derive(Default)]
pub(crate) struct ProfileConfiguration {
    pub(crate) revision: u64,
    pub(crate) runtime: Option<RuntimeOverrides>,
}

/// Persistent overrides use one file per state root, even when multiple named
/// profiles are open. Keep its authority alive while an admitted write retires.
#[derive(Default)]
pub(crate) struct SharedConfiguration {
    pub(crate) gate: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub(crate) state: std::sync::Mutex<ProfileConfiguration>,
    pub(crate) userscript_gate: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl std::fmt::Debug for SharedConfiguration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SharedConfiguration")
            .finish_non_exhaustive()
    }
}

impl SharedConfiguration {
    pub(crate) fn for_root(root: Option<&std::path::Path>) -> std::sync::Arc<Self> {
        use std::sync::{Arc, Mutex, OnceLock, Weak};
        type Roots = Vec<(std::path::PathBuf, Weak<SharedConfiguration>)>;
        static ROOTS: OnceLock<Mutex<Roots>> = OnceLock::new();
        let Some(root) = root else {
            return Arc::default();
        };
        // The registry contains only weak ownership, with no user callbacks or
        // mutable configuration under this lock. A poisoned registry can be
        // recovered without splitting the authority for a live root.
        let mut roots = ROOTS
            .get_or_init(Mutex::default)
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        roots.retain(|(_, shared)| shared.strong_count() > 0);
        if let Some(shared) = roots
            .iter()
            .find_map(|(candidate, shared)| (candidate == root).then(|| shared.upgrade()).flatten())
        {
            return shared;
        }
        let shared = Arc::new(Self::default());
        roots.push((root.to_owned(), Arc::downgrade(&shared)));
        shared
    }
}

impl BrowserApplication {
    pub(crate) fn profile_configuration_revision(&self) -> Result<u64, ConfigError> {
        let Some(profile) = self.active_profile.as_ref() else {
            return Ok(0);
        };
        let shared = profile
            .session
            .0
            .configuration
            .state
            .lock()
            .map_err(|_| ConfigError::Validation("Profile settings are unavailable.".into()))?;
        Ok(shared.revision)
    }

    pub(crate) fn check_profile_configuration(
        &self,
        layers: &ConfigurationLayers,
    ) -> Result<(), ConfigError> {
        let Some(profile) = self.active_profile.as_ref() else {
            return Ok(());
        };
        let shared = profile
            .session
            .0
            .configuration
            .state
            .lock()
            .map_err(|_| ConfigError::Validation("Profile settings are unavailable.".into()))?;
        if let Some(runtime) = &shared.runtime
            && runtime != &self.configuration.layers().runtime
            && runtime != &layers.runtime
        {
            return Err(ConfigError::Validation(
                "Another window saved newer settings. Wait for synchronization and retry.".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn initialize_profile_configuration(&self) -> Result<(), ConfigError> {
        let Some(profile) = self.active_profile.as_ref() else {
            return Ok(());
        };
        let mut shared = profile
            .session
            .0
            .configuration
            .state
            .lock()
            .map_err(|_| ConfigError::Validation("Profile settings are unavailable.".into()))?;
        let runtime = &self.configuration.layers().runtime;
        if shared.runtime.is_none() {
            shared.runtime = Some(runtime.clone());
            shared.revision = shared.revision.saturating_add(1);
        }
        Ok(())
    }

    /// Returns the current persistent layer with this window's temporary and
    /// command-line overrides preserved. The adapter validates the candidate
    /// before using the ordinary configuration publication path.
    ///
    /// # Errors
    /// Returns an error if profile settings cannot be read.
    pub fn peer_configuration_layers(&self) -> Result<Option<ConfigurationLayers>, ConfigError> {
        if self
            .configuration_reservation
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return Ok(None);
        }
        let Some(profile) = self.active_profile.as_ref() else {
            return Ok(None);
        };
        let shared = profile
            .session
            .0
            .configuration
            .state
            .lock()
            .map_err(|_| ConfigError::Validation("Profile settings are unavailable.".into()))?;
        let Some(runtime) = shared.runtime.as_ref() else {
            return Ok(None);
        };
        if runtime == &self.configuration.layers().runtime {
            return Ok(None);
        }
        let mut layers = self.configuration.layers().clone();
        layers.runtime = runtime.clone();
        Ok(Some(layers))
    }
}
