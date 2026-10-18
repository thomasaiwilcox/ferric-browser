//! A retry reservation follows its validated profile session, including owner retirement.

use crate::{BrowserApplication, DownloadSessionError, profile_session::ProfileSession};
use uuid::Uuid;

/// An unbound retry is released when its request is cancelled or its owner retires.
/// After admission, the retry remains reserved until its actual transport ends.
pub struct DownloadRetryPermit {
    session: ProfileSession,
    source: String,
    token: Uuid,
    owner: Uuid,
}

impl std::fmt::Debug for DownloadRetryPermit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DownloadRetryPermit")
            .finish_non_exhaustive()
    }
}

impl DownloadRetryPermit {
    /// Binds this reservation to an actual offer owned by the requesting application.
    ///
    /// # Errors
    /// Rejects missing, foreign, reused or unavailable offers.
    pub fn bind(&mut self, download: &str) -> Result<(), DownloadSessionError> {
        self.session
            .0
            .downloads
            .lock()
            .map_err(|_| DownloadSessionError::Unavailable)?
            .bind_retry(&self.source, self.token, self.owner, download)
    }
}

impl Drop for DownloadRetryPermit {
    fn drop(&mut self) {
        if let Ok(mut catalog) = self.session.0.downloads.lock() {
            catalog.release_retry(&self.source, self.token);
        }
    }
}

impl BrowserApplication {
    /// Reserves one retry of a source resolved from this profile's Downloads projection.
    /// Saved history may supply the source; only a new owned native offer can bind it.
    ///
    /// # Errors
    /// Rejects unavailable profiles, duplicate pending retries and exhausted capacity.
    pub fn reserve_session_download_retry(
        &self,
        source: &str,
    ) -> Result<DownloadRetryPermit, DownloadSessionError> {
        if !self.session_downloads_available() {
            return Err(DownloadSessionError::ProfileUnavailable);
        }
        let session = self
            .active_profile
            .as_ref()
            .ok_or(DownloadSessionError::ProfileUnavailable)?
            .session
            .clone();
        let token = Uuid::new_v4();
        session
            .0
            .downloads
            .lock()
            .map_err(|_| DownloadSessionError::Unavailable)?
            .reserve_retry(source, token, self.download_owner)?;
        Ok(DownloadRetryPermit {
            session,
            source: source.to_owned(),
            token,
            owner: self.download_owner,
        })
    }

    /// Whether this application owns an actual offer awaiting its native decision.
    #[must_use]
    pub fn owns_session_download_offer(&self, download: &str) -> bool {
        self.active_profile.as_ref().is_some_and(|profile| {
            profile
                .session
                .0
                .downloads
                .lock()
                .is_ok_and(|catalog| catalog.owns_offer(download, self.download_owner))
        })
    }
}
