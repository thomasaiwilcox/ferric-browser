//! Persistent settings complete on an owned worker before UI publication.

use ferric_browser_config::{
    ConfigError, RuntimeOverrideWriteOutcome, save_runtime_overrides_atomic,
};
use ferric_browser_runtime::worker::{Poll, SingleFlightWorker};

use crate::{BrowserApplication, ConfigurationReservation, PreparedConfigurationUpdate};

/// Fixed persistence facts for an admitted writer, without publication authority
/// or private filesystem diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigurationPersistenceOutcome {
    Saved { durability_uncertain: bool },
    Failed,
    Unknown,
}

pub type ConfigurationWriteObserver =
    Box<dyn FnOnce(ConfigurationPersistenceOutcome) + Send + 'static>;

struct WriteObservation {
    observer: Option<ConfigurationWriteObserver>,
    outcome: ConfigurationPersistenceOutcome,
}

impl WriteObservation {
    fn new(observer: Option<ConfigurationWriteObserver>) -> Self {
        Self {
            observer,
            outcome: ConfigurationPersistenceOutcome::Unknown,
        }
    }
}

impl Drop for WriteObservation {
    fn drop(&mut self) {
        if let Some(observer) = self.observer.take() {
            observer(self.outcome);
        }
    }
}

pub struct PersistedConfigurationUpdate {
    pub prepared: PreparedConfigurationUpdate,
    pub outcome: RuntimeOverrideWriteOutcome,
    /// Replacement committed, but this observer cannot safely activate it.
    /// The outcome remains committed even if ownership/synchronization changed.
    pub publication_error: Option<ConfigError>,
}

struct WriteRequest {
    reservation: ConfigurationReservation,
    path: std::path::PathBuf,
    overrides: ferric_browser_config::RuntimeOverrides,
    _profile_lock: Option<ferric_browser_storage::ProfileLock>,
    shared: std::sync::Arc<crate::profile_configuration::SharedConfiguration>,
    expected_profile_revision: u64,
    profile_revision: u64,
    // Last: abandoning the request releases its reservation before reporting.
    observation: WriteObservation,
}

struct WriteResult {
    reservation: ConfigurationReservation,
    result: Result<RuntimeOverrideWriteOutcome, ConfigError>,
    profile_revision: Option<u64>,
    publication_error: Option<ConfigError>,
    // Last: an unobserved completed result must release its reservation first.
    _observation: WriteObservation,
}

pub(crate) struct ConfigurationWrite {
    prepared: PreparedConfigurationUpdate,
    worker: SingleFlightWorker<WriteRequest, WriteResult>,
}

impl BrowserApplication {
    /// Captures the exact admitted writer's candidate without polling it or
    /// touching its reservation. A retired owner yields no current preview.
    #[must_use]
    pub fn configuration_write_inspection_candidate(
        &self,
    ) -> Option<crate::ConfigurationInspectionCandidate> {
        let candidate = self
            .configuration_write
            .as_ref()?
            .prepared
            .inspection_candidate()
            .write_pending();
        self.configuration_inspection_candidate_is_current(&candidate)
            .then_some(candidate)
    }

    /// Starts a noncancellable write against the captured durable profile.
    /// The reservation and profile lock outlive an observer that closes.
    ///
    /// # Errors
    /// Rejects stale preparations, unavailable profiles, and concurrent writes.
    pub fn request_configuration_write(
        &mut self,
        prepared: PreparedConfigurationUpdate,
    ) -> Result<(), ConfigError> {
        self.request_configuration_write_with_observer(prepared, None)
    }

