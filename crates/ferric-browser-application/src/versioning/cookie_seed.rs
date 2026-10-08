//! Prepared-child cookie seeding. Only a qualified exact backend may release
//! the child; public Qt observations cannot satisfy this contract.

use std::fmt;

use ferric_browser_core::versioning::{
    CapabilityFingerprint, CookieExpiry, CookieObservation, CookieRecord, CookieScope,
    CookieSnapshot, NativeProfileOwner,
};

use super::cookies::{CookieBackendCapabilities, CookieReadResult};

const MAX_SEED_RECORDS: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CookieSeedError {
    CookieSnapshotUnavailable,
    DestinationNotPristine,
    DestinationOwnerMismatch,
    CookieSeedUnavailable,
    ExpiredRecord,
    NonPresentRecord,
    TooManyRecords,
    Cancelled,
    CookieSeedFailed,
    CookieSeedUnverified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CookieSeedDispatchError {
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CookieSeedReadError {
    Unavailable,
}

/// A write being queued is not a commit. Only `Seeded` makes the owned child
/// available to navigation; every other result has destroyed the child.
pub enum CookieSeedOutcome<D> {
    Seeded(D),
    Failed(CookieSeedError),
    Unverified(CookieSeedError),
}

impl<D> fmt::Debug for CookieSeedOutcome<D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Seeded(_) => formatter.write_str("Seeded([destination redacted])"),
            Self::Failed(reason) => formatter.debug_tuple("Failed").field(reason).finish(),
            Self::Unverified(reason) => formatter.debug_tuple("Unverified").field(reason).finish(),
        }
    }
}

/// Read-back is explicitly bound to the native destination owner. The read
/// must be a new, exact, complete read from that owner after writes settle.
pub struct CookieDestinationRead {
    pub owner: NativeProfileOwner,
    pub read: CookieReadResult,
}

/// A destination implementation owns an unexposed prepared profile. Its
/// `dispatch_cookie` must preserve raw name/value, host-only provenance,
/// partition and all supported attributes. The supplied origin is exact and
/// must never be replaced by the current tab URL. Dispatch success alone is
/// not a write acknowledgement. `read_back` must never promote a callback
/// stream, timeout or empty rescan to an exact read.
pub trait CookieSeedDestination: Sized {
    fn owner(&self) -> NativeProfileOwner;
    fn pristine_and_unnavigated(&self) -> bool;
    fn still_unnavigated(&self) -> bool;
    fn capabilities(&self) -> &CookieBackendCapabilities;
    fn engine_fingerprint(&self) -> &CapabilityFingerprint;
    fn cancelled(&self) -> bool;
    /// # Errors
    /// The backend rejected or could not dispatch the exact record.
    fn dispatch_cookie(
        &mut self,
        record: &CookieRecord,
        origin: &str,
    ) -> Result<(), CookieSeedDispatchError>;
    /// # Errors
    /// A fresh, exact destination read could not be established.
    fn read_back(&mut self) -> Result<CookieDestinationRead, CookieSeedReadError>;
    fn destroy(self);
}

/// In-memory plan, never serializable or printable with cookie material.
pub struct CookieSeedPlan {
    expected: CookieSnapshot,
    writes: Vec<(CookieRecord, String)>,
    scope: CookieScope,
}

impl fmt::Debug for CookieSeedPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CookieSeedPlan")
            .field("write_count", &self.writes.len())
            .finish_non_exhaustive()
    }
}

