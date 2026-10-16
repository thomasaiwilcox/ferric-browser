use super::{
    Command, CommandExt, Duration, HyprlandConfig, Instant, Path, PathBuf, SingleFlightWorker,
    Stdio, StorageRoots, SubmitError, Value, WorkerPoll, fs, hyprland, network_policy,
    terminate_child_process, thread, userscript,
};
use std::io::Write;

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

#[derive(Debug)]
pub(super) enum UserscriptManagerResult {
    Inventory(Result<Vec<userscript::InstalledScript>, String>),
    Installed(UserscriptMutationOutcome),
    Removed(UserscriptMutationOutcome),
    SetEnabled {
        enabled: bool,
        result: UserscriptMutationOutcome,
    },
}

#[derive(Debug)]
pub(super) enum UserscriptMutationOutcome {
    Failed(String),
    Applied {
        inventory: Result<Vec<userscript::InstalledScript>, String>,
    },
}

impl UserscriptMutationOutcome {
    fn after_change(root: &Path, change: Result<(), String>) -> Self {
        match change {
            Ok(()) => Self::Applied {
                inventory: userscript::installed_scripts(root),
            },
            Err(error) => Self::Failed(error),
        }
    }
}

pub(super) struct UserscriptManagerWorker {
    inner: SingleFlightWorker<UserscriptManagerRequest, UserscriptManagerResult>,
    pending: Option<UserscriptManagerOperation>,
}

#[derive(Clone, Copy)]
enum UserscriptManagerOperation {
    Inventory,
    Install,
    Remove,
    SetEnabled(bool),
}

