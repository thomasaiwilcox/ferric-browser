//! Qt-independent application orchestration for Ferric Browser.
//!
//! This is the only layer allowed to combine the reducer runtime, effective
//! configuration, and Rust-owned durable metadata. Presentation adapters call
//! the domain-specific methods on [`BrowserApplication`] and execute the
//! returned effects; they do not own application state or persistence
//! scheduling.

pub mod userscript;
mod userscript_discovery;
mod userscript_install;
pub mod userscript_io;
mod userscript_manifest;
mod userscript_protocol;
mod userscript_storage;

use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool},
};

use ferric_browser_config::{Config, ConfigError, ConfigurationApplyBoundary, RuntimeOverrides};
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

mod view_requests;
pub use view_requests::{
    ScriptWorld, ScrollDirection, ScrollEdge, ScrollTargetPolicy, VerticalDirection, ViewCommand,
    ViewRequest,
};
mod operations;
pub use operations::{
    OperationCatalog, OperationCompletion, OperationFailure, OperationRecord, OperationState,
    OperationUpdate,
};

mod configuration_write;
pub use configuration_write::PersistedConfigurationUpdate;

mod configuration;
mod configuration_reservation;
pub use configuration_reservation::ConfigurationReservation;
mod contexts;
mod download_catalog;
mod profile;
mod profile_configuration;
mod userscript_jobs;
pub use userscript_jobs::{
    UserscriptManagerRequest, UserscriptManagerResult, UserscriptMutationOutcome,
};
mod profile_open;
mod profile_session;
pub use download_catalog::{
    DownloadFailure, DownloadRetryState, DownloadRow, DownloadSaveFailure, DownloadSessionError,
};
mod session_download_retries;
pub use session_download_retries::DownloadRetryPermit;
mod session_downloads;
pub use session_downloads::DownloadHistoryStatus;
mod session_permissions;
mod storage;

use configuration::ConfigurationState;
pub use configuration::{ConfigurationLayers, ConfigurationSnapshot, PreparedConfigurationUpdate};
pub use contexts::{ContextCatalog, ContextReloadEffect, ContextReloadError, ContextReloadJob};
pub use ferric_browser_storage::StorageCommand as StorageRequest;
use profile::ActiveProfile;
use profile::ProfileActivation;
pub use profile::{ContextChange, ContextCommand, ContextEffect, ProfileSnapshot};
use profile_open::{PreparedProfile, ProfileOpenCoordinator};
pub use profile_open::{ProfileOpenEffect, ProfileOpenError, ProfileOpenRequest, ProfileStorage};
pub use session_permissions::{
    PermissionPreferencesStatus, PermissionResolution, SavedPermissionPreferences,
    SessionPermissionError,
};
use storage::StorageCoordinator;
pub use storage::{StorageEffect, StorageTicket};

/// The Qt-independent owner of runtime state, effective configuration, and
/// optional durable metadata work.
pub struct BrowserApplication {
    operations: OperationCatalog,
    page_scripts: Result<Arc<userscript::UserscriptCatalog>, String>,
    userscript_root: Option<std::path::PathBuf>,
    userscript_worker: Option<userscript_jobs::UserscriptManagerWorker>,
    runtime: BrowserRuntime,
    configuration: ConfigurationState,
    configuration_reservation: Arc<AtomicBool>,
    configuration_write: Option<configuration_write::ConfigurationWrite>,
    active_profile: Option<ActiveProfile>,
    next_profile_generation: u64,
    download_owner: uuid::Uuid,
    storage: StorageCoordinator,
    profile_open: Option<ProfileOpenCoordinator>,
}

impl Drop for BrowserApplication {
    fn drop(&mut self) {
        self.close_session_storage();
    }
}

