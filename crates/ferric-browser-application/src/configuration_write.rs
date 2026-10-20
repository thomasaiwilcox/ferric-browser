//! Persistent settings complete on an owned worker before UI publication.

use ferric_browser_config::{
    ConfigError, RuntimeOverrideWriteOutcome, save_runtime_overrides_atomic,
};
use ferric_browser_runtime::worker::{Poll, SingleFlightWorker};

use crate::{BrowserApplication, ConfigurationReservation, PreparedConfigurationUpdate};

pub struct PersistedConfigurationUpdate {
    pub prepared: PreparedConfigurationUpdate,
    pub outcome: RuntimeOverrideWriteOutcome,
}

struct WriteRequest {
    reservation: ConfigurationReservation,
    path: std::path::PathBuf,
    overrides: ferric_browser_config::RuntimeOverrides,
    _profile_lock: Option<ferric_browser_storage::ProfileLock>,
    shared: std::sync::Arc<crate::profile_configuration::SharedConfiguration>,
}

struct WriteResult {
    reservation: ConfigurationReservation,
    result: Result<RuntimeOverrideWriteOutcome, ConfigError>,
    profile_revision: u64,
}

pub(crate) struct ConfigurationWrite {
    prepared: PreparedConfigurationUpdate,
    worker: SingleFlightWorker<WriteRequest, WriteResult>,
}

impl BrowserApplication {
    /// Starts a noncancellable write against the captured durable profile.
    /// The reservation and profile lock outlive an observer that closes.
    ///
    /// # Errors
    /// Rejects stale preparations, unavailable profiles, and concurrent writes.
    pub fn request_configuration_write(
        &mut self,
        prepared: PreparedConfigurationUpdate,
    ) -> Result<(), ConfigError> {
        if self.configuration_write.is_some()
            || prepared.expected_revision != self.configuration.revision()
            || prepared.profile_revision != self.profile_configuration_revision()?
        {
            return Err(ConfigError::Validation(
                "Settings changed or are already being saved.".into(),
            ));
        }
        let profile = self
            .active_profile
            .as_ref()
            .filter(|profile| profile.snapshot().durable)
            .ok_or_else(|| {
                ConfigError::Validation("Persistent settings require a durable profile.".into())
            })?;
        let path = profile
            .snapshot()
            .roots
            .ok_or_else(|| ConfigError::Validation("Settings storage is unavailable.".into()))?
            .state
            .join("runtime-overrides.toml");
        let profile_lock = profile.persistence_lease();
        let shared = profile.session.0.configuration.clone();
        let overrides = prepared.state.layers().runtime.clone();
        let reservation = self.reserve_configuration_reload()?;
        if prepared.profile_revision != self.profile_configuration_revision()? {
            return Err(ConfigError::Validation(
                "Profile settings changed before write admission.".into(),
            ));
        }
        let mut worker =
            SingleFlightWorker::spawn("ferric-settings-persistence", |input: WriteRequest| {
                let result = save_runtime_overrides_atomic(input.path, &input.overrides);
                // The persistent layer belongs to the root, independently of
                // the UI observer. A retired observer cannot strand a committed
                // write behind an old in-memory layer in another live window.
                let (result, profile_revision) = match result {
                    Ok(outcome) => match input.shared.state.lock() {
                        Ok(mut state) => {
                            if state.runtime.as_ref() != Some(&input.overrides) {
                                state.runtime = Some(input.overrides);
                                state.revision = state.revision.saturating_add(1);
                            }
                            (Ok(outcome), state.revision)
                        }
                        Err(_) => (
                            Err(ConfigError::Validation(
                                "Settings were saved but profile synchronization is unavailable."
                                    .into(),
                            )),
                            0,
                        ),
                    },
                    Err(error) => (Err(error), 0),
                };
                WriteResult {
                    reservation: input.reservation,
                    result,
                    profile_revision,
                }
            })
            .map_err(|error| {
                ConfigError::Validation(format!("Settings worker could not start: {error}"))
            })?;
        worker
            .submit(WriteRequest {
                reservation,
                path,
                overrides,
                _profile_lock: profile_lock,
                shared,
            })
            .map_err(|error| {
                ConfigError::Validation(format!("Settings write could not start: {error:?}"))
            })?;
        self.configuration_write = Some(ConfigurationWrite { prepared, worker });
        Ok(())
    }

    #[must_use]
    pub const fn configuration_write_pending(&self) -> bool {
        self.configuration_write.is_some()
    }

