//! Qt-owned transient state shapes shared by browser presentation modules.

use super::{BrowserApplication, BrowserUiRust, TabId, Uuid, Value};

#[derive(Clone, Debug)]
pub(super) struct PendingContextRoute {
    pub(super) route_id: String,
    pub(super) behavior: String,
    pub(super) context_name: String,
    pub(super) profile_name: String,
    pub(super) url: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum PermissionLifetimeKey {
    ProfileSession,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct PermissionDecisionKey {
    pub(super) scope_id: Uuid,
    pub(super) origin: String,
    pub(super) permission: String,
    pub(super) lifetime: PermissionLifetimeKey,
}

pub(super) fn permission_session_key(
    scope_id: Uuid,
    origin: &str,
    permission: &str,
) -> PermissionDecisionKey {
    PermissionDecisionKey {
        scope_id,
        origin: origin.to_owned(),
        permission: permission.to_owned(),
        lifetime: PermissionLifetimeKey::ProfileSession,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum ProfilePersistence {
    #[default]
    Unavailable,
    Transient,
    Durable,
}

impl ProfilePersistence {
    pub(super) const fn is_durable(self) -> bool {
        matches!(self, Self::Durable)
    }

    pub(super) const fn lacks_durable_storage(self) -> bool {
        !self.is_durable()
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct ContextSnapshot {
    records: Vec<ferric_browser_storage::ContextRecord>,
}

impl ContextSnapshot {
    pub(super) fn from_records(records: &[ferric_browser_storage::ContextRecord]) -> Self {
        Self {
            records: records.to_vec(),
        }
    }

    pub(super) fn contexts(&self) -> &[ferric_browser_storage::ContextRecord] {
        &self.records
    }
}

pub(super) struct PendingProfileConfiguration {
    pub(super) private_profile: bool,
    pub(super) profile_name: String,
    pub(super) storage_base: String,
    pub(super) state: BrowserApplication,
    pub(super) window: super::WindowId,
    pub(super) tab: TabId,
}

#[derive(Clone, Debug)]
pub(super) struct SwitcherQueryCache {
    pub(super) params: Value,
    pub(super) state_revision: u64,
    pub(super) storage_library_revision: i64,
    pub(super) session_names: Vec<String>,
    pub(super) contexts_json: String,
    pub(super) config_fingerprint: String,
    pub(super) profile_name: String,
    pub(super) result: Value,
}

impl SwitcherQueryCache {
    pub(super) fn matches(&self, rust: &BrowserUiRust, params: &Value) -> bool {
        self.params == *params
            && !rust.storage_library_dirty.get()
            && self.state_revision == rust.state.as_ref().map_or(0, BrowserApplication::revision)
            && self.storage_library_revision == rust.storage_library_revision
            && self.session_names == rust.session_names
            && self.contexts_json == rust.contexts_json.to_string()
            && self.config_fingerprint == serde_json::to_string(&rust.config).unwrap_or_default()
            && self.profile_name == rust.profile_name
    }
}

#[derive(Debug, Default)]
pub(super) struct SessionCheckpoint {
    pub(super) dirty: bool,
    pub(super) dirty_since_ms: Option<u64>,
    pub(super) last_saved_ms: u64,
    pub(super) restore_pending: Option<TabId>,
}
