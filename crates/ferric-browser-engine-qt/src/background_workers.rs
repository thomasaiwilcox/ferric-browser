use super::{
    Command, CommandExt, Duration, HyprlandConfig, Instant, PathBuf, SingleFlightWorker, Stdio,
    StorageRoots, SubmitError, Value, WorkerPoll, fs, hyprland, network_policy,
    terminate_child_process, thread,
};
use std::io::Write;

const HYPRLAND_OBSERVATION_BASE_BACKOFF: Duration = Duration::from_secs(1);
const HYPRLAND_OBSERVATION_MAX_BACKOFF: Duration = Duration::from_secs(30);

pub(super) struct PrintWorker {
    inner: SingleFlightWorker<PathBuf, Result<PathBuf, String>>,
}

pub(super) struct EditorWriteWorker {
    inner: SingleFlightWorker<(PathBuf, Vec<u8>), Result<PathBuf, String>>,
}

struct NetworkPolicyRequest {
    roots: Option<StorageRoots>,
    config: Value,
}

pub(super) struct NetworkPolicyWorker {
    inner: SingleFlightWorker<NetworkPolicyRequest, Result<network_policy::PolicySnapshot, String>>,
}

pub(super) enum HyprlandRequest {
    Observe {
        config: HyprlandConfig,
    },
    RouteWorkspace {
        config: HyprlandConfig,
        workspace: String,
    },
    MoveActiveWindow {
        config: HyprlandConfig,
        workspace: String,
    },
    BrowserClients {
        config: HyprlandConfig,
    },
}

pub(super) enum HyprlandResponse {
    Observe(super::hyprland_events::ObservationResult),
    RouteWorkspace(Result<(), String>),
    MoveActiveWindow(Result<(), String>),
    BrowserClients(Result<Vec<hyprland::BrowserClient>, String>),
}

pub(super) struct HyprlandWorker {
    inner: SingleFlightWorker<HyprlandRequest, HyprlandResponse>,
    observed_state: Option<super::hyprland_events::ObservedState>,
    observation_backoff: Duration,
    next_observation_at: Instant,
    observations_enabled: bool,
    disconnect_pending: bool,
}

impl HyprlandWorker {
    pub(super) fn spawn() -> Result<Self, String> {
        let observation = std::sync::Arc::new(std::sync::Mutex::new(
            super::hyprland_events::ObservationSession::default(),
        ));
        let worker_observation = observation.clone();
        let inner =
            SingleFlightWorker::spawn(
                "ferric-browser-hyprland-ipc",
                move |request| match request {
                    HyprlandRequest::Observe { config } => {
                        let mut session = match worker_observation.lock() {
                            Ok(session) => session,
                            Err(_) => {
                                return HyprlandResponse::Observe(
                                super::hyprland_events::ObservationResult {
                                    capability:
                                        super::hyprland_events::ObservationCapability::Unavailable,
                                    state: None,
                                },
                            );
                            }
                        };
                        if config.enabled == ferric_browser_config::TriState::Off {
                            session.disconnect();
                            return HyprlandResponse::Observe(
                                super::hyprland_events::ObservationResult {
                                    capability:
                                        super::hyprland_events::ObservationCapability::Disabled,
                                    state: None,
                                },
                            );
                        }
                        let adapter = hyprland::HyprlandAdapter::from_config(&config);
                        HyprlandResponse::Observe(adapter.observe_events(&mut session))
                    }
                    HyprlandRequest::RouteWorkspace { config, workspace } => {
                        let adapter = hyprland::HyprlandAdapter::from_config(&config);
                        HyprlandResponse::RouteWorkspace(
                            adapter
                                .route_workspace(&workspace)
                                .map_err(|error| error.to_string()),
                        )
                    }
                    HyprlandRequest::MoveActiveWindow { config, workspace } => {
                        let adapter = hyprland::HyprlandAdapter::from_config(&config);
                        HyprlandResponse::MoveActiveWindow(
                            adapter
                                .move_active_window_to_workspace(&workspace)
                                .map_err(|error| error.to_string()),
                        )
                    }
                    HyprlandRequest::BrowserClients { config } => {
                        let adapter = hyprland::HyprlandAdapter::from_config(&config);
                        HyprlandResponse::BrowserClients(
                            adapter.browser_clients().map_err(|error| error.to_string()),
                        )
                    }
                },
            )
            .map_err(|error| error.to_string())?;
        Ok(Self {
            inner,
            observed_state: None,
            observation_backoff: HYPRLAND_OBSERVATION_BASE_BACKOFF,
            next_observation_at: Instant::now(),
            observations_enabled: false,
            disconnect_pending: false,
        })
    }

