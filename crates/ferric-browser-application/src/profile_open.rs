use std::{
    fs,
    path::{Path, PathBuf},
};

use ferric_browser_config::{
    Config, ContextsConfig, RuntimeOverrides, load, load_profile_runtime_overrides,
    load_runtime_overrides, profile_config_path,
};
use ferric_browser_core::PrivacyKind;
use ferric_browser_storage::{
    ContextDefinitionUpdate, ContextRegistry, ProfileLock, ProfileMetadataCommit,
    ProfileMetadataSnapshot, RootSpec, StorageRoots, current_session_paths,
};

use crate::{ConfigurationLayers, ProfileActivation};

use ferric_browser_runtime::worker::{Poll as WorkerPoll, SingleFlightWorker, SubmitError};
#[cfg(test)]
use std::thread;

mod admission;
pub(crate) use admission::PublishedProfileValidation;
pub use admission::{ProfileOpenCandidate, ProfileOpenValidationCapture};

/// Typed inputs required to open one browser profile.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileOpenRequest {
    pub name: String,
    pub label: String,
    pub privacy: PrivacyKind,
    pub storage: ProfileStorage,
    pub base_configuration: Config,
    pub base_authored: std::sync::Arc<ferric_browser_config::ConfigurationSources>,
    pub initial_profile_overrides: RuntimeOverrides,
    pub command_line_overrides: RuntimeOverrides,
    pub config_path: Option<PathBuf>,
    /// Read-only installed-script root, independent of a custom configuration
    /// file's location. Transient profiles may read this catalog without
    /// acquiring durable profile storage or permission to mutate it.
    pub script_catalog_root: Option<PathBuf>,
    pub contexts: ContextsConfig,
    pub context_configuration_error: Option<String>,
}

impl ProfileOpenRequest {
    /// Capture the fallback base and its validation provenance together before
    /// profile/runtime overrides are applied or worker fields are moved.
    fn fallback_layers(&self) -> ConfigurationLayers {
        let mut layers = ConfigurationLayers::new(self.base_configuration.clone());
        layers.base_authored = self.base_authored.clone();
        layers
    }
}

/// Storage selection for a profile-open request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileStorage {
    Transient,
    Durable(RootSpec),
}

/// Failure to submit or execute profile bootstrap work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileOpenError {
    Busy,
    QueueFull,
    WorkerStopped,
    WorkerStart(String),
    InvalidRequest(String),
    AdmittedWorkFailed(String),
}

impl std::fmt::Display for ProfileOpenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => formatter.write_str("profile bootstrap is already pending"),
            Self::QueueFull => formatter.write_str("profile bootstrap queue is full"),
            Self::WorkerStopped => formatter.write_str("profile bootstrap worker stopped"),
            Self::WorkerStart(message) => {
                write!(
                    formatter,
                    "profile bootstrap worker could not start: {message}"
                )
            }
            Self::InvalidRequest(message) => formatter.write_str(message),
            Self::AdmittedWorkFailed(message) => write!(
                formatter,
                "Profile setup was admitted and may have changed durable metadata; activation did not complete: {message}"
            ),
        }
    }
}

impl std::error::Error for ProfileOpenError {}

/// Application-owned result of opening and activating one profile.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileOpenEffect {
    pub configuration: crate::ConfigurationSnapshot,
    pub profile: crate::ProfileSnapshot,
    pub config_sources: Vec<PathBuf>,
    pub profile_overrides: RuntimeOverrides,
    pub runtime_overrides: RuntimeOverrides,
    pub session_path: Option<PathBuf>,
    pub session_state_root: Option<PathBuf>,
    /// Current-session files discovered before this profile session writes.
    pub recovery_paths: Vec<PathBuf>,
    pub storage_error: Option<String>,
    pub configuration_error: Option<String>,
    pub context_error: Option<String>,
}

pub(crate) struct PreparedProfile {
    pub(crate) durable: Option<DurableProfileProposal>,
    pub(crate) preparation_error: Option<String>,
    pub(crate) pending_policy_owner: Option<crate::profile_configuration::PendingRootOwner>,
    pub(crate) userscript_root: Option<PathBuf>,
    pub(crate) page_scripts: Result<std::sync::Arc<crate::userscript::UserscriptCatalog>, String>,
    pub(crate) activation: ProfileActivation,
    pub(crate) layers: ConfigurationLayers,
    /// The successfully decoded saved source, which may differ from an
    /// engine-validated safe active fallback. It grants no activation evidence.
    pub(crate) saved_runtime_overrides: Option<RuntimeOverrides>,
    pub(crate) fallback_layers: ConfigurationLayers,
    pub(crate) config_sources: Vec<PathBuf>,
    pub(crate) session_path: Option<PathBuf>,
    pub(crate) session_state_root: Option<PathBuf>,
    pub(crate) recovery_paths: Vec<PathBuf>,
    pub(crate) storage_error: Option<String>,
    pub(crate) configuration_error: Option<String>,
    pub(crate) context_error: Option<String>,
}

