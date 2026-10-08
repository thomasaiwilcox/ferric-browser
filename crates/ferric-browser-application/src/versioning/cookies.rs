//! Application-owned cookie backend contract. Native observations are not
//! snapshots merely because they can be enumerated or held in memory.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use ferric_browser_core::versioning::{
    BackendAttributeName, CapabilityFingerprint, CookieKey, CookieModelError, CookieObservation,
    CookieRecord, CookieScope, CookieSnapshot, CookieSnapshotMeta, EvidenceId, NativeProfileOwner,
    SafeReasonCode, SnapshotCompleteness, SnapshotConsistency, SnapshotIdentityFidelity,
};
use serde::Serialize;
use uuid::Uuid;

const MAX_EVIDENCE_IDS: usize = 16;
const MAX_REVISION_TOKEN_BYTES: usize = 128;
// Each core record is individually bounded below 49 KiB, so 256 records
// remain below the core's 16 MiB snapshot material limit even at maximum size.
const MAX_MIRROR_RECORDS: usize = 256;

/// These separate claims must never be inferred from a single successful
/// cookie callback. Positive claims are scope-specific and certificate-owned.
#[derive(Clone, Eq, PartialEq)]
pub enum IdentityCapability {
    Lossless(CookieScope),
    Lossy(SafeReasonCode),
}

impl fmt::Debug for IdentityCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Lossless(_) => "Lossless([scope redacted])",
            Self::Lossy(_) => "Lossy([reason redacted])",
        })
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum SnapshotCapability {
    Consistent(CookieScope),
    ObservedOnly,
    Unavailable,
}

impl fmt::Debug for SnapshotCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Consistent(_) => "Consistent([scope redacted])",
            Self::ObservedOnly => "ObservedOnly",
            Self::Unavailable => "Unavailable",
        })
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum ReadBackCapability {
    Exact(CookieScope),
    ObservedOnly,
    Unavailable,
}

impl fmt::Debug for ReadBackCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Exact(_) => "Exact([scope redacted])",
            Self::ObservedOnly => "ObservedOnly",
            Self::Unavailable => "Unavailable",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteCapability {
    IndividualAsync,
    VerifiedBatch,
    Unavailable,
}

#[derive(Clone, Eq, PartialEq)]
pub enum ExclusiveApplyCapability {
    Supported(CookieScope),
    Unavailable,
}

impl fmt::Debug for ExclusiveApplyCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Supported(_) => "Supported([scope redacted])",
            Self::Unavailable => "Unavailable",
        })
    }
}

/// Application-side facts about one exact backend build. Fields are private:
/// the public Qt constructor can never assert snapshot or identity fidelity.
#[derive(Clone, Eq, PartialEq)]
pub struct CookieBackendCapabilities {
    engine_fingerprint: CapabilityFingerprint,
    backend_identity_version: u32,
    required_identity_dimensions: BTreeSet<Vec<u8>>,
    supported_attributes: BTreeSet<BackendAttributeName>,
    identity: IdentityCapability,
    snapshot: SnapshotCapability,
    read_back: ReadBackCapability,
    writes: WriteCapability,
    exclusive_apply: ExclusiveApplyCapability,
    evidence_ids: Vec<EvidenceId>,
}

impl fmt::Debug for CookieBackendCapabilities {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CookieBackendCapabilities")
            .field("identity", &self.identity)
            .field("snapshot", &self.snapshot)
            .field("read_back", &self.read_back)
            .field("writes", &self.writes)
            .field("exclusive_apply", &self.exclusive_apply)
            .finish_non_exhaustive()
    }
}

impl CookieBackendCapabilities {
    /// Public Qt yields a diagnostic observation stream. A known fresh
    /// fixture profile does not strengthen the general-profile contract.
    ///
    /// # Panics
    /// Only if the fixed, compile-time-safe reason or evidence token is
    /// rejected by the core token validator.
    #[must_use]
    pub fn qt_public(engine_fingerprint: CapabilityFingerprint) -> Self {
        Self {
            engine_fingerprint,
            backend_identity_version: 1,
            required_identity_dimensions: BTreeSet::new(),
            supported_attributes: BTreeSet::new(),
            identity: IdentityCapability::Lossy(
                SafeReasonCode::try_from("partition-identity-unavailable")
                    .expect("fixed reason is valid"),
            ),
            snapshot: SnapshotCapability::ObservedOnly,
            read_back: ReadBackCapability::ObservedOnly,
            writes: WriteCapability::IndividualAsync,
            exclusive_apply: ExclusiveApplyCapability::Unavailable,
            evidence_ids: vec![EvidenceId::try_from("VB-005").expect("fixed evidence is valid")],
        }
    }

    #[must_use]
    pub fn identity(&self) -> &IdentityCapability {
        &self.identity
    }

    #[must_use]
    pub fn snapshot(&self) -> &SnapshotCapability {
        &self.snapshot
    }

    #[must_use]
    pub fn read_back(&self) -> &ReadBackCapability {
        &self.read_back
    }

    #[must_use]
    pub const fn writes(&self) -> WriteCapability {
        self.writes
    }

    #[must_use]
    pub fn exclusive_apply(&self) -> &ExclusiveApplyCapability {
        &self.exclusive_apply
    }

    pub(super) fn supports_seed_destination(
        &self,
        source: &Self,
        engine: &CapabilityFingerprint,
        scope: &CookieScope,
    ) -> bool {
        &self.engine_fingerprint == engine
            && self.backend_identity_version == source.backend_identity_version
            && self.required_identity_dimensions == source.required_identity_dimensions
            && self.supported_attributes == source.supported_attributes
            && matches!(&self.identity, IdentityCapability::Lossless(s) if s == scope)
            && matches!(&self.snapshot, SnapshotCapability::Consistent(s) if s == scope)
            && matches!(&self.read_back, ReadBackCapability::Exact(s) if s == scope)
            && matches!(&self.exclusive_apply, ExclusiveApplyCapability::Supported(s) if s == scope)
            && !matches!(self.writes, WriteCapability::Unavailable)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadInterval {
    started_ms: u64,
    finished_ms: u64,
}

impl ReadInterval {
    /// # Errors
    /// Refuses a reversed observation interval.
    pub const fn new(started_ms: u64, finished_ms: u64) -> Result<Self, CookieBackendError> {
        if started_ms > finished_ms {
            return Err(CookieBackendError::InvalidInterval);
        }
        Ok(Self {
            started_ms,
            finished_ms,
        })
    }

    #[must_use]
    pub const fn started_ms(self) -> u64 {
        self.started_ms
    }

    #[must_use]
    pub const fn finished_ms(self) -> u64 {
        self.finished_ms
    }
}

#[derive(Clone, Eq, PartialEq)]
enum ReadBoundary {
    ObservedOnly,
    Consistent {
        evidence_id: EvidenceId,
        revision_token: Option<Vec<u8>>,
    },
}

impl fmt::Debug for ReadBoundary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ObservedOnly => "ObservedOnly",
            Self::Consistent { .. } => "Consistent([redacted])",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CookieBackendError {
    InvalidInterval,
    InvalidEvidence,
    EngineMismatch,
    ScopeMismatch,
    IdentityLossy,
    SnapshotUnavailable,
    MissingReadBoundary,
    IncompleteRead,
    InexactRecord,
    ContractMismatch,
    InvalidRecords,
}

impl fmt::Display for CookieBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cookie backend contract refused: {self:?}")
    }
}

