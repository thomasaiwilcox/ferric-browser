//! Explicitly allowlisted, revision-checked configuration capture and restore.
//!
//! The persisted schema intentionally contains one declarative value today.
//! It never transports permission grants, trust exceptions, paths, or whole
//! configuration documents.

use std::{
    fmt,
    time::{Duration, Instant},
};

use ferric_browser_config::SettingRegistry;
use ferric_browser_core::{
    CommitId, NativeProfileOwner, VersioningFailure, VersioningReason, WindowId,
};
use ferric_browser_storage::versioning::{ManifestPolicySnapshot, VersionedCaptureManifest};
use uuid::Uuid;

use crate::{BrowserApplication, ConfigurationSnapshot};

pub const ALLOWLISTED_POLICY_SCHEMA_SHA256: &str =
    "3264959c4cb3dddc529ec17fc2103bef6f53e6775c657a8e40fa75404d2dfcc2";
const POLICY_SETTING: &str = "content.javascript";
const POLICY_PREVIEW_LIFETIME: Duration = Duration::from_secs(60);

/// One safe-to-display, explicitly allowlisted policy value comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicySettingChange {
    pub key: &'static str,
    pub current_enabled: bool,
    pub captured_enabled: bool,
}

/// A short-lived typed preview. Private owner and captured-value fields bind
/// application to the exact active profile, setting, and configuration epoch.
#[derive(Clone, Eq, PartialEq)]
pub struct PolicyRestorePreview {
    pub commit: CommitId,
    pub configuration_revision: u64,
    pub change: PolicySettingChange,
    source_window: WindowId,
    profile_uuid: Uuid,
    native_owner: NativeProfileOwner,
    schema_sha256: String,
    created_at: Instant,
}

impl fmt::Debug for PolicyRestorePreview {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PolicyRestorePreview")
            .field("commit", &"[redacted]")
            .field("configuration_revision", &self.configuration_revision)
            .field("change", &self.change)
            .finish_non_exhaustive()
    }
}

impl PolicyRestorePreview {
    pub(crate) fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= POLICY_PREVIEW_LIFETIME
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum PolicyRestoreEvent {
    PreviewReady {
        request_id: u64,
        preview: PolicyRestorePreview,
    },
    Failed {
        request_id: u64,
    },
}

impl fmt::Debug for PolicyRestoreEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PreviewReady { preview, .. } => formatter
                .debug_struct("PolicyRestoreEvent::PreviewReady")
                .field("request_id", &"[redacted]")
                .field("preview", preview)
                .finish(),
            Self::Failed { .. } => formatter
                .debug_struct("PolicyRestoreEvent::Failed")
                .field("request_id", &"[redacted]")
                .finish(),
        }
    }
}

impl BrowserApplication {
    /// Captures the current sole allowlisted setting for a new commit.
    #[must_use]
    pub fn allowlisted_policy_snapshot(&self) -> ManifestPolicySnapshot {
        ManifestPolicySnapshot {
            schema_sha256: ALLOWLISTED_POLICY_SCHEMA_SHA256.to_owned(),
            javascript_enabled: self.configuration().content.javascript,
        }
    }

    /// Compares an exact validated manifest policy snapshot with current
    /// configuration. This is read-only and does not apply any setting.
    ///
    /// # Errors
    /// Rejects a wrong owner, missing/obsolete policy schema, and unsupported
    /// or sensitive keys.
    pub fn preview_allowlisted_policy_restore(
        &self,
        source_window: WindowId,
        manifest: &VersionedCaptureManifest,
    ) -> Result<PolicyRestorePreview, VersioningFailure> {
        let (profile_uuid, native_owner) = self
            .structural_commit_owner(source_window)
            .map_err(|_| policy_failure(VersioningReason::StaleOwner))?;
        if manifest.profile_uuid != profile_uuid || manifest.validate().is_err() {
            return Err(policy_failure(VersioningReason::WrongProfile));
        }
        let snapshot = manifest
            .policy_snapshot
            .as_ref()
            .ok_or_else(|| policy_failure(VersioningReason::UnsupportedCapability))?;
        if snapshot.schema_sha256 != ALLOWLISTED_POLICY_SCHEMA_SHA256
            || !is_current_policy_allowlisted()
        {
            return Err(policy_failure(VersioningReason::UnsupportedCapability));
        }
        let commit = manifest
            .id
            .parse::<CommitId>()
            .map_err(|_| policy_failure(VersioningReason::UnsafeRestoreDescriptor))?;
        Ok(PolicyRestorePreview {
            commit,
            configuration_revision: self.configuration_revision(),
            change: PolicySettingChange {
                key: POLICY_SETTING,
                current_enabled: self.configuration().content.javascript,
                captured_enabled: snapshot.javascript_enabled,
            },
            source_window,
            profile_uuid,
            native_owner,
            schema_sha256: snapshot.schema_sha256.clone(),
            created_at: Instant::now(),
        })
    }

