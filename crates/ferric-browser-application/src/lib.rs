//! Qt-independent application orchestration for Ferric Browser.
//!
//! This is the only layer allowed to combine the reducer runtime, effective
//! configuration, and Rust-owned durable metadata. Presentation adapters call
//! the domain-specific methods on [`BrowserApplication`] and execute the
//! returned effects; they do not own application state or persistence
//! scheduling.

use std::collections::BTreeMap;

use ferric_browser_config::{Config, ConfigError, RuntimeOverrides};
use ferric_browser_core::{
    CommandContext, CommandSource, DispatchTarget, JourneyGraph, PrivacyKind, ProfileId,
    ProfileState, ShutdownState, TabId, TabState, Target, WindowId, WindowState,
};
use ferric_browser_runtime::{
    BrowserRuntime, RuntimeEffect, RuntimeError, RuntimeInput, RuntimeSnapshot, RuntimeView,
};
use ferric_browser_storage::{
    ContextRecord, ContextRegistry, ContextRegistryError, StorageWorkerError,
};

mod configuration;
mod profile;
mod profile_open;
mod storage;

use configuration::ConfigurationState;
pub use configuration::{ConfigurationLayers, ConfigurationSnapshot};
pub use ferric_browser_storage::StorageCommand as StorageRequest;
use profile::ActiveProfile;
use profile::ProfileActivation;
pub use profile::{ContextChange, ContextCommand, ContextEffect, ProfileSnapshot};
use profile_open::{PreparedProfile, ProfileOpenCoordinator};
pub use profile_open::{ProfileOpenEffect, ProfileOpenError, ProfileOpenRequest, ProfileStorage};
use storage::StorageCoordinator;
pub use storage::{StorageEffect, StorageTicket};

/// The Qt-independent owner of runtime state, effective configuration, and
/// optional durable metadata work.
pub struct BrowserApplication {
    runtime: BrowserRuntime,
    configuration: ConfigurationState,
    active_profile: Option<ActiveProfile>,
    next_profile_generation: u64,
    storage: StorageCoordinator,
    profile_open: Option<ProfileOpenCoordinator>,
}

impl BrowserApplication {
    #[must_use]
    pub fn new(configuration: Config) -> Self {
        Self {
            runtime: BrowserRuntime::new(),
            configuration: ConfigurationState::new(configuration),
            active_profile: None,
            next_profile_generation: 1,
            storage: StorageCoordinator::default(),
            profile_open: None,
        }
    }

    /// Adopts an already bootstrapped runtime at the application boundary.
    ///
    /// This exists for presentation adapters that create their runtime during
    /// native startup. New entry points should prefer [`Self::bootstrap`].
    #[must_use]
    pub fn from_runtime(runtime: BrowserRuntime, configuration: Config) -> Self {
        Self {
            runtime,
            configuration: ConfigurationState::new(configuration),
            active_profile: None,
            next_profile_generation: 1,
            storage: StorageCoordinator::default(),
            profile_open: None,
        }
    }

    /// Creates initial state through the same application boundary used after
    /// startup.
    ///
    /// # Errors
    ///
    /// Returns an error if the runtime cannot establish its initial profile,
    /// window, and tab.
    pub fn bootstrap(
        configuration: Config,
        privacy: PrivacyKind,
        profile_label: impl Into<String>,
    ) -> Result<(Self, WindowId, TabId), RuntimeError> {
        let (runtime, window, tab) = BrowserRuntime::bootstrap(privacy, profile_label)?;
        Ok((
            Self {
                runtime,
                configuration: ConfigurationState::new(configuration),
                active_profile: None,
                next_profile_generation: 1,
                storage: StorageCoordinator::default(),
                profile_open: None,
            },
            window,
            tab,
        ))
    }