impl std::error::Error for CookieBackendError {}

impl From<CookieModelError> for CookieBackendError {
    fn from(_: CookieModelError) -> Self {
        Self::InvalidRecords
    }
}

/// The exact read remains private to the application. Even this type is not
/// serializable, and Debug exposes no key, value, domain, path or token.
pub struct CookieReadResult {
    handle: Uuid,
    engine_fingerprint: CapabilityFingerprint,
    snapshot: CookieSnapshot,
    interval: ReadInterval,
    boundary: ReadBoundary,
    observed_change: bool,
}

impl fmt::Debug for CookieReadResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CookieReadResult")
            .field("observations", &self.snapshot.len())
            .field("boundary", &self.boundary_kind())
            .finish_non_exhaustive()
    }
}

/// The ordinary UI/IPC projection contains only a fresh opaque handle,
/// observation count and diagnostic indicators, never cookie material.
#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct CookieReadProjection {
    handle: String,
    observed_count: usize,
    observed_change: bool,
    complete: bool,
    consistent: bool,
}

impl fmt::Debug for CookieReadProjection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CookieReadProjection")
            .field("observed_count", &self.observed_count)
            .field("observed_change", &self.observed_change)
            .field("admitted_complete", &self.complete)
            .finish_non_exhaustive()
    }
}

impl CookieReadProjection {
    #[must_use]
    pub const fn observed_change(&self) -> bool {
        self.observed_change
    }
}

impl CookieReadResult {
    /// Captures a bounded diagnostic read from the public Qt cookie store.
    /// It cannot be represented as complete or consistent by this API.
    ///
    /// # Errors
    /// Rejects malformed, duplicate or over-limit records.
    ///
    /// # Panics
    /// Only if the fixed, compile-time-safe diagnostic reason is rejected
    /// by the core token validator.
    pub fn observed(
        capabilities: &CookieBackendCapabilities,
        observed_scope: CookieScope,
        observations: impl IntoIterator<Item = CookieObservation>,
        interval: ReadInterval,
        observed_change: bool,
    ) -> Result<Self, CookieBackendError> {
        let meta = CookieSnapshotMeta {
            backend_identity_version: capabilities.backend_identity_version,
            required_identity_dimensions: capabilities.required_identity_dimensions.clone(),
            scope: observed_scope,
            identity: SnapshotIdentityFidelity::Lossy(
                SafeReasonCode::try_from("diagnostic-observation").expect("fixed reason is valid"),
            ),
            completeness: SnapshotCompleteness::Unknown,
            consistency: SnapshotConsistency::ObservedOnly,
            supported_attributes: capabilities.supported_attributes.clone(),
        };
        let snapshot = CookieSnapshot::new(meta, observations)?;
        Ok(Self::from_backend(
            capabilities.engine_fingerprint.clone(),
            snapshot,
            interval,
            ReadBoundary::ObservedOnly,
            observed_change,
        ))
    }

    /// Only a backend already represented by an exact, evidence-bearing
    /// capability record can promote a native read. The current Qt default
    /// can never pass this constructor's admission check.
    ///
    /// # Errors
    /// Refuses mismatched engine, scope, identity, boundary or records.
    pub fn from_qualified_backend(
        capabilities: &CookieBackendCapabilities,
        actual_engine_fingerprint: &CapabilityFingerprint,
        snapshot: CookieSnapshot,
        interval: ReadInterval,
        evidence_id: EvidenceId,
        revision_token: Option<Vec<u8>>,
    ) -> Result<Self, CookieBackendError> {
        let read = Self::from_backend(
            actual_engine_fingerprint.clone(),
            snapshot,
            interval,
            ReadBoundary::Consistent {
                evidence_id,
                revision_token,
            },
            false,
        );
        let scope = &read.snapshot.meta().scope;
        read.try_consistent_snapshot(capabilities, actual_engine_fingerprint, scope)?;
        Ok(read)
    }

    fn from_backend(
        engine_fingerprint: CapabilityFingerprint,
        snapshot: CookieSnapshot,
        interval: ReadInterval,
        boundary: ReadBoundary,
        observed_change: bool,
    ) -> Self {
        Self {
            handle: Uuid::new_v4(),
            engine_fingerprint,
            snapshot,
            interval,
            boundary,
            observed_change,
        }
    }

    fn boundary_kind(&self) -> &'static str {
        match self.boundary {
            ReadBoundary::ObservedOnly => "observed-only",
            ReadBoundary::Consistent { .. } => "consistent-claimed",
        }
    }

    #[must_use]
    pub fn projection(
        &self,
        capabilities: &CookieBackendCapabilities,
        actual_engine_fingerprint: &CapabilityFingerprint,
        requested_scope: &CookieScope,
    ) -> CookieReadProjection {
        let admitted = self
            .try_consistent_snapshot(capabilities, actual_engine_fingerprint, requested_scope)
            .is_ok();
        CookieReadProjection {
            handle: self.handle.hyphenated().to_string(),
            observed_count: self.snapshot.len(),
            observed_change: self.observed_change,
            complete: admitted,
            consistent: admitted,
        }
    }

    #[must_use]
    pub const fn interval(&self) -> ReadInterval {
        self.interval
    }

    /// Returns a pure, memory-only snapshot only when every independently
    /// supplied backend, scope, evidence and data-fidelity claim agrees.
    /// It does not enable native cookie seeding or merge application.
    ///
    /// # Errors
    /// A diagnostic mirror, unknown identity, missing boundary, stale engine
    /// build or partial read is always refused.
    pub fn try_consistent_snapshot(
        &self,
        capabilities: &CookieBackendCapabilities,
        actual_engine_fingerprint: &CapabilityFingerprint,
        requested_scope: &CookieScope,
    ) -> Result<&CookieSnapshot, CookieBackendError> {
        if &capabilities.engine_fingerprint != actual_engine_fingerprint
            || &self.engine_fingerprint != actual_engine_fingerprint
        {
            return Err(CookieBackendError::EngineMismatch);
        }
        if &self.snapshot.meta().scope != requested_scope {
            return Err(CookieBackendError::ScopeMismatch);
        }
        if !matches!(&capabilities.identity, IdentityCapability::Lossless(scope) if scope == requested_scope)
        {
            return Err(CookieBackendError::IdentityLossy);
        }
        if !matches!(&capabilities.snapshot, SnapshotCapability::Consistent(scope) if scope == requested_scope)
        {
            return Err(CookieBackendError::SnapshotUnavailable);
        }
        if self.snapshot.meta().backend_identity_version != capabilities.backend_identity_version
            || self.snapshot.meta().required_identity_dimensions
                != capabilities.required_identity_dimensions
            || self.snapshot.meta().supported_attributes != capabilities.supported_attributes
        {
            return Err(CookieBackendError::ContractMismatch);
        }
        let ReadBoundary::Consistent {
            evidence_id,
            revision_token,
        } = &self.boundary
        else {
            return Err(CookieBackendError::MissingReadBoundary);
        };
        if capabilities.evidence_ids.is_empty()
            || capabilities.evidence_ids.len() > MAX_EVIDENCE_IDS
            || !capabilities.evidence_ids.contains(evidence_id)
            || revision_token
                .as_ref()
                .is_some_and(|token| token.is_empty() || token.len() > MAX_REVISION_TOKEN_BYTES)
        {
            return Err(CookieBackendError::InvalidEvidence);
        }
        if self.snapshot.meta().identity != SnapshotIdentityFidelity::Lossless
            || self.snapshot.meta().completeness != SnapshotCompleteness::Complete
            || self.snapshot.meta().consistency != SnapshotConsistency::Consistent
        {
            return Err(CookieBackendError::IncompleteRead);
        }
        if !self.snapshot.records_exact() {
            return Err(CookieBackendError::InexactRecord);
        }
        Ok(&self.snapshot)
    }
}

