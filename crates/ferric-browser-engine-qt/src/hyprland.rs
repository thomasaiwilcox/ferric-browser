//! Failure-safe, runtime-only Hyprland integration.
//!
//! This module deliberately does not write Hyprland configuration.  It only
//! discovers the current instance sockets and sends bounded IPC requests when
//! the user has enabled workspace routing.

mod page_groups;
mod queued_send;
mod unix_deadline;
pub(crate) use page_groups::{
    GroupOutcome, GroupReply, ManagedCapability, ManagedRequest, ManagedRole,
};
pub(crate) use queued_send::QueuedSend;

use ferric_browser_config::{HyprlandConfig, TriState};
use serde::{Deserialize, Serialize};
use std::{
    fmt, fs,
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
    time::Duration,
};

pub const FERRIC_BROWSER_DESKTOP_ID: &str = "io.github.ferricbrowser.FerricBrowser";

const IO_TIMEOUT: Duration = Duration::from_millis(500);
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_SIGNATURE_BYTES: usize = 128;
const MAX_WORKSPACE_BYTES: usize = 128;
const MAX_CLIENT_FIELD_BYTES: usize = 256;
const MAX_VERSION_FIELD_BYTES: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HyprlandError {
    Disabled,
    Unavailable(String),
    InvalidWorkspace,
    Protocol(String),
    Io(String),
}