impl BrowserApplication {
    #[must_use]
    pub fn new(configuration: Config) -> Self {
        Self {
            operations: OperationCatalog::default(),
            page_scripts: Ok(Arc::default()),
            userscript_root: None,
            userscript_worker: None,
            runtime: BrowserRuntime::new(),
            configuration: ConfigurationState::new(configuration),
            configuration_reservation: Arc::default(),
            configuration_write: None,
            active_profile: None,
            next_profile_generation: 1,
            download_owner: uuid::Uuid::new_v4(),
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
            operations: OperationCatalog::default(),
            page_scripts: Ok(Arc::default()),
            userscript_root: None,
            userscript_worker: None,
            runtime,
            configuration: ConfigurationState::new(configuration),
            configuration_reservation: Arc::default(),
            configuration_write: None,
            active_profile: None,
            next_profile_generation: 1,
            download_owner: uuid::Uuid::new_v4(),
            storage: StorageCoordinator::default(),
            profile_open: None,
        }
    }

    /// Creates an independent window owner from an already validated profile.
    /// Profile locks and configuration are retained without reopening files on
    /// the GUI thread. Durable metadata work remains on its own worker.
    ///
    /// # Errors
    /// Returns an error for an unavailable profile, a privacy mismatch, or a
    /// metadata worker that cannot start. The source remains unchanged.
    pub fn inherit_profile(&self, runtime: BrowserRuntime) -> Result<Self, ProfileOpenError> {
        let profile = self
            .active_profile
            .as_ref()
            .filter(|_| {
                self.profile_open.is_none()
                    && self.runtime.view().shutdown() == ferric_browser_core::ShutdownState::Running
            })
            .ok_or_else(|| {
                ProfileOpenError::InvalidRequest("The source profile is not ready".into())
            })?;
        if runtime.view().profiles().len() != 1
            || runtime
                .view()
                .profiles()
                .values()
                .any(|candidate| candidate.privacy != profile.snapshot().privacy)
        {
            return Err(ProfileOpenError::InvalidRequest(
                "Window profile privacy does not match its source".into(),
            ));
        }
        let mut storage = StorageCoordinator::default();
        storage
            .replace(profile.storage_path())
            .map_err(|error| ProfileOpenError::InvalidRequest(error.to_string()))?;
        Ok(Self {
            operations: OperationCatalog::default(),
            page_scripts: self.page_scripts.clone(),
            userscript_root: self.userscript_root.clone(),
            userscript_worker: None,
            runtime,
            configuration: self.configuration.clone(),
            configuration_reservation: self.configuration_reservation.clone(),
            configuration_write: None,
            active_profile: Some(profile.clone()),
            next_profile_generation: self.next_profile_generation,
            download_owner: uuid::Uuid::new_v4(),
            storage,
            profile_open: None,
        })
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
                operations: OperationCatalog::default(),
                page_scripts: Ok(Arc::default()),
                userscript_root: None,
                userscript_worker: None,
                runtime,
                configuration: ConfigurationState::new(configuration),
                configuration_reservation: Arc::default(),
                configuration_write: None,
                active_profile: None,
                next_profile_generation: 1,
                download_owner: uuid::Uuid::new_v4(),
                storage: StorageCoordinator::default(),
                profile_open: None,
            },
            window,
            tab,
        ))
    }

    /// Root selected by profile bootstrap; adapters may watch it but cannot
    /// redirect discovery or mutation jobs to another directory.
    #[must_use]
    pub fn userscript_root(&self) -> Option<&std::path::Path> {
        self.userscript_root.as_deref()
    }

    /// Borrows the validated manifest and source catalog without filesystem I/O.
    /// # Errors
    /// Returns the last catalog loading failure without publishing partial data.
    pub fn userscript_catalog(&self) -> Result<&userscript::UserscriptCatalog, String> {
        self.page_scripts
            .as_ref()
            .map(AsRef::as_ref)
            .map_err(Clone::clone)
    }

    /// Matches an already loaded catalog without performing filesystem I/O.
    /// # Errors
    /// Reports a catalog loading failure; invalid catalogs never partially apply.
    pub fn matching_page_scripts(
        &self,
        url: &str,
        private: bool,
    ) -> Result<Vec<userscript::PageScript>, String> {
        self.page_scripts
            .as_ref()
            .map(|catalog| catalog.matching(url, private))
            .map_err(Clone::clone)
    }

    #[must_use]
    pub fn userscript_watch_sources(&self) -> Vec<std::path::PathBuf> {
        let mut sources = self
            .page_scripts
            .as_ref()
            .map_or_else(|_| Vec::new(), |catalog| catalog.sources().to_vec());
        // A failed catalog must remain observable so correcting or adding a
        // manifest can recover it, including when the directory is created later.
        if let Some(root) = &self.userscript_root {
            sources.push(root.join("userscripts/.catalog-watch"));
        }
        sources.sort();
        sources.dedup();
        sources
    }

    pub fn replace_page_script_catalog(
        &mut self,
        catalog: Result<Arc<userscript::UserscriptCatalog>, String>,
    ) {
        self.page_scripts = catalog;
    }

    #[must_use]
    pub const fn operations(&self) -> &OperationCatalog {
        &self.operations
    }

    pub fn operations_mut(&mut self) -> &mut OperationCatalog {
        &mut self.operations
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
    pub fn configuration_layers(&self) -> &ConfigurationLayers {
        self.configuration.layers()
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
        let mut layers = self.configuration.layers().clone();
        layers.temporary = overrides.clone();
        self.update_configuration(layers)
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
        self.ensure_configuration_writable()?;
        self.configuration.activate_layers(layers)
    }

    /// Atomically updates live settings and preserves pending reload-,
    /// startup- and restart-scoped changes.
    ///
    /// # Errors
    ///
    /// Returns a configuration error without changing active configuration
    /// when the candidate layers are invalid.
    pub fn update_configuration(
        &mut self,
        layers: ConfigurationLayers,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        let prepared = self.prepare_configuration_update(layers)?;
        self.publish_configuration_update(prepared, || Ok(()))
    }

    /// Prepares a runtime update without publishing it or its revision.
    ///
    /// # Errors
    /// Returns a validation error for invalid configuration layers.
    pub fn prepare_configuration_update(
        &self,
        layers: ConfigurationLayers,
    ) -> Result<PreparedConfigurationUpdate, ConfigError> {
        self.prepare_configuration_at(layers, ConfigurationApplyBoundary::RuntimeUpdate)
    }

    /// Prepares live, navigation and reload settings, retaining startup/restart values.
    ///
    /// # Errors
    /// Returns a validation error without changing the active configuration.
    pub fn prepare_configuration_reload(
        &self,
        layers: ConfigurationLayers,
    ) -> Result<PreparedConfigurationUpdate, ConfigError> {
        self.prepare_configuration_at(layers, ConfigurationApplyBoundary::Reload)
    }

    fn prepare_configuration_at(
        &self,
        layers: ConfigurationLayers,
        boundary: ConfigurationApplyBoundary,
    ) -> Result<PreparedConfigurationUpdate, ConfigError> {
        self.ensure_configuration_writable()?;
        self.check_profile_configuration(&layers)?;
        let mut state = self.configuration.clone();
        state.update_layers_at(layers, boundary)?;
        Ok(PreparedConfigurationUpdate {
            expected_revision: self.configuration.revision(),
            profile_revision: self.profile_configuration_revision()?,
            state,
        })
    }

    /// Checks freshness, performs persistence, then publishes without another
    /// fallible operation. The callback must not reenter the application.
    ///
    /// # Errors
    /// Returns an error without publishing when preparation is stale or the
    /// persistence callback rejects the update.
    pub fn publish_configuration_update(
        &mut self,
        prepared: PreparedConfigurationUpdate,
        persist: impl FnOnce() -> Result<(), ConfigError>,
    ) -> Result<ConfigurationSnapshot, ConfigError> {
        self.ensure_configuration_writable()?;
        let session = self
            .active_profile
            .as_ref()
            .map(|profile| profile.session.clone());
        let mut shared = session
            .as_ref()
            .map(|session| {
                session.0.configuration.state.lock().map_err(|_| {
                    ConfigError::Validation("Profile settings are unavailable.".into())
                })
            })
            .transpose()?;
        if self.configuration.revision() != prepared.expected_revision
            || shared.as_ref().map_or(0, |shared| shared.revision) != prepared.profile_revision
        {
            return Err(ConfigError::Validation(
                "prepared configuration became stale".into(),
            ));
        }
        persist()?;
        if let Some(shared) = shared.as_mut() {
            let runtime = &prepared.state.layers().runtime;
            if shared.runtime.as_ref() != Some(runtime) {
                shared.runtime = Some(runtime.clone());
                shared.revision = shared.revision.saturating_add(1);
            }
        }
        self.configuration = prepared.state;
        Ok(self.configuration.snapshot())
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
        self.ensure_configuration_writable()?;
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
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        self.drain_session_downloads(deadline);
        self.storage
            .replace_until(activation.storage_path.as_deref(), deadline)?;
        let generation = self.next_profile_generation;
        self.next_profile_generation = self.next_profile_generation.saturating_add(1);
        self.active_profile = Some(ActiveProfile::activate(generation, activation));
        self.configuration_reservation = self
            .active_profile
            .as_ref()
            .unwrap()
            .session
            .0
            .configuration_gate
            .clone();
        self.initialize_profile_configuration()
            .map_err(|error| StorageWorkerError::Spawn(error.to_string()))?;
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
        let configuration = match self.configuration.activate_layers(prepared.layers) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                prepared
                    .configuration_error
                    .get_or_insert_with(|| error.to_string());
                self.configuration
                    .replace_effective(prepared.fallback_configuration)
                    .map_err(|error| ProfileOpenError::InvalidRequest(error.to_string()))?
            }
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        self.drain_session_downloads(deadline);
        if let Err(error) = self
            .storage
            .replace_until(prepared.activation.storage_path.as_deref(), deadline)
        {
            prepared
                .storage_error
                .get_or_insert_with(|| error.to_string());
            prepared.activation.storage_path = None;
            self.storage.close();
        }
        self.page_scripts = prepared.page_scripts;
        self.userscript_root = prepared.userscript_root;
        self.userscript_worker = None;
        let generation = self.next_profile_generation;
        self.next_profile_generation = self.next_profile_generation.saturating_add(1);
        self.active_profile = Some(ActiveProfile::activate(generation, prepared.activation));
        self.configuration_reservation = self
            .active_profile
            .as_ref()
            .unwrap()
            .session
            .0
            .configuration_gate
            .clone();
        if let Err(error) = self.initialize_profile_configuration() {
            prepared.configuration_error = Some(error.to_string());
        }
        let profile = self
            .profile_snapshot()
            .ok_or(ProfileOpenError::WorkerStopped)?;
        // Independently opened windows share the first owner's startup source
        // set. Later bootstrap scans can already contain this run's files.
        let recovery_paths = self
            .active_profile
            .as_ref()
            .unwrap()
            .session
            .0
            .recovery_paths
            .get_or_init(|| prepared.recovery_paths)
            .clone();
        Ok(ProfileOpenEffect {
            configuration,
            profile,
            config_sources: prepared.config_sources,
            profile_overrides: prepared.profile_overrides,
            runtime_overrides: prepared.runtime_overrides,
            session_path: prepared.session_path,
            session_state_root: prepared.session_state_root,
            recovery_paths,
            storage_error: prepared.storage_error,
            configuration_error: prepared.configuration_error,
            context_error: prepared.context_error,
        })
    }

    /// Closes the active profile after draining its durable worker.
    pub fn close_profile(&mut self) {
        self.operations.retire();
        self.configuration_write.take();
        self.userscript_worker.take();
        self.userscript_root.take();
        self.page_scripts = Ok(Arc::default());
        self.close_session_storage();
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
        let generation = profile.generation;
        let runtime_effects = self
            .context_catalog()
            .and_then(|catalog| self.refresh_context_catalog(&catalog))
            .unwrap_or_default();
        Ok(ContextEffect {
            generation,
            change,
            context,
            runtime_effects,
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
        let observation = session_permissions::durable::PermissionWork::prepare(
            self.active_profile
                .as_ref()
                .map(|profile| profile.session.clone()),
            &request,
        );
        self.storage.submit(
            request,
            observation.map(storage::StorageObservation::Permission),
        )
    }

    /// Polls storage lifecycle events in the adapter-facing representation.
    #[must_use]
    pub fn poll_storage_effects(&mut self) -> Vec<StorageEffect> {
        let effects = self.storage.poll();
        self.flush_session_downloads();
        effects
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
        self.close_session_storage();
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
    use ferric_browser_config::ContextsConfig;
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
                .any(|effect| matches!(effect, RuntimeEffect::UiModelChanged { .. }))
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
                script_catalog_root: None,
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
        let (runtime, _, _) =
            BrowserRuntime::bootstrap(PrivacyKind::Normal, "child").expect("child runtime");
        let child = application
            .inherit_profile(runtime)
            .expect("inherit ready profile");
        assert_eq!(child.profile_snapshot(), application.profile_snapshot());
        assert_eq!(
            child.configuration_snapshot(),
            application.configuration_snapshot()
        );
        assert_eq!(child.contexts(), application.contexts());
        application.close_profile();
        assert!(child.storage_is_open());
        assert!(child.profile_snapshot().is_some());
        drop(child);
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn profile_windows_share_recovery_sources_captured_before_current_run_writes() {
        fn open(application: &mut BrowserApplication, base: &std::path::Path) -> ProfileOpenEffect {
            application
                .request_profile_open(ProfileOpenRequest {
                    name: "default".into(),
                    label: "Default".into(),
                    privacy: PrivacyKind::Normal,
                    storage: ProfileStorage::Durable(RootSpec::Base(base.to_path_buf())),
                    base_configuration: Config::default(),
                    initial_profile_overrides: RuntimeOverrides::default(),
                    command_line_overrides: RuntimeOverrides::default(),
                    config_path: None,
                    script_catalog_root: None,
                    contexts: ContextsConfig::default(),
                    context_configuration_error: None,
                })
                .unwrap();
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(result) = application.poll_profile_open() {
                    return result.unwrap();
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "profile open timed out"
                );
                thread::yield_now();
            }
        }
        let base = std::env::temp_dir().join(format!("ferric-recovery-sources-{}", Uuid::new_v4()));
        let mut setup = BrowserApplication::new(Config::default());
        let old_path = open(&mut setup, &base).session_path.unwrap();
        drop(setup);
        fs::create_dir_all(old_path.parent().unwrap()).unwrap();
        fs::write(&old_path, b"previous-run checkpoint").unwrap();
        let mut first = BrowserApplication::new(Config::default());
        let first_effect = open(&mut first, &base);
        assert_eq!(
            first_effect.recovery_paths.as_slice(),
            std::slice::from_ref(&old_path)
        );
        let new_path = old_path.with_file_name(format!("current-{}.json", Uuid::new_v4()));
        fs::write(&new_path, b"current-run checkpoint").unwrap();
        let mut second = BrowserApplication::new(Config::default());
        let second_effect = open(&mut second, &base);
        assert_eq!(second_effect.recovery_paths, [old_path]);
        assert!(!second_effect.recovery_paths.contains(&new_path));
        drop(second);
        drop(first);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn inherited_profile_rejects_unready_closed_and_wrong_privacy_sources() {
        for privacy in [
            PrivacyKind::Normal,
            PrivacyKind::Private,
            PrivacyKind::Ephemeral,
        ] {
            let (mut source, _, _) =
                BrowserApplication::bootstrap(Config::default(), privacy, "source")
                    .expect("source runtime");
            let child_runtime = || {
                BrowserRuntime::bootstrap(privacy, "child")
                    .expect("child runtime")
                    .0
            };
            assert!(source.inherit_profile(child_runtime()).is_err());
            source
                .activate_profile(ProfileActivation {
                    name: "source".into(),
                    id: None,
                    privacy,
                    roots: None,
                    lock: None,
                    contexts: None,
                    storage_path: None,
                })
                .expect("activate profile");
            let wrong_privacy = if privacy == PrivacyKind::Normal {
                PrivacyKind::Private
            } else {
                PrivacyKind::Normal
            };
            let wrong_runtime = BrowserRuntime::bootstrap(wrong_privacy, "wrong")
                .expect("wrong runtime")
                .0;
            assert!(source.inherit_profile(wrong_runtime).is_err());
            let child = source
                .inherit_profile(child_runtime())
                .expect("inherit profile");
            source
                .dispatch_runtime(RuntimeInput::Lifecycle(Event::RequestShutdown))
                .expect("begin source shutdown");
            assert!(source.inherit_profile(child_runtime()).is_err());
            source.close_profile();
            assert!(source.inherit_profile(child_runtime()).is_err());
            assert_eq!(
                child.profile_snapshot().expect("independent child").privacy,
                privacy
            );
            assert!(!child.storage_is_open());
        }
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
                runtime_effects,
            } if context.name == "research" && runtime_effects.is_empty()
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
        assert_ne!(effects, Vec::<RuntimeEffect>::new());
    }

    #[test]
    fn storage_intents_cannot_bypass_the_application_owner() {
        let mut application = BrowserApplication::new(Config::default());
        let error = application
            .submit_storage(StorageRequest::Library { limit: 1 })
            .expect_err("unopened storage must be rejected");
        assert!(matches!(error, StorageWorkerError::Stopped));
    }

    fn wait_for_storage_effects(
        application: &mut BrowserApplication,
        count: usize,
    ) -> Vec<StorageEffect> {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut effects = Vec::new();
        loop {
            effects.extend(application.poll_storage_effects());
            if effects.len() >= count {
                return effects;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "expected {count} storage effects before the deadline, received {effects:?}"
            );
            thread::sleep(Duration::from_millis(5));
        }
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

        let effects = wait_for_storage_effects(&mut application, 1);
        assert!(
            matches!(effects[0].completion, StorageCompletion::Library(Ok(_))),
            "library request failed: {effects:?}"
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

        let effects = wait_for_storage_effects(&mut application, 2);
        assert!(
            effects
                .iter()
                .all(|effect| matches!(effect.completion, StorageCompletion::Library(Ok(_)))),
            "both application-scheduled requests must succeed: {effects:?}"
        );
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

        let effects = wait_for_storage_effects(&mut application, 2);
        let tickets = effects
            .iter()
            .map(|effect| effect.ticket)
            .collect::<Vec<_>>();
        assert_eq!(tickets, vec![first, second]);
        assert!(
            effects
                .iter()
                .all(|effect| matches!(effect.completion, StorageCompletion::Library(Ok(_)))),
            "library requests failed: {effects:?}"
        );
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

        let effects = wait_for_storage_effects(&mut application, 1);
        assert!(
            matches!(effects[0].completion, StorageCompletion::Flush(Ok(()))),
            "flush request failed: {effects:?}"
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

#[cfg(test)]
mod configuration_transaction_tests {
    use super::*;

    fn changed_layers() -> ConfigurationLayers {
        let mut layers = ConfigurationLayers::new(Config::default());
        layers.runtime.set_literal("content.zoom", "1.25").unwrap();
        layers
    }

    #[test]
    fn reload_activates_selectors_and_retains_startup_restart_deferral() {
        use ferric_browser_config::{HintSelectorFamily, HintSelectorMode, HintSelectorPolicy};
        let mut app = BrowserApplication::new(Config::default());
        let mut base = Config::default();
        for family in HintSelectorFamily::ALL {
            base.hints.selectors.insert(
                family,
                HintSelectorPolicy {
                    mode: HintSelectorMode::Replace,
                    selectors: vec![],
                },
            );
        }
        base.ui.font_size_pt = 17.0;
        base.ipc.enabled = false;
        base.navigation.start_pages = vec!["rb://blank".into()];
        let layers = ConfigurationLayers::new(base);
        let live = app.prepare_configuration_update(layers.clone()).unwrap();
        assert!(live.snapshot().effective.hints.selectors.is_empty());
        assert_eq!(
            live.snapshot()
                .pending_changes
                .iter()
                .filter(|p| p.apply_time == "reload")
                .count(),
            7
        );
        let old = app.configuration_snapshot();
        let reload = app.prepare_configuration_reload(layers).unwrap();
        assert_eq!(app.configuration_snapshot(), old);
        let published = app.publish_configuration_update(reload, || Ok(())).unwrap();
        assert_eq!(published.effective.hints.selectors.len(), 7);
        assert!((published.effective.ui.font_size_pt - 17.0).abs() < f64::EPSILON);
        assert!(published.effective.ipc.enabled);
        assert_eq!(
            published.effective.navigation.start_pages,
            Config::default().navigation.start_pages
        );
        assert_eq!(published.pending_changes.len(), 2);
        assert!(
            published
                .pending_changes
                .iter()
                .all(|p| p.apply_time != "reload")
        );
        // Removal stays pending at runtime, then restores inheritance at reload.
        let defaults = ConfigurationLayers::new(Config::default());
        let removal = app.prepare_configuration_update(defaults.clone()).unwrap();
        assert_eq!(removal.snapshot().effective.hints.selectors.len(), 7);
        assert_eq!(removal.snapshot().pending_changes.len(), 7);
        let removal = app.prepare_configuration_reload(defaults).unwrap();
        let published = app
            .publish_configuration_update(removal, || Ok(()))
            .unwrap();
        assert!(published.effective.hints.selectors.is_empty());
        assert_eq!(published.pending_changes, []);
    }

    #[test]
    fn reload_preparation_rejects_invalid_stale_and_failed_publication() {
        let mut app = BrowserApplication::new(Config::default());
        let before = app.configuration_snapshot();
        let mut invalid = Config::default();
        invalid.ui.font_size_pt = -1.0;
        assert!(
            app.prepare_configuration_reload(ConfigurationLayers::new(invalid))
                .is_err()
        );
        assert_eq!(app.configuration_snapshot(), before);
        let prepared = app.prepare_configuration_reload(changed_layers()).unwrap();
        assert!(
            app.publish_configuration_update(prepared, || Err(ConfigError::Validation(
                "write failed".into()
            )))
            .is_err()
        );
        assert_eq!(app.configuration_snapshot(), before);
        let prepared = app.prepare_configuration_reload(changed_layers()).unwrap();
        app.update_configuration(changed_layers()).unwrap();
        let updated = app.configuration_snapshot();
        assert!(
            app.publish_configuration_update(prepared, || panic!("stale reload must not persist"))
                .is_err()
        );
        assert_eq!(app.configuration_snapshot(), updated);
    }

    #[test]
    fn persistence_rejection_preserves_full_configuration_and_revision() {
        let mut app = BrowserApplication::new(Config::default());
        let old = app.configuration_snapshot();
        let prepared = app.prepare_configuration_update(changed_layers()).unwrap();
        assert_eq!(app.configuration_snapshot(), old);
        let result = app.publish_configuration_update(prepared, || {
            Err(ConfigError::Validation(
                "injected persistence rejection".into(),
            ))
        });
        assert!(result.is_err());
        assert_eq!(app.configuration_snapshot(), old);
    }

    #[test]
    fn stale_preparation_is_rejected_before_persistence() {
        let mut app = BrowserApplication::new(Config::default());
        let prepared = app.prepare_configuration_update(changed_layers()).unwrap();
        app.update_configuration(ConfigurationLayers::new(Config::default()))
            .unwrap();
        let old = app.configuration_snapshot();
        assert!(
            app.publish_configuration_update(prepared, || {
                panic!("stale candidate must not persist")
            })
            .is_err()
        );
        assert_eq!(app.configuration_snapshot(), old);
    }

    #[test]
    fn successful_publication_commits_once_after_persistence() {
        let mut app = BrowserApplication::new(Config::default());
        let prepared = app.prepare_configuration_update(changed_layers()).unwrap();
        let expected = prepared.snapshot();
        let mut persisted = false;
        let published = app
            .publish_configuration_update(prepared, || {
                persisted = true;
                Ok(())
            })
            .unwrap();
        assert!(persisted);
        assert_eq!(published, expected);
        assert_eq!(published.revision, 1);
        assert_eq!(app.configuration_snapshot(), published);
    }
}