    pub(super) fn request(&mut self, request: HyprlandRequest) -> Result<(), SubmitError> {
        self.inner.submit(request)
    }

    pub(super) fn poll(&mut self) -> Option<HyprlandResponse> {
        match self.inner.poll() {
            WorkerPoll::Ready(HyprlandResponse::Observe(result)) => {
                if let Some(state) = result.state.as_ref() {
                    let is_newer = self.observed_state.as_ref().is_none_or(|current| {
                        state.instance_epoch > current.instance_epoch
                            || (state.instance_epoch == current.instance_epoch
                                && state.sequence >= current.sequence)
                    });
                    if is_newer {
                        self.observed_state = Some(state.clone());
                    }
                } else {
                    self.observed_state = None;
                }
                let disabled_while_enabled = result.capability
                    == super::hyprland_events::ObservationCapability::Disabled
                    && self.observations_enabled;
                self.observation_backoff = match &result.capability {
                    super::hyprland_events::ObservationCapability::Available => {
                        HYPRLAND_OBSERVATION_BASE_BACKOFF
                    }
                    super::hyprland_events::ObservationCapability::Disabled => {
                        if disabled_while_enabled {
                            HYPRLAND_OBSERVATION_BASE_BACKOFF
                        } else {
                            HYPRLAND_OBSERVATION_MAX_BACKOFF
                        }
                    }
                    super::hyprland_events::ObservationCapability::UnsupportedVersion
                    | super::hyprland_events::ObservationCapability::Unavailable
                    | super::hyprland_events::ObservationCapability::InvalidData => self
                        .observation_backoff
                        .saturating_mul(2)
                        .min(HYPRLAND_OBSERVATION_MAX_BACKOFF),
                };
                self.next_observation_at = if disabled_while_enabled {
                    Instant::now()
                } else {
                    Instant::now() + self.observation_backoff
                };
                Some(HyprlandResponse::Observe(result))
            }
            WorkerPoll::Ready(response) => Some(response),
            WorkerPoll::Pending | WorkerPoll::Stopped => None,
        }
    }

