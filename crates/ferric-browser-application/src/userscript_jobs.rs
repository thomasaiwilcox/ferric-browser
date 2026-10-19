//! Profile-owned userscript discovery and mutation jobs.
use crate::{BrowserApplication, userscript};
use ferric_browser_runtime::worker::{Poll as WorkerPoll, SingleFlightWorker, SubmitError};
use std::path::{Path, PathBuf};

pub enum UserscriptManagerRequest {
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
pub enum UserscriptManagerResult {
    Inventory(
        Result<Vec<userscript::InstalledScript>, String>,
        Result<std::sync::Arc<userscript::UserscriptCatalog>, String>,
    ),
    Installed(UserscriptMutationOutcome),
    Removed(UserscriptMutationOutcome),
    SetEnabled {
        enabled: bool,
        result: UserscriptMutationOutcome,
    },
}

#[derive(Debug)]
pub enum UserscriptMutationOutcome {
    Failed(String),
    Applied {
        inventory: Result<Vec<userscript::InstalledScript>, String>,
        catalog: Result<std::sync::Arc<userscript::UserscriptCatalog>, String>,
    },
}

impl UserscriptMutationOutcome {
    fn after_change(root: &Path, change: Result<(), String>) -> Self {
        match change {
            Ok(()) => Self::Applied {
                inventory: userscript::installed_scripts(root),
                catalog: userscript::UserscriptCatalog::load(root).map(std::sync::Arc::new),
            },
            Err(error) => Self::Failed(error),
        }
    }
}

struct UserscriptMutationReservation {
    shared: std::sync::Arc<crate::profile_configuration::SharedConfiguration>,
}
impl Drop for UserscriptMutationReservation {
    fn drop(&mut self) {
        self.shared
            .userscript_gate
            .store(false, std::sync::atomic::Ordering::Release);
    }
}
struct OwnedRequest {
    request: UserscriptManagerRequest,
    reservation: Option<UserscriptMutationReservation>,
}

pub(crate) struct UserscriptManagerWorker {
    inner: SingleFlightWorker<OwnedRequest, UserscriptManagerResult>,
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
            Self::Inventory => UserscriptManagerResult::Inventory(Err(error.clone()), Err(error)),
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
    pub(crate) fn spawn(
        lease: Option<ferric_browser_storage::ProfileLock>,
    ) -> Result<Self, String> {
        let inner = SingleFlightWorker::spawn(
            "ferric-browser-userscript-manager",
            move |owned: OwnedRequest| {
                let _lease = &lease;
                let _reservation = owned.reservation;
                match owned.request {
                    UserscriptManagerRequest::Refresh { root } => {
                        UserscriptManagerResult::Inventory(
                            userscript::installed_scripts(&root),
                            userscript::UserscriptCatalog::load(&root).map(std::sync::Arc::new),
                        )
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
                }
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            inner,
            pending: None,
        })
    }

    #[cfg(test)]
    fn request(&mut self, request: UserscriptManagerRequest) -> Result<(), String> {
        self.request_reserved(request, None)
    }

    fn request_reserved(
        &mut self,
        request: UserscriptManagerRequest,
        reservation: Option<UserscriptMutationReservation>,
    ) -> Result<(), String> {
        let operation = match &request {
            UserscriptManagerRequest::Refresh { .. } => UserscriptManagerOperation::Inventory,
            UserscriptManagerRequest::Install { .. } => UserscriptManagerOperation::Install,
            UserscriptManagerRequest::Remove { .. } => UserscriptManagerOperation::Remove,
            UserscriptManagerRequest::SetEnabled { enabled, .. } => {
                UserscriptManagerOperation::SetEnabled(*enabled)
            }
        };
        match self.inner.submit(OwnedRequest {
            request,
            reservation,
        }) {
            Ok(()) => {
                self.pending = Some(operation);
                Ok(())
            }
            Err(SubmitError::Busy) => Err("userscript manager is already busy".into()),
            Err(SubmitError::QueueFull) => Err("userscript manager queue is full".into()),
            Err(SubmitError::Stopped) => Err("userscript manager stopped".into()),
        }
    }

    pub(crate) fn poll(&mut self) -> Option<UserscriptManagerResult> {
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

    pub(crate) fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }
}

impl UserscriptManagerRequest {
    fn root(&self) -> &Path {
        match self {
            Self::Refresh { root }
            | Self::Install { root, .. }
            | Self::Remove { root, .. }
            | Self::SetEnabled { root, .. } => root,
        }
    }
}

impl BrowserApplication {
    /// Starts a bounded job against this profile's configured userscript root.
    /// # Errors
    /// Rejects unavailable or mismatched roots, private mutations and busy workers.
    pub fn request_userscript_job(
        &mut self,
        request: UserscriptManagerRequest,
    ) -> Result<(), String> {
        if self.userscript_root.as_deref() != Some(request.root()) {
            return Err("Userscript request does not belong to the active profile.".into());
        }
        if !matches!(request, UserscriptManagerRequest::Refresh { .. })
            && self
                .profile_snapshot()
                .is_none_or(|profile| profile.privacy.is_transient())
        {
            return Err("Userscript changes require a regular profile.".into());
        }
        let reservation = if matches!(request, UserscriptManagerRequest::Refresh { .. }) {
            None
        } else {
            let shared = self
                .active_profile
                .as_ref()
                .ok_or_else(|| "Userscript owner is unavailable.".to_owned())?
                .session
                .0
                .configuration
                .clone();
            shared
                .userscript_gate
                .compare_exchange(
                    false,
                    true,
                    std::sync::atomic::Ordering::AcqRel,
                    std::sync::atomic::Ordering::Acquire,
                )
                .map_err(|_| {
                    "Another window is changing userscripts. Try again after it finishes."
                        .to_owned()
                })?;
            Some(UserscriptMutationReservation { shared })
        };
        if self.userscript_worker.is_none() {
            let lease = self
                .active_profile
                .as_ref()
                .and_then(crate::profile::ActiveProfile::persistence_lease);
            self.userscript_worker = Some(UserscriptManagerWorker::spawn(lease)?);
        }
        self.userscript_worker
            .as_mut()
            .ok_or_else(|| "Userscript worker is unavailable.".to_owned())?
            .request_reserved(request, reservation)
    }