impl UserscriptManagerOperation {
    fn failed(self, error: String) -> UserscriptManagerResult {
        match self {
            Self::Inventory => UserscriptManagerResult::Inventory(Err(error)),
            Self::Install => {
                UserscriptManagerResult::Installed(UserscriptMutationOutcome::Failed(error))
            }
            Self::Remove => {
                UserscriptManagerResult::Removed(UserscriptMutationOutcome::Failed(error))
            }
            Self::SetEnabled(enabled) => UserscriptManagerResult::SetEnabled {
                enabled,
                result: UserscriptMutationOutcome::Failed(error),
            },
        }
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
                    let result = UserscriptMutationOutcome::after_change(
                        &root,
                        userscript::install_manifest(&root, &source).map(|_| ()),
                    );
                    UserscriptManagerResult::Installed(result)
                }
                UserscriptManagerRequest::Remove { root, name } => {
                    let result = UserscriptMutationOutcome::after_change(
                        &root,
                        userscript::remove(&root, &name),
                    );
                    UserscriptManagerResult::Removed(result)
                }
                UserscriptManagerRequest::SetEnabled {
                    root,
                    name,
                    enabled,
                } => {
                    let result = UserscriptMutationOutcome::after_change(
                        &root,
                        userscript::set_enabled(&root, &name, enabled),
                    );
                    UserscriptManagerResult::SetEnabled { enabled, result }
                }
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            inner,
            pending: None,
        })
    }

    pub(super) fn request(&mut self, request: UserscriptManagerRequest) -> Result<(), String> {
        let operation = match &request {
            UserscriptManagerRequest::Refresh { .. } => UserscriptManagerOperation::Inventory,
            UserscriptManagerRequest::Install { .. } => UserscriptManagerOperation::Install,
            UserscriptManagerRequest::Remove { .. } => UserscriptManagerOperation::Remove,
            UserscriptManagerRequest::SetEnabled { enabled, .. } => {
                UserscriptManagerOperation::SetEnabled(*enabled)
            }
        };
        match self.inner.submit(request) {
            Ok(()) => {
                self.pending = Some(operation);
                Ok(())
            }
            Err(SubmitError::Busy) => Err("userscript manager is already busy".into()),
            Err(SubmitError::QueueFull) => Err("userscript manager queue is full".into()),
            Err(SubmitError::Stopped) => Err("userscript manager stopped".into()),
        }
    }

    pub(super) fn poll(&mut self) -> Option<UserscriptManagerResult> {
        match self.inner.poll() {
            WorkerPoll::Pending => None,
            WorkerPoll::Ready(result) => {
                self.pending = None;
                Some(result)
            }
            WorkerPoll::Stopped => self.pending.take().map(|operation| {
                operation
                    .failed("The userscript service stopped. Restart Ferric and try again.".into())
            }),
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

#[cfg(test)]
mod userscript_lifecycle_tests {
    use super::*;

    fn wait_for_result(worker: &mut UserscriptManagerWorker) -> UserscriptManagerResult {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(result) = worker.poll() {
                return result;
            }
            assert!(Instant::now() < deadline, "worker did not answer");
            thread::yield_now();
        }
    }

    #[test]
    fn applied_script_changes_survive_a_separate_inventory_failure() {
        let root =
            std::env::temp_dir().join(format!("ferric-script-outcome-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("userscripts")).expect("script directory");
        fs::write(root.join("userscripts/broken.toml"), "invalid = [").expect("broken manifest");
        let source = root.join("example.toml");
        fs::write(
            &source,
            "schema_version = 1\nname = \"example\"\nexecutable = \"/bin/true\"\n",
        )
        .expect("valid manifest");
        let mut worker = UserscriptManagerWorker::spawn().expect("script service");
        worker
            .request(UserscriptManagerRequest::Install {
                root: root.clone(),
                source,
            })
            .expect("install");
        assert!(matches!(
            wait_for_result(&mut worker),
            UserscriptManagerResult::Installed(UserscriptMutationOutcome::Applied {
                inventory: Err(_)
            })
        ));
        let installed = root.join("userscripts/example.toml");
        assert!(
            installed.is_file(),
            "the install succeeded despite the unrelated bad manifest"
        );
        worker
            .request(UserscriptManagerRequest::SetEnabled {
                root: root.clone(),
                name: "example".into(),
                enabled: false,
            })
            .expect("toggle");
        assert!(matches!(
            wait_for_result(&mut worker),
            UserscriptManagerResult::SetEnabled {
                enabled: false,
                result: UserscriptMutationOutcome::Applied { inventory: Err(_) }
            }
        ));
        assert!(
            fs::read_to_string(&installed)
                .expect("saved manifest")
                .contains("enabled = false")
        );
        worker
            .request(UserscriptManagerRequest::Remove {
                root: root.clone(),
                name: "example".into(),
            })
            .expect("remove");
        assert!(matches!(
            wait_for_result(&mut worker),
            UserscriptManagerResult::Removed(UserscriptMutationOutcome::Applied {
                inventory: Err(_)
            })
        ));
        assert!(
            !installed.exists(),
            "the removal succeeded despite the unrelated bad manifest"
        );
        drop(worker);
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn stopped_userscript_work_completes_its_own_operation_once() {
        for operation in [
            UserscriptManagerOperation::Inventory,
            UserscriptManagerOperation::Install,
            UserscriptManagerOperation::Remove,
            UserscriptManagerOperation::SetEnabled(false),
            UserscriptManagerOperation::SetEnabled(true),
        ] {
            let mut worker = UserscriptManagerWorker {
                inner: SingleFlightWorker::spawn("userscript-stop-test", |_| {
                    panic!("simulate an unexpected userscript service exit")
                })
                .expect("test worker"),
                pending: None,
            };
            let root = PathBuf::new();
            let request = match operation {
                UserscriptManagerOperation::Inventory => UserscriptManagerRequest::Refresh { root },
                UserscriptManagerOperation::Install => UserscriptManagerRequest::Install {
                    root,
                    source: PathBuf::new(),
                },
                UserscriptManagerOperation::Remove => UserscriptManagerRequest::Remove {
                    root,
                    name: "example".into(),
                },
                UserscriptManagerOperation::SetEnabled(enabled) => {
                    UserscriptManagerRequest::SetEnabled {
                        root,
                        name: "example".into(),
                        enabled,
                    }
                }
            };
            worker.request(request).expect("operation accepted");
            assert!(
                worker
                    .request(UserscriptManagerRequest::Refresh {
                        root: PathBuf::new(),
                    })
                    .is_err()
            );
            let result = wait_for_result(&mut worker);
            match (operation, result) {
                (
                    UserscriptManagerOperation::Inventory,
                    UserscriptManagerResult::Inventory(Err(_)),
                )
                | (
                    UserscriptManagerOperation::Install,
                    UserscriptManagerResult::Installed(UserscriptMutationOutcome::Failed(_)),
                )
                | (
                    UserscriptManagerOperation::Remove,
                    UserscriptManagerResult::Removed(UserscriptMutationOutcome::Failed(_)),
                ) => {}
                (
                    UserscriptManagerOperation::SetEnabled(expected),
                    UserscriptManagerResult::SetEnabled {
                        enabled,
                        result: UserscriptMutationOutcome::Failed(_),
                    },
                ) => assert_eq!(enabled, expected),
                (_, result) => panic!("failure delivered to the wrong operation: {result:?}"),
            }
            assert!(!worker.is_pending());
            assert!(
                worker.poll().is_none(),
                "failure must not repeat on idle polls"
            );
        }
    }
}
