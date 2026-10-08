//! Backend-neutral, memory-only cookie comparison. This module cannot read or
//! write an engine store, and its secret-bearing types are not serializable.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use super::SafeReasonCode;
use super::limits::{MAX_COOKIE_MATERIAL_BYTES, MAX_COOKIE_OBSERVATIONS};

const MAX_NAME_BYTES: usize = 1_024;
const MAX_VALUE_BYTES: usize = 16 * 1_024;
const MAX_PATH_BYTES: usize = 4_096;
const MAX_PARTITION_BYTES: usize = 512;
const MAX_BACKEND_DIMENSIONS: usize = 16;
const MAX_ATTRIBUTE_BYTES: usize = 1_024;

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub enum PartitionIdentity {
    Unpartitioned,
    Known(Vec<u8>),
    Unknown,
}

impl fmt::Debug for PartitionIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unpartitioned => "Unpartitioned",
            Self::Known(_) => "Known([redacted])",
            Self::Unknown => "Unknown",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IdentityProvenance {
    BackendExact,
    Unknown,
}

/// Exact backend identity; name and backend-specific dimensions remain raw
/// bytes. This type does not assume name/domain/path is universally sufficient.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct CookieKey {
    backend_identity_version: u32,
    name: Vec<u8>,
    canonical_domain: String,
    path: String,
    host_only: bool,
    host_only_provenance: IdentityProvenance,
    partition: PartitionIdentity,
    backend_dimensions: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl fmt::Debug for CookieKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CookieKey([redacted])")
    }
}

pub struct CookieKeyInput {
    pub backend_identity_version: u32,
    pub name: Vec<u8>,
    pub domain: String,
    pub path: String,
    pub host_only: bool,
    pub host_only_provenance: IdentityProvenance,
    pub partition: PartitionIdentity,
    pub backend_dimensions: BTreeMap<Vec<u8>, Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CookieModelError {
    InvalidKey,
    InvalidRecord,
    InvalidScope,
    OutOfScope,
    DuplicateKey,
    BackendMismatch,
    IdentityContractMismatch,
    AttributeContractMismatch,
    TooManyRecords,
    MaterialLimit,
}

impl fmt::Display for CookieModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cookie model rejected: {self:?}")
    }
}

impl std::error::Error for CookieModelError {}

impl CookieKey {
    /// # Errors
    /// Rejects malformed, excessive or ambiguous identity components.
    pub fn new(input: CookieKeyInput) -> Result<Self, CookieModelError> {
        if input.backend_identity_version == 0
            || input.name.len() > MAX_NAME_BYTES
            || input.path.len() > MAX_PATH_BYTES
            || !input.path.starts_with('/')
            || input.path.chars().any(char::is_control)
            || input.backend_dimensions.len() > MAX_BACKEND_DIMENSIONS
            || matches!(&input.partition, PartitionIdentity::Known(bytes)
                if bytes.is_empty() || bytes.len() > MAX_PARTITION_BYTES)
            || input.backend_dimensions.iter().any(|(name, value)| {
                name.is_empty() || name.len() > 64 || value.len() > MAX_PARTITION_BYTES
            })
        {
            return Err(CookieModelError::InvalidKey);
        }
        let host = input.domain.trim_start_matches('.');
        if host.is_empty()
            || host.contains(['/', '\\', '@', '?', '#', ':'])
            || input.domain.starts_with("..")
        {
            return Err(CookieModelError::InvalidKey);
        }
        let origin = crate::canonical_origin(&format!("https://{host}"))
            .ok_or(CookieModelError::InvalidKey)?;
        let canonical_domain = origin
            .strip_prefix("https://")
            .ok_or(CookieModelError::InvalidKey)?
            .to_owned();
        if canonical_domain.len() > 253 {
            return Err(CookieModelError::InvalidKey);
        }
        Ok(Self {
            backend_identity_version: input.backend_identity_version,
            name: input.name,
            canonical_domain,
            path: input.path,
            host_only: input.host_only,
            host_only_provenance: input.host_only_provenance,
            partition: input.partition,
            backend_dimensions: input.backend_dimensions,
        })
    }

    #[must_use]
    pub fn name_bytes(&self) -> &[u8] {
        &self.name
    }

    #[must_use]
    pub fn canonical_domain(&self) -> &str {
        &self.canonical_domain
    }

    fn material_bytes(&self) -> usize {
        self.name.len()
            + self.canonical_domain.len()
            + self.path.len()
            + match &self.partition {
                PartitionIdentity::Known(bytes) => bytes.len(),
                PartitionIdentity::Unpartitioned | PartitionIdentity::Unknown => 0,
            }
            + self
                .backend_dimensions
                .iter()
                .map(|(name, value)| name.len() + value.len())
                .sum::<usize>()
    }