    pub fn poll_configuration_write(
        &mut self,
    ) -> Option<Result<PersistedConfigurationUpdate, ConfigError>> {
        let mut pending = self.configuration_write.take()?;
        match pending.worker.poll() {
            Poll::Pending => {
                self.configuration_write = Some(pending);
                None
            }
            Poll::Stopped => Some(Err(ConfigError::Validation(
                "Settings worker stopped before completion.".into(),
            ))),
            Poll::Ready(result) => Some(
                self.release_configuration_reload(result.reservation)
                    .and(result.result)
                    .map(|outcome| PersistedConfigurationUpdate {
                        prepared: {
                            let mut prepared = pending.prepared;
                            prepared.profile_revision = result.profile_revision;
                            prepared
                        },
                        outcome,
                    }),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConfigurationLayers, profile::ProfileActivation};
    use ferric_browser_config::{Config, load_runtime_overrides};
    use ferric_browser_core::PrivacyKind;
    use ferric_browser_runtime::BrowserRuntime;
    use ferric_browser_storage::{ProfileRegistry, RootSpec, StorageRoots};
    use std::time::{Duration, Instant};

    fn fixture() -> (BrowserApplication, StorageRoots) {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        let id = ProfileRegistry::open(&roots)
            .unwrap()
            .get_or_create_default()
            .unwrap()
            .id;
        let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "settings").unwrap();
        let mut app = BrowserApplication::from_runtime(runtime, Config::default());
        app.activate_profile(ProfileActivation {
            name: "default".into(),
            id: Some(id),
            privacy: PrivacyKind::Normal,
            roots: Some(roots.clone()),
            lock: None,
            contexts: None,
            storage_path: Some(roots.data.join("browser.sqlite")),
        })
        .unwrap();
        (app, roots)
    }

    fn preparation(app: &BrowserApplication) -> PreparedConfigurationUpdate {
        let mut layers = ConfigurationLayers::new(Config::default());
        layers.runtime.set_literal("content.zoom", "1.25").unwrap();
        app.prepare_configuration_update(layers).unwrap()
    }

    fn wait(app: &mut BrowserApplication) -> Result<PersistedConfigurationUpdate, ConfigError> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(result) = app.poll_configuration_write() {
                return result;
            }
            assert!(Instant::now() < deadline, "configuration worker timed out");
            std::thread::yield_now();
        }
    }

    #[test]
    fn durable_replacement_precedes_publication_and_reserves_peer_owners() {
        let (mut app, roots) = fixture();
        let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "peer").unwrap();
        let mut peer = app.inherit_profile(runtime).unwrap();
        let previous = app.configuration_snapshot();
        app.request_configuration_write(preparation(&app)).unwrap();
        assert_eq!(app.configuration_snapshot(), previous);
        assert!(
            peer.prepare_configuration_update(ConfigurationLayers::new(Config::default()))
                .is_err()
        );
        let receipt = wait(&mut app).unwrap();
        assert_eq!(app.configuration_snapshot(), previous);
        let saved = load_runtime_overrides(roots.state.join("runtime-overrides.toml")).unwrap();
        assert!(saved.settings.contains_key("content.zoom"));
        app.publish_configuration_update(receipt.prepared, || Ok(()))
            .unwrap();
        assert!((app.configuration().content.zoom - 1.25).abs() < f64::EPSILON);
        assert_eq!(app.configuration_revision(), previous.revision + 1);
        assert!(
            peer.prepare_configuration_update(ConfigurationLayers::new(Config::default()))
                .is_err()
        );
        let mut layers = peer
            .peer_configuration_layers()
            .unwrap()
            .expect("new persistent layer");
        layers.temporary.set_literal("content.zoom", "1.5").unwrap();
        let prepared = peer.prepare_configuration_update(layers).unwrap();
        peer.publish_configuration_update(prepared, || Ok(()))
            .unwrap();
        assert!((peer.configuration().content.zoom - 1.5).abs() < f64::EPSILON);
        assert!(peer.peer_configuration_layers().unwrap().is_none());
    }

    #[test]
    fn retiring_a_writer_still_synchronizes_a_live_peer() {
        let (mut app, _roots) = fixture();
        let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "peer").unwrap();
        let peer = app.inherit_profile(runtime).unwrap();
        app.request_configuration_write(preparation(&app)).unwrap();
        drop(app);
        let deadline = Instant::now() + Duration::from_secs(3);
        let layers = loop {
            if let Some(layers) = peer.peer_configuration_layers().unwrap() {
                break layers;
            }
            assert!(
                Instant::now() < deadline,
                "retired write was not synchronized"
            );
            std::thread::yield_now();
        };
        assert!((layers.resolve().unwrap().content.zoom - 1.25).abs() < f64::EPSILON);
    }

    #[test]
    fn named_profiles_share_the_authority_for_their_persistent_root() {
        let (mut app, roots) = fixture();
        let (runtime, _, _) =
            BrowserRuntime::bootstrap(PrivacyKind::Normal, "other-profile").unwrap();
        let mut other = BrowserApplication::from_runtime(runtime, Config::default());
        other
            .activate_profile(ProfileActivation {
                name: "other".into(),
                id: Some(uuid::Uuid::new_v4()),
                privacy: PrivacyKind::Normal,
                roots: Some(roots.clone()),
                lock: None,
                contexts: None,
                storage_path: Some(roots.data.join("other.sqlite")),
            })
            .unwrap();
        app.request_configuration_write(preparation(&app)).unwrap();
        assert!(
            other
                .prepare_configuration_update(ConfigurationLayers::new(Config::default()))
                .is_err()
        );
        let receipt = wait(&mut app).unwrap();
        app.publish_configuration_update(receipt.prepared, || Ok(()))
            .unwrap();
        assert!(other.peer_configuration_layers().unwrap().is_some());
    }

    #[test]
    fn failed_write_preserves_the_active_configuration_and_releases_the_gate() {
        let (mut app, roots) = fixture();
        std::fs::create_dir_all(roots.state.join("runtime-overrides.toml")).unwrap();
        let previous = app.configuration_snapshot();
        app.request_configuration_write(preparation(&app)).unwrap();
        assert!(wait(&mut app).is_err());
        assert_eq!(app.configuration_snapshot(), previous);
        assert!(
            app.prepare_configuration_update(ConfigurationLayers::new(Config::default()))
                .is_ok()
        );
    }
}