    #[must_use]
    pub fn userscript_job_pending(&self) -> bool {
        self.userscript_worker
            .as_ref()
            .is_some_and(UserscriptManagerWorker::is_pending)
    }

    /// Publishes only into the application that admitted the job. Retiring the
    /// profile drops its observer while the owned job retains the profile lease.
    pub fn poll_userscript_job(&mut self) -> Option<UserscriptManagerResult> {
        let result = self.userscript_worker.as_mut()?.poll()?;
        let catalog = match &result {
            UserscriptManagerResult::Inventory(_, catalog) => Some(catalog),
            UserscriptManagerResult::Installed(outcome)
            | UserscriptManagerResult::Removed(outcome)
            | UserscriptManagerResult::SetEnabled {
                result: outcome, ..
            } => match outcome {
                UserscriptMutationOutcome::Applied { catalog, .. } => Some(catalog),
                UserscriptMutationOutcome::Failed(_) => None,
            },
        };
        if let Some(catalog) = catalog {
            self.replace_page_script_catalog(catalog.clone());
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, thread, time::Duration};
    use uuid::Uuid;
    #[test]
    #[allow(clippy::too_many_lines)]
    fn userscript_manager_worker_reads_and_updates_off_thread() {
        let root = std::env::temp_dir().join(format!(
            "ferric-browser-userscript-worker-{}-{}",
            std::process::id(),
            Uuid::new_v4()
        ));
        let directory = root.join("userscripts");
        fs::create_dir_all(&directory).expect("userscript worker directory");
        fs::write(
            directory.join("worker.toml"),
            r#"
schema_version = 1
name = "worker"
executable = "/bin/true"
enabled = true
matches = ["https://example.test/*"]
"#,
        )
        .expect("userscript worker manifest");

        let mut worker = UserscriptManagerWorker::spawn(None).expect("userscript manager");
        worker
            .request(UserscriptManagerRequest::Refresh { root: root.clone() })
            .expect("inventory request");
        let inventory = (0..100).find_map(|_| {
            let value = worker.poll();
            if value.is_none() {
                thread::sleep(Duration::from_millis(1));
            }
            value
        });
        match inventory.expect("inventory response") {
            UserscriptManagerResult::Inventory(Ok(scripts), _) => {
                assert_eq!(scripts.len(), 1);
                assert!(scripts[0].enabled);
            }
            other => panic!("unexpected inventory result: {other:?}"),
        }

        worker
            .request(UserscriptManagerRequest::SetEnabled {
                root: root.clone(),
                name: "worker".into(),
                enabled: false,
            })
            .expect("toggle request");
        let toggled = (0..100).find_map(|_| {
            let value = worker.poll();
            if value.is_none() {
                thread::sleep(Duration::from_millis(1));
            }
            value
        });
        match toggled.expect("toggle response") {
            UserscriptManagerResult::SetEnabled {
                enabled: false,
                result:
                    UserscriptMutationOutcome::Applied {
                        inventory: Ok(scripts),
                        ..
                    },
            } => assert!(!scripts[0].enabled),
            other => panic!("unexpected toggle result: {other:?}"),
        }

        let source = root.join("installed.toml");
        fs::write(
            &source,
            r#"
schema_version = 1
name = "installed"
executable = "/bin/true"
"#,
        )
        .expect("source userscript manifest");
        worker
            .request(UserscriptManagerRequest::Install {
                root: root.clone(),
                source,
            })
            .expect("install request");
        let installed = (0..100).find_map(|_| {
            let value = worker.poll();
            if value.is_none() {
                thread::sleep(Duration::from_millis(1));
            }
            value
        });
        match installed.expect("install response") {
            UserscriptManagerResult::Installed(UserscriptMutationOutcome::Applied {
                inventory: Ok(scripts),
                ..
            }) => {
                assert_eq!(scripts.len(), 2);
            }
            other => panic!("unexpected install result: {other:?}"),
        }

        worker
            .request(UserscriptManagerRequest::Remove {
                root: root.clone(),
                name: "installed".into(),
            })
            .expect("remove request");
        let removed = (0..100).find_map(|_| {
            let value = worker.poll();
            if value.is_none() {
                thread::sleep(Duration::from_millis(1));
            }
            value
        });
        match removed.expect("remove response") {
            UserscriptManagerResult::Removed(UserscriptMutationOutcome::Applied {
                inventory: Ok(scripts),
                ..
            }) => assert_eq!(scripts.len(), 1),
            other => panic!("unexpected remove result: {other:?}"),
        }
        assert!(!root.join("userscripts/installed.toml").exists());

        drop(worker);
        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(test)]
mod userscript_lifecycle_tests {
    use super::*;
    use std::{
        fs, thread,
        time::{Duration, Instant},
    };

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
        let mut worker = UserscriptManagerWorker::spawn(None).expect("script service");
        worker
            .request(UserscriptManagerRequest::Install {
                root: root.clone(),
                source,
            })
            .expect("install");
        assert!(matches!(
            wait_for_result(&mut worker),
            UserscriptManagerResult::Installed(UserscriptMutationOutcome::Applied {
                inventory: Err(_),
                ..
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
                result: UserscriptMutationOutcome::Applied {
                    inventory: Err(_),
                    ..
                }
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
                inventory: Err(_),
                ..
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
                    UserscriptManagerResult::Inventory(Err(_), _),
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