    fn identity_complete(&self) -> bool {
        self.host_only_provenance == IdentityProvenance::BackendExact
            && self.partition != PartitionIdentity::Unknown
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct BackendAttributeName(String);

impl BackendAttributeName {
    /// # Errors
    /// Accepts only a bounded, stable backend attribute identifier.
    pub fn new(value: &str) -> Result<Self, CookieModelError> {
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(CookieModelError::InvalidRecord);
        }
        Ok(Self(value.to_owned()))
    }
}

impl fmt::Debug for BackendAttributeName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BackendAttributeName([redacted])")
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum AttributePresence {
    Absent,
    Present(Vec<u8>),
    Unknown,
}

impl fmt::Debug for AttributePresence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Absent => "Absent",
            Self::Present(_) => "Present([redacted])",
            Self::Unknown => "Unknown",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SameSite {
    Unspecified,
    None,
    Lax,
    Strict,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CookieExpiry {
    Session,
    AtUnixMillis(i64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordFidelity {
    Exact,
    ObservedOnly,
}

#[derive(Clone, Eq, PartialEq)]
pub struct CookieRecord {
    key: CookieKey,
    value: Vec<u8>,
    expiry: CookieExpiry,
    secure: bool,
    http_only: bool,
    same_site: SameSite,
    extra_attributes: BTreeMap<BackendAttributeName, AttributePresence>,
    fidelity: RecordFidelity,
}

impl fmt::Debug for CookieRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CookieRecord([redacted])")
    }
}

pub struct CookieRecordInput {
    pub key: CookieKey,
    pub value: Vec<u8>,
    pub expiry: CookieExpiry,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: SameSite,
    pub extra_attributes: BTreeMap<BackendAttributeName, AttributePresence>,
    pub fidelity: RecordFidelity,
}

impl CookieRecord {
    /// # Errors
    /// Rejects a record beyond bounded secret/attribute limits.
    pub fn new(input: CookieRecordInput) -> Result<Self, CookieModelError> {
        if input.value.len() > MAX_VALUE_BYTES
            || input.extra_attributes.len() > MAX_BACKEND_DIMENSIONS
            || input.extra_attributes.values().any(|attribute| {
                matches!(attribute, AttributePresence::Present(value) if value.len() > MAX_ATTRIBUTE_BYTES)
            })
        {
            return Err(CookieModelError::InvalidRecord);
        }
        Ok(Self {
            key: input.key,
            value: input.value,
            expiry: input.expiry,
            secure: input.secure,
            http_only: input.http_only,
            same_site: input.same_site,
            extra_attributes: input.extra_attributes,
            fidelity: input.fidelity,
        })
    }

    #[must_use]
    pub fn key(&self) -> &CookieKey {
        &self.key
    }

    #[must_use]
    pub fn value_bytes(&self) -> &[u8] {
        &self.value
    }

    #[must_use]
    pub const fn expiry(&self) -> CookieExpiry {
        self.expiry
    }

    fn material_bytes(&self) -> usize {
        self.key.material_bytes()
            + self.value.len()
            + self
                .extra_attributes
                .iter()
                .map(|(name, value)| {
                    name.0.len()
                        + match value {
                            AttributePresence::Present(bytes) => bytes.len(),
                            AttributePresence::Absent | AttributePresence::Unknown => 0,
                        }
                })
                .sum::<usize>()
    }

    fn semantic_gap(&self) -> Option<CookieMergeUnavailable> {
        if self.fidelity != RecordFidelity::Exact {
            Some(CookieMergeUnavailable::ObservedOnly)
        } else if self.same_site == SameSite::Unknown
            || self
                .extra_attributes
                .values()
                .any(|value| *value == AttributePresence::Unknown)
        {
            Some(CookieMergeUnavailable::AttributeUnknown)
        } else {
            None
        }
    }

    fn semantic_eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.value == other.value
            && self.expiry == other.expiry
            && self.secure == other.secure
            && self.http_only == other.http_only
            && self.same_site == other.same_site
            && self.extra_attributes == other.extra_attributes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbsenceReason {
    VerifiedMissing,
    ObservedDelete,
    Expired,
}

#[derive(Clone, Eq, PartialEq)]
pub enum CookieObservation {
    Present(CookieRecord),
    Absent {
        key: CookieKey,
        reason: AbsenceReason,
    },
    Unknown {
        key: CookieKey,
        reason: SafeReasonCode,
    },
}

impl fmt::Debug for CookieObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Present(_) => "Present([redacted])",
            Self::Absent { .. } => "Absent([redacted])",
            Self::Unknown { .. } => "Unknown([redacted])",
        })
    }
}

