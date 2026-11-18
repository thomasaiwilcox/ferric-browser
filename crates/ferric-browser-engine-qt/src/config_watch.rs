//! Portable configuration-file watching with an inotify fast path on Linux.

use std::{
    collections::{BTreeMap, BTreeSet},
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
    sources: BTreeSet<PathBuf>,
    signatures: BTreeMap<PathBuf, WatchSignature>,
    pending_since: Option<Instant>,
}

fn watch_signature(path: &Path, source: bool) -> WatchSignature {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return WatchSignature {
            exists: false,
            is_directory: false,
            length: 0,
            modified_ns: None,
        };
    };
    // Parent watches detect source replacement/disappearance. Their unrelated
    // directory entries must not turn an explicit export into a config reload.
    if metadata.is_dir() && !source {
        return WatchSignature {
            exists: true,
            is_directory: true,
            length: 0,
            modified_ns: None,
        };
    }
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
            sources: BTreeSet::new(),
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
            sources: BTreeSet::new(),
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
        self.sources = source_paths.into_iter().collect();
        self.watched_paths = paths.clone();
        self.signatures = paths
            .iter()
            .map(|path| {
                (
                    path.clone(),
                    watch_signature(path, self.sources.contains(path)),
                )
            })
            .collect();
        #[cfg(target_os = "linux")]
        self.start_inotify(&paths);
        self.pending_since = None;
    }

    /// Rebind a completed reload without acknowledging edits observed while
    /// it was pending, including edits during the watcher handoff itself.
    pub(super) fn refresh_sources(&mut self, root: &Path, sources: &[PathBuf]) {
        let pending = self.pending_since;
        let signatures = std::mem::take(&mut self.signatures);
        self.set_sources(root, sources);
        for (path, signature) in signatures {
            if let Some(current) = self.signatures.get_mut(&path) {
                *current = signature;
            }
        }
        self.pending_since = pending;
    }

    #[cfg(target_os = "linux")]
    fn start_inotify(&mut self, paths: &[PathBuf]) {
        self.stop_inotify();
        let directories = paths
            .iter()
            .filter_map(|path| path.parent().map(Path::to_owned))
            .collect::<std::collections::BTreeSet<_>>();
        let source_directories = self
            .sources
            .iter()
            .filter(|path| {
                self.signatures
                    .get(*path)
                    .is_some_and(|signature| signature.is_directory)
            })
            .cloned()
            .collect();
        let Some((receiver, stop, thread)) = spawn_inotify_watcher(
            directories,
            paths.iter().cloned().collect(),
            source_directories,
        ) else {
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
            let signature = watch_signature(path, self.sources.contains(path));
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
#[allow(unsafe_code)]
fn spawn_inotify_watcher(
    directories: std::collections::BTreeSet<PathBuf>,
    targets: BTreeSet<PathBuf>,
    source_directories: BTreeSet<PathBuf>,
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
        | libc::IN_MODIFY
        | libc::IN_MOVE_SELF
        | libc::IN_DELETE_SELF;
    let mut watched: BTreeMap<i32, Vec<PathBuf>> = BTreeMap::new();
    for directory in directories {
        let Ok(path) = std::ffi::CString::new(directory.as_os_str().as_bytes()) else {
            continue;
        };
        let watch = unsafe { libc::inotify_add_watch(descriptor, path.as_ptr(), mask) };
        if watch >= 0 {
            watched.entry(watch).or_default().push(directory);
        }
    }
    if watched.is_empty() {
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
                    if read > 0
                        && usize::try_from(read).ok().is_none_or(|count| {
                            buffer.get(..count).is_none_or(|bytes| {
                                inotify_events_relevant(
                                    bytes,
                                    &watched,
                                    &targets,
                                    &source_directories,
                                )
                            })
                        })
                    {
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

#[cfg(target_os = "linux")]
fn inotify_events_relevant(
    mut bytes: &[u8],
    watched: &BTreeMap<i32, Vec<PathBuf>>,
    targets: &BTreeSet<PathBuf>,
    source_directories: &BTreeSet<PathBuf>,
) -> bool {
    use std::os::unix::ffi::OsStrExt;
    while !bytes.is_empty() {
        // Linux inotify headers are four native-endian 32-bit words followed
        // by a bounded, NUL-padded filename. Parse slices without pointer casts.
        if bytes.len() < 16 {
            return true;
        }
        let descriptor = i32::from_ne_bytes(bytes[..4].try_into().expect("checked header"));
        let mask = u32::from_ne_bytes(bytes[4..8].try_into().expect("checked header"));
        let length = u32::from_ne_bytes(bytes[12..16].try_into().expect("checked header"));
        let Some(end) = usize::try_from(length)
            .ok()
            .and_then(|length| length.checked_add(16))
        else {
            return true;
        };
        if end > bytes.len() || mask & libc::IN_Q_OVERFLOW != 0 {
            return true;
        }
        let Some(paths) = watched.get(&descriptor) else {
            return true;
        };
        let name = &bytes[16..end];
        let name = &name[..name
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(name.len())];
        if name.is_empty() {
            return true;
        }
        let name = std::ffi::OsStr::from_bytes(name);
        if paths
            .iter()
            .any(|path| source_directories.contains(path) || targets.contains(&path.join(name)))
        {
            return true;
        }
        bytes = &bytes[end..];
    }
    false
}

#[cfg(test)]
mod reload_tests {
    use super::*;

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new(name: &str) -> Self {
            let stamp = std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "ferric-watch-{name}-{}-{stamp}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn reload_retains_a_debounced_edit() {
        let directory = TestDirectory::new("pending");
        let path = directory.0.join("config.toml");
        fs::write(&path, "before").unwrap();
        let mut watch = ConfigWatch::default();
        watch.set_sources(&path, &[]);
        let pending = Instant::now();
        watch.mark_pending(pending);
        watch.refresh_sources(&path, &[]);
        assert_eq!(watch.pending_since(), Some(pending));
    }

    #[test]
    fn reload_does_not_acknowledge_a_change_during_handoff() {
        let directory = TestDirectory::new("handoff");
        let path = directory.0.join("config.toml");
        fs::write(&path, "before").unwrap();
        let mut watch = ConfigWatch::default();
        watch.set_sources(&path, &[]);
        fs::write(&path, "after with a different length").unwrap();
        watch.refresh_sources(&path, &[]);
        assert!(watch.changed());
    }

    #[test]
    fn changing_profile_still_resets_old_pending_work() {
        let directory = TestDirectory::new("profile");
        let before = directory.0.join("before.toml");
        let after = directory.0.join("after.toml");
        fs::write(&before, "before").unwrap();
        fs::write(&after, "after").unwrap();
        let mut watch = ConfigWatch::default();
        watch.set_sources(&before, &[]);
        watch.mark_pending(Instant::now());
        watch.set_sources(&after, &[]);
        assert_eq!(watch.pending_since(), None);
    }
}
