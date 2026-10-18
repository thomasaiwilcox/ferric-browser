//! Admission and bounded byte delivery for browser-owned fetch controllers.
use super::download_stream_writer::{Phase, StreamDownloadWriter};
use super::{CxxQtType, Pin, QString, Threading, qobject};
use cxx_qt_lib::QByteArray;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

impl qobject::BrowserUi {
    pub(super) fn set_download_controller_available(mut self: Pin<&mut Self>, available: bool) {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .download_controller_available = available;
    }
    pub(super) fn stream_download_available(&self) -> bool {
        self.rust().download_controller_available
            && self
                .rust()
                .download_stream_writer
                .as_ref()
                .is_none_or(StreamDownloadWriter::available)
    }
    // QML invokables require their QObject receiver.
    #[allow(clippy::unused_self)]
    pub(super) fn canonical_download_origin(&self, url: &QString) -> QString {
        super::download_stream_policy::origin(&url.to_string())
            .map_or_else(QString::default, QString::from)
    }
    #[allow(clippy::unused_self)]
    pub(super) fn download_stream_name(
        &self,
        url: &QString,
        disposition: &QString,
        mime: &QString,
    ) -> QString {
        QString::from(super::download_stream_policy::suggested_name(
            &url.to_string(),
            &disposition.to_string(),
            &mime.to_string(),
        ))
    }
    pub(super) fn admit_download_stream_target(
        mut self: Pin<&mut Self>,
        token: &QString,
        url: &QString,
    ) -> bool {
        if self.as_ref().get_ref().download_request_source_index(token) < 0 {
            return false;
        }
        let Some(url) = super::download_stream_policy::address(&url.to_string()) else {
            self.set_status_text(QString::from("Use an HTTP or HTTPS download link without embedded sign-in details. Sign in on the page, then retry."));
            return false;
        };
        let mut rust = self.as_mut().rust_mut();
        let request = rust
            .as_mut()
            .get_mut()
            .pending_download
            .as_mut()
            .expect("current issued request");
        let downgrade = request
            .last_url
            .as_ref()
            .is_some_and(|previous| previous.starts_with("https://") && url.starts_with("http://"));
        let looped = request.visited.len() >= 8 || request.visited.contains(&url);
        if downgrade || looped {
            drop(rust);
            self.set_status_text(QString::from(if downgrade {
                "This download redirects to an insecure connection. Open the link to review it before downloading."
            } else { "This download keeps redirecting. Open the link to check its destination, then retry." }));
            return false;
        }
        request.visited.insert(url.clone());
        request.last_url = Some(url);
        true
    }
    pub(super) fn reserve_stream_download(mut self: Pin<&mut Self>, id: &QString) -> bool {
        if !self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .is_some_and(|state| state.owns_session_download_offer(&id.to_string()))
        {
            return false;
        }
        if self.as_ref().rust().download_stream_writer.is_none() {
            let qt_thread = self.as_ref().get_ref().qt_thread();
            let queued = Arc::new(AtomicBool::new(false));
            let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
                if queued.swap(true, Ordering::AcqRel) {
                    return;
                }
                let delivered = Arc::clone(&queued);
                if qt_thread
                    .queue(move |mut ui| {
                        delivered.store(false, Ordering::Release);
                        ui.as_mut().poll_stream_downloads();
                    })
                    .is_err()
                {
                    queued.store(false, Ordering::Release);
                }
            });
            let Ok(writer) = StreamDownloadWriter::spawn(wake) else {
                return false;
            };
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .download_stream_writer = Some(writer);
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .download_stream_writer
            .as_mut()
            .is_some_and(|writer| writer.reserve(id.to_string()))
    }
    pub(super) fn begin_stream_download(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let id = id.to_string();
        if !self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .is_some_and(|state| state.session_download_write_current(&id))
        {
            return false;
        }
        let Some(path) = self
            .as_ref()
            .rust()
            .staged_downloads
            .get(&id)
            .map(|staged| staged.staging_path().to_owned())
        else {
            return false;
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .download_stream_writer
            .as_mut()
            .is_some_and(|writer| writer.begin(&id, path))
    }
    pub(super) fn write_stream_download(
        mut self: Pin<&mut Self>,
        id: &QString,
        bytes: &QByteArray,
    ) -> bool {
        let id = id.to_string();
        if !self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .is_some_and(|state| state.session_download_write_current(&id))
        {
            return false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .download_stream_writer
            .as_mut()
            .is_some_and(|writer| writer.write(&id, bytes.as_slice()))
    }
    pub(super) fn finish_stream_download(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let id = id.to_string();
        if !self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .is_some_and(|state| state.session_download_write_current(&id))
        {
            return false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .download_stream_writer
            .as_mut()
            .is_some_and(|writer| writer.finish(&id))
    }
    pub(super) fn cancel_stream_download(mut self: Pin<&mut Self>, id: &QString) {
        if let Some(writer) = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .download_stream_writer
            .as_mut()
        {
            writer.cancel(&id.to_string());
        }
    }
    pub(super) fn release_stream_download(mut self: Pin<&mut Self>, id: &QString) {
        if let Some(writer) = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .download_stream_writer
            .as_mut()
        {
            writer.release(&id.to_string());
        }
    }
    pub(super) fn stream_download_state(mut self: Pin<&mut Self>, id: &QString) -> i32 {
        self.as_mut().poll_stream_downloads();
        self.as_ref()
            .rust()
            .download_stream_writer
            .as_ref()
            .map_or(Phase::Cancelled, |writer| writer.phase(&id.to_string())) as i32
    }
    pub(super) fn stream_download_bytes(&self, id: &QString) -> i64 {
        self.rust()
            .download_stream_writer
            .as_ref()
            .map_or(0, |writer| writer.bytes(&id.to_string()))
    }
    pub(super) fn stream_download_error(&self, id: &QString) -> QString {
        QString::from(self.rust().download_stream_writer.as_ref().and_then(|writer| writer.error(&id.to_string()))
            .map_or("The download stopped unexpectedly. Retry it from Downloads.", |kind| match kind {
                std::io::ErrorKind::StorageFull => "There isn't enough space to save this file. Free some space or choose another folder, then retry.",
                std::io::ErrorKind::PermissionDenied => "The save folder isn't writable. Choose another folder and retry the download.",
                _ => "The downloaded file couldn't be saved. Check the save folder and retry.",
            }))
    }
    pub(super) fn poll_stream_downloads(mut self: Pin<&mut Self>) {
        let changed = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .download_stream_writer
            .as_mut()
            .map_or_else(Vec::new, StreamDownloadWriter::poll);
        for id in changed {
            let failure = self
                .as_ref()
                .rust()
                .download_stream_writer
                .as_ref()
                .and_then(|writer| writer.error(&id));
            if let Some(kind) = failure {
                use ferric_browser_application::{DownloadFailure, DownloadSaveFailure};
                let failure = DownloadFailure::Save(match kind {
                    std::io::ErrorKind::StorageFull => DownloadSaveFailure::StorageFull,
                    std::io::ErrorKind::PermissionDenied => DownloadSaveFailure::PermissionDenied,
                    _ => DownloadSaveFailure::Unavailable,
                });
                if let Some(state) = self.as_mut().rust_mut().as_mut().get_mut().state.as_mut() {
                    let total = state
                        .session_download_rows(&[])
                        .ok()
                        .and_then(|rows| rows.into_iter().find(|row| row.record.id == id))
                        .map_or(-1, |row| row.total_bytes);
                    let _ = state.note_session_download_metrics(&id, total, Some(failure));
                }
            }
            self.as_mut().stream_download_ready(QString::from(id));
        }
    }
}
