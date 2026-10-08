use super::{
    Command, CommandExt, Duration, HyprlandConfig, Instant, PathBuf, SingleFlightWorker, Stdio,
    StorageRoots, SubmitError, Value, WorkerPoll, fs, hyprland, network_policy,
    terminate_child_process, thread, userscript,
};
use ferric_browser_storage::{MediaFingerprintKey, MediaFingerprintKeyError};
use std::{
    io::Write,
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
};
use uuid::Uuid;

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
    RouteWorkspace(Result<(), String>),
    MoveActiveWindow(Result<(), String>),
    BrowserClients(Result<Vec<hyprland::BrowserClient>, String>),
}

pub(super) struct HyprlandWorker {
    inner: SingleFlightWorker<HyprlandRequest, HyprlandResponse>,
}

impl HyprlandWorker {
    pub(super) fn spawn() -> Result<Self, String> {
        let inner =
            SingleFlightWorker::spawn("ferric-browser-hyprland-ipc", |request| match request {
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
            })
            .map_err(|error| error.to_string())?;
        Ok(Self { inner })
    }

    pub(super) fn request(&mut self, request: HyprlandRequest) -> Result<(), SubmitError> {
        self.inner.submit(request)
    }

    pub(super) fn poll(&mut self) -> Option<HyprlandResponse> {
        match self.inner.poll() {
            WorkerPoll::Ready(response) => Some(response),
            WorkerPoll::Pending | WorkerPoll::Stopped => None,
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }
}

pub(super) enum UserscriptManagerRequest {
    Refresh {
        root: PathBuf,
    },
    Install {
        root: PathBuf,
        source: PathBuf,
    },
    Remove {
        root: PathBuf,
        name: String,
    },
    SetEnabled {
        root: PathBuf,
        name: String,
        enabled: bool,
    },
}

pub(super) enum UserscriptManagerResult {
    Inventory(Result<Vec<userscript::InstalledScript>, String>),
    Installed(Result<Vec<userscript::InstalledScript>, String>),
    Removed(Result<Vec<userscript::InstalledScript>, String>),
    SetEnabled {
        enabled: bool,
        result: Result<Vec<userscript::InstalledScript>, String>,
    },
}

pub(super) struct UserscriptManagerWorker {
    inner: SingleFlightWorker<UserscriptManagerRequest, UserscriptManagerResult>,
}

pub(super) struct MediaFingerprintKeyWorker {
    sender: SyncSender<(StorageRoots, Uuid, bool)>,
    receiver: Receiver<(Uuid, Result<MediaFingerprintKey, MediaFingerprintKeyError>)>,
    pending: bool,
}

impl MediaFingerprintKeyWorker {
    pub(super) fn spawn() -> Result<Self, String> {
        let (sender, requests) = mpsc::sync_channel(1);
        let (responses, receiver) = mpsc::sync_channel(1);
        let detached_worker = std::thread::Builder::new()
            .name("ferric-browser-media-fingerprint-key".into())
            .spawn(move || {
                while let Ok((roots, profile_id, create_missing)) = requests.recv() {
                    let result = if create_missing {
                        MediaFingerprintKey::open_blocking(&roots, profile_id)
                    } else {
                        MediaFingerprintKey::load_blocking(&roots, profile_id)
                    };
                    if responses.send((profile_id, result)).is_err() {
                        break;
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        // Detach deliberately: Secret Service may block during shutdown. The
        // worker owns no Qt object and drops a late key if its receiver closes.
        drop(detached_worker);
        Ok(Self {
            sender,
            receiver,
            pending: false,
        })
    }

    pub(super) fn request(
        &mut self,
        roots: StorageRoots,
        profile_id: Uuid,
        create_missing: bool,
    ) -> Result<(), SubmitError> {
        if self.pending {
            return Err(SubmitError::Busy);
        }
        match self.sender.try_send((roots, profile_id, create_missing)) {
            Ok(()) => {
                self.pending = true;
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(SubmitError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(SubmitError::Stopped),
        }
    }

    pub(super) fn poll(
        &mut self,
    ) -> Option<(Uuid, Result<MediaFingerprintKey, MediaFingerprintKeyError>)> {
        match self.receiver.try_recv() {
            Ok(result) => {
                self.pending = false;
                Some(result)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.pending = false;
                None
            }
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        self.pending
    }
}

impl UserscriptManagerWorker {
    pub(super) fn spawn() -> Result<Self, String> {
        let inner = SingleFlightWorker::spawn(
            "ferric-browser-userscript-manager",
            |request: UserscriptManagerRequest| match request {
                UserscriptManagerRequest::Refresh { root } => {
                    UserscriptManagerResult::Inventory(userscript::installed_scripts(&root))
                }
                UserscriptManagerRequest::Install { root, source } => {
                    let result = userscript::install_manifest(&root, &source)
                        .and_then(|_installed_name| userscript::installed_scripts(&root));
                    UserscriptManagerResult::Installed(result)
                }
                UserscriptManagerRequest::Remove { root, name } => {
                    let result = userscript::remove(&root, &name)
                        .and_then(|()| userscript::installed_scripts(&root));
                    UserscriptManagerResult::Removed(result)
                }
                UserscriptManagerRequest::SetEnabled {
                    root,
                    name,
                    enabled,
                } => {
                    let result = userscript::set_enabled(&root, &name, enabled)
                        .and_then(|()| userscript::installed_scripts(&root));
                    UserscriptManagerResult::SetEnabled { enabled, result }
                }
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(Self { inner })
    }

    pub(super) fn request(&mut self, request: UserscriptManagerRequest) -> Result<(), String> {
        match self.inner.submit(request) {
            Ok(()) => Ok(()),
            Err(SubmitError::Busy) => Err("userscript manager is already busy".into()),
            Err(SubmitError::QueueFull) => Err("userscript manager queue is full".into()),
            Err(SubmitError::Stopped) => Err("userscript manager stopped".into()),
        }
    }

    pub(super) fn poll(&mut self) -> Option<UserscriptManagerResult> {
        match self.inner.poll() {
            WorkerPoll::Pending => None,
            WorkerPoll::Ready(result) => Some(result),
            WorkerPoll::Stopped => Some(UserscriptManagerResult::Inventory(Err(
                "userscript manager stopped".into(),
            ))),
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
    pub(super) fn spawn() -> Result<Self, String> {
        let inner = SingleFlightWorker::spawn(
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
        )
        .map_err(|error| error.to_string())?;
        Ok(Self { inner })
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
    pub(super) fn spawn() -> Result<Self, String> {
        let inner = SingleFlightWorker::spawn("ferric-browser-print-submit", |path: PathBuf| {
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
        })
        .map_err(|error| error.to_string())?;
        Ok(Self { inner })
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