impl CookieObservation {
    #[must_use]
    pub fn key(&self) -> &CookieKey {
        match self {
            Self::Present(record) => record.key(),
            Self::Absent { key, .. } | Self::Unknown { key, .. } => key,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum CookieScope {
    FullProfile,
    Site(String),
}

impl fmt::Debug for CookieScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FullProfile => formatter.write_str("FullProfile"),
            Self::Site(_) => formatter.write_str("Site([redacted])"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotIdentityFidelity {
    Lossless,
    Lossy(SafeReasonCode),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotCompleteness {
    Complete,
    Incomplete(SafeReasonCode),
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotConsistency {
    Consistent,
    ObservedOnly,
}

#[derive(Clone, Eq, PartialEq)]
pub struct CookieSnapshotMeta {
    pub backend_identity_version: u32,
    pub required_identity_dimensions: BTreeSet<Vec<u8>>,
    pub scope: CookieScope,
    pub identity: SnapshotIdentityFidelity,
    pub completeness: SnapshotCompleteness,
    pub consistency: SnapshotConsistency,
    pub supported_attributes: BTreeSet<BackendAttributeName>,
}

impl fmt::Debug for CookieSnapshotMeta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CookieSnapshotMeta")
            .field("backend_identity_version", &self.backend_identity_version)
            .field(
                "required_identity_dimension_count",
                &self.required_identity_dimensions.len(),
            )
            .field("scope", &self.scope)
            .field("identity", &self.identity)
            .field("completeness", &self.completeness)
            .field("consistency", &self.consistency)
            .field(
                "supported_attribute_count",
                &self.supported_attributes.len(),
            )
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct CookieSnapshot {
    meta: CookieSnapshotMeta,
    observations: BTreeMap<CookieKey, CookieObservation>,
}

impl fmt::Debug for CookieSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CookieSnapshot")
            .field("records", &self.observations.len())
            .finish_non_exhaustive()
    }
}

impl CookieSnapshot {
    /// # Errors
    /// Rejects duplicate keys, mismatched backend/attribute contracts and
    /// resource limits. A modelled incomplete snapshot is still constructible
    /// for diagnostics, but the merge planner refuses it.
    pub fn new(
        meta: CookieSnapshotMeta,
        observations: impl IntoIterator<Item = CookieObservation>,
    ) -> Result<Self, CookieModelError> {
        if meta.backend_identity_version == 0
            || meta.supported_attributes.len() > MAX_BACKEND_DIMENSIONS
            || meta.required_identity_dimensions.len() > MAX_BACKEND_DIMENSIONS
            || meta
                .required_identity_dimensions
                .iter()
                .any(|name| name.is_empty() || name.len() > 64)
        {
            return Err(CookieModelError::BackendMismatch);
        }
        if let CookieScope::Site(host) = &meta.scope {
            let requested_origin = format!("https://{host}");
            if host.contains(':')
                || crate::canonical_origin(&requested_origin).as_deref()
                    != Some(requested_origin.as_str())
            {
                return Err(CookieModelError::InvalidScope);
            }
        }
        let mut by_key = BTreeMap::new();
        let mut bytes = 0usize;
        for observation in observations {
            let key = observation.key();
            if key.backend_identity_version != meta.backend_identity_version {
                return Err(CookieModelError::BackendMismatch);
            }
            if !key
                .backend_dimensions
                .keys()
                .eq(meta.required_identity_dimensions.iter())
            {
                return Err(CookieModelError::IdentityContractMismatch);
            }
            if let CookieScope::Site(host) = &meta.scope {
                let matches_host = host == &key.canonical_domain
                    || (!key.host_only
                        && host
                            .strip_suffix(&key.canonical_domain)
                            .is_some_and(|prefix| prefix.ends_with('.')));
                if !matches_host {
                    return Err(CookieModelError::OutOfScope);
                }
            }
            let material = match &observation {
                CookieObservation::Present(record) => {
                    if !record
                        .extra_attributes
                        .keys()
                        .eq(meta.supported_attributes.iter())
                    {
                        return Err(CookieModelError::AttributeContractMismatch);
                    }
                    record.material_bytes()
                }
                CookieObservation::Absent { .. } | CookieObservation::Unknown { .. } => {
                    key.material_bytes()
                }
            };
            bytes = bytes
                .checked_add(material)
                .ok_or(CookieModelError::MaterialLimit)?;
            if bytes > MAX_COOKIE_MATERIAL_BYTES {
                return Err(CookieModelError::MaterialLimit);
            }
            if by_key.insert(key.clone(), observation).is_some() {
                return Err(CookieModelError::DuplicateKey);
            }
            if by_key.len() > MAX_COOKIE_OBSERVATIONS {
                return Err(CookieModelError::TooManyRecords);
            }
        }
        Ok(Self {
            meta,
            observations: by_key,
        })
    }

    #[must_use]
    pub fn meta(&self) -> &CookieSnapshotMeta {
        &self.meta
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.observations.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.observations.is_empty()
    }

    /// Exact, in-memory consumers can inspect records without serializing
    /// cookie material into a UI, log, or persistence representation.
    pub fn observations(&self) -> impl Iterator<Item = &CookieObservation> {
        self.observations.values()
    }

    /// Diagnostic observations may be constructible for analysis, but they
    /// cannot be promoted to an exact backend read when any identity or
    /// semantic attribute is unknown. This says nothing about enumeration
    /// completeness or engine consistency; those need separate evidence.
    #[must_use]
    pub fn records_exact(&self) -> bool {
        self.observations.values().all(|observation| {
            observation.key().identity_complete()
                && match observation {
                    CookieObservation::Present(record) => record.semantic_gap().is_none(),
                    CookieObservation::Absent { .. } => true,
                    CookieObservation::Unknown { .. } => false,
                }
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CookieMergeDecision {
    KeepParent,
    ProposeSet,
    ProposeDelete,
    Converged,
    Conflict,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CookieMergeUnavailable {
    ObservationUnknown,
    AttributeUnknown,
    ObservedOnly,
}

#[derive(Clone, Eq, PartialEq)]
pub struct CookieMergeItem {
    key: CookieKey,
    decision: CookieMergeDecision,
    expected_parent: Option<CookieRecord>,
    proposed: Option<CookieRecord>,
    base_absence: Option<AbsenceReason>,
    parent_absence: Option<AbsenceReason>,
    child_absence: Option<AbsenceReason>,
    unavailable: Option<CookieMergeUnavailable>,
}

impl fmt::Debug for CookieMergeItem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CookieMergeItem")
            .field("decision", &self.decision)
            .finish_non_exhaustive()
    }
}

impl CookieMergeItem {
    #[must_use]
    pub const fn decision(&self) -> CookieMergeDecision {
        self.decision
    }

    #[must_use]
    pub fn key(&self) -> &CookieKey {
        &self.key
    }

    #[must_use]
    pub fn proposed_record(&self) -> Option<&CookieRecord> {
        self.proposed.as_ref()
    }

    #[must_use]
    pub fn expected_parent(&self) -> Option<&CookieRecord> {
        self.expected_parent.as_ref()
    }

    #[must_use]
    pub const fn absence_reasons(&self) -> [Option<AbsenceReason>; 3] {
        [self.base_absence, self.parent_absence, self.child_absence]
    }

    #[must_use]
    pub const fn unavailable_reason(&self) -> Option<CookieMergeUnavailable> {
        self.unavailable
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct CookieMergePlan {
    evaluation_unix_millis: i64,
    items: Vec<CookieMergeItem>,
    fully_classified: bool,
}

impl fmt::Debug for CookieMergePlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CookieMergePlan")
            .field("items", &self.items.len())
            .field("fully_classified", &self.fully_classified)
            .finish_non_exhaustive()
    }
}

impl CookieMergePlan {
    #[must_use]
    pub fn items(&self) -> &[CookieMergeItem] {
        &self.items
    }

    #[must_use]
    pub const fn evaluation_unix_millis(&self) -> i64 {
        self.evaluation_unix_millis
    }

    #[must_use]
    pub const fn fully_classified(&self) -> bool {
        self.fully_classified
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CookieMergeError {
    BackendMismatch,
    ScopeMismatch,
    AttributeContractMismatch,
    IdentityContractMismatch,
    IdentityLossy,
    SnapshotIncomplete,
    SnapshotInconsistent,
    TooManyKeys,
}

impl fmt::Display for CookieMergeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cookie planner unavailable: {self:?}")
    }
}

impl std::error::Error for CookieMergeError {}

#[derive(Clone, Copy)]
enum Effective<'a> {
    Present(&'a CookieRecord),
    Absent(AbsenceReason),
    Unknown(CookieMergeUnavailable),
}

impl Effective<'_> {
    fn absence(self) -> Option<AbsenceReason> {
        match self {
            Self::Absent(reason) => Some(reason),
            Self::Present(_) | Self::Unknown(_) => None,
        }
    }

    fn present(self) -> Option<CookieRecord> {
        match self {
            Self::Present(record) => Some(record.clone()),
            Self::Absent(_) | Self::Unknown(_) => None,
        }
    }

    fn semantic_eq(self, other: Self) -> bool {
        match (self, other) {
            (Self::Present(left), Self::Present(right)) => left.semantic_eq(right),
            (Self::Absent(_), Self::Absent(_)) => true,
            _ => false,
        }
    }

    fn unavailable(self) -> Option<CookieMergeUnavailable> {
        match self {
            Self::Unknown(reason) => Some(reason),
            Self::Present(_) | Self::Absent(_) => None,
        }
    }
}

fn effective(
    observation: Option<&CookieObservation>,
    evaluation_unix_millis: i64,
) -> Effective<'_> {
    match observation {
        None => Effective::Absent(AbsenceReason::VerifiedMissing),
        Some(CookieObservation::Absent { reason, .. }) => Effective::Absent(*reason),
        Some(CookieObservation::Unknown { .. }) => {
            Effective::Unknown(CookieMergeUnavailable::ObservationUnknown)
        }
        Some(CookieObservation::Present(record)) => {
            if let Some(reason) = record.semantic_gap() {
                Effective::Unknown(reason)
            } else if matches!(record.expiry, CookieExpiry::AtUnixMillis(until) if until <= evaluation_unix_millis)
            {
                Effective::Absent(AbsenceReason::Expired)
            } else {
                Effective::Present(record)
            }
        }
    }
}

fn validate_snapshots(snapshots: [&CookieSnapshot; 3]) -> Result<(), CookieMergeError> {
    let reference = snapshots[0].meta();
    for snapshot in snapshots {
        let meta = snapshot.meta();
        if meta.backend_identity_version != reference.backend_identity_version {
            return Err(CookieMergeError::BackendMismatch);
        }
        if meta.scope != reference.scope {
            return Err(CookieMergeError::ScopeMismatch);
        }
        if meta.supported_attributes != reference.supported_attributes {
            return Err(CookieMergeError::AttributeContractMismatch);
        }
        if meta.required_identity_dimensions != reference.required_identity_dimensions {
            return Err(CookieMergeError::IdentityContractMismatch);
        }
        if meta.identity != SnapshotIdentityFidelity::Lossless
            || snapshot
                .observations
                .keys()
                .any(|key| !key.identity_complete())
        {
            return Err(CookieMergeError::IdentityLossy);
        }
        if meta.completeness != SnapshotCompleteness::Complete {
            return Err(CookieMergeError::SnapshotIncomplete);
        }
        if meta.consistency != SnapshotConsistency::Consistent {
            return Err(CookieMergeError::SnapshotInconsistent);
        }
    }
    Ok(())
}

/// Computes a deterministic, memory-only three-way comparison from complete,
/// lossless, mutually compatible snapshots. It never reads a clock or issues a
/// native write. Unknown per-key observations become unavailable items with no
/// proposed operation; a global coverage/identity failure rejects the plan.
///
/// # Errors
/// Returns a payload-free reason for incompatible or unqualified inputs.
pub fn plan_cookie_merge(
    base: &CookieSnapshot,
    parent: &CookieSnapshot,
    child: &CookieSnapshot,
    evaluation_unix_millis: i64,
) -> Result<CookieMergePlan, CookieMergeError> {
    validate_snapshots([base, parent, child])?;
    let keys: BTreeSet<_> = base
        .observations
        .keys()
        .chain(parent.observations.keys())
        .chain(child.observations.keys())
        .collect();
    if keys.len() > MAX_COOKIE_OBSERVATIONS {
        return Err(CookieMergeError::TooManyKeys);
    }
    let mut items = Vec::with_capacity(keys.len());
    let mut fully_classified = true;
    for key in keys {
        let b = effective(base.observations.get(key), evaluation_unix_millis);
        let m = effective(parent.observations.get(key), evaluation_unix_millis);
        let c = effective(child.observations.get(key), evaluation_unix_millis);
        let decision = if matches!(b, Effective::Unknown(_))
            || matches!(m, Effective::Unknown(_))
            || matches!(c, Effective::Unknown(_))
        {
            CookieMergeDecision::Unavailable
        } else if c.semantic_eq(b) {
            CookieMergeDecision::KeepParent
        } else if m.semantic_eq(b) {
            if matches!(c, Effective::Present(_)) {
                CookieMergeDecision::ProposeSet
            } else {
                CookieMergeDecision::ProposeDelete
            }
        } else if m.semantic_eq(c) {
            CookieMergeDecision::Converged
        } else {
            CookieMergeDecision::Conflict
        };
        if matches!(
            decision,
            CookieMergeDecision::Conflict | CookieMergeDecision::Unavailable
        ) {
            fully_classified = false;
        }
        items.push(CookieMergeItem {
            key: key.clone(),
            decision,
            expected_parent: matches!(
                decision,
                CookieMergeDecision::ProposeSet | CookieMergeDecision::ProposeDelete
            )
            .then(|| m.present())
            .flatten(),
            proposed: matches!(decision, CookieMergeDecision::ProposeSet)
                .then(|| c.present())
                .flatten(),
            base_absence: b.absence(),
            parent_absence: m.absence(),
            child_absence: c.absence(),
            unavailable: b
                .unavailable()
                .or_else(|| m.unavailable())
                .or_else(|| c.unavailable()),
        });
    }
    Ok(CookieMergePlan {
        evaluation_unix_millis,
        items,
        fully_classified,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &[u8]) -> CookieKey {
        CookieKey::new(CookieKeyInput {
            backend_identity_version: 1,
            name: name.to_vec(),
            domain: ".Example.test".into(),
            path: "/".into(),
            host_only: false,
            host_only_provenance: IdentityProvenance::BackendExact,
            partition: PartitionIdentity::Unpartitioned,
            backend_dimensions: BTreeMap::new(),
        })
        .unwrap()
    }

    fn record(key: CookieKey, value: &[u8]) -> CookieRecord {
        CookieRecord::new(CookieRecordInput {
            key,
            value: value.to_vec(),
            expiry: CookieExpiry::Session,
            secure: true,
            http_only: true,
            same_site: SameSite::Lax,
            extra_attributes: BTreeMap::new(),
            fidelity: RecordFidelity::Exact,
        })
        .unwrap()
    }

    fn meta() -> CookieSnapshotMeta {
        CookieSnapshotMeta {
            backend_identity_version: 1,
            required_identity_dimensions: BTreeSet::new(),
            scope: CookieScope::FullProfile,
            identity: SnapshotIdentityFidelity::Lossless,
            completeness: SnapshotCompleteness::Complete,
            consistency: SnapshotConsistency::Consistent,
            supported_attributes: BTreeSet::new(),
        }
    }

    #[test]
    fn cookie_scope_and_snapshot_meta_debug_redact_site_identity() {
        let sentinel = "private-cookie-site-sentinel-3a21";
        let scope = CookieScope::Site(sentinel.into());
        let metadata = CookieSnapshotMeta {
            backend_identity_version: 1,
            required_identity_dimensions: [sentinel.as_bytes().to_vec()].into_iter().collect(),
            scope: scope.clone(),
            identity: SnapshotIdentityFidelity::Lossless,
            completeness: SnapshotCompleteness::Complete,
            consistency: SnapshotConsistency::Consistent,
            supported_attributes: BTreeSet::new(),
        };

        assert!(!format!("{scope:?}").contains(sentinel));
        assert!(!format!("{metadata:?}").contains(sentinel));
    }

    fn snapshot(observations: impl IntoIterator<Item = CookieObservation>) -> CookieSnapshot {
        CookieSnapshot::new(meta(), observations).unwrap()
    }

    fn state(key: &CookieKey, value: Option<&[u8]>) -> CookieSnapshot {
        snapshot(
            value
                .into_iter()
                .map(|bytes| CookieObservation::Present(record(key.clone(), bytes))),
        )
    }

    fn decision(
        base: Option<&[u8]>,
        parent: Option<&[u8]>,
        child: Option<&[u8]>,
    ) -> CookieMergeDecision {
        let key = key(b"session");
        let plan = plan_cookie_merge(
            &state(&key, base),
            &state(&key, parent),
            &state(&key, child),
            1_000,
        )
        .unwrap();
        plan.items()[0].decision()
    }

    #[test]
    fn section_18_truth_table_is_exhaustive_for_present_and_absent_states() {
        const A: &[u8] = b"A";
        const B: &[u8] = b"B";
        const C: &[u8] = b"C";
        use CookieMergeDecision::{Conflict, Converged, KeepParent, ProposeDelete, ProposeSet};
        for (base, parent, child, expected) in [
            (Some(A), Some(A), Some(B), ProposeSet),
            (Some(A), Some(B), Some(A), KeepParent),
            (Some(A), Some(B), Some(B), Converged),
            (Some(A), Some(B), Some(C), Conflict),
            (Some(A), Some(A), None, ProposeDelete),
            (Some(A), None, Some(A), KeepParent),
            (Some(A), None, Some(B), Conflict),
            (None, None, Some(B), ProposeSet),
            (None, Some(B), Some(B), Converged),
            (None, Some(B), Some(C), Conflict),
            (Some(A), None, None, Converged),
        ] {
            assert_eq!(
                decision(base, parent, child),
                expected,
                "base={base:?} parent={parent:?} child={child:?}"
            );
        }
    }

    #[test]
    fn raw_bytes_and_attribute_only_changes_are_semantic_not_textual() {
        let key = key(&[0xff, 0x00, b'n']);
        let base = record(key.clone(), &[0x00, 0xff, 0x80]);
        let mut child = base.clone();
        child.secure = false;
        let parent = base.clone();
        let plan = plan_cookie_merge(
            &snapshot([CookieObservation::Present(base.clone())]),
            &snapshot([CookieObservation::Present(parent)]),
            &snapshot([CookieObservation::Present(child.clone())]),
            1_000,
        )
        .unwrap();
        assert_eq!(plan.items()[0].decision(), CookieMergeDecision::ProposeSet);
        assert_eq!(
            plan.items()[0].proposed_record().unwrap().value_bytes(),
            &[0x00, 0xff, 0x80]
        );
        assert!(!format!("{plan:?} {key:?} {child:?}").contains("session"));

        let mut changed_parent = base.clone();
        changed_parent.http_only = false;
        let conflict = plan_cookie_merge(
            &snapshot([CookieObservation::Present(base.clone())]),
            &snapshot([CookieObservation::Present(changed_parent)]),
            &snapshot([CookieObservation::Present(child.clone())]),
            1_000,
        )
        .unwrap();
        assert_eq!(
            conflict.items()[0].decision(),
            CookieMergeDecision::Conflict
        );
        assert!(!conflict.fully_classified());

        child.same_site = SameSite::Strict;
        let mut attribute = BTreeMap::new();
        let name = BackendAttributeName::new("priority").unwrap();
        attribute.insert(name.clone(), AttributePresence::Present(vec![0xff]));
        let mut absent = attribute.clone();
        absent.insert(name.clone(), AttributePresence::Absent);
        let mut with_attribute = child.clone();
        with_attribute.extra_attributes = attribute;
        let mut without_attribute = child;
        without_attribute.extra_attributes = absent;
        let mut contract = meta();
        contract.supported_attributes.insert(name);
        let changed = plan_cookie_merge(
            &CookieSnapshot::new(
                contract.clone(),
                [CookieObservation::Present(without_attribute.clone())],
            )
            .unwrap(),
            &CookieSnapshot::new(
                contract.clone(),
                [CookieObservation::Present(without_attribute)],
            )
            .unwrap(),
            &CookieSnapshot::new(contract, [CookieObservation::Present(with_attribute)]).unwrap(),
            1_000,
        )
        .unwrap();
        assert_eq!(
            changed.items()[0].decision(),
            CookieMergeDecision::ProposeSet
        );
    }

    #[test]
    fn one_injected_time_evaluates_expiry_without_resurrection_or_logout_claim() {
        let key = key(b"auth");
        let mut base = record(key.clone(), b"opaque");
        base.expiry = CookieExpiry::AtUnixMillis(2_000);
        let mut child = base.clone();
        child.expiry = CookieExpiry::AtUnixMillis(500);
        let base_snapshot = snapshot([CookieObservation::Present(base.clone())]);
        let parent_snapshot = snapshot([CookieObservation::Present(base)]);
        let child_snapshot = snapshot([CookieObservation::Present(child)]);
        let early =
            plan_cookie_merge(&base_snapshot, &parent_snapshot, &child_snapshot, 1_000).unwrap();
        assert_eq!(
            early.items()[0].decision(),
            CookieMergeDecision::ProposeDelete
        );
        assert_eq!(
            early.items()[0].absence_reasons()[2],
            Some(AbsenceReason::Expired)
        );
        assert!(early.items()[0].proposed_record().is_none());
        let late =
            plan_cookie_merge(&base_snapshot, &parent_snapshot, &child_snapshot, 3_000).unwrap();
        assert_eq!(late.items()[0].decision(), CookieMergeDecision::KeepParent);
        assert_eq!(
            late.items()[0].absence_reasons(),
            [Some(AbsenceReason::Expired); 3]
        );
        assert!(late.items()[0].proposed_record().is_none());
    }

    #[test]
    fn unknown_items_are_unavailable_and_global_gaps_reject_the_plan() {
        let key = key(b"opaque");
        let reason = SafeReasonCode::try_from("unobserved-attribute").unwrap();
        let complete = state(&key, Some(b"A"));
        let unknown = snapshot([CookieObservation::Unknown {
            key: key.clone(),
            reason: reason.clone(),
        }]);
        let plan = plan_cookie_merge(&complete, &complete, &unknown, 1_000).unwrap();
        assert_eq!(plan.items()[0].decision(), CookieMergeDecision::Unavailable);
        assert_eq!(
            plan.items()[0].unavailable_reason(),
            Some(CookieMergeUnavailable::ObservationUnknown)
        );
        assert!(!plan.fully_classified());
        assert!(plan.items()[0].proposed_record().is_none());
        let mut unknown_same_site = record(key.clone(), b"A");
        unknown_same_site.same_site = SameSite::Unknown;
        let unknown_attribute = snapshot([CookieObservation::Present(unknown_same_site)]);
        let attribute_plan =
            plan_cookie_merge(&complete, &complete, &unknown_attribute, 1_000).unwrap();
        let item = &attribute_plan.items()[0];
        assert_eq!(item.decision(), CookieMergeDecision::Unavailable);
        assert_eq!(
            item.unavailable_reason(),
            Some(CookieMergeUnavailable::AttributeUnknown)
        );
        let attribute_name = BackendAttributeName::new("priority").unwrap();
        let mut unknown_extra = record(key.clone(), b"A");
        unknown_extra
            .extra_attributes
            .insert(attribute_name.clone(), AttributePresence::Unknown);
        let mut attribute_meta = meta();
        attribute_meta
            .supported_attributes
            .insert(attribute_name.clone());
        let mut known_extra = record(key.clone(), b"A");
        known_extra
            .extra_attributes
            .insert(attribute_name, AttributePresence::Absent);
        let known = CookieSnapshot::new(
            attribute_meta.clone(),
            [CookieObservation::Present(known_extra)],
        )
        .unwrap();
        let unknown =
            CookieSnapshot::new(attribute_meta, [CookieObservation::Present(unknown_extra)])
                .unwrap();
        let extra_plan = plan_cookie_merge(&known, &known, &unknown, 1_000).unwrap();
        assert_eq!(
            extra_plan.items()[0].decision(),
            CookieMergeDecision::Unavailable
        );
        assert_eq!(
            extra_plan.items()[0].unavailable_reason(),
            Some(CookieMergeUnavailable::AttributeUnknown)
        );
        let mut observed_record = record(key.clone(), b"A");
        observed_record.fidelity = RecordFidelity::ObservedOnly;
        let observed_plan = plan_cookie_merge(
            &complete,
            &complete,
            &snapshot([CookieObservation::Present(observed_record)]),
            1_000,
        )
        .unwrap();
        let observed_item = &observed_plan.items()[0];
        assert_eq!(
            observed_item.unavailable_reason(),
            Some(CookieMergeUnavailable::ObservedOnly)
        );
        let mut incomplete_meta = meta();
        incomplete_meta.completeness = SnapshotCompleteness::Incomplete(reason.clone());
        let incomplete = CookieSnapshot::new(
            incomplete_meta,
            [CookieObservation::Present(record(key.clone(), b"A"))],
        )
        .unwrap();
        assert_eq!(
            plan_cookie_merge(&complete, &complete, &incomplete, 1_000),
            Err(CookieMergeError::SnapshotIncomplete)
        );
        let mut observed_meta = meta();
        observed_meta.consistency = SnapshotConsistency::ObservedOnly;
        let observed = CookieSnapshot::new(observed_meta, []).unwrap();
        assert_eq!(
            plan_cookie_merge(&complete, &complete, &observed, 1_000),
            Err(CookieMergeError::SnapshotInconsistent)
        );
        let mut lossy_meta = meta();
        lossy_meta.identity = SnapshotIdentityFidelity::Lossy(reason);
        let lossy = CookieSnapshot::new(lossy_meta, []).unwrap();
        assert_eq!(
            plan_cookie_merge(&complete, &complete, &lossy, 1_000),
            Err(CookieMergeError::IdentityLossy)
        );
    }

    #[test]
    fn unknown_partition_or_host_scope_cannot_be_reconciled() {
        let mut input = CookieKeyInput {
            backend_identity_version: 1,
            name: b"n".to_vec(),
            domain: "example.test".into(),
            path: "/".into(),
            host_only: true,
            host_only_provenance: IdentityProvenance::BackendExact,
            partition: PartitionIdentity::Unknown,
            backend_dimensions: BTreeMap::new(),
        };
        let unknown_partition = CookieKey::new(input).unwrap();
        let observed = snapshot([CookieObservation::Present(record(unknown_partition, b"x"))]);
        assert_eq!(
            plan_cookie_merge(&observed, &observed, &observed, 1_000),
            Err(CookieMergeError::IdentityLossy)
        );
        input = CookieKeyInput {
            backend_identity_version: 1,
            name: b"n".to_vec(),
            domain: "example.test".into(),
            path: "/".into(),
            host_only: true,
            host_only_provenance: IdentityProvenance::Unknown,
            partition: PartitionIdentity::Unpartitioned,
            backend_dimensions: BTreeMap::new(),
        };
        let unknown_host_scope = CookieKey::new(input).unwrap();
        let observed = snapshot([CookieObservation::Present(record(unknown_host_scope, b"x"))]);
        assert_eq!(
            plan_cookie_merge(&observed, &observed, &observed, 1_000),
            Err(CookieMergeError::IdentityLossy)
        );
    }

    #[test]
    fn site_scope_admits_only_cookies_applicable_to_that_host() {
        let mut site_meta = meta();
        site_meta.scope = CookieScope::Site("sub.example.test".into());
        let domain_cookie = key(b"domain");
        assert!(
            CookieSnapshot::new(
                site_meta.clone(),
                [CookieObservation::Present(record(domain_cookie, b"x"))]
            )
            .is_ok()
        );
        let host_only_parent = CookieKey::new(CookieKeyInput {
            backend_identity_version: 1,
            name: b"host".to_vec(),
            domain: "example.test".into(),
            path: "/".into(),
            host_only: true,
            host_only_provenance: IdentityProvenance::BackendExact,
            partition: PartitionIdentity::Unpartitioned,
            backend_dimensions: BTreeMap::new(),
        })
        .unwrap();
        assert_eq!(
            CookieSnapshot::new(
                site_meta.clone(),
                [CookieObservation::Present(record(host_only_parent, b"x"))]
            ),
            Err(CookieModelError::OutOfScope)
        );
        let sibling_domain = CookieKey::new(CookieKeyInput {
            backend_identity_version: 1,
            name: b"sibling".to_vec(),
            domain: "otherexample.test".into(),
            path: "/".into(),
            host_only: false,
            host_only_provenance: IdentityProvenance::BackendExact,
            partition: PartitionIdentity::Unpartitioned,
            backend_dimensions: BTreeMap::new(),
        })
        .unwrap();
        assert_eq!(
            CookieSnapshot::new(
                site_meta,
                [CookieObservation::Present(record(sibling_domain, b"x"))]
            ),
            Err(CookieModelError::OutOfScope)
        );
    }

    #[test]
    fn order_and_identity_dimensions_are_deterministic_and_bounded() {
        let first = key(b"a");
        let second = key(b"b");
        let left = snapshot([
            CookieObservation::Present(record(second.clone(), b"2")),
            CookieObservation::Present(record(first.clone(), b"1")),
        ]);
        let right = snapshot([
            CookieObservation::Present(record(first.clone(), b"1")),
            CookieObservation::Present(record(second.clone(), b"2")),
        ]);
        let empty = snapshot([]);
        let a = plan_cookie_merge(&empty, &empty, &left, 1_000).unwrap();
        let b = plan_cookie_merge(&empty, &empty, &right, 1_000).unwrap();
        assert_eq!(a, b);
        assert_eq!(
            a.items()
                .iter()
                .map(|item| item.key().name_bytes())
                .collect::<Vec<_>>(),
            vec![b"a".as_slice(), b"b".as_slice()]
        );
        assert!(a.fully_classified());
        assert_eq!(a.items()[0].decision(), CookieMergeDecision::ProposeSet);
        let mut other_path = CookieKeyInput {
            backend_identity_version: 1,
            name: b"a".to_vec(),
            domain: "example.test".into(),
            path: "/other".into(),
            host_only: false,
            host_only_provenance: IdentityProvenance::BackendExact,
            partition: PartitionIdentity::Known(vec![0, 0xff]),
            backend_dimensions: BTreeMap::new(),
        };
        let partitioned = CookieKey::new(other_path).unwrap();
        assert_ne!(partitioned, first);
        other_path = CookieKeyInput {
            backend_identity_version: 1,
            name: b"a".to_vec(),
            domain: "example.test".into(),
            path: "/".into(),
            host_only: true,
            host_only_provenance: IdentityProvenance::BackendExact,
            partition: PartitionIdentity::Unpartitioned,
            backend_dimensions: BTreeMap::new(),
        };
        assert_ne!(CookieKey::new(other_path).unwrap(), first);
        assert_eq!(
            CookieRecord::new(CookieRecordInput {
                key: first.clone(),
                value: vec![0; MAX_VALUE_BYTES + 1],
                expiry: CookieExpiry::Session,
                secure: false,
                http_only: false,
                same_site: SameSite::Unspecified,
                extra_attributes: BTreeMap::new(),
                fidelity: RecordFidelity::Exact,
            }),
            Err(CookieModelError::InvalidRecord)
        );
        assert_eq!(
            CookieSnapshot::new(
                meta(),
                [
                    CookieObservation::Present(record(first.clone(), b"1")),
                    CookieObservation::Present(record(first, b"2"))
                ]
            ),
            Err(CookieModelError::DuplicateKey)
        );
    }

    #[test]
    fn required_identity_dimensions_are_exact_and_match_across_snapshots() {
        let name = b"backend-slot".to_vec();
        let mut required = meta();
        required.required_identity_dimensions.insert(name.clone());
        let undimensioned = key(b"session");
        assert_eq!(
            CookieSnapshot::new(
                required.clone(),
                [CookieObservation::Present(record(undimensioned, b"secret"))]
            ),
            Err(CookieModelError::IdentityContractMismatch)
        );
        let mut key_input = CookieKeyInput {
            backend_identity_version: 1,
            name: b"session".to_vec(),
            domain: "example.test".into(),
            path: "/".into(),
            host_only: false,
            host_only_provenance: IdentityProvenance::BackendExact,
            partition: PartitionIdentity::Unpartitioned,
            backend_dimensions: BTreeMap::from([(name.clone(), b"slot-one".to_vec())]),
        };
        let dimensioned = CookieKey::new(key_input).unwrap();
        let complete = CookieSnapshot::new(
            required.clone(),
            [CookieObservation::Present(record(
                dimensioned.clone(),
                b"secret",
            ))],
        )
        .unwrap();
        let empty_required = CookieSnapshot::new(required, []).unwrap();
        let empty_default = snapshot([]);
        assert_eq!(
            plan_cookie_merge(&empty_default, &empty_default, &complete, 1_000),
            Err(CookieMergeError::IdentityContractMismatch)
        );
        let plan = plan_cookie_merge(&empty_required, &empty_required, &complete, 1_000).unwrap();
        assert_eq!(plan.items()[0].decision(), CookieMergeDecision::ProposeSet);

        key_input = CookieKeyInput {
            backend_identity_version: 1,
            name: b"session".to_vec(),
            domain: "example.test".into(),
            path: "/".into(),
            host_only: false,
            host_only_provenance: IdentityProvenance::BackendExact,
            partition: PartitionIdentity::Unpartitioned,
            backend_dimensions: BTreeMap::from([(b"unexpected".to_vec(), b"slot".to_vec())]),
        };
        assert_eq!(
            CookieSnapshot::new(
                meta(),
                [CookieObservation::Present(record(
                    CookieKey::new(key_input).unwrap(),
                    b"secret"
                ))]
            ),
            Err(CookieModelError::IdentityContractMismatch)
        );
    }

    #[test]
    fn proposed_opaque_value_and_expiry_are_exact_and_debug_is_redacted() {
        let key = key(b"private-cookie-name");
        let mut child = record(key.clone(), b"private-cookie-value");
        child.expiry = CookieExpiry::AtUnixMillis(5_000);
        let empty = snapshot([]);
        let child_snapshot = snapshot([CookieObservation::Present(child.clone())]);
        let plan = plan_cookie_merge(&empty, &empty, &child_snapshot, 1_000).unwrap();
        let proposed = plan.items()[0].proposed_record().unwrap();
        assert_eq!(proposed.value_bytes(), child.value_bytes());
        assert_eq!(proposed.expiry(), CookieExpiry::AtUnixMillis(5_000));
        let debug = format!(
            "{key:?} {child:?} {child_snapshot:?} {:?} {plan:?}",
            plan.items()[0]
        );
        assert!(!debug.contains("private-cookie-name"));
        assert!(!debug.contains("private-cookie-value"));
    }
}
