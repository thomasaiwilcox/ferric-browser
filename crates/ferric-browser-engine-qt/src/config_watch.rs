//! Portable configuration-file watching with an inotify fast path on Linux.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Instant, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WatchSignature {
    exists: bool,
    is_directory: bool,
    length: u64,
    modified_ns: Option<u128>,
}

pub(super) struct ConfigWatch {
    #[cfg(target_os = "linux")]
    receiver: Option<std::sync::mpsc::Receiver<()>>,
    #[cfg(target_os = "linux")]
    stop: Option<Arc<AtomicBool>>,
    #[cfg(target_os = "linux")]
    thread: Option<thread::JoinHandle<()>>,
    watched_paths: Vec<PathBuf>,
    signatures: BTreeMap<PathBuf, WatchSignature>,
    pending_since: Option<Instant>,
}

fn watch_signature(path: &Path) -> WatchSignature {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return WatchSignature {
            exists: false,
            is_directory: false,
            length: 0,
            modified_ns: None,
        };
    };
    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos());
    WatchSignature {
        exists: true,
        is_directory: metadata.is_dir(),
        length: metadata.len(),
        modified_ns,
    }
}

impl ConfigWatch {
    #[cfg(not(target_os = "linux"))]
    fn new() -> Self {
        Self {
            watched_paths: Vec::new(),
            signatures: BTreeMap::new(),
            pending_since: None,
        }
    }

    #[cfg(target_os = "linux")]
    fn new() -> Self {
        Self {
            receiver: None,
            stop: None,
            thread: None,
            watched_paths: Vec::new(),
            signatures: BTreeMap::new(),
            pending_since: None,
        }
    }

    pub(super) fn set_sources(&mut self, root: &Path, sources: &[PathBuf]) {
        let mut paths = sources.to_vec();
        paths.push(root.to_owned());
        let source_paths = paths.clone();
        for source in &source_paths {
            if let Some(parent) = source.parent() {
                paths.push(parent.to_owned());
            }
        }
        paths.sort();
        paths.dedup();
        self.watched_paths = paths.clone();
        self.signatures = paths
            .iter()
            .map(|path| (path.clone(), watch_signature(path)))
            .collect();
        #[cfg(target_os = "linux")]
        self.start_inotify(&paths);
        self.pending_since = None;
    }

    #[cfg(target_os = "linux")]
    fn start_inotify(&mut self, paths: &[PathBuf]) {
        self.stop_inotify();
        let directories = paths
            .iter()
            .filter_map(|path| path.parent().map(Path::to_owned))
            .collect::<std::collections::BTreeSet<_>>();
        let Some((receiver, stop, thread)) = spawn_inotify_watcher(directories) else {
            return;
        };
        self.receiver = Some(receiver);
        self.stop = Some(stop);
        self.thread = Some(thread);
    }

    #[cfg(target_os = "linux")]
    pub(super) fn stop_inotify(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.receiver = None;
    }

    pub(super) fn changed(&mut self) -> bool {
        let polled = self.poll_signatures_changed();
        #[cfg(target_os = "linux")]
        {
            let notified = self
                .receiver
                .as_ref()
                .is_some_and(|receiver| receiver.try_iter().next().is_some());
            notified || polled
        }
        #[cfg(not(target_os = "linux"))]
        {
            polled
        }
    }

    pub(super) fn mark_pending(&mut self, now: Instant) {
        self.pending_since = Some(now);
    }

    #[must_use]
    pub(super) fn pending_since(&self) -> Option<Instant> {
        self.pending_since
    }

    pub(super) fn clear_pending(&mut self) {
        self.pending_since = None;
    }

    #[must_use]
    pub(super) fn is_watching(&self) -> bool {
        !self.watched_paths.is_empty()
    }

    fn poll_signatures_changed(&mut self) -> bool {
        let mut changed = false;
        for path in &self.watched_paths {
            let signature = watch_signature(path);
            if self.signatures.get(path).copied() != Some(signature) {
                changed = true;
                self.signatures.insert(path.clone(), signature);
            }
        }
        changed
    }
}

impl Default for ConfigWatch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "linux")]
impl Drop for ConfigWatch {
    fn drop(&mut self) {
        self.stop_inotify();
    }
}

#[cfg(target_os = "linux")]
fn spawn_inotify_watcher(
    directories: std::collections::BTreeSet<PathBuf>,
) -> Option<(
    std::sync::mpsc::Receiver<()>,
    Arc<AtomicBool>,
    thread::JoinHandle<()>,
)> {
    use std::os::unix::ffi::OsStrExt;

    let descriptor = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
    if descriptor < 0 {
        return None;
    }
    let mask = libc::IN_CLOSE_WRITE
        | libc::IN_MOVED_TO
        | libc::IN_MOVED_FROM
        | libc::IN_CREATE
        | libc::IN_DELETE
        | libc::IN_ATTRIB
        | libc::IN_MODIFY;
    let mut watched = false;
    for directory in directories {
        let Ok(path) = std::ffi::CString::new(directory.as_os_str().as_bytes()) else {
            continue;
        };
        let watch = unsafe { libc::inotify_add_watch(descriptor, path.as_ptr(), mask) };
        watched |= watch >= 0;
    }
    if !watched {
        unsafe {
            libc::close(descriptor);
        }
        return None;
    }

    let (sender, receiver) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let thread = thread::Builder::new()
        .name("ferric-browser-config-watch".into())
        .spawn(move || {
            let mut buffer = [0_u8; 8192];
            while !thread_stop.load(Ordering::Relaxed) {
                let mut pollfd = libc::pollfd {
                    fd: descriptor,
                    events: libc::POLLIN,
                    revents: 0,
                };
                let ready = unsafe { libc::poll(&raw mut pollfd, 1, 100) };
                if ready > 0 && pollfd.revents & libc::POLLIN != 0 {
                    let read =
                        unsafe { libc::read(descriptor, buffer.as_mut_ptr().cast(), buffer.len()) };
                    if read > 0 {
                        let _ = sender.send(());
                    }
                }
            }
            unsafe {
                libc::close(descriptor);
            }
        })
        .ok()?;
    Some((receiver, stop, thread))
}
