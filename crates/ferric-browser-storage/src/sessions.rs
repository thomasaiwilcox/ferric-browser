use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const SNAPSHOT_SCHEMA_VERSION: u32 = 2;
const MAX_WINDOWS: usize = 256;
const MAX_TABS: usize = 5_000;
const MAX_CLOSED_TABS: usize = 100;
const MAX_SESSION_NAME_LENGTH: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestoreMethod {
    SafeGet,
    Placeholder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestoreSafety {
    SafeGet,
    Unsafe,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionTab {
    pub id: Uuid,
    pub safe_restore_url: Option<String>,
    pub restore_method: RestoreMethod,
    pub placeholder_reason: Option<String>,
    pub pinned: bool,
    pub muted: bool,
    pub zoom: f64,
    pub scroll_position: Option<(f64, f64)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionWindow {
    pub selected_tab: Option<Uuid>,
    pub workspace: Option<String>,
    pub tabs: Vec<SessionTab>,
}

/// A safe, profile-local descriptor retained for bounded closed-tab undo.
///
/// This intentionally contains no engine state, credentials, fragments, or
/// private-profile data. It is persisted only as part of a normal-profile
/// session checkpoint and is revalidated before reopening.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedTabSnapshot {
    pub id: Uuid,
    pub url: String,
    pub title: String,
    pub closed_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionSnapshot {
    pub schema_version: u32,
    pub session_id: Uuid,
    pub name: String,
    pub profile_id: Uuid,
    pub revision: u64,
    pub saved_at: String,
    pub windows: Vec<SessionWindow>,
    #[serde(default)]
    pub closed_tabs: Vec<ClosedTabSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotTabInput {
    pub id: Uuid,
    pub url: String,
    pub safety: RestoreSafety,
    pub private: bool,
    pub pinned: bool,
    pub muted: bool,
    pub zoom: f64,
    pub scroll_position: Option<(f64, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotWindowInput {
    pub selected_tab: Option<Uuid>,
    pub workspace: Option<String>,
    pub tabs: Vec<SnapshotTabInput>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RestorePlanEntry {
    pub tab_id: Uuid,
    pub selected: bool,
    pub url: Option<String>,
    pub method: RestoreMethod,
    pub placeholder_reason: Option<String>,
    pub pinned: bool,
    pub muted: bool,
    pub zoom: f64,
    pub scroll_position: Option<(f64, f64)>,
}

/// Returns the profile-scoped path for a named session without accepting path
/// separators or traversal components from the caller.
///
/// # Errors
///
/// Returns an error when the name is empty or unsafe for a filename.
pub fn named_session_path(
    state_root: impl AsRef<Path>,
    profile_id: Uuid,
    name: &str,
) -> Result<PathBuf, SessionError> {
    validate_session_name(name)?;
    Ok(state_root
        .as_ref()
        .join("sessions")
        .join(profile_id.to_string())
        .join(format!("{name}.json")))
}

/// Lists valid named session files in deterministic lexical order.
///
/// # Errors
///
/// Returns an error when the profile session directory cannot be read.
pub fn list_named_sessions(
    state_root: impl AsRef<Path>,
    profile_id: Uuid,
) -> Result<Vec<String>, SessionError> {
    let directory = state_root
        .as_ref()
        .join("sessions")
        .join(profile_id.to_string());
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error(&directory, &error)),
    };
    let mut names = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            (entry.file_type().ok()?.is_file()
                && entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
            .then(|| entry.path().file_stem()?.to_str().map(str::to_owned))
            .flatten()
        })
        .filter(|name| validate_session_name(name).is_ok())
        .filter(|name| name != "current")
        .collect::<Vec<_>>();
    names.sort_unstable();
    Ok(names)
}

/// Returns the current-session checkpoint generations for a profile in
/// deterministic recovery order. Missing profile directories are treated as
/// having no recoverable session.
pub fn current_session_paths(state_root: impl AsRef<Path>, profile_id: Uuid) -> Vec<PathBuf> {
    let directory = state_root
        .as_ref()
        .join("sessions")
        .join(profile_id.to_string());
    let Ok(entries) = fs::read_dir(&directory) else {
        return Vec::new();
    };
    let mut paths = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if !entry.file_type().ok()?.is_file() {
                return None;
            }
            let file_name = path.file_name()?.to_str()?;
            if file_name == "current.json" {
                return Some(path);
            }
            let session_id = file_name.strip_prefix("current-")?.strip_suffix(".json")?;
            Uuid::parse_str(session_id).ok()?;
            Some(path)
        })
        .collect::<Vec<_>>();
    let has_window_snapshots = paths.iter().any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name.starts_with("current-")
                    && Path::new(name)
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            })
    });
    if has_window_snapshots {
        paths.retain(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("current-")
                        && Path::new(name)
                            .extension()
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
                })
        });
    }
    paths.sort_unstable();
    paths
}

