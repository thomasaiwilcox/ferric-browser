//! Bounded, read-only Hyprland event/snapshot observations.
//!
//! INVARIANT: compositor events and snapshots are observations only. They do
//! not grant workspace-routing or page-grouping authority.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

use super::hyprland::HyprlandVersion;

const MAX_EVENT_BATCH_BYTES: usize = 64 * 1024;
const MAX_EVENT_LINE_BYTES: usize = 2048;
const MAX_EVENTS_PER_BATCH: usize = 512;
pub(super) const MAX_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;
const MAX_CLIENTS: usize = 4096;
const MAX_GROUPS: usize = 2048;
const MAX_GROUP_MEMBERS: usize = 64;
const MAX_FIELD_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ObservationCapability {
    Available,
    Disabled,
    UnsupportedVersion,
    Unavailable,
    InvalidData,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ObservationResult {
    pub capability: ObservationCapability,
    pub state: Option<ObservedState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ObservedState {
    pub instance_epoch: u64,
    pub sequence: u64,
    pub version: HyprlandVersion,
    pub clients: Vec<ObservedClient>,
    pub groups: Vec<ObservedGroup>,
    pub active_address: Option<String>,
    pub workspace_order: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ObservedClient {
    pub address: String,
    pub generation: u64,
    pub class: String,
    pub initial_class: String,
    pub pid: Option<u32>,
    pub workspace: Option<String>,
    pub mapped: bool,
    pub focus_history_id: Option<u32>,
    pub group_addresses: Vec<String>,
}

/// Read-only identity token for rejecting stale observations across reconnects.
/// It is not a compositor command or a capability to mutate a window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ObservationHandle {
    pub instance_epoch: u64,
    pub address: String,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ObservedGroup {
    pub members: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum HyprlandEvent {
    OpenWindow {
        address: String,
        workspace: String,
        class: String,
    },
    CloseWindow {
        address: String,
    },
    MoveWindow {
        address: String,
        workspace: String,
    },
    ActiveWindow {
        address: Option<String>,
    },
    Workspace {
        id: i32,
        name: String,
    },
    FocusedWorkspace {
        monitor: String,
        workspace: String,
    },
    Ignored,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SequencedEvent {
    pub sequence: u64,
    pub event: HyprlandEvent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ObservationError {
    TooLarge,
    Malformed,
    OutOfOrder,
    InvalidIdentity,
}

#[derive(Debug, Deserialize)]
struct RawClient {
    address: String,
    #[serde(default)]
    class: String,
    #[serde(rename = "initialClass", default)]
    initial_class: String,
    pid: Option<u32>,
    workspace: Option<RawWorkspace>,
    #[serde(default = "default_mapped")]
    mapped: bool,
    #[serde(rename = "focusHistoryID")]
    focus_history_id: Option<u32>,
    #[serde(default)]
    grouped: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawWorkspace {
    name: String,
}

fn default_mapped() -> bool {
    true
}

#[derive(Clone, Debug)]
struct IdentityRecord {
    generation: u64,
    class: String,
    initial_class: String,
    pid: Option<u32>,
    group_addresses: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ObservationCache {
    instance: Option<String>,
    instance_epoch: u64,
    sequence: u64,
    next_generation: u64,
    identities: BTreeMap<String, IdentityRecord>,
    active_address: Option<String>,
    workspace_order: Vec<String>,
}

#[derive(Default)]
pub(super) struct ObservationSession {
    pub cache: ObservationCache,
    event_path: Option<PathBuf>,
    event_stream: Option<std::os::unix::net::UnixStream>,
    pending_event_bytes: Vec<u8>,
}

impl ObservationSession {
    pub fn disconnect(&mut self) {
        self.event_stream = None;
        self.event_path = None;
        self.pending_event_bytes.clear();
        self.cache.invalidate();
    }

    pub fn read_events(
        &mut self,
        path: Option<&PathBuf>,
    ) -> Result<Vec<SequencedEvent>, ObservationError> {
        if self.event_path.as_ref() != path {
            self.event_stream = None;
            self.pending_event_bytes.clear();
            self.cache.invalidate();
            self.event_path = path.cloned();
        }
        if self.event_stream.is_none() {
            let path = path.ok_or(ObservationError::InvalidIdentity)?;
            let stream = std::os::unix::net::UnixStream::connect(path)
                .map_err(|_| ObservationError::Malformed)?;
            stream
                .set_read_timeout(Some(Duration::from_millis(40)))
                .map_err(|_| ObservationError::Malformed)?;
            self.event_stream = Some(stream);
        }
        let Some(mut stream) = self.event_stream.take() else {
            return Err(ObservationError::Malformed);
        };
        let mut chunk = [0_u8; 4096];
        let mut total = 0_usize;
        loop {
            match std::io::Read::read(&mut stream, &mut chunk) {
                Ok(0) => {
                    self.event_stream = None;
                    self.pending_event_bytes.clear();
                    self.cache.invalidate();
                    return Err(ObservationError::Malformed);
                }
                Ok(count) => {
                    total = total.saturating_add(count);
                    if total > MAX_EVENT_BATCH_BYTES
                        || self.pending_event_bytes.len().saturating_add(count)
                            > MAX_EVENT_BATCH_BYTES
                    {
                        self.event_stream = None;
                        self.pending_event_bytes.clear();
                        self.cache.invalidate();
                        return Err(ObservationError::TooLarge);
                    }
                    self.pending_event_bytes.extend_from_slice(&chunk[..count]);
                    if total >= MAX_EVENT_BATCH_BYTES {
                        break;
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    break;
                }
                Err(_) => {
                    self.event_stream = None;
                    self.pending_event_bytes.clear();
                    self.cache.invalidate();
                    return Err(ObservationError::Malformed);
                }
            }
        }
        let complete = self
            .pending_event_bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |position| position + 1);
        let remainder = self.pending_event_bytes.split_off(complete);
        let batch = std::mem::replace(&mut self.pending_event_bytes, remainder);
        self.event_stream = Some(stream);
        let events = match parse_event_batch(&batch, self.cache.sequence()) {
            Ok(events) => events,
            Err(error) => {
                self.event_stream = None;
                self.pending_event_bytes.clear();
                self.cache.invalidate();
                return Err(error);
            }
        };
        if self.pending_event_bytes.len() > MAX_EVENT_LINE_BYTES {
            self.pending_event_bytes.clear();
            self.event_stream = None;
            self.cache.invalidate();
            return Err(ObservationError::TooLarge);
        }
        Ok(events)
    }
}

impl ObservationCache {
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn handle_for(&self, state: &ObservedState, address: &str) -> Option<ObservationHandle> {
        let client = state
            .clients
            .iter()
            .find(|client| client.address == address)?;
        Some(ObservationHandle {
            instance_epoch: state.instance_epoch,
            address: client.address.clone(),
            generation: client.generation,
        })
    }

    pub fn is_current(&self, handle: &ObservationHandle) -> bool {
        handle.instance_epoch == self.instance_epoch
            && self
                .identities
                .get(&handle.address)
                .is_some_and(|identity| identity.generation == handle.generation)
    }
    pub fn invalidate(&mut self) {
        self.instance = None;
        self.instance_epoch = self.instance_epoch.saturating_add(1);
        self.sequence = 0;
        self.identities.clear();
        self.active_address = None;
        self.workspace_order.clear();
    }

    pub fn observe(
        &mut self,
        instance: &str,
        version: HyprlandVersion,
        snapshot_bytes: &[u8],
        events: &[SequencedEvent],
    ) -> Result<ObservedState, ObservationError> {
        if instance.is_empty()
            || instance.len() > MAX_FIELD_BYTES
            || instance.chars().any(char::is_control)
        {
            self.invalidate();
            return Err(ObservationError::InvalidIdentity);
        }
        if self.instance.as_deref() != Some(instance) {
            self.instance_epoch = self.instance_epoch.saturating_add(1);
            self.sequence = 0;
            self.identities.clear();
            self.active_address = None;
            self.workspace_order.clear();
            self.instance = Some(instance.to_owned());
        }

        let snapshot = match parse_snapshot(snapshot_bytes) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.invalidate();
                return Err(error);
            }
        };
        let mut closed_before_snapshot = BTreeSet::new();
        if let Err(error) = self.accept_events(events, &mut closed_before_snapshot) {
            self.invalidate();
            return Err(error);
        }

        let observed = match self.install_snapshot(snapshot, &closed_before_snapshot) {
            Ok(observed) => observed,
            Err(error) => {
                self.invalidate();
                return Err(error);
            }
        };
        let groups = groups_from_clients(&self.identities);
        if groups.len() > MAX_GROUPS {
            self.invalidate();
            return Err(ObservationError::TooLarge);
        }
        Ok(ObservedState {
            instance_epoch: self.instance_epoch,
            sequence: self.sequence,
            version,
            clients: observed,
            groups,
            active_address: self.active_address.clone(),
            workspace_order: self.workspace_order.clone(),
        })
    }

    fn accept_events(
        &mut self,
        events: &[SequencedEvent],
        closed: &mut BTreeSet<String>,
    ) -> Result<(), ObservationError> {
        if events.len() > MAX_EVENTS_PER_BATCH {
            return Err(ObservationError::TooLarge);
        }
        for sequenced in events {
            if sequenced.sequence <= self.sequence {
                return Err(ObservationError::OutOfOrder);
            }
            self.sequence = sequenced.sequence;
            match &sequenced.event {
                HyprlandEvent::OpenWindow { address, .. }
                | HyprlandEvent::CloseWindow { address } => {
                    if matches!(&sequenced.event, HyprlandEvent::CloseWindow { .. }) {
                        self.identities.remove(address);
                        closed.insert(address.clone());
                    } else {
                        closed.insert(address.clone());
                    }
                }
                HyprlandEvent::ActiveWindow { address } => {
                    self.active_address = address.clone();
                }
                HyprlandEvent::Workspace { id, name } => {
                    let item = format!("{id}:{name}");
                    self.workspace_order.retain(|entry| entry != &item);
                    self.workspace_order.push(item);
                }
                HyprlandEvent::FocusedWorkspace { workspace, .. } => {
                    self.workspace_order.retain(|entry| entry != workspace);
                    self.workspace_order.push(workspace.clone());
                }
                HyprlandEvent::MoveWindow { address, workspace } => {
                    if let Some(identity) = self.identities.get_mut(address) {
                        let _ = workspace;
                        identity.generation = identity.generation.saturating_add(0);
                    }
                }
                HyprlandEvent::Ignored => {}
            }
        }
        Ok(())
    }

    fn install_snapshot(
        &mut self,
        snapshot: Vec<RawClient>,
        retired: &BTreeSet<String>,
    ) -> Result<Vec<ObservedClient>, ObservationError> {
        if snapshot.len() > MAX_CLIENTS {
            return Err(ObservationError::TooLarge);
        }
        let mut seen = BTreeSet::new();
        let mut output = Vec::with_capacity(snapshot.len());
        let mut next = BTreeMap::new();
        for client in snapshot {
            validate_address(&client.address)?;
            if !seen.insert(client.address.clone())
                || !valid_field(&client.class)
                || !valid_field(&client.initial_class)
                || client.grouped.len() > MAX_GROUP_MEMBERS
            {
                return Err(ObservationError::Malformed);
            }
            let workspace = client.workspace.map(|workspace| workspace.name);
            if workspace.as_ref().is_some_and(|value| !valid_field(value)) {
                return Err(ObservationError::Malformed);
            }
            for address in &client.grouped {
                validate_address(address)?;
            }
            let previous = self.identities.get(&client.address);
            let same_identity = previous.is_some_and(|previous| {
                !retired.contains(&client.address)
                    && previous.class == client.class
                    && previous.initial_class == client.initial_class
                    && previous.pid == client.pid
            });
            let generation = if same_identity {
                previous.map_or(0, |record| record.generation)
            } else {
                self.next_generation = self.next_generation.saturating_add(1).max(1);
                self.next_generation
            };
            let identity = IdentityRecord {
                generation,
                class: client.class.clone(),
                initial_class: client.initial_class.clone(),
                pid: client.pid,
                group_addresses: client.grouped.clone(),
            };
            next.insert(client.address.clone(), identity);
            output.push(ObservedClient {
                address: client.address,
                generation,
                class: client.class,
                initial_class: client.initial_class,
                pid: client.pid,
                workspace,
                mapped: client.mapped,
                focus_history_id: client.focus_history_id,
                group_addresses: client.grouped,
            });
        }
        output.sort_by(|left, right| {
            left.focus_history_id
                .unwrap_or(u32::MAX)
                .cmp(&right.focus_history_id.unwrap_or(u32::MAX))
                .then_with(|| left.address.cmp(&right.address))
        });
        for client in &output {
            if client
                .group_addresses
                .iter()
                .any(|address| !seen.contains(address))
            {
                return Err(ObservationError::InvalidIdentity);
            }
        }
        self.identities = next;
        if self
            .active_address
            .as_ref()
            .is_some_and(|active| !seen.contains(active))
        {
            self.active_address = None;
        }
        if self.active_address.is_none() {
            self.active_address = output
                .iter()
                .find(|client| client.focus_history_id == Some(0))
                .map(|client| client.address.clone());
        }
        self.workspace_order = output
            .iter()
            .filter_map(|client| client.workspace.clone())
            .fold(Vec::new(), |mut order, workspace| {
                if !order.contains(&workspace) {
                    order.push(workspace);
                }
                order
            });
        Ok(output)
    }
}

pub(super) fn parse_event_batch(
    bytes: &[u8],
    first_sequence: u64,
) -> Result<Vec<SequencedEvent>, ObservationError> {
    if bytes.len() > MAX_EVENT_BATCH_BYTES {
        return Err(ObservationError::TooLarge);
    }
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        return Err(ObservationError::Malformed);
    }
    let mut events = Vec::new();
    for (index, line) in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .enumerate()
    {
        if line.len() > MAX_EVENT_LINE_BYTES || index >= MAX_EVENTS_PER_BATCH {
            return Err(ObservationError::TooLarge);
        }
        let text = std::str::from_utf8(line).map_err(|_| ObservationError::Malformed)?;
        let (kind, payload) = text.split_once(">>").ok_or(ObservationError::Malformed)?;
        let event = parse_event(kind, payload)?;
        let offset = u64::try_from(index + 1).map_err(|_| ObservationError::TooLarge)?;
        events.push(SequencedEvent {
            sequence: first_sequence
                .checked_add(offset)
                .ok_or(ObservationError::TooLarge)?,
            event,
        });
    }
    Ok(events)
}

fn parse_event(kind: &str, payload: &str) -> Result<HyprlandEvent, ObservationError> {
    let mut fields = payload.split(',');
    match kind {
        "openwindow" => {
            let address = normalized_address(fields.next().ok_or(ObservationError::Malformed)?)?;
            let workspace = bounded_field(fields.next().ok_or(ObservationError::Malformed)?)?;
            let _workspace_rule = fields.next().ok_or(ObservationError::Malformed)?;
            let class = bounded_field(fields.next().ok_or(ObservationError::Malformed)?)?;
            Ok(HyprlandEvent::OpenWindow {
                address,
                workspace,
                class,
            })
        }
        "closewindow" => Ok(HyprlandEvent::CloseWindow {
            address: normalized_address(payload)?,
        }),
        "movewindow" => Ok(HyprlandEvent::MoveWindow {
            address: normalized_address(fields.next().ok_or(ObservationError::Malformed)?)?,
            workspace: bounded_field(fields.next().ok_or(ObservationError::Malformed)?)?,
        }),
        "activewindowv2" => {
            let address = if payload.is_empty() {
                None
            } else {
                Some(normalized_address(payload)?)
            };
            Ok(HyprlandEvent::ActiveWindow { address })
        }
        "workspacev2" => {
            let id = fields
                .next()
                .ok_or(ObservationError::Malformed)?
                .parse()
                .map_err(|_| ObservationError::Malformed)?;
            Ok(HyprlandEvent::Workspace {
                id,
                name: bounded_field(fields.next().ok_or(ObservationError::Malformed)?)?,
            })
        }
        "focusedmon" => Ok(HyprlandEvent::FocusedWorkspace {
            monitor: bounded_field(fields.next().ok_or(ObservationError::Malformed)?)?,
            workspace: bounded_field(fields.next().ok_or(ObservationError::Malformed)?)?,
        }),
        _ => Ok(HyprlandEvent::Ignored),
    }
}

fn parse_snapshot(bytes: &[u8]) -> Result<Vec<RawClient>, ObservationError> {
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err(ObservationError::TooLarge);
    }
    let clients: Vec<RawClient> =
        serde_json::from_slice(bytes).map_err(|_| ObservationError::Malformed)?;
    if clients.len() > MAX_CLIENTS {
        return Err(ObservationError::TooLarge);
    }
    Ok(clients)
}

fn groups_from_clients(identities: &BTreeMap<String, IdentityRecord>) -> Vec<ObservedGroup> {
    let mut unique = BTreeSet::new();
    for (address, identity) in identities {
        if identity.group_addresses.is_empty() {
            continue;
        }
        let mut members = identity.group_addresses.clone();
        members.push(address.clone());
        members.sort();
        members.dedup();
        unique.insert(members);
    }
    unique
        .into_iter()
        .map(|members| ObservedGroup { members })
        .collect()
}

fn normalized_address(value: &str) -> Result<String, ObservationError> {
    let value = value.strip_prefix("0x").unwrap_or(value);
    if value.is_empty()
        || value.len() > MAX_FIELD_BYTES - 2
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ObservationError::InvalidIdentity);
    }
    Ok(format!("0x{value}"))
}

fn validate_address(value: &str) -> Result<(), ObservationError> {
    normalized_address(value).map(|_| ())
}

fn valid_field(value: &str) -> bool {
    value.len() <= MAX_FIELD_BYTES && !value.chars().any(char::is_control)
}

fn bounded_field(value: &str) -> Result<String, ObservationError> {
    if valid_field(value) {
        Ok(value.to_owned())
    } else {
        Err(ObservationError::Malformed)
    }
}

#[cfg(test)]
#[path = "tests/hyprland_groups.rs"]
mod hyprland_groups;
