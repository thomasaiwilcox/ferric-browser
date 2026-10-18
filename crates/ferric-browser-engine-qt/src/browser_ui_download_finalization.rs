//! GUI admission for asynchronously prepared files. Workers cannot publish.

use super::download_finalization::{DownloadFinalizer, SaveError};
use super::{CxxQtType, Pin, QString, cleanup_staged_download, qobject};

impl qobject::BrowserUi {
    pub(super) fn download_save_capacity(&self) -> bool {
        self.rust().staged_downloads.len()
            + self
                .rust()
                .download_finalizer
                .as_ref()
                .map_or(0, DownloadFinalizer::len)
            < super::download_finalization::CAPACITY
    }

    pub(super) fn finalize_download(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let id = id.to_string();
        if self
            .as_ref()
            .rust()
            .download_finalizer
            .as_ref()
            .is_some_and(|worker| worker.contains(&id))
        {
            return true;
        }
        let Some(staged) = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .staged_downloads
            .remove(&id)
        else {
            self.set_status_text(QString::from(
                "The downloaded file is no longer available. Retry the download.",
            ));
            return false;
        };
        if !self.as_ref().rust().state.as_ref().is_some_and(|state| {
            state.session_download_finalization_current(&id, staged.final_path())
        }) {
            cleanup_staged_download(&staged);
            return false;
        }
        let session = self.as_ref().rust().session_id;
        let result = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.download_finalizer.is_none() {
                this.download_finalizer = DownloadFinalizer::spawn().ok();
            }
            if let Some(worker) = this.download_finalizer.as_mut() {
                worker.submit(id, session, staged)
            } else {
                cleanup_staged_download(&staged);
                Err(SaveError::Unavailable)
            }
        };
        if result.is_err() {
            self.set_status_text(QString::from(
                "The file couldn't be saved. Reopen this window and retry the download.",
            ));
            return false;
        }
        true
    }

    pub(super) fn download_finalization_pending(&self, id: &QString) -> bool {
        self.rust()
            .download_finalizer
            .as_ref()
            .is_some_and(|worker| worker.contains(&id.to_string()))
    }

    pub(super) fn has_finalizing_downloads(&self) -> bool {
        self.rust()
            .state
            .as_ref()
            .and_then(|state| state.session_download_rows(&[]).ok())
            .is_some_and(|rows| {
                rows.iter().any(|row| {
                    row.finishing
                        && row.active
                        && self
                            .rust()
                            .download_finalizer
                            .as_ref()
                            .is_some_and(|worker| worker.contains(&row.record.id))
                })
            })
    }

    pub(super) fn cancel_download_finalization(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let cancelled = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .download_finalizer
            .as_mut()
            .is_some_and(|worker| worker.cancel(&id.to_string()));
        if cancelled {
            self.as_mut()
                .update_download(id, &QString::from("cancelled"), 0, true);
        }
        cancelled
    }

    pub(super) fn record_download_metrics(
        mut self: Pin<&mut Self>,
        id: &QString,
        total: i64,
        code: i32,
        reason: &QString,
    ) {
        let reason = reason.to_string();
        let failure = (code != 0).then_some(ferric_browser_application::DownloadFailure::Native {
            code,
            message: reason,
        });
        if let Some(state) = self.as_mut().rust_mut().as_mut().get_mut().state.as_mut() {
            let _ = state.note_session_download_metrics(&id.to_string(), total.max(-1), failure);
        }
    }

    pub(super) fn poll_download_finalizations(mut self: Pin<&mut Self>) {
        let completions = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .download_finalizer
            .as_mut()
            .map_or_else(Vec::new, DownloadFinalizer::poll);
        for completion in completions {
            let id = QString::from(&completion.id);
            let result = completion.result.and_then(|prepared| {
                if completion.session != self.as_ref().rust().session_id
                    || !self.as_ref().rust().state.as_ref().is_some_and(|state| {
                        state.session_download_finalization_current(
                            &completion.id,
                            prepared.destination(),
                        )
                    })
                {
                    return Err(SaveError::Cancelled);
                }
                // No event-loop turn separates the live ownership check,
                // exclusive same-directory publication and catalog completion.
                prepared.publish()?;
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .state
                    .as_mut()
                    .ok_or(SaveError::Cancelled)?
                    .finalize_session_download(&completion.id)
                    .map_err(|_| SaveError::Unavailable)
            });
            let state = match result {
                Ok(()) => "completed",
                Err(SaveError::Cancelled) => "cancelled",
                Err(error) => {
                    let failure = error.failure();
                    let message = failure.message().to_owned();
                    if let Some(state) = self.as_mut().rust_mut().as_mut().get_mut().state.as_mut()
                    {
                        let total = state
                            .session_download_rows(&[])
                            .ok()
                            .and_then(|rows| {
                                rows.into_iter().find(|row| row.record.id == completion.id)
                            })
                            .map_or(-1, |row| row.total_bytes);
                        let _ = state.note_session_download_metrics(
                            &completion.id,
                            total,
                            Some(failure),
                        );
                    }
                    self.as_mut().set_status_text(QString::from(message));
                    "interrupted"
                }
            };
            self.as_mut()
                .update_download(&id, &QString::from(state), 0, true);
        }
    }
}