/// The public Qt callback does not distinguish a rescan from a live change.
/// Even a caller that knows it requested a rescan only describes provenance,
/// not a complete enumeration or mutation intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CookieObservationSource {
    Indeterminate,
    RequestedRescan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CookieMirrorInvalidation {
    Overflow,
    IdentityCollision,
    MalformedIdentity,
    UnsupportedAttributes,
    ProfileReplaced,
    SequenceGap,
    CapacityExceeded,
    InvalidRecord,
}

/// A conversion failure is explicit; it must not be silently dropped from a
/// possibly partial observation stream.
pub enum CookieMirrorChange {
    ObservedAdded(CookieRecord),
    ObservedRemoved(CookieKey),
    Invalid(CookieMirrorInvalidation),
}

impl fmt::Debug for CookieMirrorChange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ObservedAdded(_) => "ObservedAdded([redacted])",
            Self::ObservedRemoved(_) => "ObservedRemoved([redacted])",
            Self::Invalid(reason) => return reason.fmt(formatter),
        })
    }
}

pub struct CookieMirrorEvent {
    pub owner: NativeProfileOwner,
    pub epoch: u64,
    pub sequence: u64,
    pub source: CookieObservationSource,
    pub change: CookieMirrorChange,
}

impl fmt::Debug for CookieMirrorEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CookieMirrorEvent")
            .field("epoch", &self.epoch)
            .field("sequence", &self.sequence)
            .field("source", &self.source)
            .field("change", &self.change)
            .finish_non_exhaustive()
    }
}

/// Pure, memory-only diagnostic state for one exact native profile binding.
/// It never records absence, branches, intent or a completed enumeration.
pub struct CookieObservationMirror {
    owner: NativeProfileOwner,
    epoch: u64,
    last_sequence: u64,
    capabilities: CookieBackendCapabilities,
    scope: CookieScope,
    records: BTreeMap<CookieKey, CookieRecord>,
    invalidation: Option<CookieMirrorInvalidation>,
    saw_rescan: bool,
    saw_indeterminate: bool,
}

impl fmt::Debug for CookieObservationMirror {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CookieObservationMirror")
            .field("epoch", &self.epoch)
            .field("last_sequence", &self.last_sequence)
            .field("record_count", &self.records.len())
            .field("invalidation", &self.invalidation)
            .finish_non_exhaustive()
    }
}

impl CookieObservationMirror {
    #[must_use]
    pub fn new(
        owner: NativeProfileOwner,
        epoch: u64,
        capabilities: CookieBackendCapabilities,
        scope: CookieScope,
    ) -> Self {
        Self {
            owner,
            epoch,
            last_sequence: 0,
            capabilities,
            scope,
            records: BTreeMap::new(),
            invalidation: (epoch == 0).then_some(CookieMirrorInvalidation::MalformedIdentity),
            saw_rescan: false,
            saw_indeterminate: false,
        }
    }

    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    #[must_use]
    pub const fn last_sequence(&self) -> u64 {
        self.last_sequence
    }

    #[must_use]
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub const fn invalidation(&self) -> Option<CookieMirrorInvalidation> {
        self.invalidation
    }

    #[must_use]
    pub const fn observed_sources(&self) -> (bool, bool) {
        (self.saw_rescan, self.saw_indeterminate)
    }

    /// Drops all retained secrets and invalidates plans tied to the old
    /// binding. The new binding needs its own mirror and fresh epoch.
    pub fn profile_replaced(&mut self) {
        self.invalidate(CookieMirrorInvalidation::ProfileReplaced);
    }

    pub fn queue_overflowed(&mut self) {
        self.invalidate(CookieMirrorInvalidation::Overflow);
    }

    fn invalidate(&mut self, reason: CookieMirrorInvalidation) {
        self.records.clear();
        self.invalidation.get_or_insert(reason);
    }