    /// Starts the same writer with an outcome observer which survives closing
    /// its application observer. Notification follows reservation release and
    /// precedes GUI publication; it grants no permission to activate settings.
    /// The callback must not block. A returned error is authoritative rejection,
    /// even if discarded staged work reports an unknown outcome.
    ///
    /// # Errors
    /// Returns the ordinary write-admission errors before accepting a request.
    pub fn request_configuration_write_with_observer(
        &mut self,
        mut prepared: PreparedConfigurationUpdate,
        observer: Option<ConfigurationWriteObserver>,
    ) -> Result<(), ConfigError> {
        self.ensure_configuration_writable()?;
        self.check_configuration_owner(&prepared.owner)?;
        prepared.admission.ensure_fresh()?;
        if self.configuration_write.is_some() {
            return Err(ConfigError::Validation(
                "Settings changed or are already being saved.".into(),
            ));
        }
        self.check_configuration_preparation(&prepared, self.profile_configuration_revision()?)?;
        let shared_contexts = self.preflight_shared_policy_contexts(&prepared)?;
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
        let profile_revision = {
            let state = shared.state.lock().map_err(|_| {
                ConfigError::validation(format_args!("Profile settings are unavailable."))
            })?;
            // This check deliberately accepts our own busy reservation gate.
            self.check_configuration_preparation(&prepared, state.revision)?;
            if let Some(contexts) = shared_contexts {
                contexts.check_current(&shared)?;
            }
            state.revision_for(&overrides)?
        };
        let mut worker =
            SingleFlightWorker::spawn("ferric-settings-persistence", write_configuration).map_err(
                |error| {
                    ConfigError::Validation(format!("Settings worker could not start: {error}"))
                },
            )?;
        worker
            .submit(WriteRequest {
                reservation,
                path,
                overrides,
                _profile_lock: profile_lock,
                shared,
                expected_profile_revision: prepared.profile_revision,
                profile_revision,
                observation: WriteObservation::new(observer),
            })
            .map_err(|error| {
                ConfigError::Validation(format!("Settings write could not start: {error:?}"))
            })?;
        prepared.admission = crate::configuration::ConfigurationAdmission::RuntimeWrite;
        self.configuration_write = Some(ConfigurationWrite { prepared, worker });
        Ok(())
    }

    #[must_use]
    pub const fn configuration_write_pending(&self) -> bool {
        self.configuration_write.is_some()
    }

    /// Detaches only this observer. The existing worker keeps its admitted
    /// request, profile lease and reservation until its actual outcome.
    pub fn detach_configuration_write_observer(&mut self) {
        drop(self.configuration_write.take());
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
            Poll::Ready(result) => Some(self.finish_configuration_write(pending.prepared, result)),
        }
    }

    fn finish_configuration_write(
        &self,
        mut prepared: PreparedConfigurationUpdate,
        result: WriteResult,
    ) -> Result<PersistedConfigurationUpdate, ConfigError> {
        let owner_error = self.release_configuration_reload(result.reservation).err();
        let outcome = result.result?;
        if let Some(revision) = result.profile_revision {
            prepared.profile_revision = revision;
        }
        Ok(PersistedConfigurationUpdate {
            prepared,
            outcome,
            publication_error: result.publication_error.or(owner_error),
        })
    }
}

