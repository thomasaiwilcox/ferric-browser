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
