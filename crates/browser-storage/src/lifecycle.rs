use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::StorageRoots;

const MARKER_NAME: &str = "running";
const CRASH_LOG_NAME: &str = "crash-events.json";
const CRASH_LOG_SCHEMA: u32 = 1;
const MAX_CRASH_EVENTS: usize = 32;

#[derive(Debug)]
pub enum CrashMarkerError {
    Io { path: PathBuf, message: String },
}

impl std::fmt::Display for CrashMarkerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => write!(formatter, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for CrashMarkerError {}

/// A process-lifetime marker used to distinguish a clean exit from a process
/// that disappeared before it could finalize its session checkpoint.
#[derive(Debug)]
pub struct CrashMarker {
    path: PathBuf,
    previous_unclean: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CrashLog {
    schema: u32,
    events: Vec<CrashEvent>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CrashEvent {
    event: String,
    process: String,
    pid: u32,
    timestamp: u64,
    previous_unclean: bool,
}

impl Default for CrashLog {
    fn default() -> Self {
        Self {
            schema: CRASH_LOG_SCHEMA,
            events: Vec::new(),
        }
    }
}

impl CrashMarker {
    /// Creates the marker and reports whether a prior marker was left behind.
    /// Marker contents contain no browsing data.
    ///
    /// # Errors
    ///
    /// Returns an error if the state root cannot be prepared or the marker
    /// cannot be written and synced.
    pub fn begin(roots: &StorageRoots) -> Result<Self, CrashMarkerError> {
        roots
            .ensure()
            .map_err(|error| io_error(&roots.state, error))?;
        let path = roots.state.join(MARKER_NAME);
        let previous_unclean = path.exists();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let temp_path = path.with_extension(format!("tmp-{}-{nonce}", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temp_path)
            .map_err(|error| io_error(&temp_path, error))?;
        let contents = format!("pid={}\ntime={timestamp}\n", std::process::id());
        if let Err(error) = file
            .write_all(contents.as_bytes())
            .and_then(|()| file.sync_all())
            .and_then(|()| fs::rename(&temp_path, &path))
        {
            let _ = fs::remove_file(&temp_path);
            return Err(io_error(&path, error));
        }
        sync_parent(&path)?;
        let _ = append_event(
            &path,
            CrashEvent {
                event: "startup".into(),
                process: "browser".into(),
                pid: std::process::id(),
                timestamp,
                previous_unclean,
            },
        );
        Ok(Self {
            path,
            previous_unclean,
        })
    }

    #[must_use]
    pub fn previous_unclean(&self) -> bool {
        self.previous_unclean
    }

    /// Removes the marker and fsyncs its containing directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the marker cannot be removed or its parent cannot
    /// be synced.
    pub fn finish(self) -> Result<(), CrashMarkerError> {
        fs::remove_file(&self.path).map_err(|error| io_error(&self.path, error))?;
        sync_parent(&self.path)?;
        let timestamp = unix_timestamp();
        let _ = append_event(
            &self.path,
            CrashEvent {
                event: "clean-shutdown".into(),
                process: "browser".into(),
                pid: std::process::id(),
                timestamp,
                previous_unclean: false,
            },
        );
        Ok(())
    }
}

/// Returns a privacy-safe snapshot of the process marker and recent lifecycle
/// events. The snapshot contains no browsing URLs, filesystem paths, page
/// content, account names, cookies, or credentials.
#[must_use]
pub fn crash_diagnostics(roots: &StorageRoots) -> Value {
    let marker_path = roots.state.join(MARKER_NAME);
    let marker_present = marker_path.is_file();
    let log_path = roots.state.join(CRASH_LOG_NAME);
    let log = load_log(&log_path);

    let structured_log = match log {
        Ok(log) => json!({
            "status": "available",
            "schema": log.schema,
            "events": log.events,
            "limit": MAX_CRASH_EVENTS,
            "provenance": "profile-local",
        }),
        Err(_) => json!({
            "status": "unavailable",
            "reason": "the profile-local structured crash log could not be read",
            "provenance": "profile-local",
        }),
    };

    json!({
        "schema": CRASH_LOG_SCHEMA,
        "marker": {
            "status": if marker_present { "unclean" } else { "clear" },
            "present": marker_present,
            "reason": if marker_present {
                "the previous browser process did not finalize its marker"
            } else {
                "no unclean browser marker is present"
            },
            "provenance": "profile-local",
        },
        "structured_log": structured_log,
        "core_dumps": {
            "status": "excluded",
            "value": "not-collected",
            "reason": "core dumps can contain page and credential memory; RustBrowser never collects or exports them automatically",
            "provenance": "policy",
        },
        "debug_symbols": {
            "status": "not-tested",
            "reason": "symbol availability depends on the release packaging channel",
            "provenance": "not-probed",
        },
        "process_correlation": {
            "browser": "lifecycle marker and sanitized events are available",
            "qt_helper": "not-claimed",
            "gpu_driver": "not-claimed",
            "reason": "RustBrowser does not infer that a Rust marker prevents native Qt, helper, or GPU-driver failures",
        },
    })
}

/// Performs a bounded, privacy-safe scan for a caller-supplied transient
/// marker. Only entry names are inspected; paths, file contents, and user
/// output files are never returned. This is intended for release verification
/// and diagnostics, not for deleting or repairing state.
#[must_use]
pub fn transient_marker_scan(roots: &StorageRoots, marker: &str) -> Value {
    const SCHEMA: u32 = 1;
    const MAX_DEPTH: usize = 8;
    const MAX_ENTRIES: usize = 4096;

    let locations = [
        ("config", roots.config.as_path()),
        ("data", roots.data.as_path()),
        ("state", roots.state.as_path()),
        ("cache", roots.cache.as_path()),
        ("runtime", roots.runtime.as_path()),
    ];
    let mut scanned_entries = 0usize;
    let mut matching_entries = 0usize;
    let mut read_errors = 0usize;
    if !marker.is_empty() {
        for (_, path) in locations {
            scan_marker_entries(
                path,
                marker,
                0,
                MAX_DEPTH,
                MAX_ENTRIES,
                &mut scanned_entries,
                &mut matching_entries,
                &mut read_errors,
            );
        }
        if let Some(path) = roots.temporary_root() {
            scan_marker_entries(
                path,
                marker,
                0,
                MAX_DEPTH,
                MAX_ENTRIES,
                &mut scanned_entries,
                &mut matching_entries,
                &mut read_errors,
            );
        }
    }
    let marker_path = roots.state.join(MARKER_NAME);
    let marker_present = marker_path.is_file();
    let status = if read_errors > 0 {
        "incomplete"
    } else if matching_entries > 0 {
        "matches-found"
    } else {
        "clean"
    };
    json!({
        "schema": SCHEMA,
        "status": status,
        "marker": if marker_present { "unclean" } else { "clear" },
        "scanned_locations": ["config", "data", "state", "cache", "runtime", "temporary"],
        "scanned_entries": scanned_entries,
        "matching_entries": matching_entries,
        "read_errors": read_errors,
        "marker_policy": "entry names only; no paths or file contents are exported",
        "external_outputs": "explicit downloads and clipboard copies are user-owned and excluded",
        "outside_guarantee": [
            "engine and OS swap",
            "core dumps",
            "downloaded files",
            "clipboard managers",
            "sites and network observers"
        ]
    })
}

#[allow(clippy::too_many_arguments)]
fn scan_marker_entries(
    path: &Path,
    marker: &str,
    depth: usize,
    max_depth: usize,
    max_entries: usize,
    scanned_entries: &mut usize,
    matching_entries: &mut usize,
    read_errors: &mut usize,
) {
    if depth > max_depth || *scanned_entries >= max_entries {
        return;
    }
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return,
        Err(_) => {
            *read_errors += 1;
            return;
        }
    };
    for entry in entries {
        if *scanned_entries >= max_entries {
            break;
        }
        let Ok(entry) = entry else {
            *read_errors += 1;
            continue;
        };
        *scanned_entries += 1;
        if entry.file_name().to_string_lossy().contains(marker) {
            *matching_entries += 1;
        }
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            scan_marker_entries(
                &entry.path(),
                marker,
                depth + 1,
                max_depth,
                max_entries,
                scanned_entries,
                matching_entries,
                read_errors,
            );
        }
    }
}

fn append_event(marker_path: &Path, event: CrashEvent) -> Result<(), CrashMarkerError> {
    let state = marker_path
        .parent()
        .ok_or_else(|| io_error(marker_path, "marker path has no parent directory"))?;
    let log_path = state.join(CRASH_LOG_NAME);
    let mut log = load_log(&log_path)?;
    log.events.push(event);
    if log.events.len() > MAX_CRASH_EVENTS {
        let first = log.events.len() - MAX_CRASH_EVENTS;
        log.events.drain(..first);
    }
    let bytes = serde_json::to_vec(&log)
        .map_err(|error| io_error(&log_path, format!("could not serialize crash log: {error}")))?;
    write_private_atomic(&log_path, &bytes)
}

fn load_log(path: &Path) -> Result<CrashLog, CrashMarkerError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(CrashLog::default()),
        Err(error) => return Err(io_error(path, error)),
    };
    let log: CrashLog = serde_json::from_slice(&bytes)
        .map_err(|error| io_error(path, format!("could not parse crash log: {error}")))?;
    if log.schema != CRASH_LOG_SCHEMA || log.events.len() > MAX_CRASH_EVENTS {
        return Err(io_error(path, "unsupported or oversized crash log"));
    }
    Ok(log)
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<(), CrashMarkerError> {
    let nonce = unix_timestamp_nanos();
    let temp_path = path.with_extension(format!("tmp-{}-{nonce}", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temp_path)
        .map_err(|error| io_error(&temp_path, error))?;
    let result = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = result.and_then(|()| fs::rename(&temp_path, path)) {
        let _ = fs::remove_file(&temp_path);
        return Err(io_error(path, error));
    }
    sync_parent(path)
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn unix_timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos())
}

fn sync_parent(path: &Path) -> Result<(), CrashMarkerError> {
    let parent = path.parent().ok_or_else(|| CrashMarkerError::Io {
        path: path.to_owned(),
        message: "marker path has no parent directory".into(),
    })?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| io_error(parent, error))
}

