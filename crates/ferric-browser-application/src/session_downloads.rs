//! Live transfer state survives history backpressure and is scoped to a profile session.
use crate::{
    BrowserApplication, DownloadRow, DownloadSessionError,
    profile_session::{ProfileSession, SessionScope},
    storage::StorageObservation,
};
use ferric_browser_core::ShutdownState;
use ferric_browser_storage::{
    DownloadRecord, DownloadState, StorageCommand, StorageCompletion, StorageWorkerError,
    is_safe_history_url,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Saved history availability, independent of native transfer availability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DownloadHistoryStatus {
    NotStored,
    Ready,
    Saving,
    Unavailable,
}

impl BrowserApplication {
    /// Whether this owner may still write the selected private staging file.
    #[must_use]
    pub fn session_download_write_current(&self, id: &str) -> bool {
        self.download_session(true).is_ok_and(|session| {
            session
                .0
                .downloads
                .lock()
                .is_ok_and(|catalog| catalog.write_current(id, self.download_owner))
        })
    }

    /// Checks the live owner, transport completion and exact save destination.
    /// Run immediately before exclusive publication on the application thread.
    #[must_use]
    pub fn session_download_finalization_current(&self, id: &str, destination: &Path) -> bool {
        self.download_session(true).is_ok_and(|session| {
            session.0.downloads.lock().is_ok_and(|catalog| {
                catalog.finalization_current(id, self.download_owner, destination)
            })
        })
    }

    /// Captures bounded, session-only details independently of saved history.
    ///
    /// # Errors
    /// Rejects foreign/unavailable requests and invalid byte counts.
    pub fn note_session_download_metrics(
        &mut self,
        id: &str,
        total: i64,
        failure: Option<crate::DownloadFailure>,
    ) -> Result<(), DownloadSessionError> {
        let failure = failure.map(|failure| match failure {
            crate::DownloadFailure::Native { code, message } => crate::DownloadFailure::Native {
                code,
                message: message
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(2048)
                    .collect(),
            },
            crate::DownloadFailure::Save(kind) => crate::DownloadFailure::Save(kind),
        });
        self.download_session(false)?
            .0
            .downloads
            .lock()
            .map_err(|_| DownloadSessionError::Unavailable)?
            .metrics(id, self.download_owner, total, failure)
    }

    fn download_session(&self, accepting: bool) -> Result<&ProfileSession, DownloadSessionError> {
        if accepting
            && (self.profile_open.is_some()
                || self.runtime.view().shutdown() != ShutdownState::Running)
        {
            return Err(DownloadSessionError::ProfileUnavailable);
        }
        self.active_profile
            .as_ref()
            .map(|profile| &profile.session)
            .ok_or(DownloadSessionError::ProfileUnavailable)
    }

    /// Whether this window can admit download actions, independent of saved history.
    #[must_use]
    pub fn session_downloads_available(&self) -> bool {
        self.download_session(true)
            .is_ok_and(|session| session.0.downloads.lock().is_ok())
    }

    /// Records an offer before destination selection, without waiting for history.
    ///
    /// # Errors
    /// Rejects unavailable profiles, duplicate IDs, and a full or unavailable catalog.
    pub fn offer_session_download(
        &mut self,
        id: Uuid,
        source: &str,
        name: &str,
        at: i64,
    ) -> Result<(), DownloadSessionError> {
        let source_url = if source.len() <= 16 * 1024 && is_safe_history_url(source) {
            source.to_owned()
        } else {
            "[redacted]".to_owned()
        };
        let name = ferric_browser_storage::sanitize_download_filename(name);
        self.download_session(true)?
            .0
            .downloads
            .lock()
            .map_err(|_| DownloadSessionError::Unavailable)?
            .offer(
                DownloadRecord {
                    id: id.to_string(),
                    source_url,
                    destination: String::new(),
                    state: DownloadState::Offered,
                    bytes_received: 0,
                    created_at: at,
                    completed_at: None,
                },
                name,
                self.download_owner,
            )?;
        self.flush_session_downloads();
        Ok(())
    }

    /// Records the selected final destination. History failures cannot reject a transfer.
    ///
    /// # Errors
    /// Rejects unavailable profiles, foreign requests and unsafe destinations.
    pub fn set_session_download_destination(
        &mut self,
        id: &str,
        destination: &Path,
    ) -> Result<(), DownloadSessionError> {
        let path = destination
            .to_str()
            .filter(|value| value.len() <= 16 * 1024 && !value.chars().any(char::is_control))
            .ok_or(DownloadSessionError::InvalidDestination)?;
        if !destination.is_absolute() {
            return Err(DownloadSessionError::InvalidDestination);
        }
        let name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or(DownloadSessionError::InvalidDestination)?;
        self.download_session(true)?
            .0
            .downloads
            .lock()
            .map_err(|_| DownloadSessionError::Unavailable)?
            .destination(id, self.download_owner, path.to_owned(), name.to_owned())?;
        self.flush_session_downloads();
        Ok(())
    }

    /// Publishes successful filesystem finalization before a completed row permits Open.
    ///
    /// # Errors
    /// Rejects unknown, foreign or unavailable requests.
    pub fn finalize_session_download(&mut self, id: &str) -> Result<(), DownloadSessionError> {
        self.download_session(false)?
            .0
            .downloads
            .lock()
            .map_err(|_| DownloadSessionError::Unavailable)?
            .finalized(id, self.download_owner)
    }

    /// Accepts captured-owner native progress, including cancellation while closing.
    ///
    /// # Errors
    /// Rejects foreign/unknown requests, invalid progress and premature file completion.
    pub fn update_session_download(
        &mut self,
        id: &str,
        state: DownloadState,
        bytes: i64,
        finished: bool,
        completed_at: Option<i64>,
    ) -> Result<(), DownloadSessionError> {
        self.download_session(false)?
            .0
            .downloads
            .lock()
            .map_err(|_| DownloadSessionError::Unavailable)?
            .update(
                id,
                self.download_owner,
                state,
                bytes,
                finished,
                completed_at,
            )?;
        self.flush_session_downloads();
        Ok(())
    }

    /// Merges available saved history with authoritative session rows. Private sessions
    /// ignore saved input entirely; no UUID, label or string scope supplied by UI grants access.
    ///
    /// # Errors
    /// Returns an error when the profile session or its catalog is unavailable.
    pub fn session_download_rows(
        &self,
        saved: &[DownloadRecord],
    ) -> Result<Vec<DownloadRow>, DownloadSessionError> {
        Ok(self
            .download_session(false)?
            .0
            .downloads
            .lock()
            .map_err(|_| DownloadSessionError::Unavailable)?
            .rows(saved))
    }

    #[must_use]
    pub fn download_history_status(&self) -> DownloadHistoryStatus {
        let Ok(session) = self.download_session(false) else {
            return DownloadHistoryStatus::Unavailable;
        };
        if !matches!(session.0.scope, SessionScope::Durable { .. }) {
            return DownloadHistoryStatus::NotStored;
        }
        let Ok(catalog) = session.0.downloads.lock() else {
            return DownloadHistoryStatus::Unavailable;
        };
        if catalog.history_failed() {
            DownloadHistoryStatus::Unavailable
        } else if !catalog.has_unsaved() {
            DownloadHistoryStatus::Ready
        } else {
            DownloadHistoryStatus::Saving
        }
    }

    /// Explicitly retries a failed history projection. Native controls remain available.
    pub fn retry_download_history(&mut self) {
        if let Ok(session) = self.download_session(false)
            && let Ok(mut catalog) = session.0.downloads.lock()
        {
            catalog.retry_history();
        }
        self.flush_session_downloads();
    }

    pub(crate) fn flush_session_downloads(&mut self) {
        let Ok(session) = self.download_session(false).cloned() else {
            return;
        };
        let records = match session.0.downloads.lock() {
            Ok(mut catalog) => catalog.begin_write(),
            Err(_) => return,
        };
        if records.is_empty() {
            return;
        }
        let request = StorageCommand::DownloadSave {
            records: records.iter().map(|(_, record)| record.clone()).collect(),
        };
        let work = DownloadWork {
            session: session.clone(),
            records: Some(records),
        };
        if let Err(error) = self
            .storage
            .submit(request, Some(StorageObservation::Download(work)))
            && !matches!(
                error,
                StorageWorkerError::Busy | StorageWorkerError::QueueFull
            )
            && let Ok(mut catalog) = session.0.downloads.lock()
        {
            catalog.finish_write(&[], false);
        }
    }

    pub(crate) fn close_session_storage(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(2);
        self.drain_session_downloads(deadline);
        self.storage.close_until(deadline);
    }

    pub(crate) fn drain_session_downloads(&mut self, deadline: Instant) {
        loop {
            self.storage.collect_for_drain();
            self.flush_session_downloads();
            if self.download_history_status() != DownloadHistoryStatus::Saving
                || Instant::now() >= deadline
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

/// Completion accounting retains the captured session even across profile replacement.
/// Dropping work that was never admitted releases its reservation without acknowledging it.
pub(crate) struct DownloadWork {
    session: ProfileSession,
    records: Option<Vec<(u64, DownloadRecord)>>,
}
impl DownloadWork {
    pub(crate) fn finish(mut self, completion: &StorageCompletion) {
        if let Some(records) = self.records.take()
            && let Ok(mut catalog) = self.session.0.downloads.lock()
        {
            let success = matches!(completion, StorageCompletion::DownloadSave(Ok(count)) if *count == records.len());
            catalog.finish_write(&records, success);
        }
    }
}
impl Drop for DownloadWork {
    fn drop(&mut self) {
        if self.records.is_some()
            && let Ok(mut catalog) = self.session.0.downloads.lock()
        {
            catalog.release_write();
        }
    }
}

#[cfg(test)]
#[path = "session_downloads_tests.rs"]
mod tests;