/// Removes all current-session checkpoint generations for a profile.
pub fn clear_current_session_checkpoints(
    state_root: impl AsRef<Path>,
    profile_id: Uuid,
) -> Result<(), SessionError> {
    for path in current_session_paths(state_root, profile_id) {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(&path, &error)),
        }
    }
    Ok(())
}

/// Deletes exactly one named session file. Callers must perform any user
/// confirmation before invoking this operation.
///
/// # Errors
///
/// Returns an error when the name is unsafe or the exact session file cannot
/// be removed.
pub fn delete_named_session(
    state_root: impl AsRef<Path>,
    profile_id: Uuid,
    name: &str,
) -> Result<(), SessionError> {
    let path = named_session_path(state_root, profile_id, name)?;
    fs::remove_file(&path).map_err(|error| io_error(&path, &error))
}

fn validate_session_name(name: &str) -> Result<(), SessionError> {
    if name.is_empty()
        || name.len() > MAX_SESSION_NAME_LENGTH
        || name == "."
        || name == ".."
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(SessionError::Invalid(
            "session name must be a safe 1–64 character filename".into(),
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub enum SessionError {
    Io { path: PathBuf, message: String },
    Json { path: PathBuf, message: String },
    Invalid(String),
    LegacySchema(u32),
    NewerSchema(u32),
    SizeLimit,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } | Self::Json { path, message } => {
                write!(formatter, "{}: {message}", path.display())
            }
            Self::Invalid(message) => formatter.write_str(message),
            Self::LegacySchema(version) => write!(
                formatter,
                "session schema {version} belongs to an earlier clean-break release; preserve it and create a new session"
            ),
            Self::NewerSchema(version) => write!(
                formatter,
                "session schema {version} is newer than supported schema {SNAPSHOT_SCHEMA_VERSION}"
            ),
            Self::SizeLimit => formatter.write_str("session snapshot exceeds its size limit"),
        }
    }
}

impl std::error::Error for SessionError {}

impl SessionSnapshot {
    /// Builds a snapshot while dropping private tabs and replacing unsafe
    /// navigation descriptors with URL-free placeholders.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty name/profile, excessive windows/tabs, or
    /// invalid zoom/scroll values.
    pub fn new(
        session_id: Uuid,
        name: impl Into<String>,
        profile_id: Uuid,
        revision: u64,
        saved_at: impl Into<String>,
        windows: Vec<SnapshotWindowInput>,
    ) -> Result<Self, SessionError> {
        let name = name.into();
        if name.trim().is_empty() || name.chars().any(char::is_control) {
            return Err(SessionError::Invalid(
                "session name must be nonempty and safe".into(),
            ));
        }
        if windows.len() > MAX_WINDOWS {
            return Err(SessionError::Invalid("session has too many windows".into()));
        }
        let total_tabs = windows
            .iter()
            .map(|window| window.tabs.len())
            .sum::<usize>();
        if total_tabs > MAX_TABS {
            return Err(SessionError::Invalid("session has too many tabs".into()));
        }
        let windows = windows
            .into_iter()
            .map(|window| {
                let tabs = window
                    .tabs
                    .into_iter()
                    .filter(|tab| !tab.private)
                    .map(sanitize_tab)
                    .collect::<Result<Vec<_>, _>>()?;
                let selected_tab = window
                    .selected_tab
                    .filter(|selected| tabs.iter().any(|tab| tab.id == *selected));
                Ok(SessionWindow {
                    selected_tab,
                    workspace: window.workspace,
                    tabs,
                })
            })
            .collect::<Result<Vec<_>, SessionError>>()?;
        Ok(Self {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            session_id,
            name,
            profile_id,
            revision,
            saved_at: saved_at.into(),
            windows,
            closed_tabs: Vec::new(),
        })
    }

