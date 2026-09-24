//! Off-thread configuration validation and private export writes.
//!
//! Qt submits work here and applies the completed typed result; parsing and
//! filesystem policy never execute on the GUI thread.

use crate::single_flight::{Poll as WorkerPoll, SingleFlightWorker, SubmitError};
use ferric_browser_config::{
    LoadedConfig, RuntimeOverrides, load, load_contexts, load_profile_runtime_overrides,
    load_profiles, profile_config_path,
};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub(super) struct ConfigReloadResult {
    pub(super) loaded: LoadedConfig,
    pub(super) profile_overrides: RuntimeOverrides,
    pub(super) profiles_path: Option<PathBuf>,
    pub(super) operation: ConfigReadOperation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConfigReadOperation {
    Reload,
    Check,
}

enum ConfigReloadRequest {
    Load { path: PathBuf, profile_name: String },
    Check { path: PathBuf },
}

pub(super) struct ConfigReloadWorker {
    inner: SingleFlightWorker<ConfigReloadRequest, Result<ConfigReloadResult, String>>,
}

pub(super) struct ConfigWriteResult {
    pub(super) path: PathBuf,
    pub(super) bytes: usize,
}

enum ConfigWriteRequest {
    Write { path: PathBuf, bytes: Vec<u8> },
}

pub(super) struct ConfigWriteWorker {
    inner: SingleFlightWorker<ConfigWriteRequest, Result<ConfigWriteResult, String>>,
}

impl ConfigWriteWorker {
    pub(super) fn spawn() -> Result<Self, String> {
        let inner = SingleFlightWorker::spawn(
            "ferric-browser-config-writer",
            |request: ConfigWriteRequest| match request {
                ConfigWriteRequest::Write { path, bytes } => (|| {
                    let mut options = fs::OpenOptions::new();
                    options.write(true).create_new(true);
                    #[cfg(unix)]
                    options.mode(0o600);
                    let mut file = options.open(&path).map_err(|error| error.to_string())?;
                    if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
                        let _ = fs::remove_file(&path);
                        return Err(error.to_string());
                    }
                    Ok(ConfigWriteResult {
                        path,
                        bytes: bytes.len(),
                    })
                })(),
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(Self { inner })
    }

    pub(super) fn request(&mut self, path: PathBuf, bytes: Vec<u8>) -> Result<(), String> {
        match self.inner.submit(ConfigWriteRequest::Write { path, bytes }) {
            Ok(()) => Ok(()),
            Err(SubmitError::Busy) => Err("configuration write is already pending".into()),
            Err(SubmitError::QueueFull) => Err("configuration write queue is full".into()),
            Err(SubmitError::Stopped) => Err("configuration writer stopped".into()),
        }
    }

    pub(super) fn poll(&mut self) -> Option<Result<ConfigWriteResult, String>> {
        match self.inner.poll() {
            WorkerPoll::Pending => None,
            WorkerPoll::Ready(result) => Some(result),
            WorkerPoll::Stopped => Some(Err("configuration writer stopped".into())),
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }
}

impl ConfigReloadWorker {
    pub(super) fn spawn() -> Result<Self, String> {
        let inner = SingleFlightWorker::spawn(
            "ferric-browser-config-loader",
            |request: ConfigReloadRequest| match request {
                ConfigReloadRequest::Load { path, profile_name } => load(&path)
                    .map_err(|error| error.to_string())
                    .and_then(|loaded| {
                        let directory = path.parent().unwrap_or_else(|| Path::new("."));
                        let contexts_path = directory.join("contexts.toml");
                        if contexts_path.exists() {
                            load_contexts(&contexts_path).map_err(|error| {
                                format!("contexts configuration is invalid: {error}")
                            })?;
                        }
                        let profile_overrides =
                            load_profile_runtime_overrides(&path, &profile_name)
                                .map_err(|error| error.to_string())?;
                        let profiles_path = profile_config_path(&path);
                        Ok(ConfigReloadResult {
                            loaded,
                            profile_overrides,
                            profiles_path: profiles_path.exists().then_some(profiles_path),
                            operation: ConfigReadOperation::Reload,
                        })
                    }),
                ConfigReloadRequest::Check { path } => load(&path)
                    .map_err(|error| format!("configuration is invalid: {error}"))
                    .and_then(|loaded| {
                        let directory = path.parent().unwrap_or_else(|| Path::new("."));
                        let contexts_path = directory.join("contexts.toml");
                        if contexts_path.exists() {
                            load_contexts(&contexts_path).map_err(|error| {
                                format!("contexts configuration is invalid: {error}")
                            })?;
                        }
                        let profiles_path = directory.join("profiles.toml");
                        if profiles_path.exists() {
                            load_profiles(&profiles_path).map_err(|error| {
                                format!("profiles configuration is invalid: {error}")
                            })?;
                        }
                        Ok(ConfigReloadResult {
                            loaded,
                            profile_overrides: RuntimeOverrides::default(),
                            profiles_path: profiles_path.exists().then_some(profiles_path),
                            operation: ConfigReadOperation::Check,
                        })
                    }),
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(Self { inner })
    }

    pub(super) fn request(&mut self, path: PathBuf, profile_name: String) -> Result<(), String> {
        match self
            .inner
            .submit(ConfigReloadRequest::Load { path, profile_name })
        {
            Ok(()) => Ok(()),
            Err(SubmitError::Busy) => Err("configuration reload is already pending".into()),
            Err(SubmitError::QueueFull) => Err("configuration reload queue is full".into()),
            Err(SubmitError::Stopped) => Err("configuration reload worker stopped".into()),
        }
    }

    pub(super) fn request_check(&mut self, path: PathBuf) -> Result<(), String> {
        match self.inner.submit(ConfigReloadRequest::Check { path }) {
            Ok(()) => Ok(()),
            Err(SubmitError::Busy) => Err("configuration read is already pending".into()),
            Err(SubmitError::QueueFull) => Err("configuration read queue is full".into()),
            Err(SubmitError::Stopped) => Err("configuration reload worker stopped".into()),
        }
    }

    pub(super) fn poll(&mut self) -> Option<Result<ConfigReloadResult, String>> {
        match self.inner.poll() {
            WorkerPoll::Pending => None,
            WorkerPoll::Ready(result) => Some(result),
            WorkerPoll::Stopped => Some(Err("configuration reload worker stopped".into())),
        }
    }

    pub(super) fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }
}
