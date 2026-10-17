//! Qt-owned transient state shapes shared by browser presentation modules.

use super::{
    BrowserApplication, BrowserUiRust, ConfigurationLayers, ProfileOpenEffect, RuntimeOverrides,
    TabId, Value,
};
use std::{path::PathBuf, time::Instant};

#[derive(Clone, Debug)]
pub(super) struct PendingContextRoute {
    pub(super) route_id: String,
    pub(super) behavior: String,
    pub(super) context_name: String,
    pub(super) profile_name: String,
    pub(super) url: String,
    pub(super) new_window: bool,
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
    pub(super) context_catalog_revision: u64,
    pub(super) private_profile: bool,
    pub(super) profile_name: String,
    pub(super) storage_base: String,
    pub(super) state: BrowserApplication,
    pub(super) window: super::WindowId,
    pub(super) tab: TabId,
}

pub(super) struct PendingSelectorValidation {
    pub(super) generation: u64,
    pub(super) expected_revision: u64,
    pub(super) started_at: Instant,
    pub(super) fingerprint: String,
    pub(super) action: PendingSelectorAction,
}

pub(super) enum PendingSelectorAction {
    RuntimeConfig {
        candidate: Value,
        layers: ConfigurationLayers,
        status: String,
        persistent_overrides: RuntimeOverrides,
        persist_path: Option<PathBuf>,
    },
    Reload {
        candidate: Value,
        layers: ConfigurationLayers,
        status: String,
        path: PathBuf,
        base_json: String,
        profile_overrides: RuntimeOverrides,
        sources: Vec<PathBuf>,
        profiles_path: Option<PathBuf>,
    },
    Profile {
        pending: Box<PendingProfileConfiguration>,
        effect: Box<ProfileOpenEffect>,
    },
    Check {
        source_count: usize,
    },
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
    pub(super) generation: u64,
}

impl SessionCheckpoint {
    pub(super) fn mark_dirty(&mut self, now: u64) {
        self.generation = self.generation.wrapping_add(1);
        self.dirty = true;
        self.dirty_since_ms.get_or_insert(now);
    }

    pub(super) fn saved(&mut self, generation: u64, now: u64) {
        self.last_saved_ms = now;
        if self.generation == generation {
            self.dirty = false;
            self.dirty_since_ms = None;
            self.restore_pending = None;
        } else if self.dirty {
            // Coalesce newer changes instead of retrying every polling tick
            // after a slow save finishes during continuous scrolling.
            self.dirty_since_ms = Some(now);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SessionSaveIntent {
    Checkpoint { generation: u64 },
    Named,
}

#[cfg(test)]
mod checkpoint_tests {
    use super::SessionCheckpoint;

    #[test]
    fn completing_an_older_snapshot_keeps_newer_changes_dirty() {
        let mut checkpoint = SessionCheckpoint::default();
        checkpoint.mark_dirty(10);
        let saving = checkpoint.generation;
        checkpoint.mark_dirty(20);
        checkpoint.saved(saving, 30);
        assert!(checkpoint.dirty);
        assert_eq!(checkpoint.dirty_since_ms, Some(30));
        assert_eq!(checkpoint.last_saved_ms, 30);
        checkpoint.saved(checkpoint.generation, 40);
        assert!(!checkpoint.dirty);
        assert_eq!(checkpoint.dirty_since_ms, None);
        assert_eq!(checkpoint.last_saved_ms, 40);
    }
}