fn write_configuration(mut input: WriteRequest) -> WriteResult {
    let result = save_runtime_overrides_atomic(&input.path, &input.overrides);
    input.observation.outcome = match &result {
        Ok(outcome) => ConfigurationPersistenceOutcome::Saved {
            durability_uncertain: matches!(
                outcome,
                RuntimeOverrideWriteOutcome::DurabilityUncertain(_)
            ),
        },
        Err(_) => ConfigurationPersistenceOutcome::Failed,
    };
    let mut profile_revision = None;
    let mut publication_error = None;
    if result.is_ok() {
        // The admitted write belongs to its captured root, independently of a
        // retired observer. Successful replacement can never become rollback.
        match input.shared.state.lock() {
            Ok(mut state) if state.revision == input.expected_profile_revision => {
                if state.runtime.as_ref() != Some(&input.overrides) {
                    state.runtime = Some(input.overrides);
                }
                state.revision = input.profile_revision;
                profile_revision = Some(input.profile_revision);
            }
            Ok(_) => {
                publication_error = Some(ConfigError::validation(format_args!(
                    "Settings were saved but profile settings changed during synchronization."
                )));
            }
            Err(_) => {
                publication_error = Some(ConfigError::validation(format_args!(
                    "Settings were saved but profile synchronization is unavailable."
                )));
            }
        }
    }
    if profile_revision.is_some() {
        input.shared.wake_policy_owners();
    }
    WriteResult {
        reservation: input.reservation,
        result,
        profile_revision,
        publication_error,
        _observation: input.observation,
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

    fn interaction_candidate(app: &mut BrowserApplication) -> PreparedConfigurationUpdate {
        let mut layers = app.configuration_layers().clone();
        let rule = |id: &str, patch: &str| ferric_browser_config::SiteRule {
            id: id.into(),
            pattern: "https://example.test/*".into(),
            priority: 0,
            set: std::collections::BTreeMap::new(),
            interaction: Some(toml::from_str(patch).unwrap()),
        };
        layers.base.site_rules.push(rule(
            "anchor",
            "[[anchors]]\nid='main'\ncode='m'\nfamilies=['links']\ntarget_selector='#main'",
        ));
        app.activate_configuration(layers.clone()).unwrap();
        layers.runtime.site_rules.push(rule("collection", "[[collections]]\nid='list'\nfamilies=['links']\ncodes=['m']\ncontainer_self=true\nitem_selector='a.item'\nprimary_self=true\norder='dom'"));
        app.prepare_configuration_reload(layers).unwrap()
    }

    fn known_page(app: &mut BrowserApplication) -> ferric_browser_core::WindowId {
        use ferric_browser_core::{Event, ValidatedUrl};
        use ferric_browser_runtime::RuntimeInput;
        let window = app.active_window().unwrap();
        let tab = app.active_tab().unwrap().id;
        app.dispatch_runtime(RuntimeInput::Lifecycle(Event::RestoreTabAddress {
            target: app.capture_target(tab).unwrap(),
            url: ValidatedUrl::parse("https://example.test/page").unwrap(),
        }))
        .unwrap();
        window
    }

    #[test]
    fn known_interaction_conflict_rejects_before_durable_worker_admission() {
        let (mut app, roots) = fixture();
        let prepared = interaction_candidate(&mut app);
        known_page(&mut app);
        let before = app.configuration_snapshot();
        let sources = app.configuration_policy_sources();
        let revision = app.profile_configuration_revision().unwrap();
        let path = roots.state.join("runtime-overrides.toml");
        assert!(!path.exists());
        let error = app
            .request_configuration_write(prepared)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("Interaction active policy conflicts"),
            "{error}"
        );
        assert!(!app.configuration_write_pending());
        assert!(!path.exists());
        assert_eq!(app.configuration_snapshot(), before);
        assert_eq!(app.configuration_policy_sources(), sources);
        assert_eq!(app.profile_configuration_revision().unwrap(), revision);
        assert!(app.reserve_configuration_reload().is_ok());
    }

    #[test]
    fn a_context_opened_after_durable_admission_cannot_revoke_the_saved_receipt() {
        let (mut app, roots) = fixture();
        let prepared = interaction_candidate(&mut app);
        assert!(prepared.admission.requires_context_preflight());
        app.request_configuration_write(prepared).unwrap();
        let window = known_page(&mut app);
        let receipt = wait(&mut app).unwrap();
        assert!(receipt.publication_error.is_none());
        assert!(!receipt.prepared.admission.requires_context_preflight());
        assert_eq!(receipt.outcome, RuntimeOverrideWriteOutcome::Durable);
        assert_eq!(
            load_runtime_overrides(roots.state.join("runtime-overrides.toml"))
                .unwrap()
                .site_rules
                .len(),
            1
        );
        app.publish_configuration_update(receipt.prepared, || Ok(()))
            .unwrap();
        let policy = app
            .refresh_interaction_snapshot(window, "https://example.test/page")
            .unwrap();
        assert_eq!(
            policy.state(),
            crate::InteractionCompilationState::Conflicted
        );
        assert!(policy.policy().is_none());
        assert!(policy.diagnostic().unwrap().contains("duplicate"));
    }

    #[test]
    fn writer_inspection_preserves_active_state_reservation_and_worker_ownership() {
        use crate::{
            ConfigurationInspectionCandidateStage, SettingActivationBoundary,
            SettingInspectionPending, SettingInspectionRequest,
        };
        let (mut app, _roots) = fixture();
        let before = app.configuration_snapshot();
        app.request_configuration_write(preparation(&app)).unwrap();
        let inputs = app.configuration_inspection_inputs();
        let request = SettingInspectionRequest::new("content.zoom", None, true).unwrap();
        for _ in 0..3 {
            let candidate = app.configuration_write_inspection_candidate().unwrap();
            assert_eq!(
                candidate.stage(),
                ConfigurationInspectionCandidateStage::WritePending
            );
            let report = inputs
                .inspect_setting_with_candidate(&request, &candidate)
                .unwrap();
            assert_eq!(
                report
                    .active
                    .unwrap()
                    .value
                    .unwrap()
                    .as_float()
                    .map(f64::to_bits),
                Some(1.0_f64.to_bits())
            );
            assert_eq!(
                report
                    .desired
                    .unwrap()
                    .value
                    .unwrap()
                    .as_float()
                    .map(f64::to_bits),
                Some(1.25_f64.to_bits())
            );
            assert!(matches!(
                report.pending,
                SettingInspectionPending::Pending {
                    boundary: SettingActivationBoundary::Publication,
                    ..
                }
            ));
            assert!(app.configuration_write_pending());
            assert!(
                app.prepare_configuration_update(ConfigurationLayers::new(Config::default()))
                    .is_err()
            );
            assert_eq!(app.configuration_snapshot(), before);
        }
        let receipt = wait(&mut app).unwrap();
        assert!(app.configuration_write_inspection_candidate().is_none());
        let candidate = receipt.prepared.inspection_candidate();
        assert_eq!(
            candidate.stage(),
            ConfigurationInspectionCandidateStage::Prepared
        );
        assert_eq!(app.configuration_snapshot(), before);
        app.publish_configuration_update(receipt.prepared, || Ok(()))
            .unwrap();
        assert!(!app.configuration_inspection_candidate_is_current(&candidate));
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
    fn detached_write_results_release_the_gate_before_notifying_the_original_observer() {
        use std::sync::{atomic::Ordering, mpsc};
        for destroy_application in [false, true] {
            let (app, roots) = fixture();
            let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "peer").unwrap();
            let peer = app.inherit_profile(runtime).unwrap();
            let gate = app.configuration_reservation.clone();
            let mut owner = Some(app);
            let app = owner.as_mut().unwrap();
            let (sender, receiver) = mpsc::sync_channel(1);
            app.request_configuration_write_with_observer(
                preparation(app),
                Some(Box::new(move |outcome| {
                    sender
                        .send((outcome, gate.load(Ordering::Acquire)))
                        .unwrap();
                })),
            )
            .unwrap();
            assert!(receiver.try_recv().is_err());
            if destroy_application {
                drop(owner.take());
            } else {
                owner.as_mut().unwrap().retire_shared_policy_observer();
                owner
                    .as_mut()
                    .unwrap()
                    .detach_configuration_write_observer();
                assert!(!owner.as_ref().unwrap().configuration_write_pending());
            }
            assert_eq!(
                receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
                (
                    ConfigurationPersistenceOutcome::Saved {
                        durability_uncertain: false
                    },
                    false
                ),
            );
            let layers = peer.peer_configuration_layers().unwrap().unwrap();
            assert!((layers.resolve().unwrap().content.zoom - 1.25).abs() < f64::EPSILON);
            assert_eq!(
                load_runtime_overrides(roots.state.join("runtime-overrides.toml")).unwrap(),
                layers.runtime
            );
            assert!(peer.reserve_configuration_reload().is_ok());
        }
    }

    #[test]
    fn detached_failed_writes_report_failure_and_unobserved_work_stays_unknown() {
        use std::sync::{atomic::Ordering, mpsc};
        let (mut app, roots) = fixture();
        std::fs::create_dir(roots.state.join("runtime-overrides.toml")).unwrap();
        let gate = app.configuration_reservation.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        app.request_configuration_write_with_observer(
            preparation(&app),
            Some(Box::new(move |outcome| {
                sender
                    .send((outcome, gate.load(Ordering::Acquire)))
                    .unwrap();
            })),
        )
        .unwrap();
        app.detach_configuration_write_observer();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
            (ConfigurationPersistenceOutcome::Failed, false)
        );
        assert!(roots.state.join("runtime-overrides.toml").is_dir());
        let (sender, receiver) = mpsc::sync_channel(1);
        let stopped = std::thread::spawn(move || {
            let _observation = WriteObservation::new(Some(Box::new(move |outcome| {
                sender.send(outcome).unwrap();
            })));
            panic!("injected unobserved worker stop");
        });
        assert!(stopped.join().is_err());
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
            ConfigurationPersistenceOutcome::Unknown
        );
    }

    #[test]
    fn rejected_write_never_acquires_an_admitted_observation() {
        use std::sync::mpsc;
        let (mut app, _roots) = fixture();
        let prepared = preparation(&app);
        let mut layers = app.configuration_layers().clone();
        layers.temporary.set_literal("content.zoom", "1.5").unwrap();
        app.update_configuration(layers).unwrap();
        let (sender, receiver) = mpsc::sync_channel(1);
        assert!(
            app.request_configuration_write_with_observer(
                prepared,
                Some(Box::new(move |outcome| {
                    sender.send(outcome).unwrap();
                }))
            )
            .is_err()
        );
        assert_eq!(receiver.try_recv(), Err(mpsc::TryRecvError::Disconnected));
        assert!(!app.configuration_write_pending());
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

    #[test]
    fn replacement_stays_committed_when_the_observer_changes_profile() {
        let (mut app, roots) = fixture();
        let path = roots.state.join("runtime-overrides.toml");
        app.request_configuration_write(preparation(&app)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !path.exists() {
            assert!(Instant::now() < deadline, "replacement did not complete");
            std::thread::yield_now();
        }
        let (other, other_roots) = fixture();
        let snapshot = other.profile_snapshot().unwrap();
        app.activate_profile(ProfileActivation {
            name: snapshot.name,
            id: snapshot.id,
            privacy: snapshot.privacy,
            roots: Some(other_roots.clone()),
            lock: None,
            contexts: None,
            storage_path: Some(other_roots.data.join("browser.sqlite")),
        })
        .unwrap();
        let before = app.configuration_snapshot();
        let sources = app.configuration_policy_sources();
        let receipt = wait(&mut app).expect("the original profile's replacement committed");
        assert_eq!(receipt.outcome, RuntimeOverrideWriteOutcome::Durable);
        assert!(receipt.publication_error.is_some());
        let persisted = std::cell::Cell::new(false);
        assert!(
            app.publish_configuration_update(receipt.prepared, || {
                persisted.set(true);
                Ok(())
            })
            .is_err()
        );
        assert!(!persisted.get());
        assert_eq!(app.configuration_snapshot(), before);
        assert_eq!(app.configuration_policy_sources(), sources);
        assert_eq!(
            load_runtime_overrides(path).unwrap().settings["content.zoom"].as_float(),
            Some(1.25)
        );
        assert!(!other_roots.state.join("runtime-overrides.toml").exists());
    }

    #[test]
    fn replacement_stays_committed_when_shared_synchronization_fails() {
        let (app, roots) = fixture();
        let prepared = preparation(&app);
        let profile = app.active_profile.as_ref().unwrap();
        let shared = profile.session.0.configuration.clone();
        let overrides = prepared.state.layers().runtime.clone();
        let input = WriteRequest {
            reservation: app.reserve_configuration_reload().unwrap(),
            path: roots.state.join("runtime-overrides.toml"),
            profile_revision: shared
                .state
                .lock()
                .unwrap()
                .revision_for(&overrides)
                .unwrap(),
            expected_profile_revision: prepared.profile_revision,
            overrides,
            _profile_lock: profile.persistence_lease(),
            shared: shared.clone(),
            observation: WriteObservation::new(None),
        };
        let before = app.configuration_snapshot();
        let sources = app.configuration_policy_sources();
        assert!(
            std::panic::catch_unwind(|| {
                let _guard = shared.state.lock().unwrap();
                panic!("injected profile synchronization failure");
            })
            .is_err()
        );
        let result = write_configuration(input);
        let receipt = app.finish_configuration_write(prepared, result).unwrap();
        assert_eq!(receipt.outcome, RuntimeOverrideWriteOutcome::Durable);
        assert!(receipt.publication_error.is_some());
        assert_eq!(app.configuration_snapshot(), before);
        assert_eq!(app.configuration_policy_sources(), sources);
        assert_eq!(
            load_runtime_overrides(roots.state.join("runtime-overrides.toml"))
                .unwrap()
                .settings["content.zoom"]
                .as_float(),
            Some(1.25)
        );
    }

    #[test]
    fn inherited_peer_cannot_publish_a_foreign_preparation_with_equal_revisions() {
        let (app, _roots) = fixture();
        let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "peer").unwrap();
        let mut peer = app.inherit_profile(runtime).unwrap();
        assert_eq!(app.profile_snapshot(), peer.profile_snapshot());
        assert_eq!(app.configuration_revision(), peer.configuration_revision());
        assert_eq!(
            app.profile_configuration_revision().unwrap(),
            peer.profile_configuration_revision().unwrap()
        );
        let before = peer.configuration_snapshot();
        let persisted = std::cell::Cell::new(false);
        assert!(
            peer.publish_configuration_update(preparation(&app), || {
                persisted.set(true);
                Ok(())
            })
            .is_err()
        );
        assert!(!persisted.get());
        assert_eq!(peer.configuration_snapshot(), before);
    }

    #[test]
    fn a_peer_save_during_consumer_compilation_invalidates_the_captured_revision() {
        let (mut app, _roots) = fixture();
        let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "peer").unwrap();
        let mut peer = app.inherit_profile(runtime).unwrap();
        let prepared = preparation(&app)
            .prepare_policy_consumer(|_, _| {
                let mut layers = peer.configuration_layers().clone();
                layers.runtime.set_literal("content.zoom", "1.75")?;
                peer.update_configuration(layers)?;
                Ok(())
            })
            .unwrap();
        let before = app.configuration_snapshot();
        let sources = app.configuration_policy_sources();
        let persisted = std::cell::Cell::new(false);
        assert!(
            app.publish_configuration_consumer(prepared, || {
                persisted.set(true);
                Ok(())
            })
            .is_err()
        );
        assert!(!persisted.get());
        assert_eq!(app.configuration_snapshot(), before);
        assert_eq!(app.configuration_policy_sources(), sources);
        let layers = app.peer_configuration_layers().unwrap().unwrap();
        assert_eq!(
            layers.runtime.settings["content.zoom"].as_float(),
            Some(1.75)
        );
    }

    #[test]
    fn inherited_peer_cannot_release_the_original_windows_reservation() {
        let (app, _roots) = fixture();
        let (runtime, _, _) = BrowserRuntime::bootstrap(PrivacyKind::Normal, "peer").unwrap();
        let peer = app.inherit_profile(runtime).unwrap();
        let reservation = app.reserve_configuration_reload().unwrap();
        assert!(peer.release_configuration_reload(reservation).is_err());
        // The consumed handle cancels on drop, preserving existing cancellation.
        assert!(app.reserve_configuration_reload().is_ok());
    }

    #[test]
    fn foreign_async_admission_cannot_write_into_the_destination_profile() {
        let (app, _source_roots) = fixture();
        let (mut other, roots) = fixture();
        let path = roots.state.join("runtime-overrides.toml");
        let bytes = b"schema_version=3\n[settings]\n'content.zoom'=1.0\n";
        std::fs::write(&path, bytes).unwrap();
        let before = other.configuration_snapshot();
        let revision = other.profile_configuration_revision().unwrap();
        let admitted = other.request_configuration_write(preparation(&app));
        if admitted.is_ok() {
            // Observe any incorrectly admitted write before checking its effects.
            let _ = wait(&mut other);
        }
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        assert!(admitted.is_err());
        assert!(!other.configuration_write_pending());
        assert_eq!(other.configuration_snapshot(), before);
        assert_eq!(other.profile_configuration_revision().unwrap(), revision);
    }

    #[test]
    fn changed_profile_rejects_preparation_even_with_equal_scalar_revisions() {
        let (mut app, _roots) = fixture();
        let prepared = preparation(&app);
        let (other, roots) = fixture();
        let snapshot = other.profile_snapshot().unwrap();
        app.activate_profile(ProfileActivation {
            name: snapshot.name,
            id: snapshot.id,
            privacy: snapshot.privacy,
            roots: Some(roots.clone()),
            lock: None,
            contexts: None,
            storage_path: Some(roots.data.join("browser.sqlite")),
        })
        .unwrap();
        assert_eq!(app.configuration_revision(), prepared.expected_revision);
        assert_eq!(
            app.profile_configuration_revision().unwrap(),
            prepared.profile_revision
        );
        let before = app.configuration_snapshot();
        let persisted = std::cell::Cell::new(false);
        assert!(
            app.publish_configuration_update(prepared, || {
                persisted.set(true);
                Ok(())
            })
            .is_err()
        );
        assert!(!persisted.get());
        assert_eq!(app.configuration_snapshot(), before);
    }

    #[test]
    fn exhausted_shared_revision_rejects_before_sync_persistence() {
        let (mut app, _roots) = fixture();
        app.active_profile
            .as_ref()
            .unwrap()
            .session
            .0
            .configuration
            .state
            .lock()
            .unwrap()
            .revision = u64::MAX;
        let prepared = preparation(&app);
        let before = app.configuration_snapshot();
        let persisted = std::cell::Cell::new(false);
        assert!(
            app.publish_configuration_update(prepared, || {
                persisted.set(true);
                Ok(())
            })
            .is_err()
        );
        assert!(!persisted.get());
        assert_eq!(app.configuration_snapshot(), before);
    }

    #[test]
    fn exhausted_shared_revision_rejects_before_async_admission() {
        let (mut app, roots) = fixture();
        app.active_profile
            .as_ref()
            .unwrap()
            .session
            .0
            .configuration
            .state
            .lock()
            .unwrap()
            .revision = u64::MAX;
        let prepared = preparation(&app);
        let before = app.configuration_snapshot();
        assert!(app.request_configuration_write(prepared).is_err());
        assert!(!app.configuration_write_pending());
        assert!(!roots.state.join("runtime-overrides.toml").exists());
        assert_eq!(app.configuration_snapshot(), before);
    }
}