    /// Processes already-converted, bounded native facts outside the native
    /// callback. Any unconvertible fact must arrive as `Invalid`.
    ///
    /// # Errors
    /// Returns the sticky invalidation reason for a broken observation epoch.
    ///
    /// # Panics
    /// Only if fixed, compile-time-safe interval or diagnostic reason values
    /// are rejected by their validators.
    pub fn observe(&mut self, event: CookieMirrorEvent) -> Result<(), CookieMirrorInvalidation> {
        if let Some(reason) = self.invalidation {
            return Err(reason);
        }
        if event.owner != self.owner || event.epoch != self.epoch {
            self.invalidate(CookieMirrorInvalidation::ProfileReplaced);
            return Err(CookieMirrorInvalidation::ProfileReplaced);
        }
        if self.last_sequence.checked_add(1) != Some(event.sequence) {
            self.invalidate(CookieMirrorInvalidation::SequenceGap);
            return Err(CookieMirrorInvalidation::SequenceGap);
        }
        self.last_sequence = event.sequence;
        match event.source {
            CookieObservationSource::RequestedRescan => self.saw_rescan = true,
            CookieObservationSource::Indeterminate => self.saw_indeterminate = true,
        }
        match event.change {
            CookieMirrorChange::Invalid(reason) => self.invalidate(reason),
            CookieMirrorChange::ObservedAdded(record) => {
                let key = record.key().clone();
                // A one-record core read checks backend identity version,
                // scope and supported attribute names before retention.
                if CookieReadResult::observed(
                    &self.capabilities,
                    self.scope.clone(),
                    [CookieObservation::Present(record.clone())],
                    ReadInterval::new(0, 0).expect("fixed interval is valid"),
                    false,
                )
                .is_err()
                {
                    self.invalidate(CookieMirrorInvalidation::InvalidRecord);
                } else if let Some(previous) = self.records.get(&key) {
                    if previous == &record {
                        // Duplicate enumeration is idempotent, never creation.
                    } else {
                        // Unknown backend identity can alias distinct cookies;
                        // do not silently replace a colliding value.
                        self.invalidate(CookieMirrorInvalidation::IdentityCollision);
                    }
                } else if self.records.len() == MAX_MIRROR_RECORDS {
                    self.invalidate(CookieMirrorInvalidation::CapacityExceeded);
                } else {
                    self.records.insert(key, record);
                }
            }
            CookieMirrorChange::ObservedRemoved(key) => {
                if CookieReadResult::observed(
                    &self.capabilities,
                    self.scope.clone(),
                    [CookieObservation::Unknown {
                        key: key.clone(),
                        reason: SafeReasonCode::try_from("diagnostic-removal")
                            .expect("fixed reason is valid"),
                    }],
                    ReadInterval::new(0, 0).expect("fixed interval is valid"),
                    false,
                )
                .is_err()
                {
                    self.invalidate(CookieMirrorInvalidation::MalformedIdentity);
                    return Err(CookieMirrorInvalidation::MalformedIdentity);
                }
                // The callback removes a current diagnostic record only; it
                // never creates a deletion tombstone or branch delta.
                self.records.remove(&key);
            }
        }
        self.invalidation.map_or(Ok(()), Err)
    }