    pub(super) fn request_observation_if_due(&mut self, config: HyprlandConfig) {
        let now = Instant::now();
        let enabled = config.enabled != ferric_browser_config::TriState::Off;
        if enabled != self.observations_enabled {
            self.observations_enabled = enabled;
            self.observation_backoff = HYPRLAND_OBSERVATION_BASE_BACKOFF;
            self.next_observation_at = now;
            self.disconnect_pending = !enabled;
        }

        if !enabled {
            self.observed_state = None;
            if self.disconnect_pending
                && now >= self.next_observation_at
                && !self.inner.is_pending()
            {
                let submitted = self.request(HyprlandRequest::Observe { config }).is_ok();
                if submitted {
                    self.disconnect_pending = false;
                } else {
                    self.next_observation_at = now + self.observation_backoff;
                }
            }
            return;
        }

        self.disconnect_pending = false;
        if now < self.next_observation_at || self.inner.is_pending() {
            return;
        }
        if self.request(HyprlandRequest::Observe { config }).is_ok() {
            self.next_observation_at = now + self.observation_backoff;
        } else {
            self.observation_backoff = self
                .observation_backoff
                .saturating_mul(2)
                .min(HYPRLAND_OBSERVATION_MAX_BACKOFF);
            self.next_observation_at = now + self.observation_backoff;
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }
}

impl NetworkPolicyWorker {
    pub(super) fn spawn() -> Result<Self, String> {
        let inner = SingleFlightWorker::spawn(
            "ferric-browser-network-policy",
            |NetworkPolicyRequest { roots, config }| {
                Ok(network_policy::load(roots.as_ref(), &config))
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(Self { inner })
    }

    pub(super) fn request(
        &mut self,
        roots: Option<StorageRoots>,
        config: Value,
    ) -> Result<(), String> {
        match self.inner.submit(NetworkPolicyRequest { roots, config }) {
            Ok(()) => Ok(()),
            Err(SubmitError::Busy) => Err("network policy load is already pending".into()),
            Err(SubmitError::QueueFull) => Err("network policy queue is full".into()),
            Err(SubmitError::Stopped) => Err("network policy worker stopped".into()),
        }
    }

    pub(super) fn poll(&mut self) -> Option<Result<network_policy::PolicySnapshot, String>> {
        match self.inner.poll() {
            WorkerPoll::Pending => None,
            WorkerPoll::Ready(result) => Some(result),
            WorkerPoll::Stopped => Some(Err("network policy worker stopped".into())),
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }
}

impl EditorWriteWorker {
    pub(super) fn new() -> Self {
        let inner = SingleFlightWorker::deferred(
            "ferric-browser-editor-writer",
            |(path, bytes): (PathBuf, Vec<u8>)| {
                (|| {
                    let parent = path
                        .parent()
                        .ok_or_else(|| "editor scratch path has no parent".to_owned())?;
                    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                    #[cfg(unix)]
                    fs::set_permissions(
                        parent,
                        std::os::unix::fs::PermissionsExt::from_mode(0o700),
                    )
                    .map_err(|error| error.to_string())?;
                    let mut options = fs::OpenOptions::new();
                    options.write(true).create_new(true);
                    #[cfg(unix)]
                    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
                    let mut file = options.open(&path).map_err(|error| error.to_string())?;
                    if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
                        let _ = fs::remove_file(&path);
                        return Err(error.to_string());
                    }
                    Ok(path)
                })()
            },
        );
        Self { inner }
    }

    pub(super) fn request(&mut self, path: PathBuf, bytes: Vec<u8>) -> Result<(), String> {
        match self.inner.submit((path, bytes)) {
            Ok(()) => Ok(()),
            Err(SubmitError::Busy) => Err("editor scratch write is already pending".into()),
            Err(SubmitError::QueueFull) => Err("editor scratch queue is full".into()),
            Err(SubmitError::Stopped) => Err("editor writer stopped".into()),
        }
    }

    pub(super) fn poll(&mut self) -> Option<Result<PathBuf, String>> {
        match self.inner.poll() {
            WorkerPoll::Pending => None,
            WorkerPoll::Ready(result) => Some(result),
            WorkerPoll::Stopped => Some(Err("editor writer stopped".into())),
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }
}

impl PrintWorker {
    pub(super) fn new() -> Self {
        let inner = SingleFlightWorker::deferred("ferric-browser-print-submit", |path: PathBuf| {
            let mut command = Command::new("lp");
            command
                .arg(&path)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            #[cfg(unix)]
            command.process_group(0);
            let mut child = command.spawn().map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => {
                    "no desktop printer command (lp) is installed".to_owned()
                }
                _ => format!("desktop print command could not start: {error}"),
            })?;
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                match child.try_wait() {
                    Ok(Some(status)) if status.success() => return Ok(path),
                    Ok(Some(_)) => {
                        return Err(
                            "desktop printer rejected the print job or is unavailable".into()
                        );
                    }
                    Ok(None) if Instant::now() >= deadline => {
                        terminate_child_process(&mut child);
                        return Err("desktop printer submission timed out".into());
                    }
                    Ok(None) => thread::sleep(Duration::from_millis(20)),
                    Err(error) => {
                        terminate_child_process(&mut child);
                        return Err(format!("desktop printer wait failed: {error}"));
                    }
                }
            }
        });
        Self { inner }
    }

    pub(super) fn request(&mut self, path: PathBuf) -> Result<(), String> {
        match self.inner.submit(path) {
            Ok(()) => Ok(()),
            Err(SubmitError::Busy) => Err("print submission is already pending".into()),
            Err(SubmitError::QueueFull) => Err("print submission queue is full".into()),
            Err(SubmitError::Stopped) => Err("print worker stopped".into()),
        }
    }

    pub(super) fn poll(&mut self) -> Option<Result<PathBuf, String>> {
        match self.inner.poll() {
            WorkerPoll::Pending => None,
            WorkerPoll::Ready(result) => Some(result),
            WorkerPoll::Stopped => Some(Err("print worker stopped".into())),
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }
}
