//! Exact native-hold cleanup before any freeze ticket was issued.

use super::{
    BrowserApplication, CxxQtType, NativeStashBridge, OperationState, Pin, QString, RetainedViewId,
    StashError, VersioningEffects, qobject,
};

impl NativeStashBridge {
    pub(super) fn mark_native_hold(&mut self, view: RetainedViewId) -> bool {
        let Some(pending) = self.pending.as_mut().filter(|pending| {
            pending.materialized && !pending.native_held && pending.preparation.view() == view
        }) else {
            return false;
        };
        if self.stage.is_some() || self.unstaged_cleanup.is_some() {
            return false;
        }
        pending.native_held = true;
        true
    }

    /// A materialized reservation owns a native `QObject` witness. Terminal
    /// operation cleanup cannot discard that witness before the host observes
    /// its original view at `Active` and acknowledges restoration.
    pub(super) fn pending_cleanup_view(
        &self,
        application: &BrowserApplication,
    ) -> Option<RetainedViewId> {
        let (operation, view) = if let Some(pending) = self.pending.as_ref() {
            if !pending.materialized {
                return None;
            }
            (pending.operation, pending.preparation.view())
        } else {
            self.unstaged_cleanup?
        };
        match application.versioning().operation(operation) {
            Some((_, state))
                if !matches!(
                    state,
                    OperationState::Cancelled
                        | OperationState::Failed
                        | OperationState::Partial
                        | OperationState::OutcomeUnknown
                ) =>
            {
                None
            }
            Some(_) | None => Some(view),
        }
    }

    /// The QML host has already checked the exact source `QObject`, original
    /// presenter and observed `Active`. Failed logical cleanup keeps this
    /// bridge entry so the native restoring entry can retry later.
    pub(super) fn acknowledge_held_restored(
        &mut self,
        application: &mut BrowserApplication,
        view: RetainedViewId,
    ) -> Result<VersioningEffects, StashError> {
        let (operation, expected) = if let Some(pending) = self.pending.as_ref() {
            if !pending.materialized {
                return Err(StashError::WrongStage);
            }
            (pending.operation, pending.preparation.view())
        } else {
            self.unstaged_cleanup.ok_or(StashError::UnknownOperation)?
        };
        if expected != view {
            return Err(StashError::StalePreparation);
        }
        if self.pending_cleanup_view(application) != Some(view) {
            return Err(StashError::WrongStage);
        }
        let runtime_effects = application.acknowledge_unstaged_stash_rollback(operation, view)?;
        self.pending = None;
        self.unstaged_cleanup = None;
        Ok(VersioningEffects {
            runtime_effects,
            ..VersioningEffects::default()
        })
    }
}

impl qobject::BrowserUi {
    pub(crate) fn has_native_stash_preparation(self: Pin<&mut Self>) -> bool {
        let pinned = self.as_ref();
        let rust = pinned.rust();
        let bridge = &rust.native_stash_bridge;
        bridge.pending_operation().is_some() || bridge.unstaged_cleanup.is_some()
    }

    /// QML invokes this only after its exact host has installed the native
    /// source-view hold. It is an ordering acknowledgement, not a freeze or
    /// categorical activity observation.
    pub(crate) fn acknowledge_native_stash_hold_installed(
        mut self: Pin<&mut Self>,
        view_id: &QString,
    ) -> bool {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let Some(pending) = this.native_stash_bridge.pending.as_ref() else {
            return false;
        };
        let view = pending.preparation.view();
        view.to_string() == view_id.to_string() && this.native_stash_bridge.mark_native_hold(view)
    }

    /// A post-hold acknowledgement refusal cancels the logical reservation
    /// but keeps the exact native cleanup route until observed restoration.
    pub(crate) fn abort_native_stash_held_preparation(
        mut self: Pin<&mut Self>,
        view_id: &QString,
    ) -> bool {
        let (previous_revision, effects) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(pending) = this.native_stash_bridge.pending.as_ref() else {
                return false;
            };
            if !pending.materialized
                || pending.preparation.view().to_string() != view_id.to_string()
            {
                return false;
            }
            let Some(application) = Option::as_mut(&mut this.state) else {
                return false;
            };
            let previous_revision = application.revision();
            let Ok(effects) = application.abort_explicit_stash_preparation(pending.operation)
            else {
                return false;
            };
            (previous_revision, effects)
        };
        self.as_mut()
            .accept_native_stash_effects(previous_revision, effects);
        true
    }
}