    /// Returns only present, observed-only records. An empty result is still
    /// unknown coverage; it is never a verified empty jar or exact T0.
    ///
    /// # Errors
    /// Refuses an invalidated epoch or a failed core bound/contract check.
    pub fn diagnostic_read(
        &self,
        interval: ReadInterval,
    ) -> Result<CookieReadResult, CookieBackendError> {
        if self.invalidation.is_some() {
            return Err(CookieBackendError::IncompleteRead);
        }
        CookieReadResult::observed(
            &self.capabilities,
            self.scope.clone(),
            self.records
                .values()
                .cloned()
                .map(CookieObservation::Present),
            interval,
            false,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::{BTreeMap, BTreeSet};
    use std::rc::Rc;

    use ferric_browser_core::ProfileId;
    use ferric_browser_core::versioning::{
        ApplicationOwnerId, CapabilityFingerprint, CookieExpiry, CookieKey, CookieKeyInput,
        CookieObservation, CookieRecord, CookieRecordInput, CookieScope, CookieSnapshot,
        CookieSnapshotMeta, EvidenceId, IdentityProvenance, NativeProfileOwner, PartitionIdentity,
        ProfileInstanceId, RecordFidelity, SameSite, SnapshotCompleteness, SnapshotConsistency,
        SnapshotIdentityFidelity,
    };

    use super::{
        CookieBackendCapabilities, CookieBackendError, CookieMirrorChange, CookieMirrorEvent,
        CookieMirrorInvalidation, CookieObservationMirror, CookieObservationSource,
        CookieReadResult, ExclusiveApplyCapability, IdentityCapability, ReadBackCapability,
        ReadBoundary, ReadInterval, SnapshotCapability, WriteCapability,
    };
    use crate::versioning::{
        CookieDestinationRead, CookieSeedDestination, CookieSeedDispatchError, CookieSeedError,
        CookieSeedOutcome, CookieSeedReadError, seed_prepared_destination,
    };

    fn fingerprint(package: &str) -> CapabilityFingerprint {
        CapabilityFingerprint::new(
            "6.11.2",
            "6.11.2",
            "140.0.7339.225",
            "151.0.7922.71",
            package,
        )
        .unwrap()
    }

    fn record(partition: PartitionIdentity) -> CookieObservation {
        let key = CookieKey::new(CookieKeyInput {
            backend_identity_version: 1,
            name: b"synthetic-auth-name".to_vec(),
            domain: "example.test".into(),
            path: "/".into(),
            host_only: true,
            host_only_provenance: IdentityProvenance::BackendExact,
            partition,
            backend_dimensions: BTreeMap::new(),
        })
        .unwrap();
        CookieObservation::Present(
            CookieRecord::new(CookieRecordInput {
                key,
                value: b"synthetic-secret-value".to_vec(),
                expiry: CookieExpiry::Session,
                secure: true,
                http_only: true,
                same_site: SameSite::Strict,
                extra_attributes: BTreeMap::new(),
                fidelity: RecordFidelity::Exact,
            })
            .unwrap(),
        )
    }

    fn owner() -> NativeProfileOwner {
        NativeProfileOwner::new(
            ApplicationOwnerId::fresh(),
            ProfileId::from_display("profileid-1").unwrap(),
            ProfileInstanceId::fresh(),
        )
    }

    fn mirror_record(value: &[u8]) -> CookieRecord {
        CookieRecord::new(CookieRecordInput {
            key: record(PartitionIdentity::Unknown).key().clone(),
            value: value.to_vec(),
            expiry: CookieExpiry::Session,
            secure: true,
            http_only: true,
            same_site: SameSite::Strict,
            extra_attributes: BTreeMap::new(),
            fidelity: RecordFidelity::ObservedOnly,
        })
        .unwrap()
    }

    fn mirror(owner: NativeProfileOwner) -> CookieObservationMirror {
        CookieObservationMirror::new(
            owner,
            1,
            CookieBackendCapabilities::qt_public(fingerprint("6.11.2-1")),
            CookieScope::FullProfile,
        )
    }

    fn event(
        owner: NativeProfileOwner,
        sequence: u64,
        source: CookieObservationSource,
        change: CookieMirrorChange,
    ) -> CookieMirrorEvent {
        CookieMirrorEvent {
            owner,
            epoch: 1,
            sequence,
            source,
            change,
        }
    }

    #[test]
    fn mirror_duplicate_enumeration_is_idempotent_not_creation() {
        let owner = owner();
        let mut mirror = mirror(owner);
        for sequence in 1..=2 {
            mirror
                .observe(event(
                    owner,
                    sequence,
                    CookieObservationSource::RequestedRescan,
                    CookieMirrorChange::ObservedAdded(mirror_record(b"synthetic-secret-value")),
                ))
                .unwrap();
        }
        assert_eq!(mirror.record_count(), 1);
        assert_eq!(mirror.observed_sources(), (true, false));
        let read = mirror
            .diagnostic_read(ReadInterval::new(1, 2).unwrap())
            .unwrap();
        assert_eq!(read.snapshot.len(), 1);
        assert_eq!(
            read.snapshot.meta().completeness,
            SnapshotCompleteness::Unknown
        );
        assert_eq!(
            read.snapshot.meta().consistency,
            SnapshotConsistency::ObservedOnly
        );
        assert!(
            !read
                .projection(
                    &mirror.capabilities,
                    &fingerprint("6.11.2-1"),
                    &CookieScope::FullProfile
                )
                .observed_change()
        );
    }

    #[test]
    fn mirror_removal_then_add_is_replacement_without_tombstone_or_intent() {
        let owner = owner();
        let mut mirror = mirror(owner);
        let first = mirror_record(b"synthetic-secret-old");
        let key = first.key().clone();
        mirror
            .observe(event(
                owner,
                1,
                CookieObservationSource::Indeterminate,
                CookieMirrorChange::ObservedAdded(first),
            ))
            .unwrap();
        mirror
            .observe(event(
                owner,
                2,
                CookieObservationSource::Indeterminate,
                CookieMirrorChange::ObservedRemoved(key),
            ))
            .unwrap();
        assert_eq!(mirror.record_count(), 0);
        assert!(
            mirror
                .diagnostic_read(ReadInterval::new(1, 2).unwrap())
                .unwrap()
                .snapshot
                .is_empty()
        );
        mirror
            .observe(event(
                owner,
                3,
                CookieObservationSource::Indeterminate,
                CookieMirrorChange::ObservedAdded(mirror_record(b"synthetic-secret-new")),
            ))
            .unwrap();
        let read = mirror
            .diagnostic_read(ReadInterval::new(1, 3).unwrap())
            .unwrap();
        assert_eq!(read.snapshot.len(), 1);
        assert_eq!(
            read.snapshot.meta().completeness,
            SnapshotCompleteness::Unknown
        );
        assert!(
            !read
                .projection(
                    &mirror.capabilities,
                    &fingerprint("6.11.2-1"),
                    &CookieScope::FullProfile
                )
                .observed_change()
        );
    }

    #[test]
    fn mirror_reordering_and_loss_invalidate_and_discard_secrets() {
        let owner = owner();
        let mut mirror = mirror(owner);
        mirror
            .observe(event(
                owner,
                1,
                CookieObservationSource::Indeterminate,
                CookieMirrorChange::ObservedAdded(mirror_record(b"synthetic-secret-value")),
            ))
            .unwrap();
        assert_eq!(
            mirror.observe(event(
                owner,
                3,
                CookieObservationSource::Indeterminate,
                CookieMirrorChange::ObservedRemoved(mirror_record(b"x").key().clone())
            )),
            Err(CookieMirrorInvalidation::SequenceGap)
        );
        assert_eq!(mirror.record_count(), 0);
        assert_eq!(
            mirror.invalidation(),
            Some(CookieMirrorInvalidation::SequenceGap)
        );
        assert!(
            mirror
                .diagnostic_read(ReadInterval::new(1, 3).unwrap())
                .is_err()
        );
        assert!(!format!("{mirror:?}").contains("synthetic-secret-value"));
    }

    #[test]
    fn mirror_empty_timeout_remains_unknown_not_exact_t0() {
        let owner = owner();
        let mirror = mirror(owner);
        let read = mirror
            .diagnostic_read(ReadInterval::new(1, 10_000).unwrap())
            .unwrap();
        assert!(read.snapshot.is_empty());
        assert_eq!(
            read.snapshot.meta().completeness,
            SnapshotCompleteness::Unknown
        );
        assert_eq!(
            read.try_consistent_snapshot(
                &mirror.capabilities,
                &fingerprint("6.11.2-1"),
                &CookieScope::FullProfile
            ),
            Err(CookieBackendError::IdentityLossy)
        );
    }

    #[test]
    fn timers_sentinels_and_equal_rescans_cannot_supply_a_source_boundary() {
        let capabilities = qualified_capabilities();
        let actual = fingerprint("6.11.2-1");
        let sentinel_key = CookieKey::new(CookieKeyInput {
            backend_identity_version: 1,
            name: b"synthetic-sentinel".to_vec(),
            domain: "example.test".into(),
            path: "/".into(),
            host_only: true,
            host_only_provenance: IdentityProvenance::BackendExact,
            partition: PartitionIdentity::Unpartitioned,
            backend_dimensions: BTreeMap::new(),
        })
        .unwrap();
        let sentinel = CookieObservation::Present(
            CookieRecord::new(CookieRecordInput {
                key: sentinel_key,
                value: b"synthetic-sentinel-value".to_vec(),
                expiry: CookieExpiry::Session,
                secure: true,
                http_only: true,
                same_site: SameSite::Strict,
                extra_attributes: BTreeMap::new(),
                fidelity: RecordFidelity::Exact,
            })
            .unwrap(),
        );
        // A long quiet read, an observed sentinel, and two byte-equal
        // enumerations all lack an engine-issued consistent boundary.
        let populated = vec![record(PartitionIdentity::Unpartitioned), sentinel];
        let observations = [vec![], populated.clone(), populated.clone(), populated];
        for observations in observations {
            let read = CookieReadResult::observed(
                &capabilities,
                CookieScope::FullProfile,
                observations,
                ReadInterval::new(0, 60_000).unwrap(),
                false,
            )
            .unwrap();
            assert_eq!(
                read.snapshot.meta().completeness,
                SnapshotCompleteness::Unknown
            );
            assert_eq!(
                read.snapshot.meta().consistency,
                SnapshotConsistency::ObservedOnly
            );
            assert_eq!(
                read.try_consistent_snapshot(&capabilities, &actual, &CookieScope::FullProfile),
                Err(CookieBackendError::MissingReadBoundary)
            );
        }
    }

    #[test]
    fn mirror_collision_overflow_and_profile_replacement_invalidate() {
        let owner = owner();
        let mut collision = mirror(owner);
        collision
            .observe(event(
                owner,
                1,
                CookieObservationSource::Indeterminate,
                CookieMirrorChange::ObservedAdded(mirror_record(b"synthetic-old")),
            ))
            .unwrap();
        assert_eq!(
            collision.observe(event(
                owner,
                2,
                CookieObservationSource::Indeterminate,
                CookieMirrorChange::ObservedAdded(mirror_record(b"synthetic-new"))
            )),
            Err(CookieMirrorInvalidation::IdentityCollision)
        );
        assert_eq!(collision.record_count(), 0);
        let mut overflow = mirror(owner);
        overflow.queue_overflowed();
        assert_eq!(
            overflow.invalidation(),
            Some(CookieMirrorInvalidation::Overflow)
        );
        let mut replaced = mirror(owner);
        replaced.profile_replaced();
        assert_eq!(
            replaced.invalidation(),
            Some(CookieMirrorInvalidation::ProfileReplaced)
        );
        let mut wrong_owner = mirror(owner);
        let another_owner = NativeProfileOwner::new(
            ApplicationOwnerId::fresh(),
            owner.runtime_profile(),
            ProfileInstanceId::fresh(),
        );
        assert_eq!(
            wrong_owner.observe(event(
                another_owner,
                1,
                CookieObservationSource::Indeterminate,
                CookieMirrorChange::ObservedAdded(mirror_record(b"synthetic-secret"))
            )),
            Err(CookieMirrorInvalidation::ProfileReplaced)
        );
    }

    #[test]
    fn mirror_conversion_failures_invalidate_without_secret_projection() {
        let owner = owner();
        for reason in [
            CookieMirrorInvalidation::MalformedIdentity,
            CookieMirrorInvalidation::UnsupportedAttributes,
        ] {
            let mut mirror = mirror(owner);
            assert_eq!(
                mirror.observe(event(
                    owner,
                    1,
                    CookieObservationSource::Indeterminate,
                    CookieMirrorChange::Invalid(reason)
                )),
                Err(reason)
            );
            assert_eq!(mirror.record_count(), 0);
        }
    }

    #[test]
    fn mirror_rejects_unsupported_attributes_and_bounds_current_records() {
        let owner = owner();
        let mut extra_attributes = BTreeMap::new();
        extra_attributes.insert(
            ferric_browser_core::versioning::BackendAttributeName::new("unqualified").unwrap(),
            ferric_browser_core::versioning::AttributePresence::Unknown,
        );
        let unsupported = CookieRecord::new(CookieRecordInput {
            key: mirror_record(b"x").key().clone(),
            value: b"synthetic-secret".to_vec(),
            expiry: CookieExpiry::Session,
            secure: true,
            http_only: true,
            same_site: SameSite::Strict,
            extra_attributes,
            fidelity: RecordFidelity::ObservedOnly,
        })
        .unwrap();
        let mut unsupported_mirror = mirror(owner);
        assert_eq!(
            unsupported_mirror.observe(event(
                owner,
                1,
                CookieObservationSource::Indeterminate,
                CookieMirrorChange::ObservedAdded(unsupported)
            )),
            Err(CookieMirrorInvalidation::InvalidRecord)
        );
        assert_eq!(unsupported_mirror.record_count(), 0);

        let mut bounded = mirror(owner);
        for sequence in 1..=super::MAX_MIRROR_RECORDS + 1 {
            let key = CookieKey::new(CookieKeyInput {
                backend_identity_version: 1,
                name: format!("synthetic-{sequence}").into_bytes(),
                domain: "example.test".into(),
                path: "/".into(),
                host_only: true,
                host_only_provenance: IdentityProvenance::BackendExact,
                partition: PartitionIdentity::Unknown,
                backend_dimensions: BTreeMap::new(),
            })
            .unwrap();
            let record = CookieRecord::new(CookieRecordInput {
                key,
                value: b"synthetic-secret".to_vec(),
                expiry: CookieExpiry::Session,
                secure: true,
                http_only: true,
                same_site: SameSite::Strict,
                extra_attributes: BTreeMap::new(),
                fidelity: RecordFidelity::ObservedOnly,
            })
            .unwrap();
            let result = bounded.observe(event(
                owner,
                sequence as u64,
                CookieObservationSource::Indeterminate,
                CookieMirrorChange::ObservedAdded(record),
            ));
            if sequence <= super::MAX_MIRROR_RECORDS {
                assert_eq!(result, Ok(()));
            } else {
                assert_eq!(result, Err(CookieMirrorInvalidation::CapacityExceeded));
            }
        }
        assert_eq!(bounded.record_count(), 0);
    }

    #[test]
    fn mirror_repeated_sequence_is_reordering_not_a_second_event() {
        let owner = owner();
        let mut mirror = mirror(owner);
        mirror
            .observe(event(
                owner,
                1,
                CookieObservationSource::Indeterminate,
                CookieMirrorChange::ObservedAdded(mirror_record(b"synthetic-secret")),
            ))
            .unwrap();
        assert_eq!(
            mirror.observe(event(
                owner,
                1,
                CookieObservationSource::Indeterminate,
                CookieMirrorChange::ObservedRemoved(mirror_record(b"x").key().clone())
            )),
            Err(CookieMirrorInvalidation::SequenceGap)
        );
        assert_eq!(mirror.record_count(), 0);
    }

    fn qualified_capabilities() -> CookieBackendCapabilities {
        CookieBackendCapabilities {
            engine_fingerprint: fingerprint("6.11.2-1"),
            backend_identity_version: 1,
            required_identity_dimensions: BTreeSet::new(),
            supported_attributes: BTreeSet::new(),
            identity: IdentityCapability::Lossless(CookieScope::FullProfile),
            snapshot: SnapshotCapability::Consistent(CookieScope::FullProfile),
            read_back: ReadBackCapability::Exact(CookieScope::FullProfile),
            writes: WriteCapability::VerifiedBatch,
            exclusive_apply: ExclusiveApplyCapability::Supported(CookieScope::FullProfile),
            evidence_ids: vec![EvidenceId::try_from("synthetic-only").unwrap()],
        }
    }

    fn claimed_complete(
        observations: impl IntoIterator<Item = CookieObservation>,
    ) -> CookieSnapshot {
        CookieSnapshot::new(
            CookieSnapshotMeta {
                backend_identity_version: 1,
                required_identity_dimensions: BTreeSet::new(),
                scope: CookieScope::FullProfile,
                identity: SnapshotIdentityFidelity::Lossless,
                completeness: SnapshotCompleteness::Complete,
                consistency: SnapshotConsistency::Consistent,
                supported_attributes: BTreeSet::new(),
            },
            observations,
        )
        .unwrap()
    }

    #[test]
    fn public_qt_observations_never_become_consistent_snapshots() {
        let capabilities = CookieBackendCapabilities::qt_public(fingerprint("6.11.2-1"));
        let read = CookieReadResult::observed(
            &capabilities,
            CookieScope::FullProfile,
            [record(PartitionIdentity::Unknown)],
            ReadInterval::new(1, 2).unwrap(),
            true,
        )
        .unwrap();
        assert_eq!(
            read.try_consistent_snapshot(
                &capabilities,
                &fingerprint("6.11.2-1"),
                &CookieScope::FullProfile
            )
            .unwrap_err(),
            CookieBackendError::IdentityLossy
        );
        let projection = read.projection(
            &capabilities,
            &fingerprint("6.11.2-1"),
            &CookieScope::FullProfile,
        );
        let text = serde_json::to_string(&projection).unwrap();
        assert!(projection.observed_change());
        assert!(!projection.complete);
        assert!(!projection.consistent);
        assert!(!text.contains("synthetic-secret-value"));
        assert!(!text.contains("synthetic-auth-name"));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text)
                .unwrap()
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            [
                "complete",
                "consistent",
                "handle",
                "observed_change",
                "observed_count"
            ]
        );
        assert!(!format!("{read:?}").contains("synthetic-secret-value"));
    }

