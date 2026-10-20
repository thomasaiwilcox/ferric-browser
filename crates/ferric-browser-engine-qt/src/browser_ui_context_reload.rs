//! A validated reload waits for its application-owned context disk operation.
use super::{CxxQtType, Pin, QString, qobject};
use ferric_browser_application::{
    ConfigurationLayers, ConfigurationReservation, ContextReloadEffect, ContextReloadJob,
};
use ferric_browser_config::{ContextsConfig, RuntimeOverrides};
use ferric_browser_runtime::worker::{Poll, SingleFlightWorker, SubmitError};
use serde_json::Value;
use std::path::PathBuf;

pub(super) struct ConfigReloadPublication {
    pub candidate: Value,
    pub layers: ConfigurationLayers,
    pub fingerprint: String,
    pub path: PathBuf,
    pub base_json: String,
    pub profile_overrides: RuntimeOverrides,
    pub sources: Vec<PathBuf>,
    pub profiles_path: Option<PathBuf>,
    pub contexts: ContextsConfig,
    pub announce: bool,
}
pub(super) struct PendingContextReload {
    publication: ConfigReloadPublication,
    reservation: ConfigurationReservation,
}
pub(super) struct ContextReloadWorker {
    inner: SingleFlightWorker<ContextReloadJob, Result<ContextReloadEffect, String>>,
}
impl ContextReloadWorker {
    fn spawn() -> Result<Self, String> {
        SingleFlightWorker::spawn("ferric-context-reload", |job: ContextReloadJob| {
            job.run().map_err(|error| error.to_string())
        })
        .map(|inner| Self { inner })
        .map_err(|error| error.to_string())
    }
    pub(super) fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }
    fn poll(&mut self) -> Poll<Result<ContextReloadEffect, String>> {
        self.inner.poll()
    }
}
impl qobject::BrowserUi {
    pub(super) fn queue_context_configuration_reload(
        mut self: Pin<&mut Self>,
        publication: ConfigReloadPublication,
    ) -> Result<(), String> {
        if self
            .as_ref()
            .rust()
            .configuration_io
            .pending_context_reload
            .is_some()
        {
            return Err("Settings are already being reloaded.".into());
        }
        let previous = self.as_ref().current_contexts_config()?;
        let browser = self.as_ref();
        let state = browser
            .rust()
            .state
            .as_ref()
            .ok_or("The window profile is unavailable.")?;
        // Do all fallible projection/policy preparation before starting disk work.
        let prepared = state
            .prepare_configuration_reload(publication.layers.clone())
            .map_err(|error| error.to_string())?;
        let active = serde_json::to_value(prepared.snapshot().effective)
            .map_err(|error| error.to_string())?;
        self.as_ref()
            .resolve_candidate_mixed_policy(&active, &publication.layers)?;
        let job = state
            .prepare_context_reload(&previous, publication.contexts.clone())
            .map_err(|error| error.to_string())?;
        if let Some(job) = job {
            let reservation = state
                .reserve_configuration_reload()
                .map_err(|error| error.to_string())?;
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.configuration_io.context_reload_worker.is_none() {
                this.configuration_io.context_reload_worker = Some(ContextReloadWorker::spawn()?);
            }
            let submitted = this
                .configuration_io
                .context_reload_worker
                .as_mut()
                .ok_or("Context reload is unavailable.")?
                .inner
                .submit(job);
            match submitted {
                Ok(()) => {}
                Err(SubmitError::Busy | SubmitError::QueueFull) => {
                    return Err("The context update is busy. Try reloading settings again.".into());
                }
                Err(SubmitError::Stopped) => {
                    this.configuration_io.context_reload_worker = None;
                    return Err(
                        "The context update could not start. Try reloading settings again.".into(),
                    );
                }
            }
            let announce = publication.announce;
            this.configuration_io.pending_context_reload = Some(PendingContextReload {
                publication,
                reservation,
            });
            drop(rust);
            if announce {
                self.set_status_text(QString::from("Reloading settings and contexts…"));
            }
            Ok(())
        } else {
            self.finish_context_configuration_reload(publication, None)
        }
    }
    pub(super) fn poll_context_configuration_reload(mut self: Pin<&mut Self>) {
        let result = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .configuration_io
            .context_reload_worker
            .as_mut()
            .map(ContextReloadWorker::poll);
        let result = match result {
            None | Some(Poll::Pending) => return,
            Some(Poll::Ready(result)) => result,
            Some(Poll::Stopped) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .configuration_io
                    .context_reload_worker = None;
                Err("The context update could not finish. Try reloading settings again.".into())
            }
        };
        let pending = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .configuration_io
            .pending_context_reload
            .take();
        let Some(pending) = pending else {
            return;
        };
        let announce = pending.publication.announce;
        let result = result.and_then(|effect| {
            self.as_ref()
                .rust()
                .state
                .as_ref()
                .ok_or("The window profile is unavailable.".to_owned())?
                .release_configuration_reload(pending.reservation)
                .map_err(|error| error.to_string())?;
            self.as_mut()
                .finish_context_configuration_reload(pending.publication, Some(effect))
        });
        if let Err(error) = result {
            self.report_config_reload_failure(&error, announce);
        }
    }
    fn finish_context_configuration_reload(
        mut self: Pin<&mut Self>,
        publication: ConfigReloadPublication,
        effect: Option<ContextReloadEffect>,
    ) -> Result<(), String> {
        self.as_mut().commit_reloaded_config(
            publication.candidate,
            publication.layers,
            publication.announce.then_some("Settings reloaded"),
            publication.fingerprint,
            effect,
        )?;
        if self.as_ref().active_profile_is_transient() {
            self.as_mut()
                .publish_contexts_config(&publication.contexts)?;
        }
        let mut sources = publication.sources;
        if let Some(path) = publication.profiles_path {
            sources.push(path);
        }
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if let Some(profile) = this.profile_open_effect.as_mut() {
            profile.config_sources.clone_from(&sources);
        }
        if let Some(state) = this.state.as_ref() {
            sources.extend(state.userscript_watch_sources());
        }
        this.configuration_io
            .config_watch
            .refresh_sources(&publication.path, &sources);
        drop(rust);
        self.as_mut()
            .set_config_base_json(QString::from(publication.base_json));
        self.set_profile_overrides_json(QString::from(
            serde_json::to_string(&publication.profile_overrides)
                .map_err(|error| error.to_string())?,
        ));
        Ok(())
    }
}
