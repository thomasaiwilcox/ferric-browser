//! Same-thread handoff from a materialized logical fallback to native freeze.

use super::{
    BrowserUiRust, NativeProfileOwner, OperationId, RetainedViewId, StashError, StashObservation,
    TabId, VersioningEffects,
};

/// The host may start only the exact `Ready` route after Rust has retained
/// the non-serializable stage ticket. A refused route carries effects that
/// must be delivered instead of touching the native source.
pub enum NativeStashFreezeStart {
    Ready {
        view: RetainedViewId,
        source: TabId,
        fallback: TabId,
    },
    Cancelled {
        view: RetainedViewId,
        previous_revision: u64,
        reason: StashError,
        effects: VersioningEffects,
    },
}

impl BrowserUiRust {
    /// Rechecks a fresh trusted activity observation, issues the application
    /// ticket and holds it in Rust before any native freeze request. The
    /// original view is still visible when this returns.
    ///
    /// # Errors
    /// Refuses a wrong operation/owner or a fallback not acknowledged by the
    /// native host. A failed logical rollback remains with application
    /// maintenance rather than being reported as a safe native refusal.
    pub fn begin_materialized_native_stash_freeze(
        &mut self,
        operation: OperationId,
        observation: StashObservation,
    ) -> Result<NativeStashFreezeStart, StashError> {
        let pending = self
            .native_stash_bridge
            .pending
            .as_ref()
            .filter(|pending| pending.operation == operation && pending.native_held)
            .ok_or(StashError::WrongStage)?;
        if self.native_stash_bridge.start.is_some()
            || self.native_stash_bridge.disclosure.is_some()
            || self.native_stash_bridge.stage.is_some()
            || self.native_stash_bridge.unstaged_cleanup.is_some()
        {
            return Err(StashError::WrongStage);
        }
        let owner = pending.preparation.owner();
        let window = self.window.ok_or(StashError::OwnerChanged)?;
        let (profile, instance) = self
            .native_versioning_profile
            .ok_or(StashError::OwnerChanged)?;
        let application = self.state.as_ref().ok_or(StashError::OwnerChanged)?;
        let native = NativeProfileOwner::new(application.application_owner(), profile, instance);
        if owner.window() != window
            || owner.native() != native
            || application.versioning().owner(profile) != Some(native)
            || application
                .windows()
                .get(&window)
                .is_none_or(|current| current.profile != profile)
        {
            return Err(StashError::OwnerChanged);
        }
        let previous_revision = application.revision();
        let (_, preparation) = self
            .native_stash_bridge
            .take_materialized_preparation()
            .ok_or(StashError::WrongStage)?;
        let view = preparation.view();
        self.native_stash_bridge.unstaged_cleanup = Some((operation, view));
        let application = Option::as_mut(&mut self.state).ok_or(StashError::OwnerChanged)?;
        match application.begin_explicit_stash_freeze(operation, preparation, observation) {
            Ok(stage) => {
                let route = NativeStashFreezeStart::Ready {
                    view: stage.view(),
                    source: stage.owner().target().tab,
                    fallback: stage.fallback().tab,
                };
                // This is one UI thread with no callback between the empty
                // slot check above and the ticket install. Never expose a
                // Ready route before the ticket is held.
                self.native_stash_bridge.unstaged_cleanup = None;
                self.native_stash_bridge.stage = Some(stage);
                Ok(route)
            }
            Err(reason) => {
                let effects = application
                    .cancel_versioning_operation(operation)
                    .map_err(|_| StashError::RollbackFailed)?;
                Ok(NativeStashFreezeStart::Cancelled {
                    view,
                    previous_revision,
                    reason,
                    effects,
                })
            }
        }
    }
}