impl CookieSeedPlan {
    /// # Errors
    /// Refuses any incomplete source, unsupported destination, expired or
    /// non-present record, or unbounded seed. This cannot qualify public Qt.
    pub fn new(
        source: &CookieReadResult,
        source_capabilities: &CookieBackendCapabilities,
        destination_capabilities: &CookieBackendCapabilities,
        engine: &CapabilityFingerprint,
        scope: &CookieScope,
        now_unix_ms: i64,
    ) -> Result<Self, CookieSeedError> {
        let snapshot = source
            .try_consistent_snapshot(source_capabilities, engine, scope)
            .map_err(|_| CookieSeedError::CookieSnapshotUnavailable)?;
        if !destination_capabilities.supports_seed_destination(source_capabilities, engine, scope) {
            return Err(CookieSeedError::CookieSeedUnavailable);
        }
        if snapshot.len() > MAX_SEED_RECORDS {
            return Err(CookieSeedError::TooManyRecords);
        }
        let mut writes = Vec::with_capacity(snapshot.len());
        for observation in snapshot.observations() {
            let CookieObservation::Present(record) = observation else {
                return Err(CookieSeedError::NonPresentRecord);
            };
            if matches!(record.expiry(), CookieExpiry::AtUnixMillis(at) if at <= now_unix_ms) {
                return Err(CookieSeedError::ExpiredRecord);
            }
            // The key has already passed core domain canonicalization. HTTPS
            // is a safe explicit origin for secure and non-secure cookies.
            let origin = format!("https://{}", record.key().canonical_domain());
            writes.push((record.clone(), origin));
        }
        Ok(Self {
            expected: snapshot.clone(),
            writes,
            scope: scope.clone(),
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.writes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.writes.is_empty()
    }
}

/// Consumes the prepared destination. All failure paths destroy it, so a
/// partial child cannot escape to navigation or become a clean branch.
pub fn seed_prepared_destination<D: CookieSeedDestination>(
    source_owner: NativeProfileOwner,
    source: &CookieReadResult,
    source_capabilities: &CookieBackendCapabilities,
    scope: &CookieScope,
    now_unix_ms: i64,
    mut destination: D,
) -> CookieSeedOutcome<D> {
    if source_owner == destination.owner() {
        destination.destroy();
        return CookieSeedOutcome::Failed(CookieSeedError::DestinationOwnerMismatch);
    }
    if !destination.pristine_and_unnavigated() {
        destination.destroy();
        return CookieSeedOutcome::Failed(CookieSeedError::DestinationNotPristine);
    }
    let plan = match CookieSeedPlan::new(
        source,
        source_capabilities,
        destination.capabilities(),
        destination.engine_fingerprint(),
        scope,
        now_unix_ms,
    ) {
        Ok(plan) => plan,
        Err(reason) => {
            destination.destroy();
            return CookieSeedOutcome::Failed(reason);
        }
    };
    for (record, origin) in &plan.writes {
        if destination.cancelled() {
            destination.destroy();
            return CookieSeedOutcome::Failed(CookieSeedError::Cancelled);
        }
        if !destination.still_unnavigated() {
            destination.destroy();
            return CookieSeedOutcome::Unverified(CookieSeedError::CookieSeedUnverified);
        }
        if destination.dispatch_cookie(record, origin).is_err() {
            destination.destroy();
            return CookieSeedOutcome::Failed(CookieSeedError::CookieSeedFailed);
        }
    }
    if destination.cancelled() {
        destination.destroy();
        return CookieSeedOutcome::Failed(CookieSeedError::Cancelled);
    }
    if !destination.still_unnavigated() {
        destination.destroy();
        return CookieSeedOutcome::Unverified(CookieSeedError::CookieSeedUnverified);
    }
    let Ok(read_back) = destination.read_back() else {
        destination.destroy();
        return CookieSeedOutcome::Unverified(CookieSeedError::CookieSeedUnverified);
    };
    let verified = read_back.owner == destination.owner()
        && !destination.cancelled()
        && destination.still_unnavigated()
        && read_back.read.interval().started_ms() >= source.interval().finished_ms()
        && read_back
            .read
            .try_consistent_snapshot(
                destination.capabilities(),
                destination.engine_fingerprint(),
                &plan.scope,
            )
            .is_ok_and(|actual| actual == &plan.expected);
    if !verified {
        destination.destroy();
        return CookieSeedOutcome::Unverified(CookieSeedError::CookieSeedUnverified);
    }
    CookieSeedOutcome::Seeded(destination)
}