    /// Validates a loaded or constructed snapshot and returns selected tabs
    /// first for lazy restore. No page is loaded by this method.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported schema, oversized structure, invalid
    /// zoom/geometry, or a selected tab that is not in its window.
    pub fn restore_plan(&self) -> Result<Vec<RestorePlanEntry>, SessionError> {
        validate_snapshot(self)?;
        let mut entries = Vec::new();
        for window in &self.windows {
            for tab in window
                .tabs
                .iter()
                .filter(|tab| Some(tab.id) == window.selected_tab)
            {
                entries.push(plan_entry(tab, true));
            }
            for tab in window
                .tabs
                .iter()
                .filter(|tab| Some(tab.id) != window.selected_tab)
            {
                entries.push(plan_entry(tab, false));
            }
        }
        Ok(entries)
    }
}

/// Atomically writes a validated snapshot beside the destination and fsyncs
/// both the file and containing directory before returning.
///
/// # Errors
///
/// Returns an error if validation, serialization, writing, renaming, or fsync
/// fails. The previous validated generation is retained beside the current
/// file so a crash between replacement steps can fall back safely.
pub fn save_session_atomic(
    path: impl AsRef<Path>,
    snapshot: &SessionSnapshot,
) -> Result<(), SessionError> {
    save_session_atomic_inner(path, snapshot, |_| Ok(()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AtomicSaveStep {
    TempSynced,
    PreviousRenamed,
    CurrentRenamed,
    DirectorySynced,
}

fn save_session_atomic_inner(
    path: impl AsRef<Path>,
    snapshot: &SessionSnapshot,
    mut hook: impl FnMut(AtomicSaveStep) -> Result<(), SessionError>,
) -> Result<(), SessionError> {
    validate_snapshot(snapshot)?;
    let path = path.as_ref().to_owned();
    let parent = path
        .parent()
        .ok_or_else(|| SessionError::Invalid("session path has no parent directory".into()))?;
    fs::create_dir_all(parent).map_err(|error| io_error(parent, &error))?;
    let bytes = serde_json::to_vec_pretty(snapshot).map_err(|error| SessionError::Json {
        path: path.clone(),
        message: error.to_string(),
    })?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(SessionError::SizeLimit);
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let temp_path = path.with_extension(format!("json.tmp-{}-{timestamp}", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temp_path)
        .map_err(|error| io_error(&temp_path, &error))?;
    if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temp_path);
        return Err(io_error(&path, &error));
    }
    drop(file);
    if let Err(error) = hook(AtomicSaveStep::TempSynced) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    let previous_path = previous_generation_path(&path);
    if path.exists()
        && let Err(error) = fs::rename(&path, &previous_path)
    {
        let _ = fs::remove_file(&temp_path);
        return Err(io_error(&previous_path, &error));
    }
    if let Err(error) = hook(AtomicSaveStep::PreviousRenamed) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temp_path, &path) {
        let _ = fs::remove_file(&temp_path);
        return Err(io_error(&path, &error));
    }
    hook(AtomicSaveStep::CurrentRenamed)?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| io_error(parent, &error))?;
    hook(AtomicSaveStep::DirectorySynced)?;
    Ok(())
}

/// Loads and validates one snapshot without consuming it or navigating.
///
/// # Errors
///
/// Returns an error when both the current and retained previous generations
/// are unreadable, malformed, oversized, newer-schema, or structurally
/// invalid.
pub fn load_session(path: impl AsRef<Path>) -> Result<SessionSnapshot, SessionError> {
    let path = path.as_ref().to_owned();
    match load_session_generation(&path) {
        Ok(snapshot) => Ok(snapshot),
        Err(primary_error) => {
            let previous = previous_generation_path(&path);
            if let Ok(snapshot) = load_session_generation(&previous) {
                Ok(snapshot)
            } else {
                Err(primary_error)
            }
        }
    }
}