fn io_error(path: &Path, error: impl std::fmt::Display) -> CrashMarkerError {
    CrashMarkerError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RootSpec, StorageRoots};

    #[test]
    fn marker_reports_unclean_reopen_and_finishes_cleanly() {
        let base = std::env::temp_dir().join(format!(
            "rustbrowser-marker-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("roots");
        let marker = CrashMarker::begin(&roots).expect("begin");
        assert!(!marker.previous_unclean());
        drop(marker);
        let reopened = CrashMarker::begin(&roots).expect("reopen");
        assert!(reopened.previous_unclean());
        reopened.finish().expect("finish");
        let clean = CrashMarker::begin(&roots).expect("clean");
        assert!(!clean.previous_unclean());
        clean.finish().expect("finish");
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn diagnostics_are_bounded_and_exclude_storage_paths() {
        let base = std::env::temp_dir().join(format!(
            "rustbrowser-crash-diagnostics-{}-{}",
            std::process::id(),
            unix_timestamp_nanos()
        ));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("roots");
        for _ in 0..40 {
            let marker = CrashMarker::begin(&roots).expect("begin");
            marker.finish().expect("finish");
        }
        let snapshot = crash_diagnostics(&roots);
        assert_eq!(snapshot["schema"], CRASH_LOG_SCHEMA);
        assert_eq!(snapshot["marker"]["status"], "clear");
        assert_eq!(snapshot["structured_log"]["status"], "available");
        assert_eq!(
            snapshot["structured_log"]["events"]
                .as_array()
                .expect("events")
                .len(),
            MAX_CRASH_EVENTS
        );
        assert_eq!(snapshot["core_dumps"]["status"], "excluded");
        let serialized = snapshot.to_string();
        assert!(!serialized.contains(base.to_string_lossy().as_ref()));
        assert!(!serialized.contains("http://"));
        assert!(!serialized.contains("password"));
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn transient_marker_scan_covers_clean_close_and_unclean_restart() {
        let base = std::env::temp_dir().join(format!(
            "rustbrowser-transient-scan-{}-{}",
            std::process::id(),
            unix_timestamp_nanos()
        ));
        let roots = StorageRoots::resolve(RootSpec::Base(base.clone())).expect("roots");
        let marker_name = "ephemeral-unique-marker";
        let marker = CrashMarker::begin(&roots).expect("begin");
        fs::write(roots.cache.join(format!("{marker_name}.tmp")), b"sentinel").expect("sentinel");
        let found = transient_marker_scan(&roots, marker_name);
        assert_eq!(found["status"], "matches-found");
        assert_eq!(found["marker"], "unclean");
        assert_eq!(found["matching_entries"], 1);
        fs::remove_file(roots.cache.join(format!("{marker_name}.tmp"))).expect("remove");
        marker.finish().expect("finish");
        let clean = transient_marker_scan(&roots, marker_name);
        assert_eq!(clean["status"], "clean");
        assert_eq!(clean["marker"], "clear");
        assert_eq!(clean["matching_entries"], 0);
        let _ = fs::remove_dir_all(base);
    }
}
