//! Lifecycle records and cleanup for browser-owned external editor processes.
//!
//! This is platform integration: it owns process groups, temporary artifacts,
//! and bounded stderr presentation. The Qt composition root retains the user
//! interaction and reducer dispatch decisions.

use crate::process_output::sanitize_process_stderr;
use ferric_browser_core::Target;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, ExitStatus},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const CHILD_TERMINATION_GRACE: Duration = Duration::from_millis(500);

#[derive(Clone, Debug)]
pub(super) struct PendingEditor {
    pub(super) token: String,
    pub(super) target: Target,
    pub(super) original: String,
    pub(super) file: PathBuf,
    pub(super) process: Option<Arc<Mutex<Option<Child>>>>,
    pub(super) cancellation: Arc<AtomicBool>,
    pub(super) private_temporary: bool,
    pub(super) issued: bool,
    pub(super) created_at: Instant,
}

pub(super) type ConfigEditStderr = Arc<Mutex<Option<Result<Vec<u8>, String>>>>;

#[derive(Clone, Debug)]
pub(super) struct PendingConfigEdit {
    pub(super) token: String,
    pub(super) path: PathBuf,
    pub(super) process: Arc<Mutex<Option<Child>>>,
    pub(super) exit_status: Option<ExitStatus>,
    pub(super) stderr: ConfigEditStderr,
}

impl Drop for PendingConfigEdit {
    fn drop(&mut self) {
        terminate_editor_process(&self.process);
    }
}

pub(super) fn terminate_child_process(child: &mut Child) {
    #[cfg(unix)]
    {
        if let Ok(process_group) = libc::pid_t::try_from(child.id())
            && process_group > 0
        {
            // SAFETY: a negative PID targets only the child process group that
            // Ferric created with `CommandExt::process_group(0)`.
            unsafe {
                let _ = libc::kill(-process_group, libc::SIGTERM);
            }
            let deadline = Instant::now() + CHILD_TERMINATION_GRACE;
            loop {
                let child_finished = match child.try_wait() {
                    Ok(Some(_)) => true,
                    Ok(None) | Err(_) => false,
                };
                // SAFETY: this is a non-mutating existence probe for the same
                // browser-owned process group described above.
                let group_exists = unsafe { libc::kill(-process_group, 0) == 0 };
                if child_finished && !group_exists {
                    return;
                }
                if !group_exists || Instant::now() >= deadline {
                    if group_exists {
                        // SAFETY: this is the forced termination fallback for
                        // the browser-owned group after its documented grace.
                        unsafe {
                            let _ = libc::kill(-process_group, libc::SIGKILL);
                        }
                    }
                    let _ = child.wait();
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

pub(super) fn terminate_editor_process(process: &Arc<Mutex<Option<Child>>>) {
    if let Ok(mut child) = process.lock() {
        if let Some(child) = child.as_mut() {
            terminate_child_process(child);
        }
        child.take();
    }
}

pub(super) fn discard_editor_request(request: PendingEditor) {
    request.cancellation.store(true, Ordering::Release);
    if let Some(process) = request.process.as_ref() {
        terminate_editor_process(process);
    }
    cleanup_editor_artifact(&request.file, request.private_temporary);
}

pub(super) fn cleanup_editor_artifact(path: &Path, private_temporary: bool) {
    let _ = fs::remove_file(path);
    if private_temporary && let Some(parent) = path.parent() {
        let _ = fs::remove_dir(parent);
    }
}

pub(super) fn discard_config_edit(request: PendingConfigEdit) {
    terminate_editor_process(&request.process);
}

pub(super) fn try_wait_editor_process(
    process: &Arc<Mutex<Option<Child>>>,
) -> Result<Option<ExitStatus>, String> {
    let mut child = process
        .lock()
        .map_err(|_| "editor process state is unavailable".to_owned())?;
    let Some(child_process) = child.as_mut() else {
        return Err("editor process state is unavailable".into());
    };
    let status = child_process
        .try_wait()
        .map_err(|error| format!("editor wait failed: {error}"))?;
    if status.is_some() {
        child.take();
    }
    Ok(status)
}

#[derive(Clone, Debug)]
pub(super) struct EditorCompletion {
    pub(super) token: String,
    pub(super) original: String,
    pub(super) updated: Option<String>,
    pub(super) error: Option<String>,
    pub(super) stderr: Option<String>,
}

pub(super) fn join_editor_stderr(
    reader: &mut Option<thread::JoinHandle<Result<Vec<u8>, String>>>,
) -> Option<String> {
    let result = reader.take()?.join().ok()?;
    match result {
        Ok(bytes) => {
            let text = sanitize_process_stderr(&bytes);
            (!text.is_empty()).then_some(text)
        }
        Err(error) => Some(error),
    }
}