    #[must_use]
    pub const fn runtime_view(&self) -> RuntimeView<'_> {
        self.runtime.view()
    }

    #[must_use]
    pub fn profiles(&self) -> &BTreeMap<ProfileId, ProfileState> {
        self.runtime_view().profiles()
    }

    #[must_use]
    pub fn windows(&self) -> &BTreeMap<WindowId, WindowState> {
        self.runtime_view().windows()
    }

    #[must_use]
    pub fn tabs(&self) -> &BTreeMap<TabId, TabState> {
        self.runtime_view().tabs()
    }

    #[must_use]
    pub fn journey(&self) -> &JourneyGraph {
        self.runtime_view().journey()
    }

    #[must_use]
    pub const fn active_window(&self) -> Option<WindowId> {
        self.runtime_view().active_window()
    }

    #[must_use]
    pub const fn last_focused_window(&self) -> Option<WindowId> {
        self.runtime_view().last_focused_window()
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.runtime_view().revision()
    }

    #[must_use]
    pub const fn shutdown(&self) -> ShutdownState {
        self.runtime_view().shutdown()
    }

    #[must_use]
    pub fn active_tab(&self) -> Option<&TabState> {
        self.runtime_view().active_tab()
    }

    #[must_use]
    pub fn capture_target(&self, tab: TabId) -> Option<Target> {
        self.runtime_view().capture_target(tab)
    }

    #[must_use]
    pub fn capture_command_context(
        &self,
        selector: DispatchTarget,
        source: CommandSource,
        count: u32,
        operation_id: Option<String>,
    ) -> CommandContext {
        self.runtime
            .capture_command_context(selector, source, count, operation_id)
    }

    #[must_use]
    pub fn snapshot(&self) -> RuntimeSnapshot {
        self.runtime.snapshot()
    }

    #[must_use]
    pub const fn configuration(&self) -> &Config {
        self.configuration.effective()
    }

    #[must_use]
    pub const fn configuration_revision(&self) -> u64 {
        self.configuration.revision()
    }

    #[must_use]
    pub fn configuration_snapshot(&self) -> ConfigurationSnapshot {
        self.configuration.snapshot()
    }

    #[must_use]
    pub fn profile_snapshot(&self) -> Option<ProfileSnapshot> {
        self.active_profile.as_ref().map(ActiveProfile::snapshot)
    }

    #[must_use]
    pub fn contexts(&self) -> Option<&[ContextRecord]> {
        self.active_profile
            .as_ref()?
            .contexts
            .as_ref()
            .map(ContextRegistry::contexts)
    }

    /// Applies one validated runtime-override layer without serializing the
    /// live configuration through a presentation format.
    ///
    /// # Errors
    ///
    /// Returns an error when the override violates the configuration schema.
    pub fn apply_configuration_layer(
        &mut self,
        overrides: &RuntimeOverrides,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        self.configuration.replace_temporary(overrides.clone())
    }

    /// Applies one runtime input and returns only runtime effects.
    ///
    /// # Errors
    ///
    /// Returns a typed runtime error when the input violates reducer or
    /// command-dispatch invariants.
    pub fn dispatch_runtime(
        &mut self,
        input: RuntimeInput,
    ) -> Result<Vec<RuntimeEffect>, RuntimeError> {
        self.runtime.handle(input)
    }

    /// Atomically activates every configuration layer during bootstrap.
    ///
    /// # Errors
    ///
    /// Returns a configuration error when the resolved layers are invalid.
    pub fn activate_configuration(
        &mut self,
        layers: ConfigurationLayers,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        self.configuration.activate_layers(layers)
    }

    /// Atomically updates every configuration layer and preserves pending
    /// startup/restart-scoped changes.
    ///
    /// # Errors
    ///
    /// Returns a configuration error without changing active configuration
    /// when the candidate layers are invalid.
    pub fn update_configuration(
        &mut self,
        layers: ConfigurationLayers,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        self.configuration.update_layers(layers)
    }

    /// Replaces the effective configuration for compatibility bootstrap paths.
    ///
    /// # Errors
    ///
    /// Returns a configuration error when the replacement is invalid.
    pub fn replace_configuration(
        &mut self,
        configuration: Config,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        self.configuration.replace_effective(configuration)
    }

    /// Activates a prepared profile and transfers all durable resources to the
    /// application owner.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the profile's durable worker cannot start.
    #[cfg(test)]
    fn activate_profile(
        &mut self,
        activation: ProfileActivation,
    ) -> Result<ProfileSnapshot, StorageWorkerError> {
        self.storage.replace(activation.storage_path.as_deref())?;
        let generation = self.next_profile_generation;
        self.next_profile_generation = self.next_profile_generation.saturating_add(1);
        self.active_profile = Some(ActiveProfile::activate(generation, activation));
        self.profile_snapshot().ok_or(StorageWorkerError::Stopped)
    }

    /// Starts one bounded, application-owned profile bootstrap operation.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid request, a pending request, or worker
    /// startup/admission failure.
    pub fn request_profile_open(
        &mut self,
        request: ProfileOpenRequest,
    ) -> Result<(), ProfileOpenError> {
        if self.profile_open.is_none() {
            self.profile_open = Some(ProfileOpenCoordinator::spawn()?);
        }
        self.profile_open
            .as_mut()
            .ok_or(ProfileOpenError::WorkerStopped)?
            .submit(request)
    }

    /// Polls profile bootstrap and atomically installs its application state.
    pub fn poll_profile_open(&mut self) -> Option<Result<ProfileOpenEffect, ProfileOpenError>> {
        let prepared = self.profile_open.as_mut()?.poll()?;
        self.profile_open = None;
        Some(prepared.and_then(|prepared| self.install_prepared_profile(prepared)))
    }

    fn install_prepared_profile(
        &mut self,
        mut prepared: PreparedProfile,
    ) -> Result<ProfileOpenEffect, ProfileOpenError> {
        let configuration = match self.activate_configuration(prepared.layers) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                prepared
                    .configuration_error
                    .get_or_insert_with(|| error.to_string());
                self.replace_configuration(prepared.fallback_configuration)
                    .map_err(|error| ProfileOpenError::InvalidRequest(error.to_string()))?
            }
        };
        if let Err(error) = self
            .storage
            .replace(prepared.activation.storage_path.as_deref())
        {
            prepared
                .storage_error
                .get_or_insert_with(|| error.to_string());
            prepared.activation.storage_path = None;
            self.storage.close();
        }
        let generation = self.next_profile_generation;
        self.next_profile_generation = self.next_profile_generation.saturating_add(1);
        self.active_profile = Some(ActiveProfile::activate(generation, prepared.activation));
        let profile = self
            .profile_snapshot()
            .ok_or(ProfileOpenError::WorkerStopped)?;
        Ok(ProfileOpenEffect {
            configuration,
            profile,
            config_sources: prepared.config_sources,
            profile_overrides: prepared.profile_overrides,
            runtime_overrides: prepared.runtime_overrides,
            session_path: prepared.session_path,
            session_state_root: prepared.session_state_root,
            storage_error: prepared.storage_error,
            configuration_error: prepared.configuration_error,
            context_error: prepared.context_error,
        })
    }

    /// Closes the active profile after draining its durable worker.
    pub fn close_profile(&mut self) {
        self.storage.close();
        self.active_profile = None;
    }

    /// Applies a context mutation to the active profile registry.
    ///
    /// # Errors
    ///
    /// Returns a context error when there is no active registry or the
    /// requested mutation is invalid.
    pub fn change_context(
        &mut self,
        command: ContextCommand,
    ) -> Result<ContextEffect, ContextRegistryError> {
        let profile = self
            .active_profile
            .as_mut()
            .ok_or(ContextRegistryError::NotFound)?;
        let registry = profile
            .contexts
            .as_mut()
            .ok_or(ContextRegistryError::NotFound)?;
        let (change, context) = match command {
            ContextCommand::Create {
                name,
                label,
                profile,
                workspace,
            } => (
                ContextChange::Created,
                registry
                    .create_with_workspace(&name, &label, &profile, workspace.as_deref())?
                    .clone(),
            ),
            ContextCommand::Remove { name } => (ContextChange::Removed, registry.remove(&name)?),
            ContextCommand::SaveMembership { name, members } => {
                registry.save_membership(&name, members)?;
                let context = registry
                    .contexts()
                    .iter()
                    .find(|context| context.name == name)
                    .cloned()
                    .ok_or(ContextRegistryError::NotFound)?;
                (ContextChange::MembershipSaved, context)
            }
        };
        Ok(ContextEffect {
            generation: profile.generation,
            change,
            context,
        })
    }

    /// Submits one storage command and returns its application correlation
    /// ticket without exposing the worker.
    ///
    /// # Errors
    ///
    /// Returns a storage error when no worker is active, the request is
    /// invalid, or bounded admission is full.
    pub fn submit_storage(
        &mut self,
        request: StorageRequest,
    ) -> Result<StorageTicket, StorageWorkerError> {
        self.storage.submit(request)
    }

    /// Polls storage lifecycle events in the adapter-facing representation.
    #[must_use]
    pub fn poll_storage_effects(&mut self) -> Vec<StorageEffect> {
        self.storage.poll()
    }

    /// Starts the single owner thread for one profile metadata database.
    ///
    /// Replacing an existing worker first retires the old profile within the
    /// same bounded durability budget used by shutdown.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker cannot be started.
    #[cfg(test)]
    pub fn open_storage(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), StorageWorkerError> {
        self.storage.replace(Some(path.as_ref()))
    }

    pub fn close_storage(&mut self) {
        self.storage.close();
    }

    #[must_use]
    pub const fn storage_is_open(&self) -> bool {
        self.storage.is_open()
    }

    /// Restarts the durable worker for the active profile after a recoverable
    /// worker failure.
    ///
    /// # Errors
    ///
    /// Returns [`StorageWorkerError::Stopped`] for transient or closed
    /// profiles, or the worker spawn error for a durable profile.
    pub fn restart_storage(&mut self) -> Result<(), StorageWorkerError> {
        let path = self
            .active_profile
            .as_ref()
            .and_then(ActiveProfile::storage_path)
            .map(std::path::Path::to_owned)
            .ok_or(StorageWorkerError::Stopped)?;
        self.storage.replace(Some(&path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferric_browser_core::{Event, Mode};
    use ferric_browser_runtime::{RuntimeCommand, RuntimeInput};
    use ferric_browser_storage::{RootSpec, StorageCompletion, StorageRoots};
    use std::{fs, thread, time::Duration};
    use uuid::Uuid;

    #[test]
    fn runtime_mutation_is_visible_only_as_runtime_effects() {
        let (mut application, window, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .expect("application bootstrap");
        let effects = application
            .dispatch_runtime(RuntimeInput::Command(RuntimeCommand::EnterMode {
                window,
                mode: Mode::Command,
            }))
            .expect("runtime input");
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, RuntimeEffect::UiModelUpdated(_)))
        );
        assert_eq!(
            application.runtime_view().windows()[&window].modes.last(),
            Some(&Mode::Command)
        );
    }

    #[test]
    fn configuration_layers_remain_typed_until_a_presentation_adapter_requests_them() {
        let mut application = BrowserApplication::new(Config::default());
        let mut overrides = RuntimeOverrides::default();
        overrides
            .set_literal("content.zoom", "1.25")
            .expect("valid override");
        let snapshot = application
            .apply_configuration_layer(&overrides)
            .expect("configuration layer");
        assert!((application.configuration().content.zoom - 1.25).abs() < f64::EPSILON);
        assert_eq!(snapshot.revision, 1);
    }

    #[test]
    fn invalid_configuration_update_is_atomic() {
        let mut application = BrowserApplication::new(Config::default());
        let before = application.configuration_snapshot();
        let mut layers = ConfigurationLayers::new(Config::default());
        layers
            .runtime
            .set_literal("content.zoom", "9.0")
            .expect("syntactically valid override");

        let result = application.update_configuration(layers);

        assert!(result.is_err());
        assert_eq!(application.configuration_snapshot(), before);
    }

    #[test]
    fn configuration_update_applies_live_values_and_reports_startup_values() {
        let mut application = BrowserApplication::new(Config::default());
        let mut layers = ConfigurationLayers::new(Config::default());
        layers
            .runtime
            .set_literal("content.zoom", "1.25")
            .expect("live override");
        layers
            .runtime
            .set_literal("session.restore", "always")
            .expect("startup override");

        let snapshot = application
            .update_configuration(layers)
            .expect("configuration update");

        assert!((snapshot.effective.content.zoom - 1.25).abs() < f64::EPSILON);
        assert_eq!(
            snapshot.effective.session.restore,
            Config::default().session.restore
        );
        assert!(
            snapshot.pending_changes.iter().any(|change| {
                change.key == "session.restore" && change.apply_time == "startup"
            })
        );
    }

    #[test]
    fn profile_lifecycle_is_owned_by_the_application() {
        let mut application = BrowserApplication::new(Config::default());
        let snapshot = application
            .activate_profile(ProfileActivation {
                name: "private".into(),
                id: None,
                privacy: PrivacyKind::Private,
                roots: None,
                lock: None,
                contexts: None,
                storage_path: None,
            })
            .expect("activate transient profile");

        assert_eq!(snapshot.generation, 1);
        assert_eq!(snapshot.privacy, PrivacyKind::Private);
        assert!(!snapshot.durable);
        assert!(!application.storage_is_open());

        application.close_profile();
        assert!(application.profile_snapshot().is_none());
    }

    #[test]
    fn profile_open_worker_installs_configuration_and_storage_as_one_application_effect() {
        let base = std::env::temp_dir().join(format!("ferric-app-profile-{}", Uuid::new_v4()));
        let (mut application, _, _) =
            BrowserApplication::bootstrap(Config::default(), PrivacyKind::Normal, "default")
                .expect("application bootstrap");
        let mut command_line = RuntimeOverrides::default();
        command_line
            .set_literal("content.zoom", "1.25")
            .expect("valid override");
        application
            .request_profile_open(ProfileOpenRequest {
                name: "default".into(),
                label: "Default".into(),
                privacy: PrivacyKind::Normal,
                storage: ProfileStorage::Durable(RootSpec::Base(base.clone())),
                base_configuration: Config::default(),
                initial_profile_overrides: RuntimeOverrides::default(),
                command_line_overrides: command_line,
                config_path: None,
                contexts: toml::from_str(
                    "schema_version = 3\n[[contexts]]\nname = \"worker-context\"\nlabel = \"Worker context\"\nprofile = \"default\"\n",
                )
                .expect("context configuration"),
                context_configuration_error: None,
            })
            .expect("profile request");

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let effect = loop {
            if let Some(result) = application.poll_profile_open() {
                break result.expect("profile effect");
            }
            assert!(
                std::time::Instant::now() < deadline,
                "profile open timed out"
            );
            thread::yield_now();
        };

        assert!(effect.profile.durable);
        assert!(application.storage_is_open());
        assert!((effect.configuration.effective.content.zoom - 1.25).abs() < f64::EPSILON);
        assert_eq!(application.profile_snapshot(), Some(effect.profile));
        assert_eq!(
            application.contexts().expect("contexts")[0].name,
            "worker-context"
        );
        application.close_profile();
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn context_mutations_cross_the_application_boundary() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("temporary roots");
        let registry = ContextRegistry::open(&roots).expect("context registry");
        let cleanup_roots = roots.clone();
        let mut application = BrowserApplication::new(Config::default());
        application
            .activate_profile(ProfileActivation {
                name: "default".into(),
                id: Some(Uuid::new_v4()),
                privacy: PrivacyKind::Normal,
                roots: Some(roots),
                lock: None,
                contexts: Some(registry),
                storage_path: None,
            })
            .expect("activate profile");

        let effect = application
            .change_context(ContextCommand::Create {
                name: "research".into(),
                label: "Research".into(),
                profile: "default".into(),
                workspace: Some("special:research".into()),
            })
            .expect("create context");
        assert!(matches!(
            effect,
            ContextEffect {
                generation: 1,
                change: ContextChange::Created,
                context,
            } if context.name == "research"
        ));
        assert_eq!(application.contexts().map(<[_]>::len), Some(1));

        application.close_profile();
        cleanup_roots.cleanup().expect("clean temporary roots");
    }

    #[test]
    fn runtime_dispatch_preserves_runtime_error_identity() {
        let mut application = BrowserApplication::new(Config::default());
        let effects = application
            .dispatch_runtime(RuntimeInput::EngineFact(Event::RequestShutdown))
            .expect("shutdown request");
        assert!(!effects.is_empty());
    }

    #[test]
    fn storage_intents_cannot_bypass_the_application_owner() {
        let mut application = BrowserApplication::new(Config::default());
        let error = application
            .submit_storage(StorageRequest::Library { limit: 1 })
            .expect_err("unopened storage must be rejected");
        assert!(matches!(error, StorageWorkerError::Stopped));
    }

    #[test]
    fn storage_results_return_as_application_effects() {
        let root = std::env::temp_dir().join(format!("ferric-application-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test storage directory");
        let database = root.join("profile.sqlite3");
        let mut application = BrowserApplication::new(Config::default());
        application.open_storage(&database).expect("open storage");
        application
            .submit_storage(StorageRequest::Library { limit: 1 })
            .expect("submit library request");

        let mut completed = false;
        for _ in 0..100 {
            let effects = application.poll_storage_effects();
            completed = effects
                .iter()
                .any(|effect| matches!(effect.completion, StorageCompletion::Library(Ok(_))));
            if completed {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            completed,
            "storage completion was not emitted through application"
        );
        drop(application);
        fs::remove_dir_all(root).expect("remove test storage directory");
    }

    #[test]
    fn application_retries_a_busy_storage_intent_without_adapter_queues() {
        let root = std::env::temp_dir().join(format!("ferric-application-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test storage directory");
        let database = root.join("profile.sqlite3");
        let mut application = BrowserApplication::new(Config::default());
        application.open_storage(&database).expect("open storage");
        for _ in 0..2 {
            application
                .submit_storage(StorageRequest::Library { limit: 1 })
                .expect("admit library request");
        }

        let mut completed = 0;
        for _ in 0..100 {
            for effect in application.poll_storage_effects() {
                if matches!(effect.completion, StorageCompletion::Library(Ok(_))) {
                    completed += 1;
                }
            }
            if completed == 2 {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(completed, 2, "both application-scheduled requests complete");
        drop(application);
        fs::remove_dir_all(root).expect("remove test storage directory");
    }

    #[test]
    fn storage_effects_preserve_the_ticket_of_each_scheduled_intent() {
        let root = std::env::temp_dir().join(format!("ferric-application-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test storage directory");
        let database = root.join("profile.sqlite3");
        let mut application = BrowserApplication::new(Config::default());
        application.open_storage(&database).expect("open storage");
        let first = application
            .submit_storage(StorageRequest::Library { limit: 1 })
            .expect("admit first request");
        let second = application
            .submit_storage(StorageRequest::Library { limit: 1 })
            .expect("queue second request");
        assert_ne!(first, second);

        let mut completed = Vec::new();
        for _ in 0..100 {
            for effect in application.poll_storage_effects() {
                if matches!(effect.completion, StorageCompletion::Library(Ok(_))) {
                    completed.push(effect.ticket);
                }
            }
            if completed.len() == 2 {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(completed, vec![first, second]);
        drop(application);
        fs::remove_dir_all(root).expect("remove test storage directory");
    }

    #[test]
    fn flush_completion_is_an_application_effect() {
        let root = std::env::temp_dir().join(format!("ferric-application-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test storage directory");
        let database = root.join("profile.sqlite3");
        let mut application = BrowserApplication::new(Config::default());
        application.open_storage(&database).expect("open storage");
        application
            .submit_storage(StorageRequest::Flush)
            .expect("submit flush request");

        let mut completed = false;
        for _ in 0..100 {
            completed = application
                .poll_storage_effects()
                .iter()
                .any(|effect| matches!(effect.completion, StorageCompletion::Flush(Ok(()))));
            if completed {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            completed,
            "flush completion was not emitted through application"
        );
        drop(application);
        fs::remove_dir_all(root).expect("remove test storage directory");
    }

    #[test]
    fn closing_storage_terminates_every_accepted_ticket() {
        let root = std::env::temp_dir().join(format!("ferric-application-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test storage directory");
        let database = root.join("profile.sqlite3");
        let mut application = BrowserApplication::new(Config::default());
        application.open_storage(&database).expect("open storage");

        let accepted = (0..10)
            .map(|_| {
                application
                    .submit_storage(StorageRequest::Library { limit: 1 })
                    .expect("admit storage request")
            })
            .collect::<Vec<_>>();

        application.close_storage();
        let mut terminated = application
            .poll_storage_effects()
            .into_iter()
            .map(|effect| effect.ticket)
            .collect::<Vec<_>>();
        terminated.sort_unstable();

        assert_eq!(terminated, accepted);
        assert!(!application.storage_is_open());
        fs::remove_dir_all(root).expect("remove test storage directory");
    }
}