impl fmt::Display for HyprlandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => formatter.write_str("workspace routing is disabled"),
            Self::Unavailable(message) | Self::Protocol(message) | Self::Io(message) => {
                formatter.write_str(message)
            }
            Self::InvalidWorkspace => formatter.write_str("workspace selector is invalid"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HyprlandAdapter {
    enabled: TriState,
    workspace_routing: bool,
    request_socket: Option<PathBuf>,
    event_socket: Option<PathBuf>,
    #[cfg(test)]
    test_instance_signature: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BrowserClient {
    pub address: String,
    pub class: String,
    pub initial_class: String,
    pub pid: Option<u32>,
    pub workspace: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HyprlandVersion {
    pub branch: Option<String>,
    pub tag: Option<String>,
    pub commit: Option<String>,
}

impl HyprlandVersion {
    fn uses_lua_dispatch(&self) -> bool {
        self.tag
            .as_deref()
            .or(self.branch.as_deref())
            .and_then(parse_version_prefix)
            .is_some_and(|(major, minor)| (major, minor) >= (0, 55))
    }
}

#[derive(Debug, Deserialize)]
struct RawClient {
    #[serde(default)]
    address: String,
    #[serde(default)]
    class: String,
    #[serde(rename = "initialClass", default)]
    initial_class: String,
    pid: Option<u32>,
    workspace: Option<RawWorkspace>,
    #[serde(default)]
    mapped: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RawWorkspace {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawVersion {
    branch: Option<String>,
    tag: Option<String>,
    commit: Option<String>,
}

impl HyprlandAdapter {
    pub fn from_config(config: &HyprlandConfig) -> Self {
        let signature = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
            .ok()
            .filter(|value| valid_environment_component(value, MAX_SIGNATURE_BYTES));
        let runtime = std::env::var("XDG_RUNTIME_DIR")
            .ok()
            .filter(|value| !value.is_empty() && !value.chars().any(char::is_control));
        let (request_socket, event_socket) = match (runtime, signature) {
            (Some(runtime), Some(signature)) => {
                let base = Path::new(&runtime).join("hypr").join(signature);
                (
                    Some(base.join(".socket.sock")),
                    Some(base.join(".socket2.sock")),
                )
            }
            _ => (None, None),
        };
        Self {
            enabled: config.enabled.clone(),
            workspace_routing: config.workspace_routing,
            request_socket,
            event_socket,
            #[cfg(test)]
            test_instance_signature: None,
        }
    }

    #[cfg(test)]
    pub(super) fn observation_test_adapter(
        enabled: bool,
        workspace_routing: bool,
        request_socket: Option<std::path::PathBuf>,
        event_socket: Option<std::path::PathBuf>,
        test_instance_signature: Option<&str>,
    ) -> Self {
        Self {
            enabled: if enabled { TriState::On } else { TriState::Off },
            workspace_routing,
            request_socket,
            event_socket,
            test_instance_signature: test_instance_signature.map(str::to_owned),
        }
    }

    #[cfg(test)]
    fn with_paths(
        enabled: TriState,
        workspace_routing: bool,
        request_socket: PathBuf,
        event_socket: PathBuf,
    ) -> Self {
        Self {
            enabled,
            workspace_routing,
            request_socket: Some(request_socket),
            event_socket: Some(event_socket),
            test_instance_signature: None,
        }
    }

    pub fn workspace_routing_enabled(&self) -> bool {
        self.workspace_routing && self.enabled != TriState::Off
    }

    pub fn status(&self) -> String {
        if self.enabled == TriState::Off || !self.workspace_routing {
            return "disabled".into();
        }
        let Some(request_socket) = self.request_socket.as_deref() else {
            return "unavailable: Hyprland instance is not discoverable".into();
        };
        let Some(event_socket) = self.event_socket.as_deref() else {
            return "unavailable: Hyprland event socket is not discoverable".into();
        };
        if !is_socket(request_socket) || !is_socket(event_socket) {
            return "unavailable: Hyprland IPC sockets are not available".into();
        }
        "ready".into()
    }

    pub fn route_workspace(&self, workspace: &str) -> Result<(), HyprlandError> {
        if !self.workspace_routing_enabled() {
            return Err(
                if self.enabled == TriState::Off || !self.workspace_routing {
                    HyprlandError::Disabled
                } else {
                    HyprlandError::Unavailable(self.status())
                },
            );
        }
        if !valid_workspace_selector(workspace) {
            return Err(HyprlandError::InvalidWorkspace);
        }
        let version = self.version()?;
        let command = if version.uses_lua_dispatch() {
            let workspace = lua_string_literal(workspace);
            format!("dispatch hl.dsp.focus({{ workspace = {workspace} }})")
        } else {
            format!("dispatch workspace {workspace}")
        };
        let response = self.request(&command)?;
        if response.trim() == "ok" || response.trim().is_empty() {
            Ok(())
        } else {
            Err(HyprlandError::Protocol(sanitize_response(&response)))
        }
    }

    /// Moves the compositor's currently active window to a validated workspace.
    /// The caller must establish exact Qt-window activation before invoking this
    /// operation; this method never guesses a target from titles or PIDs.
    pub fn move_active_window_to_workspace(&self, workspace: &str) -> Result<(), HyprlandError> {
        if !self.workspace_routing_enabled() {
            return Err(
                if self.enabled == TriState::Off || !self.workspace_routing {
                    HyprlandError::Disabled
                } else {
                    HyprlandError::Unavailable(self.status())
                },
            );
        }
        if !valid_workspace_selector(workspace) {
            return Err(HyprlandError::InvalidWorkspace);
        }
        let _active_client = self.active_browser_client()?;
        let version = self.version()?;
        let command = if version.uses_lua_dispatch() {
            let workspace = lua_string_literal(workspace);
            format!("dispatch hl.dsp.window.move({{ workspace = {workspace} }})")
        } else {
            format!("dispatch movetoworkspacesilent {workspace}")
        };
        let response = self.request(&command)?;
        if response.trim() == "ok" || response.trim().is_empty() {
            Ok(())
        } else {
            Err(HyprlandError::Protocol(sanitize_response(&response)))
        }
    }

    pub fn browser_clients(&self) -> Result<Vec<BrowserClient>, HyprlandError> {
        let response = self.request("j/clients")?;
        let clients: Vec<RawClient> = serde_json::from_str(&response).map_err(|error| {
            HyprlandError::Protocol(format!("invalid Hyprland clients JSON: {error}"))
        })?;
        Ok(clients
            .into_iter()
            .filter_map(parse_browser_client)
            .collect())
    }

    /// Returns the compositor's currently active `Ferric Browser` client.
    ///
    /// Qt activation is requested before this worker call. Requiring the
    /// compositor to report a mapped `Ferric Browser` client prevents a move from
    /// being applied to an unrelated active application when activation was
    /// denied or the target disappeared during the handoff.
    pub fn active_browser_client(&self) -> Result<BrowserClient, HyprlandError> {
        let response = self.request("j/activewindow")?;
        let client: RawClient = serde_json::from_str(&response).map_err(|error| {
            HyprlandError::Protocol(format!("invalid Hyprland active-window JSON: {error}"))
        })?;
        if client.mapped == Some(false) {
            return Err(HyprlandError::Unavailable(
                "the active Hyprland client is not mapped".into(),
            ));
        }
        parse_browser_client(client).ok_or_else(|| {
            HyprlandError::Unavailable(
                "the active Hyprland client is not an identifiable Ferric Browser window".into(),
            )
        })
    }

    pub fn version(&self) -> Result<HyprlandVersion, HyprlandError> {
        let response = self.request("j/version")?;
        let version: RawVersion = serde_json::from_str(&response).map_err(|error| {
            HyprlandError::Protocol(format!("invalid Hyprland version JSON: {error}"))
        })?;
        Ok(HyprlandVersion {
            branch: bounded_version_field(version.branch),
            tag: bounded_version_field(version.tag),
            commit: version.commit.filter(|value| {
                value.len() == 40 && value.chars().all(|character| character.is_ascii_hexdigit())
            }),
        })
    }

    fn request(&self, command: &str) -> Result<String, HyprlandError> {
        let Some(path) = self.request_socket.as_deref() else {
            return Err(HyprlandError::Unavailable(
                "Hyprland instance is not discoverable".into(),
            ));
        };
        let response =
            unix_deadline::request(path, command.as_bytes(), MAX_RESPONSE_BYTES, IO_TIMEOUT)
                .map_err(|error| {
                    HyprlandError::Io(format!("Hyprland IPC request failed: {error}"))
                })?;
        String::from_utf8(response).map_err(|error| {
            HyprlandError::Protocol(format!("Hyprland IPC response was not UTF-8: {error}"))
        })
    }
}

impl HyprlandAdapter {
    /// Read-only snapshot/event observation. This deliberately does not use
    /// the workspace-routing permission bit: it never dispatches a command.
    pub(super) fn observe_events(
        &self,
        session: &mut super::hyprland_events::ObservationSession,
    ) -> super::hyprland_events::ObservationResult {
        use super::hyprland_events::{
            MAX_SNAPSHOT_BYTES, ObservationCapability as Capability, ObservationResult,
        };

        if self.enabled == TriState::Off {
            return ObservationResult {
                capability: Capability::Disabled,
                state: None,
            };
        }
        let (Some(request_path), Some(event_path)) =
            (self.request_socket.as_ref(), self.event_socket.as_ref())
        else {
            session.cache.invalidate();
            return ObservationResult {
                capability: Capability::Unavailable,
                state: None,
            };
        };
        #[cfg(test)]
        let signature = self
            .test_instance_signature
            .clone()
            .unwrap_or_else(|| std::env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default());
        #[cfg(not(test))]
        let signature = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default();
        let events = match session.read_events(Some(event_path)) {
            Ok(events) => events,
            Err(_) => {
                session.cache.invalidate();
                return ObservationResult {
                    capability: Capability::Unavailable,
                    state: None,
                };
            }
        };
        let version_bytes = match bounded_hyprland_request(request_path, b"j/version", 4096) {
            Ok(bytes) => bytes,
            Err(_) => {
                session.cache.invalidate();
                return ObservationResult {
                    capability: Capability::Unavailable,
                    state: None,
                };
            }
        };
        let version_value: serde_json::Value = match serde_json::from_slice(&version_bytes) {
            Ok(value) => value,
            Err(_) => {
                session.cache.invalidate();
                return ObservationResult {
                    capability: Capability::InvalidData,
                    state: None,
                };
            }
        };
        let version = HyprlandVersion {
            branch: version_value
                .get("branch")
                .and_then(|v| v.as_str())
                .map(str::to_owned),
            tag: version_value
                .get("tag")
                .and_then(|v| v.as_str())
                .map(str::to_owned),
            commit: version_value
                .get("commit")
                .and_then(|v| v.as_str())
                .map(str::to_owned),
        };
        if version.branch.is_none() && version.tag.is_none() && version.commit.is_none() {
            session.cache.invalidate();
            return ObservationResult {
                capability: Capability::UnsupportedVersion,
                state: None,
            };
        }
        let snapshot =
            match bounded_hyprland_request(request_path, b"j/clients", MAX_SNAPSHOT_BYTES) {
                Ok(bytes) => bytes,
                Err(_) => {
                    session.cache.invalidate();
                    return ObservationResult {
                        capability: Capability::Unavailable,
                        state: None,
                    };
                }
            };
        match session
            .cache
            .observe(&signature, version, &snapshot, &events)
        {
            Ok(state) => {
                let active_handle = state.active_address.as_deref().and_then(|address| {
                    super::hyprland_events::ObservationCache::handle_for(&state, address)
                });
                let active_identity_is_current = state.active_address.is_none()
                    || active_handle
                        .as_ref()
                        .is_some_and(|handle| session.cache.is_current(handle));
                if active_identity_is_current {
                    ObservationResult {
                        capability: Capability::Available,
                        state: Some(state),
                    }
                } else {
                    session.cache.invalidate();
                    ObservationResult {
                        capability: Capability::InvalidData,
                        state: None,
                    }
                }
            }
            Err(_) => {
                session.cache.invalidate();
                ObservationResult {
                    capability: Capability::InvalidData,
                    state: None,
                }
            }
        }
    }
}

fn bounded_hyprland_request(
    path: &std::path::Path,
    command: &[u8],
    maximum: usize,
) -> Result<Vec<u8>, std::io::Error> {
    unix_deadline::request(path, command, maximum, Duration::from_millis(250))
}

#[cfg(test)]
mod observation_permission_tests {
    use super::{HyprlandAdapter, TriState};
    use crate::hyprland_events::{ObservationCapability, ObservationSession};

    #[test]
    fn disabled_observation_returns_before_socket_access() {
        let adapter = HyprlandAdapter {
            enabled: TriState::Off,
            workspace_routing: false,
            request_socket: None,
            event_socket: None,
            test_instance_signature: None,
        };
        let mut session = ObservationSession::default();
        let result = adapter.observe_events(&mut session);
        assert_eq!(result.capability, ObservationCapability::Disabled);
        assert!(result.state.is_none());
    }

    #[test]
    fn read_only_observation_is_not_gated_by_workspace_routing() {
        let adapter = HyprlandAdapter {
            enabled: TriState::On,
            workspace_routing: false,
            request_socket: None,
            event_socket: None,
            test_instance_signature: None,
        };
        let mut session = ObservationSession::default();
        let result = adapter.observe_events(&mut session);
        assert_eq!(result.capability, ObservationCapability::Unavailable);
        assert!(result.state.is_none());
    }
}

fn bounded_version_field(value: Option<String>) -> Option<String> {
    value.filter(|value| {
        !value.is_empty()
            && value.len() <= MAX_VERSION_FIELD_BYTES
            && !value.chars().any(char::is_control)
    })
}

fn parse_version_prefix(value: &str) -> Option<(u32, u32)> {
    let value = value.strip_prefix('v').unwrap_or(value);
    let mut parts = value.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

fn parse_browser_client(client: RawClient) -> Option<BrowserClient> {
    if !valid_client_address(&client.address)
        || !bounded_client_field(&client.class)
        || !bounded_client_field(&client.initial_class)
        || (client.class != FERRIC_BROWSER_DESKTOP_ID
            && client.initial_class != FERRIC_BROWSER_DESKTOP_ID)
    {
        return None;
    }
    let workspace = client.workspace.and_then(|workspace| {
        workspace
            .name
            .filter(|name| bounded_client_field(name) && !name.is_empty())
    });
    Some(BrowserClient {
        address: client.address,
        class: client.class,
        initial_class: client.initial_class,
        pid: client.pid,
        workspace,
    })
}

fn valid_client_address(value: &str) -> bool {
    value.len() >= 3
        && value.len() <= MAX_CLIENT_FIELD_BYTES
        && value.starts_with("0x")
        && value[2..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn bounded_client_field(value: &str) -> bool {
    value.len() <= MAX_CLIENT_FIELD_BYTES && !value.chars().any(char::is_control)
}

fn valid_environment_component(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && !value.chars().any(char::is_control)
        && !value.contains('/')
}

fn is_socket(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.file_type().is_socket())
}

pub fn valid_workspace_selector(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_WORKSPACE_BYTES
        && value == value.trim()
        && !value.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || matches!(
                    character,
                    ';' | '&' | '|' | '`' | '$' | '(' | ')' | '<' | '>'
                )
        })
}

fn sanitize_response(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(160)
        .collect::<String>()
}

fn lua_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        io::{Read, Write},
        os::unix::net::UnixListener,
        sync::mpsc,
        thread,
    };
    use uuid::Uuid;

    fn socket_pair() -> (PathBuf, PathBuf, PathBuf) {
        let directory =
            std::env::temp_dir().join(format!("ferric-browser-hyprland-{}", Uuid::new_v4()));
        fs::create_dir(&directory).expect("temporary directory");
        (
            directory.join(".socket.sock"),
            directory.join(".socket2.sock"),
            directory,
        )
    }

    #[test]
    fn workspace_selector_rejects_command_like_values() {
        assert!(valid_workspace_selector("3"));
        assert!(valid_workspace_selector("name:research"));
        assert!(!valid_workspace_selector("3; dispatch exec sh"));
        assert!(!valid_workspace_selector("two words"));
        assert!(!valid_workspace_selector(""));
    }

    #[test]
    fn disabled_adapter_never_requires_runtime_sockets() {
        let adapter = HyprlandAdapter::with_paths(
            TriState::Off,
            true,
            PathBuf::from("/missing/request"),
            PathBuf::from("/missing/events"),
        );
        assert_eq!(adapter.status(), "disabled");
        assert_eq!(adapter.route_workspace("3"), Err(HyprlandError::Disabled));
    }

    #[test]
    fn request_retries_transient_compositor_restart_errors_with_a_cap() {
        let (request, event, directory) = socket_pair();
        let server_request = request.clone();
        let server = thread::spawn(move || {
            thread::sleep(Duration::from_millis(25));
            let listener = UnixListener::bind(server_request).expect("request socket");
            let (mut version_stream, _) = listener.accept().expect("version client");
            let mut version_command = String::new();
            version_stream
                .read_to_string(&mut version_command)
                .expect("version command");
            assert_eq!(version_command, "j/version");
            version_stream
                .write_all(br#"{"tag":"v0.56.2"}"#)
                .expect("version response");
            drop(version_stream);
            let (mut stream, _) = listener.accept().expect("client");
            let mut command = String::new();
            stream.read_to_string(&mut command).expect("command");
            assert_eq!(command, "dispatch hl.dsp.focus({ workspace = \"3\" })");
            stream.write_all(b"ok\n").expect("response");
        });
        let adapter = HyprlandAdapter::with_paths(TriState::On, true, request, event);
        assert_eq!(adapter.route_workspace("3"), Ok(()));
        server.join().expect("server");
        fs::remove_dir_all(directory).expect("temporary directory cleanup");
    }

    #[test]
    fn workspace_request_is_bounded_and_uses_the_request_socket() {
        let (request, event, directory) = socket_pair();
        // Some restricted test runners deny AF_UNIX creation even below a
        // writable temporary directory.  This is an OS capability gap, not
        // a Hyprland assertion failure; the native Wayland qualification
        // covers the socket path on a supported host.
        let listener = match UnixListener::bind(&request) {
            Ok(listener) => listener,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
                ) =>
            {
                eprintln!("skipping Hyprland socket test: AF_UNIX creation is unavailable");
                let _ = fs::remove_dir_all(directory);
                return;
            }
            Err(error) => panic!("request socket: {error}"),
        };
        let (seen_sender, seen_receiver) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut version_stream, _) = listener.accept().expect("version client");
            let mut version_command = String::new();
            version_stream
                .read_to_string(&mut version_command)
                .expect("version command");
            assert_eq!(version_command, "j/version");
            version_stream
                .write_all(br#"{"tag":"v0.56.2"}"#)
                .expect("version response");
            drop(version_stream);
            let (mut stream, _) = listener.accept().expect("client");
            let mut command = String::new();
            stream.read_to_string(&mut command).expect("command");
            seen_sender.send(command).expect("command result");
            stream.write_all(b"ok\n").expect("response");
        });
        let adapter = HyprlandAdapter::with_paths(TriState::On, true, request, event);
        assert_eq!(adapter.route_workspace("3"), Ok(()));
        assert_eq!(
            seen_receiver.recv().expect("command"),
            "dispatch hl.dsp.focus({ workspace = \"3\" })"
        );
        server.join().expect("server");
        fs::remove_dir_all(directory).expect("temporary directory cleanup");
    }

    #[test]
    fn move_active_window_request_uses_the_validated_workspace() {
        let (request, event, directory) = socket_pair();
        let listener = match UnixListener::bind(&request) {
            Ok(listener) => listener,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Unsupported
                ) =>
            {
                eprintln!("skipping Hyprland socket test: AF_UNIX creation is unavailable");
                let _ = fs::remove_dir_all(directory);
                return;
            }
            Err(error) => panic!("request socket: {error}"),
        };
        let (seen_sender, seen_receiver) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut active_stream, _) = listener.accept().expect("active-window client");
            let mut active_command = String::new();
            active_stream
                .read_to_string(&mut active_command)
                .expect("active-window command");
            assert_eq!(active_command, "j/activewindow");
            active_stream
                .write_all(
                    br#"{"address":"0x1","class":"io.github.ferricbrowser.FerricBrowser","initialClass":"","pid":42,"mapped":true,"workspace":{"name":"3"}}"#,
                )
                .expect("active-window response");
            drop(active_stream);
            let (mut version_stream, _) = listener.accept().expect("version client");
            let mut version_command = String::new();
            version_stream
                .read_to_string(&mut version_command)
                .expect("version command");
            assert_eq!(version_command, "j/version");
            version_stream
                .write_all(br#"{"tag":"v0.56.2"}"#)
                .expect("version response");
            drop(version_stream);
            let (mut stream, _) = listener.accept().expect("client");
            let mut command = String::new();
            stream.read_to_string(&mut command).expect("command");
            seen_sender.send(command).expect("command result");
            stream.write_all(b"ok\n").expect("response");
        });
        let adapter = HyprlandAdapter::with_paths(TriState::On, true, request, event);
        assert_eq!(
            adapter.move_active_window_to_workspace("name:research"),
            Ok(())
        );
        assert_eq!(
            seen_receiver.recv().expect("command"),
            "dispatch hl.dsp.window.move({ workspace = \"name:research\" })"
        );
        server.join().expect("server");
        fs::remove_dir_all(directory).expect("temporary directory cleanup");
    }

    #[test]
    fn browser_client_discovery_uses_the_stable_desktop_id() {
        let raw = r#"[
            {"address":"0x1","class":"io.github.ferricbrowser.FerricBrowser","initialClass":"","pid":42,"workspace":{"name":"3"}},
            {"address":"0x2","class":"other","initialClass":"io.github.ferricbrowser.FerricBrowser","pid":43,"workspace":{"name":"4"}},
            {"address":"0x3","class":"other","initialClass":"other","pid":44,"workspace":{"name":"5"}}
        ]"#;
        let clients: Vec<RawClient> = serde_json::from_str(raw).expect("client JSON");
        let browser_clients = clients
            .into_iter()
            .filter(|client| {
                client.class == FERRIC_BROWSER_DESKTOP_ID
                    || client.initial_class == FERRIC_BROWSER_DESKTOP_ID
            })
            .count();
        assert_eq!(browser_clients, 2);
    }

    #[test]
    fn browser_client_discovery_discards_unbounded_or_unidentifiable_rows() {
        let raw = vec![
            RawClient {
                address: String::new(),
                class: FERRIC_BROWSER_DESKTOP_ID.into(),
                initial_class: String::new(),
                pid: Some(1),
                workspace: None,
                mapped: None,
            },
            RawClient {
                address: "0x1".into(),
                class: format!("{FERRIC_BROWSER_DESKTOP_ID}\n"),
                initial_class: String::new(),
                pid: Some(2),
                workspace: None,
                mapped: None,
            },
            RawClient {
                address: "0x2".into(),
                class: FERRIC_BROWSER_DESKTOP_ID.into(),
                initial_class: String::new(),
                pid: Some(3),
                workspace: Some(RawWorkspace {
                    name: Some("x".repeat(MAX_CLIENT_FIELD_BYTES + 1)),
                }),
                mapped: None,
            },
            RawClient {
                address: "0x3".into(),
                class: FERRIC_BROWSER_DESKTOP_ID.into(),
                initial_class: String::new(),
                pid: Some(4),
                workspace: Some(RawWorkspace {
                    name: Some("3".into()),
                }),
                mapped: None,
            },
            RawClient {
                address: "not-a-hyprland-address".into(),
                class: FERRIC_BROWSER_DESKTOP_ID.into(),
                initial_class: String::new(),
                pid: Some(5),
                workspace: None,
                mapped: None,
            },
        ];
        let clients = raw
            .into_iter()
            .filter_map(parse_browser_client)
            .collect::<Vec<_>>();
        assert_eq!(clients.len(), 2);
        assert_eq!(clients[0].workspace, None);
        assert_eq!(clients[1].address, "0x3");
        assert_eq!(clients[1].workspace.as_deref(), Some("3"));
        assert!(valid_client_address("0x1a2b"));
        assert!(!valid_client_address("0x"));
        assert!(!valid_client_address("0x12\n"));
        assert!(!valid_client_address("not-a-hyprland-address"));
    }

    #[test]
    fn version_metadata_keeps_only_bounded_safe_fields() {
        let raw = r#"{
            "branch":"main",
            "tag":"v0.51.1",
            "commit":"0123456789abcdef0123456789abcdef01234567",
            "commit_message":"not exported"
        }"#;
        let parsed: RawVersion = serde_json::from_str(raw).expect("version JSON");
        let version = HyprlandVersion {
            branch: bounded_version_field(parsed.branch),
            tag: bounded_version_field(parsed.tag),
            commit: parsed.commit.filter(|value| {
                value.len() == 40 && value.chars().all(|character| character.is_ascii_hexdigit())
            }),
        };
        assert_eq!(version.branch.as_deref(), Some("main"));
        assert_eq!(version.tag.as_deref(), Some("v0.51.1"));
        assert_eq!(
            version.commit.as_deref(),
            Some("0123456789abcdef0123456789abcdef01234567")
        );

        let unsafe_version = RawVersion {
            branch: Some("main\nsecret".into()),
            tag: Some("x".repeat(MAX_VERSION_FIELD_BYTES + 1)),
            commit: Some("not-a-commit".into()),
        };
        assert!(bounded_version_field(unsafe_version.branch).is_none());
        assert!(bounded_version_field(unsafe_version.tag).is_none());
        assert!(unsafe_version.commit.is_some_and(|value| {
            value.len() != 40 || !value.chars().all(|character| character.is_ascii_hexdigit())
        }));
        assert!(
            HyprlandVersion {
                branch: Some("main".into()),
                tag: Some("v0.56.2".into()),
                commit: None,
            }
            .uses_lua_dispatch()
        );
        assert!(
            !HyprlandVersion {
                branch: Some("main".into()),
                tag: Some("v0.54.1".into()),
                commit: None,
            }
            .uses_lua_dispatch()
        );
        assert_eq!(parse_version_prefix("v0.55.0"), Some((0, 55)));
        assert_eq!(parse_version_prefix("main"), None);
    }
}
