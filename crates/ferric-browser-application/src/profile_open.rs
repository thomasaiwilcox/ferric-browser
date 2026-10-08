use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    thread::{self, JoinHandle},
};

use ferric_browser_config::{
    Config, ContextsConfig, RuntimeOverrides, load, load_profile_runtime_overrides,
    load_runtime_overrides, profile_config_path,
};
use ferric_browser_core::PrivacyKind;
use ferric_browser_storage::{
    ContextRegistry, ProfileLock, ProfileRegistry, RootSpec, StorageRoots,
};

use crate::{ConfigurationLayers, ConfigurationSnapshot, ProfileActivation, ProfileSnapshot};

const PROFILE_REQUEST_CAPACITY: usize = 1;

/// Typed inputs required to open one browser profile.
#[derive(Clone, PartialEq)]
pub struct ProfileOpenRequest {
    pub name: String,
    pub label: String,
    pub privacy: PrivacyKind,
    pub storage: ProfileStorage,
    pub base_configuration: Config,
    pub initial_profile_overrides: RuntimeOverrides,
    pub command_line_overrides: RuntimeOverrides,
    pub config_path: Option<PathBuf>,
    pub contexts: ContextsConfig,
    pub context_configuration_issue: bool,
}

impl std::fmt::Debug for ProfileOpenRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProfileOpenRequest")
            .field("name", &"[redacted]")
            .field("label", &"[redacted]")
            .field("privacy", &self.privacy)
            .field("storage", &self.storage)
            .field("configuration", &"[redacted]")
            .field(
                "config_path",
                &self.config_path.as_ref().map(|_| "[redacted]"),
            )
            .field("contexts_count", &self.contexts.contexts.len())
            .field(
                "context_configuration_issue",
                &self.context_configuration_issue,
            )
            .finish()
    }
}

/// Storage selection for a profile-open request.
#[derive(Clone, Eq, PartialEq)]
pub enum ProfileStorage {
    Transient,
    Durable(RootSpec),
}

impl std::fmt::Debug for ProfileStorage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transient => formatter.write_str("Transient"),
            Self::Durable(_) => formatter.write_str("Durable([redacted])"),
        }
    }
}

/// Failure to submit or execute profile bootstrap work.
#[derive(Clone, Eq, PartialEq)]
pub enum ProfileOpenError {
    Busy,
    QueueFull,
    WorkerStopped,
    WorkerStart,
    EmptyNameOrLabel,
    PrivacyStorageMismatch,
    ReplacementBlocked,
    ConfigurationActivationFailed,
}

impl std::fmt::Debug for ProfileOpenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => formatter.write_str("Busy"),
            Self::QueueFull => formatter.write_str("QueueFull"),
            Self::WorkerStopped => formatter.write_str("WorkerStopped"),
            Self::WorkerStart => formatter.write_str("WorkerStart"),
            Self::EmptyNameOrLabel => formatter.write_str("EmptyNameOrLabel"),
            Self::PrivacyStorageMismatch => formatter.write_str("PrivacyStorageMismatch"),
            Self::ReplacementBlocked => formatter.write_str("ReplacementBlocked"),
            Self::ConfigurationActivationFailed => {
                formatter.write_str("ConfigurationActivationFailed")
            }
        }
    }
}

impl std::fmt::Display for ProfileOpenError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => formatter.write_str("profile bootstrap is already pending"),
            Self::QueueFull => formatter.write_str("profile bootstrap queue is full"),
            Self::WorkerStopped => formatter.write_str("profile bootstrap worker stopped"),
            Self::WorkerStart => formatter.write_str("profile bootstrap worker could not start"),
            Self::EmptyNameOrLabel => {
                formatter.write_str("profile name and label must not be empty")
            }
            Self::PrivacyStorageMismatch => {
                formatter.write_str("private and ephemeral profiles must use transient storage")
            }
            Self::ReplacementBlocked => {
                formatter.write_str("profile replacement requires versioning operations to finish")
            }
            Self::ConfigurationActivationFailed => {
                formatter.write_str("profile configuration could not be activated")
            }
        }
    }
}

impl std::error::Error for ProfileOpenError {}

/// Application-owned result of opening and activating one profile.
#[derive(Clone, PartialEq)]
pub struct ProfileOpenEffect {
    pub configuration: crate::ConfigurationSnapshot,
    pub profile: crate::ProfileSnapshot,
    pub config_sources: Vec<PathBuf>,
    pub profile_overrides: RuntimeOverrides,
    pub runtime_overrides: RuntimeOverrides,
    pub session_path: Option<PathBuf>,
    pub session_state_root: Option<PathBuf>,
    pub storage_issue: bool,
    pub configuration_issue: bool,
    pub context_issue: bool,
}