    /// Applies only the displayed allowlisted value through normal runtime
    /// override validation. The preview is consumed and rejected after 60
    /// seconds, owner change, schema drift, or any intervening config update.
    /// The caller must invoke this only after native user confirmation.
    ///
    /// # Errors
    /// Refuses stale/expired previews or configuration validation failures.
    pub(crate) fn apply_allowlisted_policy_restore(
        &mut self,
        preview: &PolicyRestorePreview,
    ) -> Result<ConfigurationSnapshot, VersioningFailure> {
        if preview.is_expired() {
            return Err(policy_failure(VersioningReason::ConfirmationExpired));
        }
        let (profile_uuid, native_owner) = self
            .structural_commit_owner(preview.source_window)
            .map_err(|_| policy_failure(VersioningReason::StaleOwner))?;
        if profile_uuid != preview.profile_uuid
            || native_owner != preview.native_owner
            || self.configuration_revision() != preview.configuration_revision
            || preview.schema_sha256 != ALLOWLISTED_POLICY_SCHEMA_SHA256
            || preview.change.key != POLICY_SETTING
            || !is_current_policy_allowlisted()
        {
            return Err(policy_failure(VersioningReason::StaleOwner));
        }
        if self.configuration().content.javascript != preview.change.current_enabled {
            return Err(policy_failure(VersioningReason::StaleOwner));
        }
        self.configuration
            .apply_versioned_javascript_policy(
                preview.change.captured_enabled,
                preview.configuration_revision,
            )
            .map_err(|_| policy_failure(VersioningReason::UnsupportedCapability))
    }

    /// Consumes the exact preview returned by an asynchronous policy request.
    /// The native caller must call this only from its explicit confirmation
    /// action, then may queue the ordinary page restore separately.
    ///
    /// # Errors
    /// Refuses a stale owner or invalid preview and propagates configuration validation failure.
    pub fn confirm_allowlisted_policy_restore(
        &mut self,
        source_window: WindowId,
        request_id: u64,
    ) -> Result<(CommitId, ConfigurationSnapshot), VersioningFailure> {
        // INVARIANT: a request ID cannot be used by another window to consume
        // or apply a preview bound to the original window's native owner.
        if self
            .pending_policy_restore_previews
            .get(&request_id)
            .is_some_and(|preview| preview.source_window != source_window)
        {
            return Err(policy_failure(VersioningReason::StaleOwner));
        }
        let preview = self
            .pending_policy_restore_previews
            .remove(&request_id)
            .ok_or_else(|| policy_failure(VersioningReason::ConfirmationRequired))?;
        if preview.source_window != source_window {
            return Err(policy_failure(VersioningReason::StaleOwner));
        }
        let commit = preview.commit;
        let snapshot = self.apply_allowlisted_policy_restore(&preview)?;
        Ok((commit, snapshot))
    }

    /// Drops one displayed policy preview only from its originating window,
    /// without changing current config.
    pub fn cancel_allowlisted_policy_restore(
        &mut self,
        source_window: WindowId,
        request_id: u64,
    ) -> bool {
        if self
            .pending_policy_restore_previews
            .get(&request_id)
            .is_none_or(|preview| preview.source_window != source_window)
        {
            return false;
        }
        self.pending_policy_restore_previews
            .remove(&request_id)
            .is_some()
    }
}

fn is_current_policy_allowlisted() -> bool {
    // INVARIANT: restore remains fail-closed if the registry changes the
    // allowlisted key's type, sensitivity, application boundary, or backend.
    SettingRegistry::default_v1()
        .resolve(POLICY_SETTING)
        .is_some_and(|setting| {
            setting.key == POLICY_SETTING
                && setting.sensitivity == "normal"
                && setting.value_type == "boolean"
                && setting.apply_time == "navigation"
                && setting.prerequisite == "qt-webengine"
        })
}

fn policy_failure(reason: VersioningReason) -> VersioningFailure {
    VersioningFailure::new(
        reason,
        ferric_browser_core::versioning::SensitiveCategory::None,
    )
}
