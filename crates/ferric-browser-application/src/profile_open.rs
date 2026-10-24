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
    ContextDefinitionUpdate, ContextRegistry, ProfileLock, ProfileRegistry, RootSpec, StorageRoots,
    current_session_paths,
};

use crate::{ConfigurationLayers, ProfileActivation};

use ferric_browser_runtime::worker::{Poll as WorkerPoll, SingleFlightWorker, SubmitError};
#[cfg(test)]
use std::thread;

/// Typed inputs required to open one browser profile.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileOpenRequest {
    pub name: String,
    pub label: String,
    pub privacy: PrivacyKind,
    pub storage: ProfileStorage,
    pub base_configuration: Config,
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
    pub(crate) userscript_root: Option<PathBuf>,
    pub(crate) page_scripts: Result<std::sync::Arc<crate::userscript::UserscriptCatalog>, String>,
    pub(crate) activation: ProfileActivation,
    pub(crate) layers: ConfigurationLayers,
    pub(crate) fallback_configuration: Config,
    pub(crate) config_sources: Vec<PathBuf>,
    pub(crate) profile_overrides: RuntimeOverrides,
    pub(crate) runtime_overrides: RuntimeOverrides,
    pub(crate) session_path: Option<PathBuf>,
    pub(crate) session_state_root: Option<PathBuf>,
    pub(crate) recovery_paths: Vec<PathBuf>,
    pub(crate) storage_error: Option<String>,
    pub(crate) configuration_error: Option<String>,
    pub(crate) context_error: Option<String>,
}

pub(crate) struct ProfileOpenCoordinator {
    worker: SingleFlightWorker<ProfileOpenRequest, PreparedProfile>,
}

impl ProfileOpenCoordinator {
    pub(crate) fn spawn() -> Result<Self, ProfileOpenError> {
        let worker = SingleFlightWorker::spawn("ferric-browser-profile-bootstrap", prepare_profile)
            .map_err(|error| ProfileOpenError::WorkerStart(error.to_string()))?;
        Ok(Self { worker })
    }

    pub(crate) fn submit(&mut self, request: ProfileOpenRequest) -> Result<(), ProfileOpenError> {
        validate_request(&request)?;
        self.worker.submit(request).map_err(|error| match error {
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
    let fallback_configuration = request.base_configuration.clone();
    let (profile_overrides, config_sources, mut configuration_error) = load_profile_layers(
        request.config_path.as_deref(),
        &request.name,
        matches!(request.storage, ProfileStorage::Transient),
        request.initial_profile_overrides,
    );
    let mut runtime_overrides = RuntimeOverrides::default();

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
    let mut recovery_paths = Vec::new();
    let mut storage_error = None;
    let mut context_error = request.context_configuration_error;

    if let ProfileStorage::Durable(spec) = request.storage {
        match prepare_durable_profile(&request.name, &request.label, spec, &request.contexts) {
            Ok(durable) => {
                recovery_paths = current_session_paths(&durable.roots.state, durable.profile_id);
                match load_runtime_overrides(durable.roots.state.join("runtime-overrides.toml")) {
                    Ok(overrides) => runtime_overrides = overrides,
                    Err(error) => {
                        configuration_error.get_or_insert_with(|| error.to_string());
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
                if durable.context_error.is_some() {
                    context_error = durable.context_error;
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
            Err(error) => storage_error = Some(error),
        }
    }

    let layers = ConfigurationLayers {
        base: request.base_configuration,
        profile: profile_overrides.clone(),
        runtime: runtime_overrides.clone(),
        command_line: request.command_line_overrides,
        temporary: RuntimeOverrides::default(),
    };
    if let Err(error) = layers.resolve() {
        configuration_error.get_or_insert_with(|| error.to_string());
    }

    let userscript_root = activation
        .roots
        .as_ref()
        .map(|roots| roots.config.clone())
        .or(request.script_catalog_root)
        .or_else(|| {
            request
                .config_path
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_owned)
        });
    let page_scripts = prepare_page_scripts(userscript_root.as_deref());
    PreparedProfile {
        userscript_root,
        page_scripts,
        activation,
        layers,
        fallback_configuration,
        config_sources,
        profile_overrides,
        runtime_overrides,
        session_path,
        session_state_root,
        recovery_paths,
        storage_error,
        configuration_error,
        context_error,
    }
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

fn prepare_durable_profile(
    name: &str,
    label: &str,
    spec: RootSpec,
    definitions: &ContextsConfig,
) -> Result<DurableProfile, String> {
    let roots = StorageRoots::resolve(spec).map_err(|error| error.to_string())?;
    let mut registry = ProfileRegistry::open(&roots).map_err(|error| error.to_string())?;
    let profile_id = registry
        .get_or_create(name, label)
        .map_err(|error| error.to_string())?
        .id;
    let profile_names = registry
        .profiles()
        .iter()
        .map(|profile| profile.name.clone())
        .collect::<Vec<_>>();
    let lock = ProfileLock::acquire(&roots, profile_id).map_err(|error| error.to_string())?;
    let profile_root = roots.data.join("profiles").join(profile_id.to_string());
    fs::create_dir_all(&profile_root).map_err(|error| error.to_string())?;
    set_private_directory_permissions(&profile_root).map_err(|error| error.to_string())?;
    let mut contexts = ContextRegistry::open(&roots).map_err(|error| error.to_string())?;
    let context_error = (|| {
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
        contexts
            .reconcile_configured(&[], &updates)
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
            script_catalog_root: None,
            contexts: ContextsConfig::default(),
            context_configuration_error: None,
        }
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
        assert!(prepared.activation.storage_path.is_some());
        drop(prepared);
        let _ = fs::remove_dir_all(base);
    }
}