    #[test]
    fn fingerprint_mismatch_cannot_requalify_a_read() {
        let capabilities = CookieBackendCapabilities::qt_public(fingerprint("6.11.2-1"));
        let read = CookieReadResult::observed(
            &capabilities,
            CookieScope::FullProfile,
            [],
            ReadInterval::new(1, 2).unwrap(),
            false,
        )
        .unwrap();
        assert_eq!(
            read.try_consistent_snapshot(
                &capabilities,
                &fingerprint("6.11.2-2"),
                &CookieScope::FullProfile
            )
            .unwrap_err(),
            CookieBackendError::EngineMismatch
        );
    }

    #[test]
    fn observation_scope_is_explicit_and_bounded() {
        let capabilities = CookieBackendCapabilities::qt_public(fingerprint("6.11.2-1"));
        let read = CookieReadResult::observed(
            &capabilities,
            CookieScope::Site("example.test".into()),
            [record(PartitionIdentity::Unknown)],
            ReadInterval::new(1, 2).unwrap(),
            false,
        )
        .unwrap();
        assert_eq!(
            read.snapshot.meta().scope,
            CookieScope::Site("example.test".into())
        );
        assert_eq!(
            read.try_consistent_snapshot(
                &capabilities,
                &fingerprint("6.11.2-1"),
                &CookieScope::FullProfile
            )
            .unwrap_err(),
            CookieBackendError::ScopeMismatch
        );
    }