pub(crate) struct DurableProfileProposal {
    name: String,
    label: String,
    roots: StorageRoots,
    definitions: Option<ContextsConfig>,
    metadata: ProfileMetadataSnapshot,
}

enum ProfileOpenWork {
    Prepare(Box<ProfileOpenRequest>),
    Commit(Box<PreparedProfile>),
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum AdmissionMode {
    Automatic,
    External,
}

enum OpenPhase {
    Preparing,
    Awaiting(std::sync::Weak<()>),
    Committing(Box<admission::ProfileOpenSeal>),
}

pub(crate) struct ProfileOpenCoordinator {
    worker: SingleFlightWorker<ProfileOpenWork, PreparedProfile>,
    mode: AdmissionMode,
    phase: OpenPhase,
}

impl ProfileOpenCoordinator {
    pub(crate) fn spawn() -> Result<Self, ProfileOpenError> {
        let worker =
            SingleFlightWorker::spawn("ferric-browser-profile-bootstrap", |work| match work {
                ProfileOpenWork::Prepare(request) => prepare_profile(*request),
                ProfileOpenWork::Commit(prepared) => commit_profile(*prepared),
            })
            .map_err(|error| ProfileOpenError::WorkerStart(error.to_string()))?;
        Ok(Self {
            worker,
            mode: AdmissionMode::Automatic,
            phase: OpenPhase::Preparing,
        })
    }

    pub(crate) fn submit(&mut self, request: ProfileOpenRequest) -> Result<(), ProfileOpenError> {
        validate_request(&request)?;
        self.worker
            .submit(ProfileOpenWork::Prepare(Box::new(request)))
            .map_err(|error| match error {
                SubmitError::Busy => ProfileOpenError::Busy,
                SubmitError::QueueFull => ProfileOpenError::QueueFull,
                SubmitError::Stopped => ProfileOpenError::WorkerStopped,
            })
    }

    fn submit_commit(&mut self, prepared: PreparedProfile) -> Result<(), ProfileOpenError> {
        self.worker
            .submit(ProfileOpenWork::Commit(Box::new(prepared)))
            .map_err(|error| match error {
                SubmitError::Busy => ProfileOpenError::Busy,
                SubmitError::QueueFull => ProfileOpenError::QueueFull,
                SubmitError::Stopped => ProfileOpenError::WorkerStopped,
            })
    }