impl std::fmt::Debug for ProfileOpenEffect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProfileOpenEffect")
            .field("profile", &"[redacted]")
            .field("configuration", &"[redacted]")
            .field("config_sources_count", &self.config_sources.len())
            .field("profile_overrides", &"[redacted]")
            .field("runtime_overrides", &"[redacted]")
            .field("has_session_path", &self.session_path.is_some())
            .field("has_session_state_root", &self.session_state_root.is_some())
            .field("storage_issue", &self.storage_issue)
            .field("configuration_issue", &self.configuration_issue)
            .field("context_issue", &self.context_issue)
            .finish()
    }
}

pub(crate) struct PreparedProfile {
    pub(crate) activation: ProfileActivation,
    pub(crate) layers: ConfigurationLayers,
    pub(crate) fallback_configuration: Config,
    pub(crate) config_sources: Vec<PathBuf>,
    pub(crate) profile_overrides: RuntimeOverrides,
    pub(crate) runtime_overrides: RuntimeOverrides,
    pub(crate) session_path: Option<PathBuf>,
    pub(crate) session_state_root: Option<PathBuf>,
    pub(crate) storage_issue: bool,
    pub(crate) configuration_issue: bool,
    pub(crate) context_issue: bool,
}

pub(crate) struct ProfileOpenCoordinator {
    sender: Option<SyncSender<ProfileOpenRequest>>,
    receiver: Receiver<PreparedProfile>,
    pending: bool,
    join: Option<JoinHandle<()>>,
}

impl ProfileOpenCoordinator {
    pub(crate) fn spawn() -> Result<Self, ProfileOpenError> {
        let (sender, requests) = mpsc::sync_channel(PROFILE_REQUEST_CAPACITY);
        let (responses, receiver) = mpsc::sync_channel(PROFILE_REQUEST_CAPACITY);
        let join = thread::Builder::new()
            .name("ferric-browser-profile-bootstrap".into())
            .spawn(move || {
                while let Ok(request) = requests.recv() {
                    if responses.send(prepare_profile(request)).is_err() {
                        break;
                    }
                }
            })
            .map_err(|_| ProfileOpenError::WorkerStart)?;
        Ok(Self {
            sender: Some(sender),
            receiver,
            pending: false,
            join: Some(join),
        })
    }