fn load_session_generation(path: &Path) -> Result<SessionSnapshot, SessionError> {
    let bytes = fs::read(path).map_err(|error| io_error(path, &error))?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(SessionError::SizeLimit);
    }
    let snapshot = serde_json::from_slice(&bytes).map_err(|error| SessionError::Json {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

fn previous_generation_path(path: &Path) -> PathBuf {
    path.with_extension("previous")
}

fn sanitize_tab(input: SnapshotTabInput) -> Result<SessionTab, SessionError> {
    if !input.zoom.is_finite() || !(0.25..=5.0).contains(&input.zoom) {
        return Err(SessionError::Invalid(
            "session tab zoom is outside 0.25..=5.0".into(),
        ));
    }
    if input
        .scroll_position
        .is_some_and(|(x, y)| !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0)
    {
        return Err(SessionError::Invalid(
            "session scroll position is invalid".into(),
        ));
    }
    let safe = input.safety == RestoreSafety::SafeGet && safe_get_url(&input.url);
    Ok(SessionTab {
        id: input.id,
        safe_restore_url: safe.then_some(input.url),
        restore_method: if safe {
            RestoreMethod::SafeGet
        } else {
            RestoreMethod::Placeholder
        },
        placeholder_reason: safe
            .then_some(String::new())
            .filter(|reason| !reason.is_empty())
            .or_else(|| Some("navigation was not proven safe to replay as GET".into())),
        pinned: input.pinned,
        muted: input.muted,
        zoom: input.zoom,
        scroll_position: input.scroll_position,
    })
}

fn safe_get_url(url: &str) -> bool {
    let Some((scheme, remainder)) = url.split_once(':') else {
        return false;
    };
    if !matches!(
        scheme.to_ascii_lowercase().as_str(),
        "http" | "https" | "file" | "about"
    ) {
        return false;
    }
    if url.contains(['\r', '\n', '\0'])
        || remainder.contains("//") && remainder.starts_with("//") && remainder[2..].is_empty()
    {
        return false;
    }
    let lower = url.to_ascii_lowercase();
    ![
        "code=",
        "state=",
        "token=",
        "auth=",
        "session=",
        "samlresponse=",
        "sso=",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn plan_entry(tab: &SessionTab, selected: bool) -> RestorePlanEntry {
    RestorePlanEntry {
        tab_id: tab.id,
        selected,
        url: tab.safe_restore_url.clone(),
        method: tab.restore_method.clone(),
        placeholder_reason: tab.placeholder_reason.clone(),
        pinned: tab.pinned,
        muted: tab.muted,
        zoom: tab.zoom,
        scroll_position: tab.scroll_position,
    }
}

fn validate_snapshot(snapshot: &SessionSnapshot) -> Result<(), SessionError> {
    if snapshot.schema_version < SNAPSHOT_SCHEMA_VERSION {
        return Err(SessionError::LegacySchema(snapshot.schema_version));
    }
    if snapshot.schema_version > SNAPSHOT_SCHEMA_VERSION {
        return Err(SessionError::NewerSchema(snapshot.schema_version));
    }
    if snapshot.name.trim().is_empty() || snapshot.name.chars().any(char::is_control) {
        return Err(SessionError::Invalid("session name is invalid".into()));
    }
    if snapshot.windows.len() > MAX_WINDOWS
        || snapshot
            .windows
            .iter()
            .map(|window| window.tabs.len())
            .sum::<usize>()
            > MAX_TABS
    {
        return Err(SessionError::Invalid(
            "session structure exceeds limits".into(),
        ));
    }
    if snapshot.closed_tabs.len() > MAX_CLOSED_TABS {
        return Err(SessionError::Invalid(
            "closed-tab undo history exceeds limits".into(),
        ));
    }
    for window in &snapshot.windows {
        if let Some(selected) = window.selected_tab
            && !window.tabs.iter().any(|tab| tab.id == selected)
        {
            return Err(SessionError::Invalid(
                "selected session tab is not in its window".into(),
            ));
        }
        for tab in &window.tabs {
            if !tab.zoom.is_finite() || !(0.25..=5.0).contains(&tab.zoom) {
                return Err(SessionError::Invalid("session tab zoom is invalid".into()));
            }
            if tab.restore_method == RestoreMethod::SafeGet && tab.safe_restore_url.is_none() {
                return Err(SessionError::Invalid("safe-get tab has no safe URL".into()));
            }
            if tab.restore_method == RestoreMethod::Placeholder && tab.safe_restore_url.is_some() {
                return Err(SessionError::Invalid(
                    "placeholder tab contains a restore URL".into(),
                ));
            }
        }
    }
    for closed in &snapshot.closed_tabs {
        if closed.url.is_empty()
            || closed.url.len() > 16 * 1024
            || closed.url.chars().any(char::is_control)
            || closed.title.len() > 8 * 1024
            || closed.title.chars().any(char::is_control)
        {
            return Err(SessionError::Invalid(
                "closed-tab undo descriptor is invalid".into(),
            ));
        }
    }
    Ok(())
}

fn io_error(path: &Path, error: &io::Error) -> SessionError {
    SessionError::Io {
        path: path.to_owned(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tab(url: &str, safe_get: bool, private: bool) -> SnapshotTabInput {
        SnapshotTabInput {
            id: Uuid::new_v4(),
            url: url.into(),
            safety: if safe_get {
                RestoreSafety::SafeGet
            } else {
                RestoreSafety::Unsafe
            },
            private,
            pinned: false,
            muted: false,
            zoom: 1.0,
            scroll_position: None,
        }
    }

    #[test]
    fn private_and_unsafe_tabs_never_persist_replayable_urls() {
        let safe = tab("https://example.test/", true, false);
        let safe_id = safe.id;
        let snapshot = SessionSnapshot::new(
            Uuid::new_v4(),
            "work",
            Uuid::new_v4(),
            4,
            "2026-09-15T00:00:00Z",
            vec![SnapshotWindowInput {
                selected_tab: Some(safe_id),
                workspace: Some("3".into()),
                tabs: vec![
                    safe,
                    tab("https://example.test/post", false, false),
                    tab("data:text/html,secret", true, false),
                    tab("https://private.test/", true, true),
                ],
            }],
        )
        .expect("snapshot");
        assert_eq!(snapshot.windows[0].tabs.len(), 3);
        assert_eq!(
            snapshot.windows[0].tabs[0].restore_method,
            RestoreMethod::SafeGet
        );
        assert_eq!(
            snapshot.windows[0].tabs[1].restore_method,
            RestoreMethod::Placeholder
        );
        let json = serde_json::to_string(&snapshot).expect("json");
        assert!(!json.contains("data:text/html"));
        assert!(!json.contains("secret"));
    }

    #[test]
    fn atomic_round_trip_and_selected_first_restore_plan() {
        let directory =
            std::env::temp_dir().join(format!("ferric-browser-session-{}", std::process::id()));
        let path = directory.join("session.json");
        let mut first = tab("https://first.test/", true, false);
        first.scroll_position = Some((12.5, 99.0));
        let second = tab("https://second.test/", true, false);
        let mut snapshot = SessionSnapshot::new(
            Uuid::new_v4(),
            "test",
            Uuid::new_v4(),
            9,
            "now",
            vec![SnapshotWindowInput {
                selected_tab: Some(second.id),
                workspace: None,
                tabs: vec![first.clone(), second.clone()],
            }],
        )
        .expect("snapshot");
        snapshot.closed_tabs.push(ClosedTabSnapshot {
            id: Uuid::new_v4(),
            url: "https://closed.test/".into(),
            title: "Closed tab".into(),
            closed_at: 42,
        });
        save_session_atomic(&path, &snapshot).expect("save");
        let mut newer = snapshot.clone();
        newer.revision = 10;
        save_session_atomic(&path, &newer).expect("save newer");
        let loaded = load_session(&path).expect("load");
        assert_eq!(loaded.revision, 10);
        assert_eq!(loaded.closed_tabs, snapshot.closed_tabs);
        assert_eq!(loaded.session_id, snapshot.session_id);
        assert_eq!(loaded.profile_id, snapshot.profile_id);
        assert_eq!(loaded.windows[0].tabs[0].id, first.id);
        assert_eq!(loaded.windows[0].tabs[1].id, second.id);
        fs::remove_file(&path).expect("remove current generation");
        let recovered = load_session(&path).expect("load previous generation");
        assert_eq!(recovered.revision, 9);
        let plan = loaded.restore_plan().expect("plan");
        assert_eq!(plan[0].tab_id, second.id);
        assert!(plan[0].selected);
        assert_eq!(plan[1].tab_id, first.id);
        assert!(!plan[1].selected);
        assert_eq!(plan[1].scroll_position, Some((12.5, 99.0)));
        let _ = fs::remove_file(previous_generation_path(&path));
        let _ = fs::remove_dir(&directory);
    }

    #[test]
    fn injected_atomic_interruption_keeps_a_recoverable_generation() {
        let path = std::env::temp_dir().join(format!(
            "ferric-browser-session-interruption-{}-{}.json",
            std::process::id(),
            Uuid::new_v4()
        ));
        let tab = tab("https://example.test/", true, false);
        let snapshot = SessionSnapshot::new(
            Uuid::new_v4(),
            "interruption",
            Uuid::new_v4(),
            1,
            "now",
            vec![SnapshotWindowInput {
                selected_tab: Some(tab.id),
                workspace: None,
                tabs: vec![tab],
            }],
        )
        .expect("snapshot");
        save_session_atomic(&path, &snapshot).expect("initial save");

        let mut newer = snapshot.clone();
        newer.revision = 2;
        let fail_at = |step| {
            (step != AtomicSaveStep::TempSynced)
                .then_some(())
                .ok_or_else(|| SessionError::Invalid("injected interruption".into()))
        };
        assert!(save_session_atomic_inner(&path, &newer, fail_at).is_err());
        assert_eq!(load_session(&path).expect("current generation").revision, 1);

        let fail_at = |step| {
            (step != AtomicSaveStep::PreviousRenamed)
                .then_some(())
                .ok_or_else(|| SessionError::Invalid("injected interruption".into()))
        };
        assert!(save_session_atomic_inner(&path, &newer, fail_at).is_err());
        assert!(!path.exists());
        assert_eq!(
            load_session(&path).expect("previous generation").revision,
            1
        );

        save_session_atomic(&path, &newer).expect("restore current generation");
        let mut latest = newer.clone();
        latest.revision = 3;
        let fail_at = |step| {
            (step != AtomicSaveStep::CurrentRenamed)
                .then_some(())
                .ok_or_else(|| SessionError::Invalid("injected interruption".into()))
        };
        assert!(save_session_atomic_inner(&path, &latest, fail_at).is_err());
        assert_eq!(
            load_session(&path)
                .expect("renamed current generation")
                .revision,
            3
        );

        let mut directory_synced = latest.clone();
        directory_synced.revision = 4;
        let fail_at = |step| {
            (step != AtomicSaveStep::DirectorySynced)
                .then_some(())
                .ok_or_else(|| SessionError::Invalid("injected interruption".into()))
        };
        assert!(save_session_atomic_inner(&path, &directory_synced, fail_at).is_err());
        assert_eq!(
            load_session(&path)
                .expect("directory-sync generation")
                .revision,
            4
        );

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(previous_generation_path(&path));
    }

    #[test]
    fn newer_or_structurally_unsafe_snapshots_are_rejected() {
        let mut snapshot =
            SessionSnapshot::new(Uuid::new_v4(), "test", Uuid::new_v4(), 1, "now", Vec::new())
                .expect("snapshot");
        snapshot.schema_version = 3;
        assert!(matches!(
            snapshot.restore_plan(),
            Err(SessionError::NewerSchema(3))
        ));
        snapshot.schema_version = 1;
        assert!(matches!(
            snapshot.restore_plan(),
            Err(SessionError::LegacySchema(1))
        ));
        snapshot.schema_version = 2;
        snapshot.windows.push(SessionWindow {
            selected_tab: Some(Uuid::new_v4()),
            workspace: None,
            tabs: Vec::new(),
        });
        assert!(snapshot.restore_plan().is_err());
    }

    #[test]
    fn named_sessions_are_profile_scoped_listed_and_deleted_exactly() {
        let directory = std::env::temp_dir().join(format!(
            "ferric-browser-named-sessions-{}",
            std::process::id()
        ));
        let profile = Uuid::new_v4();
        let snapshot = SessionSnapshot::new(Uuid::new_v4(), "work", profile, 1, "now", Vec::new())
            .expect("snapshot");
        let path = named_session_path(&directory, profile, "work").expect("path");
        save_session_atomic(&path, &snapshot).expect("save");
        let current = directory
            .join("sessions")
            .join(profile.to_string())
            .join("current.json");
        save_session_atomic(&current, &snapshot).expect("save current");
        let other = named_session_path(&directory, profile, "personal").expect("path");
        save_session_atomic(&other, &snapshot).expect("save");
        assert_eq!(
            list_named_sessions(&directory, profile).expect("list"),
            vec!["personal", "work"]
        );
        assert!(named_session_path(&directory, profile, "../escape").is_err());
        delete_named_session(&directory, profile, "work").expect("delete");
        assert_eq!(
            list_named_sessions(&directory, profile).expect("list"),
            vec!["personal"]
        );
        let _ = fs::remove_dir_all(directory);
    }
}