    #[test]
    fn invalid_read_interval_is_refused() {
        assert_eq!(
            ReadInterval::new(2, 1).unwrap_err(),
            CookieBackendError::InvalidInterval
        );
    }

    #[test]
    fn unknown_partition_refuses_even_a_claimed_complete_read() {
        let capabilities = qualified_capabilities();
        let error = CookieReadResult::from_qualified_backend(
            &capabilities,
            &fingerprint("6.11.2-1"),
            claimed_complete([record(PartitionIdentity::Unknown)]),
            ReadInterval::new(1, 2).unwrap(),
            EvidenceId::try_from("synthetic-only").unwrap(),
            None,
        )
        .unwrap_err();
        assert_eq!(error, CookieBackendError::InexactRecord);
    }

    #[test]
    fn an_observation_boundary_cannot_be_promoted_by_complete_metadata() {
        let capabilities = qualified_capabilities();
        let read = CookieReadResult::from_backend(
            fingerprint("6.11.2-1"),
            claimed_complete([record(PartitionIdentity::Unpartitioned)]),
            ReadInterval::new(1, 2).unwrap(),
            ReadBoundary::ObservedOnly,
            true,
        );
        assert_eq!(
            read.try_consistent_snapshot(
                &capabilities,
                &fingerprint("6.11.2-1"),
                &CookieScope::FullProfile
            )
            .unwrap_err(),
            CookieBackendError::MissingReadBoundary
        );
    }

    #[test]
    fn only_matching_synthetic_qualified_read_is_admitted() {
        let capabilities = qualified_capabilities();
        let read = CookieReadResult::from_qualified_backend(
            &capabilities,
            &fingerprint("6.11.2-1"),
            claimed_complete([record(PartitionIdentity::Unpartitioned)]),
            ReadInterval::new(1, 2).unwrap(),
            EvidenceId::try_from("synthetic-only").unwrap(),
            Some(b"synthetic-revision".to_vec()),
        )
        .unwrap();
        assert_eq!(read.interval().started_ms(), 1);
        assert_eq!(read.interval().finished_ms(), 2);
        assert_eq!(
            read.try_consistent_snapshot(
                &capabilities,
                &fingerprint("6.11.2-1"),
                &CookieScope::FullProfile
            )
            .unwrap()
            .len(),
            1
        );
        assert!(
            read.projection(
                &capabilities,
                &fingerprint("6.11.2-1"),
                &CookieScope::FullProfile
            )
            .complete
        );
        assert_eq!(
            read.try_consistent_snapshot(
                &capabilities,
                &fingerprint("6.11.2-1"),
                &CookieScope::Site("example.test".into())
            )
            .unwrap_err(),
            CookieBackendError::ScopeMismatch
        );
        assert!(!format!("{read:?}").contains("synthetic-revision"));
    }

    #[test]
    fn evidence_and_partial_coverage_cannot_be_self_asserted() {
        let capabilities = qualified_capabilities();
        let result = CookieReadResult::from_qualified_backend(
            &capabilities,
            &fingerprint("6.11.2-1"),
            claimed_complete([]),
            ReadInterval::new(1, 2).unwrap(),
            EvidenceId::try_from("unreviewed").unwrap(),
            None,
        );
        assert_eq!(result.unwrap_err(), CookieBackendError::InvalidEvidence);

        let mut partial = claimed_complete([]).meta().clone();
        partial.completeness = SnapshotCompleteness::Unknown;
        let result = CookieReadResult::from_qualified_backend(
            &capabilities,
            &fingerprint("6.11.2-1"),
            CookieSnapshot::new(partial, []).unwrap(),
            ReadInterval::new(1, 2).unwrap(),
            EvidenceId::try_from("synthetic-only").unwrap(),
            None,
        );
        assert_eq!(result.unwrap_err(), CookieBackendError::IncompleteRead);
    }

    #[test]
    fn capability_debug_hides_site_scope() {
        let mut capabilities = qualified_capabilities();
        let private_site = CookieScope::Site("private.example.test".into());
        capabilities.identity = IdentityCapability::Lossless(private_site.clone());
        capabilities.snapshot = SnapshotCapability::Consistent(private_site.clone());
        capabilities.read_back = ReadBackCapability::Exact(private_site.clone());
        capabilities.exclusive_apply = ExclusiveApplyCapability::Supported(private_site);
        let debug = format!("{capabilities:?}");
        assert!(!debug.contains("private.example.test"));
    }

    #[derive(Clone, Copy)]
    enum FixtureReadBack {
        Exact,
        MissingAcknowledgement,
        AttributeChanged,
        WrongOwner,
    }

    struct FixtureDestination {
        owner: NativeProfileOwner,
        capabilities: CookieBackendCapabilities,
        dispatched: Vec<CookieObservation>,
        origins: Vec<String>,
        read_back: FixtureReadBack,
        reject: bool,
        cancel_after_first: bool,
        pristine: bool,
        destroyed: Rc<Cell<bool>>,
    }

    impl FixtureDestination {
        fn new(destroyed: Rc<Cell<bool>>) -> Self {
            Self {
                owner: owner(),
                capabilities: qualified_capabilities(),
                dispatched: Vec::new(),
                origins: Vec::new(),
                read_back: FixtureReadBack::Exact,
                reject: false,
                cancel_after_first: false,
                pristine: true,
                destroyed,
            }
        }
    }

    impl CookieSeedDestination for FixtureDestination {
        fn owner(&self) -> NativeProfileOwner {
            self.owner
        }

        fn pristine_and_unnavigated(&self) -> bool {
            self.pristine
        }

        fn still_unnavigated(&self) -> bool {
            self.pristine
        }