    pub(crate) fn submit(&mut self, request: ProfileOpenRequest) -> Result<(), ProfileOpenError> {
        validate_request(&request)?;
        if self.pending {
            return Err(ProfileOpenError::Busy);
        }
        let sender = self
            .sender
            .as_ref()
            .ok_or(ProfileOpenError::WorkerStopped)?;
        match sender.try_send(request) {
            Ok(()) => {
                self.pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(ProfileOpenError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(ProfileOpenError::WorkerStopped),
        }
    }

    pub(crate) fn poll(&mut self) -> Option<Result<PreparedProfile, ProfileOpenError>> {
        match self.receiver.try_recv() {
            Ok(response) => {
                self.pending = false;
                Some(Ok(response))
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.pending = false;
                Some(Err(ProfileOpenError::WorkerStopped))
            }
        }
    }
}

impl Drop for ProfileOpenCoordinator {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn validate_request(request: &ProfileOpenRequest) -> Result<(), ProfileOpenError> {
    if request.name.is_empty() || request.label.is_empty() {
        return Err(ProfileOpenError::EmptyNameOrLabel);
    }
    if matches!(request.storage, ProfileStorage::Transient)
        != matches!(
            request.privacy,
            PrivacyKind::Private | PrivacyKind::Ephemeral
        )
    {
        return Err(ProfileOpenError::PrivacyStorageMismatch);
    }
    Ok(())
}

fn prepare_profile(request: ProfileOpenRequest) -> PreparedProfile {
    let fallback_configuration = request.base_configuration.clone();
    let mut profile_overrides = request.initial_profile_overrides;
    let mut runtime_overrides = RuntimeOverrides::default();
    let mut config_sources = Vec::new();
    let mut configuration_issue = false;

    if let Some(config_path) = request.config_path.as_deref() {
        match load(config_path) {
            Ok(loaded) => {
                config_sources = loaded.sources;
                let profiles_path = profile_config_path(config_path);
                if profiles_path.exists() {
                    config_sources.push(profiles_path);
                }
            }
            Err(_) => configuration_issue = true,
        }
        if !matches!(request.storage, ProfileStorage::Transient) {
            match load_profile_runtime_overrides(config_path, &request.name) {
                Ok(overrides) => profile_overrides = overrides,
                Err(_) => configuration_issue = true,
            }
        }
    }

    let mut activation = ProfileActivation {
        name: request.name.clone(),
        id: None,
        privacy: request.privacy,
        roots: None,
        lock: None,
        contexts: None,
        storage_path: None,
    };
    let mut session_path = None;
    let mut session_state_root = None;
    let mut storage_issue = false;
    let mut context_issue = request.context_configuration_issue;

    if let ProfileStorage::Durable(spec) = request.storage {
        match prepare_durable_profile(&request.name, &request.label, spec, &request.contexts) {
            Ok(durable) => {
                match load_runtime_overrides(durable.roots.state.join("runtime-overrides.toml")) {
                    Ok(overrides) => runtime_overrides = overrides,
                    Err(_) => {
                        configuration_issue = true;
                    }
                }
                session_path = Some(
                    durable
                        .roots
                        .state
                        .join("sessions")
                        .join(durable.profile_id.to_string())
                        .join("current.json"),
                );
                session_state_root = Some(durable.roots.state.clone());
                if durable.context_issue {
                    context_issue = true;
                }
                activation.id = Some(durable.profile_id);
                activation.storage_path = Some(
                    durable
                        .roots
                        .data
                        .join("profiles")
                        .join(durable.profile_id.to_string())
                        .join("browser.sqlite"),
                );
                activation.contexts = Some(durable.contexts);
                activation.lock = Some(durable.lock);
                activation.roots = Some(durable.roots);
            }
            Err(_) => storage_issue = true,
        }
    }

    let layers = ConfigurationLayers {
        base: request.base_configuration,
        profile: profile_overrides.clone(),
        runtime: runtime_overrides.clone(),
        command_line: request.command_line_overrides,
        temporary: RuntimeOverrides::default(),
    };
    if layers.resolve().is_err() {
        configuration_issue = true;
    }

    PreparedProfile {
        activation,
        layers,
        fallback_configuration,
        config_sources,
        profile_overrides,
        runtime_overrides,
        session_path,
        session_state_root,
        storage_issue,
        configuration_issue,
        context_issue,
    }
}

struct DurableProfile {
    profile_id: uuid::Uuid,
    roots: StorageRoots,
    lock: ProfileLock,
    contexts: ContextRegistry,
    context_issue: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProfilePreparationFailure;

fn prepare_durable_profile(
    name: &str,
    label: &str,
    spec: RootSpec,
    definitions: &ContextsConfig,
) -> Result<DurableProfile, ProfilePreparationFailure> {
    let roots = StorageRoots::resolve(spec).map_err(|_| ProfilePreparationFailure)?;
    let mut registry = ProfileRegistry::open(&roots).map_err(|_| ProfilePreparationFailure)?;
    let profile_id = registry
        .get_or_create(name, label)
        .map_err(|_| ProfilePreparationFailure)?
        .id;
    let profile_names = registry
        .profiles()
        .iter()
        .map(|profile| profile.name.clone())
        .collect::<Vec<_>>();
    let lock = ProfileLock::acquire(&roots, profile_id).map_err(|_| ProfilePreparationFailure)?;
    let profile_root = roots.data.join("profiles").join(profile_id.to_string());
    fs::create_dir_all(&profile_root).map_err(|_| ProfilePreparationFailure)?;
    set_private_directory_permissions(&profile_root).map_err(|_| ProfilePreparationFailure)?;
    let mut contexts = ContextRegistry::open(&roots).map_err(|_| ProfilePreparationFailure)?;
    let mut context_issue = false;
    for definition in &definitions.contexts {
        if !profile_names
            .iter()
            .any(|profile| profile == &definition.profile)
        {
            context_issue = true;
            continue;
        }
        if contexts
            .ensure_definition(
                &definition.name,
                &definition.label,
                &definition.profile,
                definition.sessions.clone(),
                definition.workspace.clone(),
                definition.accent.clone(),
                &definition.default_target,
            )
            .is_err()
        {
            context_issue = true;
        }
    }
    Ok(DurableProfile {
        profile_id,
        roots,
        lock,
        contexts,
        context_issue,
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

    fn request(storage: ProfileStorage, privacy: PrivacyKind) -> ProfileOpenRequest {
        ProfileOpenRequest {
            name: "default".into(),
            label: "Default".into(),
            privacy,
            storage,
            base_configuration: Config::default(),
            initial_profile_overrides: RuntimeOverrides::default(),
            command_line_overrides: RuntimeOverrides::default(),
            config_path: None,
            contexts: ContextsConfig::default(),
            context_configuration_issue: false,
        }
    }

    #[test]
    fn rejects_mismatched_privacy_and_storage() {
        let mut worker = ProfileOpenCoordinator::spawn().expect("worker");
        assert!(matches!(
            worker.submit(request(ProfileStorage::Transient, PrivacyKind::Normal)),
            Err(ProfileOpenError::PrivacyStorageMismatch)
        ));
    }

    #[test]
    fn profile_open_errors_use_closed_fixed_categories() {
        for error in [
            ProfileOpenError::Busy,
            ProfileOpenError::QueueFull,
            ProfileOpenError::WorkerStopped,
            ProfileOpenError::WorkerStart,
            ProfileOpenError::EmptyNameOrLabel,
            ProfileOpenError::PrivacyStorageMismatch,
            ProfileOpenError::ReplacementBlocked,
            ProfileOpenError::ConfigurationActivationFailed,
        ] {
            assert!(!format!("{error:?}").contains("/private/"));
            assert!(!error.to_string().contains("/private/"));
        }
        assert_eq!(
            ProfileOpenError::EmptyNameOrLabel.to_string(),
            "profile name and label must not be empty"
        );
    }

    #[test]
    fn profile_open_preparation_keeps_only_context_issue_presence() {
        let mut requested = request(ProfileStorage::Transient, PrivacyKind::Ephemeral);
        requested.context_configuration_issue = true;

        let prepared = prepare_profile(requested);

        assert!(prepared.context_issue);
        assert!(!prepared.storage_issue);
        assert!(!prepared.configuration_issue);
    }

    #[test]
    fn profile_open_preparation_discards_configuration_and_storage_errors() {
        let id = uuid::Uuid::new_v4();
        let config_path = std::env::temp_dir().join(format!("profile-open-{id}-config.toml"));
        let storage_base = std::env::temp_dir().join(format!("profile-open-{id}-storage"));
        fs::write(&config_path, "[").expect("invalid configuration fixture");
        fs::write(&storage_base, b"not a directory").expect("file storage fixture");

        let mut config_request = request(ProfileStorage::Transient, PrivacyKind::Ephemeral);
        config_request.config_path = Some(config_path.clone());
        let configuration_issue = prepare_profile(config_request).configuration_issue;

        let storage_request = request(
            ProfileStorage::Durable(RootSpec::Base(storage_base.clone())),
            PrivacyKind::Normal,
        );
        let storage_issue = prepare_profile(storage_request).storage_issue;

        fs::remove_file(config_path).expect("remove configuration fixture");
        fs::remove_file(storage_base).expect("remove storage fixture");
        assert!(configuration_issue);
        assert!(storage_issue);
    }

    #[test]
    fn profile_open_request_and_effect_debug_redact_paths_and_values() {
        let sentinel = "/private/profiles/sensitive.sqlite3";
        let mut requested = request(
            ProfileStorage::Durable(RootSpec::Base(PathBuf::from(sentinel))),
            PrivacyKind::Normal,
        );
        requested.name = sentinel.to_owned();
        requested.label = sentinel.to_owned();
        requested.config_path = Some(PathBuf::from(sentinel));
        requested.context_configuration_issue = true;
        assert!(!format!("{requested:?}").contains(sentinel));

        let effect = ProfileOpenEffect {
            configuration: ConfigurationSnapshot {
                effective: Config::default(),
                revision: 0,
                pending_changes: Vec::new(),
            },
            profile: ProfileSnapshot {
                generation: 1,
                name: sentinel.to_owned(),
                id: None,
                privacy: PrivacyKind::Normal,
                durable: false,
                roots: None,
            },
            config_sources: vec![PathBuf::from(sentinel)],
            profile_overrides: RuntimeOverrides::default(),
            runtime_overrides: RuntimeOverrides::default(),
            session_path: Some(PathBuf::from(sentinel)),
            session_state_root: Some(PathBuf::from(sentinel)),
            storage_issue: true,
            configuration_issue: true,
            context_issue: true,
        };
        assert!(!format!("{effect:?}").contains(sentinel));
        assert!(!format!("{:?}", effect.profile).contains(sentinel));
        assert!(!format!("{:?}", requested.storage).contains(sentinel));
    }

    #[test]
    fn private_and_ephemeral_owners_have_no_durable_metadata_path() {
        for privacy in [PrivacyKind::Private, PrivacyKind::Ephemeral] {
            let requested = request(ProfileStorage::Transient, privacy);
            validate_request(&requested).expect("transient privacy pairing");
            let prepared = prepare_profile(requested);
            assert!(prepared.activation.storage_path.is_none());
            assert!(prepared.activation.roots.is_none());
            assert!(prepared.session_path.is_none());
        }
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
        assert!(prepared.activation.storage_path.is_some());
        drop(prepared);
        let _ = fs::remove_dir_all(base);
    }
}
