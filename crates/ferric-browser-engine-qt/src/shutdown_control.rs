//! `QObject` adapter for shutdown and restart control.

use std::{
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant},
};

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use ferric_browser_core::Event;

use crate::{mark_forced_shutdown, qobject};

impl qobject::BrowserUi {
    pub(super) fn versioned_archive_shutdown_ready(self: Pin<&mut Self>) -> bool {
        self.rust().archive_view_ready_for_shutdown()
    }

    pub(super) fn request_shutdown(mut self: Pin<&mut Self>) -> bool {
        match self.as_mut().reduce_event(Event::RequestShutdown) {
            Ok(_) => {
                self.set_status_text(QString::from("Shutdown requested"));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Shutdown rejected: {error}")));
                false
            }
        }
    }

    pub(super) fn begin_shutdown_gate(mut self: Pin<&mut Self>) -> bool {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if this.ipc_shutdown_gate {
            return true;
        }
        this.ipc_shutdown_gate = true;
        self.set_status_text(QString::from("Shutdown gate active"));
        true
    }

    pub(super) fn end_shutdown_gate(mut self: Pin<&mut Self>) {
        let mut rust = self.as_mut().rust_mut();
        rust.as_mut().get_mut().ipc_shutdown_gate = false;
    }

    pub(super) fn force_quit(self: Pin<&mut Self>) {
        mark_forced_shutdown();
        self.set_status_text(QString::from(
            "Forced quit requested; crash marker preserved",
        ));
    }

    // This signature is the registered QML invocation contract; grouping it
    // would change the generated QObject boundary.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn restart_software_rendering(
        self: Pin<&mut Self>,
        lock_path: &QString,
        storage_base: &QString,
        temporary_profile: bool,
        safe_mode: bool,
        userscripts_off: bool,
        instance_selector: &QString,
    ) -> bool {
        let _ = self;
        let lock_path = lock_path.to_string();
        if lock_path.is_empty() {
            return false;
        }
        let Ok(executable) = std::env::current_exe() else {
            return false;
        };
        let mut arguments = vec!["--software-rendering".to_owned()];
        if safe_mode {
            arguments.push("--safe-mode".to_owned());
        } else if userscripts_off {
            arguments.push("--userscripts-off".to_owned());
        } else if temporary_profile {
            arguments.push("--temp-basedir".to_owned());
        } else if !storage_base.is_empty() {
            arguments.extend(["--basedir".to_owned(), storage_base.to_string()]);
        }
        if !instance_selector.is_empty() {
            arguments.extend(["--instance".to_owned(), instance_selector.to_string()]);
        }
        thread::Builder::new()
            .name("ferric-browser-software-restart".into())
            .spawn(move || {
                let lock_path = PathBuf::from(lock_path);
                let deadline = Instant::now() + Duration::from_secs(5);
                while lock_path.exists() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(25));
                }
                if !lock_path.exists() {
                    let _ = Command::new(executable).args(arguments).spawn();
                }
            })
            .is_ok()
    }
}

impl crate::BrowserUiRust {
    fn archive_view_ready_for_shutdown(&self) -> bool {
        self.pending_archive_view_request.is_none()
            && self.pending_archive_view_cleanup.is_none()
            && self.versioning_archive_view_url.to_string().is_empty()
    }
}

#[cfg(test)]
mod archive_shutdown_tests {
    use crate::{BrowserUiRust, QString};
    use ferric_browser_core::OperationId;

    #[test]
    fn shutdown_waits_for_archive_preparation_presentation_and_cleanup() {
        let mut ui = BrowserUiRust::default();
        assert!(ui.archive_view_ready_for_shutdown());

        ui.pending_archive_view_request = Some(1);
        assert!(!ui.archive_view_ready_for_shutdown());
        ui.pending_archive_view_request = None;

        ui.versioning_archive_view_url = QString::from("file:///private/views/capture.mhtml");
        assert!(!ui.archive_view_ready_for_shutdown());
        ui.versioning_archive_view_url = QString::default();

        ui.pending_archive_view_cleanup = Some(2);
        assert!(!ui.archive_view_ready_for_shutdown());
        ui.pending_archive_view_cleanup = None;
        // A failed exact cleanup can leave private bytes for restart
        // quarantine after the viewer has been blanked and its URL cleared.
        ui.active_archive_view_operation = Some(OperationId::fresh());
        assert!(ui.archive_view_ready_for_shutdown());
    }
}