        fn capabilities(&self) -> &CookieBackendCapabilities {
            &self.capabilities
        }

        fn engine_fingerprint(&self) -> &CapabilityFingerprint {
            &self.capabilities.engine_fingerprint
        }

        fn cancelled(&self) -> bool {
            self.cancel_after_first && !self.dispatched.is_empty()
        }

        fn dispatch_cookie(
            &mut self,
            record: &CookieRecord,
            origin: &str,
        ) -> Result<(), CookieSeedDispatchError> {
            if self.reject {
                return Err(CookieSeedDispatchError::Rejected);
            }
            self.dispatched
                .push(CookieObservation::Present(record.clone()));
            self.origins.push(origin.to_owned());
            Ok(())
        }

        fn read_back(&mut self) -> Result<CookieDestinationRead, CookieSeedReadError> {
            let observations = match self.read_back {
                FixtureReadBack::MissingAcknowledgement => {
                    return Err(CookieSeedReadError::Unavailable);
                }
                FixtureReadBack::AttributeChanged => self
                    .dispatched
                    .iter()
                    .map(|observation| {
                        let CookieObservation::Present(existing) = observation else {
                            unreachable!()
                        };
                        CookieObservation::Present(
                            CookieRecord::new(CookieRecordInput {
                                key: existing.key().clone(),
                                value: existing.value_bytes().to_vec(),
                                expiry: existing.expiry(),
                                secure: true,
                                http_only: true,
                                same_site: SameSite::Lax,
                                extra_attributes: BTreeMap::new(),
                                fidelity: RecordFidelity::Exact,
                            })
                            .unwrap(),
                        )
                    })
                    .collect(),
                FixtureReadBack::Exact | FixtureReadBack::WrongOwner => self.dispatched.clone(),
            };
            let read = CookieReadResult::from_qualified_backend(
                &self.capabilities,
                &self.capabilities.engine_fingerprint,
                claimed_complete(observations),
                ReadInterval::new(3, 4).unwrap(),
                EvidenceId::try_from("synthetic-only").unwrap(),
                None,
            )
            .unwrap();
            Ok(CookieDestinationRead {
                owner: if matches!(self.read_back, FixtureReadBack::WrongOwner) {
                    owner()
                } else {
                    self.owner
                },
                read,
            })
        }

        fn destroy(self) {
            self.destroyed.set(true);
        }
    }

    fn fixture_source(
        observations: impl IntoIterator<Item = CookieObservation>,
    ) -> CookieReadResult {
        let capabilities = qualified_capabilities();
        CookieReadResult::from_qualified_backend(
            &capabilities,
            &capabilities.engine_fingerprint,
            claimed_complete(observations),
            ReadInterval::new(1, 2).unwrap(),
            EvidenceId::try_from("synthetic-only").unwrap(),
            None,
        )
        .unwrap()
    }

    fn run_fixture(
        destination: FixtureDestination,
        source: &CookieReadResult,
    ) -> CookieSeedOutcome<FixtureDestination> {
        seed_prepared_destination(
            owner(),
            source,
            &qualified_capabilities(),
            &CookieScope::FullProfile,
            1_000,
            destination,
        )
    }

    #[test]
    fn qualified_fixture_seed_requires_exact_destination_read_back() {
        let source = fixture_source([record(PartitionIdentity::Unpartitioned)]);
        let destroyed = Rc::new(Cell::new(false));
        let result = run_fixture(FixtureDestination::new(destroyed.clone()), &source);
        let CookieSeedOutcome::Seeded(destination) = result else {
            panic!("qualified exact fixture should seed");
        };
        assert!(!destroyed.get());
        assert_eq!(destination.origins, ["https://example.test"]);
        assert_eq!(destination.dispatched.len(), 1);
        assert!(!format!("{:?}", destination.dispatched[0]).contains("synthetic-secret"));
    }

    #[test]
    fn missing_acknowledgement_and_changed_read_back_destroy_child() {
        let source = fixture_source([record(PartitionIdentity::Unpartitioned)]);
        for mode in [
            FixtureReadBack::MissingAcknowledgement,
            FixtureReadBack::AttributeChanged,
            FixtureReadBack::WrongOwner,
        ] {
            let destroyed = Rc::new(Cell::new(false));
            let mut destination = FixtureDestination::new(destroyed.clone());
            destination.read_back = mode;
            assert!(matches!(
                run_fixture(destination, &source),
                CookieSeedOutcome::Unverified(CookieSeedError::CookieSeedUnverified)
            ));
            assert!(destroyed.get());
        }
    }

    #[test]
    fn rejected_expired_and_cancelled_seeds_destroy_child() {
        let source = fixture_source([record(PartitionIdentity::Unpartitioned)]);
        let destroyed = Rc::new(Cell::new(false));
        let mut rejected = FixtureDestination::new(destroyed.clone());
        rejected.reject = true;
        assert!(matches!(
            run_fixture(rejected, &source),
            CookieSeedOutcome::Failed(CookieSeedError::CookieSeedFailed)
        ));
        assert!(destroyed.get());

        let destroyed = Rc::new(Cell::new(false));
        let mut cancelled = FixtureDestination::new(destroyed.clone());
        cancelled.cancel_after_first = true;
        assert!(matches!(
            run_fixture(cancelled, &source),
            CookieSeedOutcome::Failed(CookieSeedError::Cancelled)
        ));
        assert!(destroyed.get());

        let CookieObservation::Present(existing) = record(PartitionIdentity::Unpartitioned) else {
            unreachable!()
        };
        let expired = CookieObservation::Present(
            CookieRecord::new(CookieRecordInput {
                key: existing.key().clone(),
                value: existing.value_bytes().to_vec(),
                expiry: CookieExpiry::AtUnixMillis(1_000),
                secure: true,
                http_only: true,
                same_site: SameSite::Strict,
                extra_attributes: BTreeMap::new(),
                fidelity: RecordFidelity::Exact,
            })
            .unwrap(),
        );
        let expired_source = fixture_source([expired]);
        let destroyed = Rc::new(Cell::new(false));
        assert!(matches!(
            run_fixture(FixtureDestination::new(destroyed.clone()), &expired_source),
            CookieSeedOutcome::Failed(CookieSeedError::ExpiredRecord)
        ));
        assert!(destroyed.get());
    }

    #[test]
    fn public_qt_capability_never_seeds_even_a_qualified_fixture_source() {
        let source = fixture_source([record(PartitionIdentity::Unpartitioned)]);
        let destroyed = Rc::new(Cell::new(false));
        let mut destination = FixtureDestination::new(destroyed.clone());
        destination.capabilities = CookieBackendCapabilities::qt_public(fingerprint("6.11.2-1"));
        assert!(matches!(
            run_fixture(destination, &source),
            CookieSeedOutcome::Failed(CookieSeedError::CookieSeedUnavailable)
        ));
        assert!(destroyed.get());
    }
}