    pub(crate) fn poll(&mut self) -> Option<Result<PreparedProfile, ProfileOpenError>> {
        match self.worker.poll() {
            WorkerPoll::Pending => None,
            WorkerPoll::Ready(profile) => Some(Ok(profile)),
            WorkerPoll::Stopped => Some(Err(ProfileOpenError::WorkerStopped)),
        }
    }
}

fn validate_request(request: &ProfileOpenRequest) -> Result<(), ProfileOpenError> {
    if request.name.is_empty() || request.label.is_empty() {
        return Err(ProfileOpenError::InvalidRequest(
            "profile name and label must not be empty".into(),
        ));
    }
    if matches!(request.storage, ProfileStorage::Transient)
        != matches!(
            request.privacy,
            PrivacyKind::Private | PrivacyKind::Ephemeral
        )
    {
        return Err(ProfileOpenError::InvalidRequest(
            "private and ephemeral profiles must use transient storage".into(),
        ));
    }
    Ok(())
}

fn prepare_profile(request: ProfileOpenRequest) -> PreparedProfile {
    let fallback_layers = request.fallback_layers();
    let (profile_overrides, config_sources, mut configuration_error) = load_profile_layers(
        request.config_path.as_deref(),
        &request.name,
        matches!(request.storage, ProfileStorage::Transient),
        request.initial_profile_overrides,
    );
    let mut saved_runtime_overrides = None;
    let mut pending_policy_owner = None;

    let activation = ProfileActivation {
        name: request.name.clone(),
        id: None,
        privacy: request.privacy,
        roots: None,
        lock: None,
        contexts: None,
        storage_path: None,
    };
    let mut durable = None;
    let mut preparation_error = None;
    let mut storage_error = None;
    let mut context_error = request.context_configuration_error;

    if let ProfileStorage::Durable(spec) = request.storage {
        match StorageRoots::resolve(spec) {
            Ok(roots) => {
                pending_policy_owner = Some(
                    crate::profile_configuration::PendingRootOwner::for_root(&roots.state),
                );
                let metadata = match ProfileMetadataSnapshot::capture(&roots) {
                    Ok(snapshot) => Some(snapshot),
                    Err(error) => {
                        preparation_error = Some(error.to_string());
                        None
                    }
                };
                if let Some(metadata) = metadata {
                    match load_runtime_overrides(roots.state.join("runtime-overrides.toml")) {
                        Ok(overrides) => saved_runtime_overrides = Some(overrides),
                        Err(error) => {
                            configuration_error.get_or_insert_with(|| error.to_string());
                        }
                    }
                    let definitions =
                        prepare_context_definitions(request.contexts, &mut context_error);
                    if let Err(error) = metadata.check_current() {
                        preparation_error = Some(error.to_string());
                    }
                    durable = Some(DurableProfileProposal {
                        name: request.name,
                        label: request.label,
                        roots,
                        definitions,
                        metadata,
                    });
                }
            }
            Err(error) => storage_error = Some(error.to_string()),
        }
    }

    let layers = ConfigurationLayers {
        base: request.base_configuration,
        base_authored: request.base_authored,
        profile: profile_overrides,
        runtime: saved_runtime_overrides.clone().unwrap_or_default(),
        command_line: request.command_line_overrides,
        temporary: RuntimeOverrides::default(),
    };
    if let Err(error) = layers.resolve() {
        configuration_error.get_or_insert_with(|| error.to_string());
    }

    let userscript_root = durable
        .as_ref()
        .map(|proposal| proposal.roots.config.clone())
        .or_else(|| {
            profile_script_root(
                &activation,
                request.script_catalog_root,
                request.config_path.as_deref(),
            )
        });
    let page_scripts = prepare_page_scripts(userscript_root.as_deref());
    PreparedProfile {
        durable,
        preparation_error,
        pending_policy_owner,
        userscript_root,
        page_scripts,
        activation,
        layers,
        saved_runtime_overrides,
        fallback_layers,
        config_sources,
        session_path: None,
        session_state_root: None,
        recovery_paths: Vec::new(),
        storage_error,
        configuration_error,
        context_error,
    }
}

fn prepare_context_definitions(
    contexts: ContextsConfig,
    error: &mut Option<String>,
) -> Option<ContextsConfig> {
    if error.is_some() {
        return None;
    }
    match ferric_browser_config::validate_contexts_configuration(&contexts) {
        Ok(()) => Some(contexts),
        Err(failure) => {
            *error = Some(failure.to_string());
            None
        }
    }
}

fn commit_profile(mut prepared: PreparedProfile) -> PreparedProfile {
    let Some(proposal) = prepared.durable.take() else {
        return prepared;
    };
    let commit = match proposal.metadata.begin_commit() {
        Ok(commit) => commit,
        Err(error) => {
            prepared.preparation_error = Some(format!(
                "Profile metadata admission rejected before catalog writes: {error}"
            ));
            return prepared;
        }
    };
    match commit_durable_profile(
        &proposal.name,
        &proposal.label,
        proposal.roots,
        proposal.definitions.as_ref(),
        &commit,
    ) {
        Ok(durable) => {
            prepared.recovery_paths =
                current_session_paths(&durable.roots.state, durable.profile_id);
            prepared.session_path = Some(
                durable
                    .roots
                    .state
                    .join("sessions")
                    .join(durable.profile_id.to_string())
                    .join("current.json"),
            );
            prepared.session_state_root = Some(durable.roots.state.clone());
            if durable.context_error.is_some() {
                prepared.context_error = durable.context_error;
            }
            prepared.activation.id = Some(durable.profile_id);
            prepared.activation.storage_path = Some(
                durable
                    .roots
                    .data
                    .join("profiles")
                    .join(durable.profile_id.to_string())
                    .join("browser.sqlite"),
            );
            prepared.activation.contexts = Some(durable.contexts);
            prepared.activation.lock = Some(durable.lock);
            prepared.activation.roots = Some(durable.roots);
        }
        Err(error) => {
            prepared.storage_error = Some(error);
            prepared.pending_policy_owner.take();
        }
    }
    prepared
}

fn load_profile_layers(
    config_path: Option<&Path>,
    name: &str,
    transient: bool,
    initial: RuntimeOverrides,
) -> (RuntimeOverrides, Vec<PathBuf>, Option<String>) {
    let mut profile_overrides = initial;
    let mut config_sources = Vec::new();
    let mut configuration_error = None;

    if let Some(config_path) = config_path {
        match load(config_path) {
            Ok(loaded) => {
                config_sources = loaded.sources;
                let profiles_path = profile_config_path(config_path);
                if profiles_path.exists() {
                    config_sources.push(profiles_path);
                }
            }
            Err(error) => configuration_error = Some(error.to_string()),
        }
        if !transient {
            match load_profile_runtime_overrides(config_path, name) {
                Ok(overrides) => profile_overrides = overrides,
                Err(error) => configuration_error = Some(error.to_string()),
            }
        }
    }

    (profile_overrides, config_sources, configuration_error)
}

fn profile_script_root(
    activation: &ProfileActivation,
    catalog: Option<PathBuf>,
    config: Option<&Path>,
) -> Option<PathBuf> {
    activation
        .roots
        .as_ref()
        .map(|roots| roots.config.clone())
        .or(catalog)
        .or_else(|| config.and_then(Path::parent).map(Path::to_owned))
}

fn prepare_page_scripts(
    root: Option<&Path>,
) -> Result<std::sync::Arc<crate::userscript::UserscriptCatalog>, String> {
    root.map_or_else(
        || Ok(crate::userscript::UserscriptCatalog::default()),
        crate::userscript::UserscriptCatalog::load,
    )
    .map(std::sync::Arc::new)
}

struct DurableProfile {
    profile_id: uuid::Uuid,
    roots: StorageRoots,
    lock: ProfileLock,
    contexts: ContextRegistry,
    context_error: Option<String>,
}

fn commit_durable_profile(
    name: &str,
    label: &str,
    roots: StorageRoots,
    definitions: Option<&ContextsConfig>,
    commit: &ProfileMetadataCommit,
) -> Result<DurableProfile, String> {
    let (profile, profile_names) = commit
        .get_or_create_profile(name, label)
        .map_err(|error| error.to_string())?;
    let profile_id = profile.id;
    let lock = ProfileLock::acquire(&roots, profile_id).map_err(|error| error.to_string())?;
    let profile_root = roots.data.join("profiles").join(profile_id.to_string());
    fs::create_dir_all(&profile_root).map_err(|error| error.to_string())?;
    set_private_directory_permissions(&profile_root).map_err(|error| error.to_string())?;
    let mut contexts = commit.open_contexts().map_err(|error| error.to_string())?;
    let context_error = (|| {
        let Some(definitions) = definitions else {
            return Ok(());
        };
        ferric_browser_config::validate_contexts_configuration(definitions)
            .map_err(|error| error.to_string())?;
        for definition in &definitions.contexts {
            if !profile_names
                .iter()
                .any(|profile| profile == &definition.profile)
            {
                return Err(format!(
                    "context {} references missing profile {}",
                    definition.name, definition.profile
                ));
            }
        }
        let updates = definitions
            .contexts
            .iter()
            .map(|definition| ContextDefinitionUpdate {
                name: definition.name.clone(),
                label: definition.label.clone(),
                profile: definition.profile.clone(),
                sessions: definition.sessions.clone(),
                workspace: definition.workspace.clone(),
                accent: definition.accent.clone(),
                default_target: definition.default_target.clone(),
            })
            .collect::<Vec<_>>();
        commit
            .reconcile_contexts(&mut contexts, &updates)
            .map_err(|error| error.to_string())
    })()
    .err();
    Ok(DurableProfile {
        profile_id,
        roots,
        lock,
        contexts,
        context_error,
    })
}

fn set_private_directory_permissions(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn stopped_admitted_profile_worker_reports_possible_metadata_commit() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).unwrap();
        let mut app = crate::BrowserApplication::new(Config::default());
        app.request_profile_open_for_validation(request(
            ProfileStorage::Durable(RootSpec::Base(roots.temporary_root().unwrap().to_owned())),
            PrivacyKind::Normal,
        ))
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let candidate = loop {
            if let Some(candidate) = app.poll_profile_candidate() {
                break candidate.unwrap();
            }
            assert!(Instant::now() < deadline);
            thread::yield_now();
        };
        app.profile_open.as_mut().unwrap().worker =
            SingleFlightWorker::spawn("profile-committed-stop-test", |work| -> PreparedProfile {
                let ProfileOpenWork::Commit(prepared) = work else {
                    panic!("unexpected preparation");
                };
                let prepared = commit_profile(*prepared);
                assert!(prepared.activation.id.is_some());
                panic!("controlled worker stop after actual registry commit");
            })
            .unwrap();
        app.admit_profile_candidate(candidate).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let error = loop {
            if let Some(result) = app.poll_profile_open() {
                break result.unwrap_err();
            }
            assert!(Instant::now() < deadline);
            thread::yield_now();
        };
        assert!(matches!(error, ProfileOpenError::AdmittedWorkFailed(_)));
        assert!(app.profile_snapshot().is_none());
        assert!(
            ferric_browser_storage::ProfileRegistry::open(&roots)
                .unwrap()
                .profiles()
                .iter()
                .any(|profile| profile.name == "default")
        );
        drop(app);
        roots.cleanup().unwrap();
    }

    #[test]
    fn stale_metadata_at_worker_commit_rejects_without_memory_activation() {
        use crate::profile_validation_tests::{
            profile_metadata_snapshot, staged_profile_candidate, staged_profile_fixture,
        };
        let (mut app, _, roots) = staged_profile_fixture();
        let before = app.profile_snapshot();
        let configuration = app.configuration_snapshot();
        let candidate = staged_profile_candidate(&mut app, &roots);
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let (resume_tx, resume_rx) = std::sync::mpsc::sync_channel(1);
        app.profile_open.as_mut().unwrap().worker =
            SingleFlightWorker::spawn("profile-precommit-metadata-race", move |work| {
                let ProfileOpenWork::Commit(prepared) = work else {
                    panic!("unexpected preparation");
                };
                ready_tx.send(()).unwrap();
                resume_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                commit_profile(*prepared)
            })
            .unwrap();
        app.admit_profile_candidate(candidate).unwrap();
        ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        ContextRegistry::open(&roots)
            .unwrap()
            .create("peer", "Peer", "kept")
            .unwrap();
        let peer_metadata = profile_metadata_snapshot(&roots);
        resume_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let error = loop {
            if let Some(result) = app.poll_profile_open() {
                break result.unwrap_err();
            }
            assert!(Instant::now() < deadline);
            thread::yield_now();
        };
        assert!(matches!(error, ProfileOpenError::InvalidRequest(_)));
        assert!(error.to_string().contains("before catalog writes"));
        assert_eq!(app.profile_snapshot(), before);
        assert_eq!(app.configuration_snapshot(), configuration);
        assert_eq!(profile_metadata_snapshot(&roots), peer_metadata);
        drop(app);
        roots.cleanup().unwrap();
    }

    #[test]
    fn stale_context_after_metadata_commit_reports_admitted_inactive_outcome() {
        use crate::profile_validation_tests::{staged_profile_candidate, staged_profile_fixture};
        let (mut app, window, roots) = staged_profile_fixture();
        let before = app.profile_snapshot();
        let candidate = staged_profile_candidate(&mut app, &roots);
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let (resume_tx, resume_rx) = std::sync::mpsc::sync_channel(1);
        app.profile_open.as_mut().unwrap().worker =
            SingleFlightWorker::spawn("profile-postcommit-context-race", move |work| {
                let ProfileOpenWork::Commit(prepared) = work else {
                    panic!("unexpected preparation");
                };
                let prepared = commit_profile(*prepared);
                assert!(prepared.activation.id.is_some());
                ready_tx.send(()).unwrap();
                resume_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                prepared
            })
            .unwrap();
        app.admit_profile_candidate(candidate).unwrap();
        ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let target = app
            .capture_target(app.windows()[&window].active_tab.unwrap())
            .unwrap();
        app.dispatch_runtime(ferric_browser_runtime::RuntimeInput::EngineFact(
            ferric_browser_core::Event::SameDocumentNavigation {
                target,
                url: ferric_browser_core::ValidatedUrl::parse("https://example.test/changed")
                    .unwrap(),
            },
        ))
        .unwrap();
        assert_eq!(
            app.tabs()[&target.tab].url.as_deref(),
            Some("https://example.test/changed")
        );
        resume_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let error = loop {
            if let Some(result) = app.poll_profile_open() {
                break result.unwrap_err();
            }
            assert!(Instant::now() < deadline);
            thread::yield_now();
        };
        assert!(matches!(error, ProfileOpenError::AdmittedWorkFailed(_)));
        assert_eq!(app.profile_snapshot(), before);
        assert!(
            ferric_browser_storage::ProfileRegistry::open(&roots)
                .unwrap()
                .profiles()
                .iter()
                .any(|profile| profile.name == "staged")
        );
        drop(app);
        roots.cleanup().unwrap();
    }

    #[test]
    fn rejected_profile_admission_preserves_the_surviving_pages_cache() {
        let (mut app, window, _) = crate::BrowserApplication::bootstrap(
            Config::default(),
            PrivacyKind::Private,
            "default",
        )
        .unwrap();
        app.activate_profile(
            prepare_profile(request(ProfileStorage::Transient, PrivacyKind::Private)).activation,
        )
        .unwrap();
        let url = "https://example.test/page";
        let before = app.refresh_interaction_snapshot(window, url).unwrap();
        let mut invalid = request(ProfileStorage::Transient, PrivacyKind::Private);
        invalid.label.clear();
        assert!(matches!(
            app.request_profile_open(invalid),
            Err(ProfileOpenError::InvalidRequest(_))
        ));
        assert!(app.profile_open.is_none());
        let context = ferric_browser_config::SiteMatchContext::for_url(
            &ferric_browser_core::ValidatedUrl::parse(url).unwrap(),
        )
        .unwrap();
        assert!(std::sync::Arc::ptr_eq(
            &before,
            app.cached_interaction_snapshot(window, &context).unwrap()
        ));
    }

    #[test]
    fn stopped_profile_worker_recompiles_the_surviving_pages_context() {
        assert_profile_failure_cache_recovery("unchanged");
    }

    #[test]
    fn stopped_profile_worker_does_not_restore_policy_after_unsupported_navigation() {
        assert_profile_failure_cache_recovery("unsupported");
    }

    #[test]
    fn stopped_profile_worker_does_not_restore_policy_after_invalid_navigation() {
        assert_profile_failure_cache_recovery("invalid");
    }

    #[test]
    fn stopped_profile_worker_uses_the_new_active_tabs_address() {
        assert_profile_failure_cache_recovery("active-tab");
    }

    #[test]
    fn stopped_profile_worker_does_not_invent_an_uncommitted_tabs_address() {
        assert_profile_failure_cache_recovery("uncommitted-active-tab");
    }

    #[test]
    fn stopped_profile_worker_does_not_rebind_the_previous_url_during_navigation() {
        assert_profile_failure_cache_recovery("provisional-navigation");
    }

    #[test]
    fn failed_profile_open_keeps_an_invalid_engine_observation_closed() {
        assert_profile_failure_cache_recovery("rejected-observation");
    }

    fn assert_profile_failure_cache_recovery(scenario: &str) {
        let (mut app, window, _) = crate::BrowserApplication::bootstrap(
            Config::default(),
            PrivacyKind::Private,
            "default",
        )
        .unwrap();
        app.activate_profile(
            prepare_profile(request(ProfileStorage::Transient, PrivacyKind::Private)).activation,
        )
        .unwrap();
        let url = "https://example.test/page";
        let before = app.refresh_interaction_snapshot(window, url).unwrap();
        let context = ferric_browser_config::SiteMatchContext::for_url(
            &ferric_browser_core::ValidatedUrl::parse(url).unwrap(),
        )
        .unwrap();
        let mut worker = SingleFlightWorker::spawn(
            "profile-cache-failure-test",
            |_: ProfileOpenWork| -> PreparedProfile {
                panic!("controlled profile worker failure");
            },
        )
        .unwrap();
        worker
            .submit(ProfileOpenWork::Prepare(Box::new(request(
                ProfileStorage::Transient,
                PrivacyKind::Private,
            ))))
            .unwrap();
        app.interaction_cache.suspend();
        app.profile_open = Some(ProfileOpenCoordinator {
            worker,
            mode: AdmissionMode::Automatic,
            phase: OpenPhase::Preparing,
        });
        change_page_during_pending_profile(&mut app, window, scenario);
        assert!(app.cached_interaction_snapshot(window, &context).is_none());
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(result) = app.poll_profile_open() {
                assert!(matches!(result, Err(ProfileOpenError::WorkerStopped)));
                break;
            }
            assert!(Instant::now() < deadline, "profile worker did not stop");
            thread::yield_now();
        }
        match scenario {
            "rejected-observation" => {
                assert!(app.refresh_interaction_snapshot(window, url).is_none());
                assert_eq!(
                    app.interaction_cache_unavailable(),
                    Some(crate::InteractionCacheUnavailable::RejectedEngineContext)
                );
            }
            "uncommitted-active-tab" | "provisional-navigation" => {
                assert!(app.cached_interaction_snapshot(window, &context).is_none());
                let new_context = ferric_browser_config::SiteMatchContext::for_url(
                    &ferric_browser_core::ValidatedUrl::parse("https://other.test/new-tab")
                        .unwrap(),
                )
                .unwrap();
                assert!(
                    app.cached_interaction_snapshot(window, &new_context)
                        .is_none()
                );
            }
            "unsupported" | "invalid" => {
                assert!(app.cached_interaction_snapshot(window, &context).is_none());
                assert_eq!(
                    app.interaction_cache_unavailable(),
                    Some(crate::InteractionCacheUnavailable::UnsupportedContext)
                );
            }
            "active-tab" => {
                assert!(app.cached_interaction_snapshot(window, &context).is_none());
                let new_context = ferric_browser_config::SiteMatchContext::for_url(
                    &ferric_browser_core::ValidatedUrl::parse("https://other.test/new-tab")
                        .unwrap(),
                )
                .unwrap();
                assert!(
                    app.cached_interaction_snapshot(window, &new_context)
                        .is_some()
                );
            }
            _ => {
                let restored = app.cached_interaction_snapshot(window, &context).unwrap();
                assert_eq!(
                    restored.revision().profile_revision,
                    before.revision().profile_revision
                );
                assert!(restored.revision().policy_revision > before.revision().policy_revision);
            }
        }
    }

    fn change_page_during_pending_profile(
        app: &mut crate::BrowserApplication,
        window: ferric_browser_core::WindowId,
        scenario: &str,
    ) {
        match scenario {
            "rejected-observation" => {
                let target = app
                    .capture_target(app.windows()[&window].active_tab.unwrap())
                    .unwrap();
                let observation = app.capture_site_policy_observation(window, target).unwrap();
                assert!(app.reject_observed_interaction_context(&observation));
            }
            "provisional-navigation" => {
                let tab = app.windows()[&window].active_tab.unwrap();
                app.dispatch_runtime(ferric_browser_runtime::RuntimeInput::Command(
                    ferric_browser_runtime::RuntimeCommand::Navigate {
                        target: app.capture_target(tab).unwrap(),
                        url: ferric_browser_core::ValidatedUrl::parse("https://other.test/new-tab")
                            .unwrap(),
                    },
                ))
                .unwrap();
            }
            "unsupported" => {
                let target = app
                    .capture_target(app.windows()[&window].active_tab.unwrap())
                    .unwrap();
                app.dispatch_runtime(ferric_browser_runtime::RuntimeInput::Command(
                    ferric_browser_runtime::RuntimeCommand::Navigate {
                        target,
                        url: ferric_browser_core::ValidatedUrl::parse("about:blank").unwrap(),
                    },
                ))
                .unwrap();
                assert!(
                    app.refresh_interaction_snapshot(window, "about:blank")
                        .is_none()
                );
            }
            "invalid" => {
                assert!(
                    app.refresh_interaction_snapshot(window, "https://user@example.test/page")
                        .is_none()
                );
            }
            "active-tab" | "uncommitted-active-tab" => {
                app.dispatch_runtime(ferric_browser_runtime::RuntimeInput::Lifecycle(
                    ferric_browser_core::Event::OpenTabWithNavigation {
                        window,
                        url: ferric_browser_core::ValidatedUrl::parse("https://other.test/new-tab")
                            .unwrap(),
                        background: false,
                    },
                ))
                .unwrap();
                if scenario == "active-tab" {
                    let tab = app.windows()[&window].active_tab.unwrap();
                    app.dispatch_runtime(ferric_browser_runtime::RuntimeInput::EngineFact(
                        ferric_browser_core::Event::CommitNavigation {
                            target: app.capture_target(tab).unwrap(),
                            url: ferric_browser_core::ValidatedUrl::parse(
                                "https://other.test/new-tab",
                            )
                            .unwrap(),
                            title: "New page".into(),
                        },
                    ))
                    .unwrap();
                }
                // Intentionally omit the adapter's eager refresh: restoration
                // must obtain the new tab's URL rather than rescope the old one.
            }
            "unchanged" => {}
            _ => panic!("unknown profile failure scenario"),
        }
    }

    fn request(storage: ProfileStorage, privacy: PrivacyKind) -> ProfileOpenRequest {
        ProfileOpenRequest {
            name: "default".into(),
            label: "Default".into(),
            privacy,
            storage,
            base_configuration: Config::default(),
            base_authored: std::sync::Arc::default(),
            initial_profile_overrides: RuntimeOverrides::default(),
            command_line_overrides: RuntimeOverrides::default(),
            config_path: None,
            script_catalog_root: None,
            contexts: ContextsConfig::default(),
            context_configuration_error: None,
        }
    }

    #[test]
    fn invalid_profile_overrides_keep_the_captured_authored_base_on_fallback() {
        let loaded = ferric_browser_config::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../ferric-browser-config/tests/fixtures/site-policy/includes/root.toml"
        ))
        .unwrap();
        let mut input = request(ProfileStorage::Transient, PrivacyKind::Private);
        input.base_configuration = loaded.config.clone();
        input.base_authored = loaded.authored.clone();
        input
            .initial_profile_overrides
            .settings
            .insert("content.zoom".into(), toml::Value::Float(99.0));
        let prepared = prepare_profile(input);
        assert!(prepared.configuration_error.is_some());
        let mut app = crate::BrowserApplication::new(Config::default());
        let mut earlier = ConfigurationLayers::new(Config::default());
        earlier.base.hints.chars = "ab".into();
        app.update_configuration(earlier).unwrap();
        let before_sources = app.configuration_policy_sources();
        let effect = app.install_prepared_profile(prepared).unwrap();
        assert!(effect.configuration_error.is_some());
        assert_eq!(effect.profile_overrides, app.configuration_layers().profile);
        assert_eq!(effect.runtime_overrides, app.configuration_layers().runtime);
        assert_eq!(app.configuration(), &loaded.config);
        assert!(std::sync::Arc::ptr_eq(
            &app.configuration_layers().base_authored,
            &loaded.authored,
        ));
        assert_eq!(
            app.configuration_layers().base_authored.documents().len(),
            2
        );
        let sources = app.configuration_policy_sources();
        assert_eq!(sources.desired_layers(), sources.reload_layers());
        assert_eq!(sources.revision(), app.configuration_revision());
        assert_eq!(sources.reload_revision(), app.configuration_revision());
        assert!(std::sync::Arc::ptr_eq(
            &sources.reload_layers().base_authored,
            &loaded.authored
        ));
        assert_eq!(before_sources.desired_layers().base.hints.chars, "ab");
    }

    #[test]
    fn private_catalog_root_is_independent_of_custom_configuration_location() {
        let root =
            std::env::temp_dir().join(format!("ferric-private-catalog-{}", uuid::Uuid::new_v4()));
        let catalog = root.join("installed");
        fs::create_dir_all(catalog.join("userscripts")).unwrap();
        fs::write(catalog.join("userscripts/example.toml"),
            "schema_version=1\nname=\"example\"\nexecutable=\"/bin/true\"\ncontext_fields=[\"url\"]\n[[actions]]\nid=\"userscript.example.inspect\"\nsubject=\"url\"\nverb=\"inspect\"\nlabel=\"Inspect\"\nrequired_fields=[\"url\"]\n").unwrap();
        let config = root.join("custom.toml");
        fs::write(&config, "schema_version=3\n").unwrap();
        let mut input = request(ProfileStorage::Transient, PrivacyKind::Private);
        input.config_path = Some(config);
        input.script_catalog_root = Some(catalog.clone());
        let prepared = prepare_profile(input);
        assert!(prepared.activation.roots.is_none());
        assert!(prepared.activation.storage_path.is_none());
        assert_eq!(prepared.userscript_root, Some(catalog));
        assert_eq!(prepared.page_scripts.unwrap().actions().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_mismatched_privacy_and_storage() {
        let mut worker = ProfileOpenCoordinator::spawn().expect("worker");
        assert!(matches!(
            worker.submit(request(ProfileStorage::Transient, PrivacyKind::Normal)),
            Err(ProfileOpenError::InvalidRequest(_))
        ));
    }

    #[test]
    fn prepares_durable_resources_off_the_calling_thread() {
        let roots = StorageRoots::resolve(RootSpec::Temporary).expect("roots");
        let base = roots.data.parent().expect("temporary base").to_owned();
        drop(roots);
        let mut worker = ProfileOpenCoordinator::spawn().expect("worker");
        worker
            .submit(request(
                ProfileStorage::Durable(RootSpec::Base(base.clone())),
                PrivacyKind::Normal,
            ))
            .expect("submit");
        let deadline = Instant::now() + Duration::from_secs(2);
        let prepared = loop {
            if let Some(result) = worker.poll() {
                break result.expect("profile prepared");
            }
            assert!(Instant::now() < deadline, "profile worker timed out");
            thread::yield_now();
        };
        assert!(prepared.activation.storage_path.is_none());
        assert!(prepared.durable.is_some());
        let loaded = ferric_browser_config::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../ferric-browser-config/tests/fixtures/site-policy/includes/root.toml"
        ))
        .unwrap();
        let mut earlier = ConfigurationLayers::new(loaded.config);
        earlier.base_authored = loaded.authored;
        let mut app = crate::BrowserApplication::new(Config::default());
        app.activate_configuration(earlier).unwrap();
        let before_sources = app.configuration_policy_sources();
        assert_eq!(
            before_sources
                .reload_layers()
                .base_authored
                .documents()
                .len(),
            2
        );
        app.install_prepared_profile(prepared).unwrap();
        let sources = app.configuration_policy_sources();
        assert_eq!(sources.desired_layers(), sources.reload_layers());
        assert_eq!(sources.revision(), app.configuration_revision());
        assert_eq!(sources.reload_revision(), app.configuration_revision());
        assert_eq!(sources.reload_layers().base_authored.documents().len(), 0);
        assert_eq!(
            before_sources
                .reload_layers()
                .base_authored
                .documents()
                .len(),
            2
        );
        drop(app);
        let _ = fs::remove_dir_all(base);
    }
}
