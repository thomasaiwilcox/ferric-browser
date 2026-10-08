//! Untrusted, bounded semantic page observations. These descriptors can help
//! find candidates later; none grants authority to mutate a live document.

use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    fmt,
};

use serde::{Deserialize, Serialize};

use crate::{ValidatedUrl, canonical_origin};
use sha2::{Digest, Sha256};

pub const MAX_PAGE_SIDECAR_BYTES: usize = 256 * 1024;
pub const PAGE_STATE_HELPER_VERSION: &str = "ferric-page-state-5";
const KEYED_MEDIA_FINGERPRINT_FORMAT_VERSION: u16 = 1;
const MAX_FRAMES: usize = 16;
const MAX_CANDIDATES: usize = 256;
const MAX_POSITIONS: usize = 512;
const MAX_CONTROLS: usize = 256;
const MAX_EDITABLE_VALUES: usize = 128;
const MAX_EXCLUSIONS: usize = 128;
const MAX_EDITABLE_VALUE_BYTES: usize = 4 * 1024;
const MAX_EDITABLE_TOTAL_BYTES: usize = 64 * 1024;
const MAX_POSITION: f64 = 1_000_000_000.0;
const MAX_MEDIA_SECONDS: f64 = 7.0 * 24.0 * 60.0 * 60.0;

/// Returns a stable SHA-256 fingerprint of an HTTP(S) URL's canonical origin
/// and path/query. Fragments are omitted because the page helper qualifies
/// routes using `location.pathname + location.search`.
#[must_use]
pub fn route_fingerprint_for_url(url: &str) -> Option<String> {
    let canonical = ValidatedUrl::parse(url.to_owned()).ok()?;
    let origin = canonical_origin(canonical.as_str())?;
    let route = page_route_for_url(canonical.as_str())?;

    let mut hasher = Sha256::new();
    hasher.update(origin.as_bytes());
    hasher.update([0]);
    hasher.update(route.as_bytes());
    Some(format!("{:x}", hasher.finalize()))
}

/// Derives a capture-local fingerprint from an admitted route and one
/// operation nonce. It is correlation metadata, not a frame identity or
/// restore authority.
#[must_use]
pub fn capture_fingerprint_for_request(route_fingerprint: &str, nonce: &str) -> Option<String> {
    if !valid_fingerprint(route_fingerprint) || !valid_nonce(nonce) {
        return None;
    }
    let mut hasher = Sha256::new();
    hasher.update(b"ferric-page-capture-v1\0");
    hasher.update(route_fingerprint.as_bytes());
    hasher.update([0]);
    hasher.update(nonce.as_bytes());
    Some(format!("{:x}", hasher.finalize()))
}

/// Returns the canonical path and query used to qualify a top document.
/// Fragments are deliberately omitted.
#[must_use]
pub fn page_route_for_url(url: &str) -> Option<String> {
    let canonical = ValidatedUrl::parse(url.to_owned()).ok()?;
    canonical_origin(canonical.as_str())?;
    let (_, after_scheme) = canonical.as_str().split_once("://")?;
    let authority_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    let route_and_fragment = &after_scheme[authority_end..];
    let route_and_query = route_and_fragment
        .split_once('#')
        .map_or(route_and_fragment, |(route, _)| route);
    let route = if route_and_query.is_empty() {
        "/".to_owned()
    } else if route_and_query.starts_with('?') {
        format!("/{route_and_query}")
    } else {
        route_and_query.to_owned()
    };
    Some(route)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SidecarError {
    Malformed,
    TooLarge,
    LegacySchema(u32),
    NewerSchema(u32),
    Invalid(&'static str),
}

impl fmt::Display for SidecarError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => formatter.write_str("page sidecar is malformed"),
            Self::TooLarge => formatter.write_str("page sidecar exceeds its byte limit"),
            Self::LegacySchema(version) => {
                write!(formatter, "page sidecar schema {version} is old")
            }
            Self::NewerSchema(version) => write!(formatter, "page sidecar schema {version} is new"),
            Self::Invalid(field) => write!(formatter, "page sidecar has invalid {field}"),
        }
    }
}

impl std::error::Error for SidecarError {}

#[derive(Deserialize)]
struct SchemaProbe {
    schema_version: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SidecarCategory {
    Geometry,
    Controls,
    Editable,
    Selection,
    Media,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum CandidateKind {
    ScrollArea,
    Details,
    Select,
    Text,
    Textarea,
    Checkbox,
    Radio,
    ContentEditable,
    Media,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SidecarFrame {
    id: u16,
    parent: Option<u16>,
    origin: String,
    route_fingerprint: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SidecarCandidate {
    frame_id: u16,
    candidate_id: u16,
    kind: CandidateKind,
    id_fingerprint: Option<String>,
    name_fingerprint: Option<String>,
    label_fingerprint: Option<String>,
    context_fingerprint: Option<String>,
    source_fingerprint: Option<String>,
}

#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
enum PositionKind {
    TopScrollX,
    TopScrollY,
    ElementScrollX,
    ElementScrollY,
    SelectionStart,
    SelectionEnd,
    MediaTime,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SidecarPosition {
    frame_id: u16,
    candidate_id: Option<u16>,
    kind: PositionKind,
    value: f64,
    paused: Option<bool>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum ControlValue {
    Details { open: bool },
    Select { option_fingerprints: Vec<String> },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SidecarControl {
    frame_id: u16,
    candidate_id: u16,
    value: ControlValue,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SidecarCoverage {
    category: SidecarCategory,
    requested: u16,
    captured: u16,
    truncated: bool,
}

impl SidecarCoverage {
    #[must_use]
    pub const fn category(self) -> SidecarCategory {
        self.category
    }

    #[must_use]
    pub const fn requested(self) -> u16 {
        self.requested
    }

    #[must_use]
    pub const fn captured(self) -> u16 {
        self.captured
    }

    #[must_use]
    pub const fn truncated(self) -> bool {
        self.truncated
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExclusionReason {
    CrossOrigin,
    OpaqueOrigin,
    DeletedFrame,
    NavigatedFrame,
    Sandboxed,
    /// A registered custom element whose public accessor exposes no open root;
    /// this may be a closed root or a component without a shadow root.
    UninspectableComponent,
    SensitiveControl,
    Unsupported,
    Limit,
    Timeout,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SidecarExclusion {
    frame_id: Option<u16>,
    reason: ExclusionReason,
}

#[derive(Clone, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum EditableValue {
    Text(String),
    Checked(bool),
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SensitiveEditableValue {
    frame_id: u16,
    candidate_id: u16,
    value: EditableValue,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPageStateSidecar {
    schema_version: u32,
    helper_version: String,
    nonce: String,
    capture_fingerprint: String,
    requested_categories: Vec<SidecarCategory>,
    frames: Vec<SidecarFrame>,
    candidates: Vec<SidecarCandidate>,
    positions: Vec<SidecarPosition>,
    controls: Vec<SidecarControl>,
    coverage: Vec<SidecarCoverage>,
    exclusions: Vec<SidecarExclusion>,
    sensitive_editable: Vec<SensitiveEditableValue>,
}

/// Full sidecar data can include private editable values. The only public
/// constructor checks byte, schema and semantic limits. It has neither
/// `Serialize` nor `Deserialize`; its `Debug` output is only a summary.
pub struct PageStateSidecar {
    schema_version: u32,
    helper_version: String,
    nonce: String,
    capture_fingerprint: String,
    requested_categories: Vec<SidecarCategory>,
    frames: Vec<SidecarFrame>,
    candidates: Vec<SidecarCandidate>,
    positions: Vec<SidecarPosition>,
    controls: Vec<SidecarControl>,
    coverage: Vec<SidecarCoverage>,
    exclusions: Vec<SidecarExclusion>,
    sensitive_editable: Vec<SensitiveEditableValue>,
}

/// Only category names and counts cross ordinary Debug/UI/IPC boundaries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SidecarSummary {
    pub schema_version: u32,
    pub helper_version: String,
    pub requested_categories: Vec<SidecarCategory>,
    pub frame_count: usize,
    pub candidate_count: usize,
    pub position_count: usize,
    pub control_count: usize,
    pub editable_count: usize,
    pub exclusion_count: usize,
    pub coverage: Vec<SidecarCoverage>,
    pub exclusions_by_reason: BTreeMap<ExclusionReason, u16>,
}

/// Independently captured document evidence for one sidecar. A stored hash
/// alone is never authority over the new document.
pub struct SidecarQualification<'a> {
    pub origin: &'a str,
    pub route_fingerprint: &'a str,
    pub capture_fingerprint: &'a str,
    pub nonce: &'a str,
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
struct GeometryCandidateKey {
    kind: CandidateKind,
    id: Option<String>,
    name: Option<String>,
    label: Option<String>,
    context: String,
    source: Option<String>,
}

fn geometry_key(candidate: &SidecarCandidate) -> Option<GeometryCandidateKey> {
    if candidate.frame_id != 0 || candidate.kind != CandidateKind::ScrollArea {
        return None;
    }
    if candidate.id_fingerprint.is_none() && candidate.name_fingerprint.is_none() {
        return None;
    }
    Some(GeometryCandidateKey {
        kind: candidate.kind,
        id: candidate.id_fingerprint.clone(),
        name: candidate.name_fingerprint.clone(),
        label: candidate.label_fingerprint.clone(),
        context: candidate.context_fingerprint.clone()?,
        source: None,
    })
}

fn geometry_identity_counts(
    candidates: &[SidecarCandidate],
) -> BTreeMap<(&'static str, String), u16> {
    let mut counts = BTreeMap::new();
    for candidate in candidates {
        if candidate.frame_id != 0 || candidate.kind != CandidateKind::ScrollArea {
            continue;
        }
        if let Some(id) = &candidate.id_fingerprint {
            *counts.entry(("id", id.clone())).or_insert(0) += 1;
        }
        if let Some(name) = &candidate.name_fingerprint {
            *counts.entry(("name", name.clone())).or_insert(0) += 1;
        }
    }
    counts
}

fn geometry_identity_unique(
    key: &GeometryCandidateKey,
    counts: &BTreeMap<(&'static str, String), u16>,
) -> bool {
    key.id
        .as_ref()
        .is_none_or(|id| counts.get(&("id", id.clone())) == Some(&1))
        && key
            .name
            .as_ref()
            .is_none_or(|name| counts.get(&("name", name.clone())) == Some(&1))
}

type IdentityCounts = BTreeMap<(&'static str, String), u16>;

enum CandidateMatch<'a> {
    Unique(&'a SidecarCandidate),
    Ambiguous,
    Missing,
}

struct CandidateLookup<'a> {
    current_by_key: BTreeMap<GeometryCandidateKey, Vec<&'a SidecarCandidate>>,
    stored_key_counts: BTreeMap<GeometryCandidateKey, u16>,
    stored_identity_counts: IdentityCounts,
    current_identity_counts: IdentityCounts,
}

impl<'a> CandidateLookup<'a> {
    fn new(
        stored: &[SidecarCandidate],
        current: &'a [SidecarCandidate],
        key_for: fn(&SidecarCandidate) -> Option<GeometryCandidateKey>,
        identity_counts: fn(&[SidecarCandidate]) -> IdentityCounts,
    ) -> Self {
        let mut current_by_key: BTreeMap<GeometryCandidateKey, Vec<&SidecarCandidate>> =
            BTreeMap::new();
        for candidate in current {
            if let Some(key) = key_for(candidate) {
                current_by_key.entry(key).or_default().push(candidate);
            }
        }
        let mut stored_key_counts = BTreeMap::new();
        for candidate in stored {
            if let Some(key) = key_for(candidate) {
                *stored_key_counts.entry(key).or_insert(0_u16) += 1;
            }
        }
        Self {
            current_by_key,
            stored_key_counts,
            stored_identity_counts: identity_counts(stored),
            current_identity_counts: identity_counts(current),
        }
    }

    fn find(&self, key: &GeometryCandidateKey) -> CandidateMatch<'a> {
        let matches = self.current_by_key.get(key);
        if self.stored_key_counts.get(key) != Some(&1)
            || matches.is_some_and(|items| items.len() > 1)
            || !geometry_identity_unique(key, &self.stored_identity_counts)
            || !geometry_identity_unique(key, &self.current_identity_counts)
        {
            return CandidateMatch::Ambiguous;
        }
        matches
            .and_then(|items| items.first())
            .map_or(CandidateMatch::Missing, |candidate| {
                CandidateMatch::Unique(candidate)
            })
    }
}

fn media_key(candidate: &SidecarCandidate) -> Option<GeometryCandidateKey> {
    if candidate.frame_id != 0 || candidate.kind != CandidateKind::Media {
        return None;
    }
    if candidate.id_fingerprint.is_none() && candidate.name_fingerprint.is_none() {
        return None;
    }
    // Unversioned source hints are legacy SHA-256 values. Accept them while
    // parsing old sidecars so unrelated categories remain recoverable, but
    // never let them authorize a media seek under the keyed scheme.
    if candidate
        .source_fingerprint
        .as_deref()
        .and_then(media_source_fingerprint_versions)
        .is_none_or(|(format_version, _)| format_version != KEYED_MEDIA_FINGERPRINT_FORMAT_VERSION)
    {
        return None;
    }
    Some(GeometryCandidateKey {
        kind: candidate.kind,
        id: candidate.id_fingerprint.clone(),
        name: candidate.name_fingerprint.clone(),
        label: candidate.label_fingerprint.clone(),
        context: candidate.context_fingerprint.clone()?,
        source: candidate.source_fingerprint.clone(),
    })
}

fn media_identity_counts(candidates: &[SidecarCandidate]) -> BTreeMap<(&'static str, String), u16> {
    let mut counts = BTreeMap::new();
    for candidate in candidates {
        if candidate.frame_id != 0 || candidate.kind != CandidateKind::Media {
            continue;
        }
        if let Some(id) = &candidate.id_fingerprint {
            *counts.entry(("id", id.clone())).or_insert(0) += 1;
        }
        if let Some(name) = &candidate.name_fingerprint {
            *counts.entry(("name", name.clone())).or_insert(0) += 1;
        }
    }
    counts
}

fn editable_key(candidate: &SidecarCandidate) -> Option<GeometryCandidateKey> {
    if candidate.frame_id != 0
        || !matches!(
            candidate.kind,
            CandidateKind::Text | CandidateKind::Textarea | CandidateKind::Checkbox
        )
        || (candidate.id_fingerprint.is_none() && candidate.name_fingerprint.is_none())
    {
        return None;
    }
    Some(GeometryCandidateKey {
        kind: candidate.kind,
        id: candidate.id_fingerprint.clone(),
        name: candidate.name_fingerprint.clone(),
        label: candidate.label_fingerprint.clone(),
        context: candidate.context_fingerprint.clone()?,
        source: None,
    })
}

fn editable_identity_counts(
    candidates: &[SidecarCandidate],
) -> BTreeMap<(&'static str, String), u16> {
    let mut counts = BTreeMap::new();
    for candidate in candidates {
        if editable_key(candidate).is_none() {
            continue;
        }
        if let Some(id) = &candidate.id_fingerprint {
            *counts.entry(("id", id.clone())).or_insert(0) += 1;
        }
        if let Some(name) = &candidate.name_fingerprint {
            *counts.entry(("name", name.clone())).or_insert(0) += 1;
        }
    }
    counts
}

type GeometryPositionMap = BTreeMap<u16, (Option<f64>, Option<f64>, bool)>;

fn geometry_positions(
    sidecar: &PageStateSidecar,
) -> (Option<f64>, Option<f64>, GeometryPositionMap) {
    let mut top_x = None;
    let mut top_y = None;
    let mut duplicate_top_x = false;
    let mut duplicate_top_y = false;
    let mut positions: GeometryPositionMap = BTreeMap::new();
    for position in &sidecar.positions {
        if position.frame_id != 0 {
            continue;
        }
        match (position.candidate_id, position.kind) {
            (None, PositionKind::TopScrollX) => {
                duplicate_top_x |= top_x.replace(position.value).is_some();
            }
            (None, PositionKind::TopScrollY) => {
                duplicate_top_y |= top_y.replace(position.value).is_some();
            }
            (Some(id), PositionKind::ElementScrollX) => {
                let entry = positions.entry(id).or_default();
                entry.2 |= entry.0.replace(position.value).is_some();
            }
            (Some(id), PositionKind::ElementScrollY) => {
                let entry = positions.entry(id).or_default();
                entry.2 |= entry.1.replace(position.value).is_some();
            }
            _ => {}
        }
    }
    if duplicate_top_x {
        top_x = None;
    }
    if duplicate_top_y {
        top_y = None;
    }
    (top_x, top_y, positions)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeometryPlanError {
    UnqualifiedStoredDocument,
    UnqualifiedCurrentDocument,
    RouteChanged,
}

/// A hint for a uniquely matched live scroll area. Fingerprints remain
/// private; a later helper must independently re-match them immediately
/// before applying offsets. `candidate_id` is not a selector or authority.
pub struct GeometryRestoreTarget {
    current_candidate_id: u16,
    id_fingerprint: Option<String>,
    name_fingerprint: Option<String>,
    label_fingerprint: Option<String>,
    context_fingerprint: String,
    x: Option<f64>,
    y: Option<f64>,
}

impl fmt::Debug for GeometryRestoreTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeometryRestoreTarget")
            .field("current_candidate_id", &self.current_candidate_id)
            .field("identity", &"[redacted]")
            .field("has_x", &self.x.is_some())
            .field("has_y", &self.y.is_some())
            .finish_non_exhaustive()
    }
}

/// Read-only plan. It never changes document scroll-target ownership and has
/// no editable, selection or media restoration actions.
pub struct GeometryRestorePlan {
    expected_origin: String,
    route_fingerprint: String,
    current_capture_fingerprint: String,
    current_nonce: String,
    top_x: Option<f64>,
    top_y: Option<f64>,
    targets: Vec<GeometryRestoreTarget>,
    media_targets: Vec<MediaRestoreTarget>,
    editable_targets: Vec<EditableRestoreTarget>,
    skipped_ambiguous: u16,
    skipped_changed: u16,
    skipped_unsupported: u16,
}

/// A one-shot seek hint. It is emitted only for media paused in both the
/// stored snapshot and the separately observed current page.
struct MediaRestoreTarget {
    current_candidate_id: u16,
    id_fingerprint: Option<String>,
    name_fingerprint: Option<String>,
    label_fingerprint: Option<String>,
    context_fingerprint: String,
    source_fingerprint: String,
    time_seconds: f64,
}

enum EditableRestoreValue {
    Text(String),
    Checked,
}

struct EditableRestoreTarget {
    current_candidate_id: u16,
    id_fingerprint: Option<String>,
    name_fingerprint: Option<String>,
    label_fingerprint: Option<String>,
    context_fingerprint: String,
    kind: &'static str,
    value: EditableRestoreValue,
}

impl fmt::Debug for GeometryRestorePlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeometryRestorePlan")
            .field("top_x_known", &self.top_x.is_some())
            .field("top_y_known", &self.top_y.is_some())
            .field("target_count", &self.targets.len())
            .field("media_target_count", &self.media_targets.len())
            .field("editable_target_count", &self.editable_targets.len())
            .field("skipped_ambiguous", &self.skipped_ambiguous)
            .field("skipped_changed", &self.skipped_changed)
            .field("skipped_unsupported", &self.skipped_unsupported)
            .finish_non_exhaustive()
    }
}

impl GeometryRestorePlan {
    /// Emits only bounded hash hints and numeric offsets for the reviewed
    /// `ApplicationWorld` helper. It is not an IPC or user-facing projection.
    /// The helper must recheck the current URL/document and unique candidates.
    ///
    /// # Errors
    /// Refuses an unsafe route string or an encoding failure.
    pub fn script_request(
        &self,
        expected_route: &str,
        include_media: bool,
        include_editable: bool,
    ) -> Result<String, SidecarError> {
        if expected_route.is_empty()
            || expected_route.len() > 2_048
            || expected_route.chars().any(char::is_control)
        {
            return Err(SidecarError::Invalid("restore route"));
        }
        let targets = self
            .targets
            .iter()
            .map(|target| {
                serde_json::json!({
                    "candidate_id": target.current_candidate_id,
                    "id_fingerprint": target.id_fingerprint,
                    "name_fingerprint": target.name_fingerprint,
                    "label_fingerprint": target.label_fingerprint,
                    "context_fingerprint": target.context_fingerprint,
                    "x": target.x,
                    "y": target.y,
                })
            })
            .collect::<Vec<_>>();
        // INVARIANT: a source equality hint leaves this boundary only for an
        // explicitly opted-in media restore. It never enters Debug/summary.
        let media_targets = if include_media {
            self.media_targets
                .iter()
                .map(|target| {
                    serde_json::json!({
                        "candidate_id": target.current_candidate_id,
                        "id_fingerprint": target.id_fingerprint,
                        "name_fingerprint": target.name_fingerprint,
                        "label_fingerprint": target.label_fingerprint,
                        "context_fingerprint": target.context_fingerprint,
                        "source_fingerprint": target.source_fingerprint,
                        "time_seconds": target.time_seconds,
                    })
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let editable_targets = if include_editable {
            self.editable_targets
                .iter()
                .map(|target| {
                    let value = match &target.value {
                        EditableRestoreValue::Text(text) => {
                            serde_json::json!({"kind":"text", "value":text})
                        }
                        EditableRestoreValue::Checked => {
                            serde_json::json!({"kind":"checked", "value":true})
                        }
                    };
                    serde_json::json!({
                        "candidate_id": target.current_candidate_id,
                        "id_fingerprint": target.id_fingerprint,
                        "name_fingerprint": target.name_fingerprint,
                        "label_fingerprint": target.label_fingerprint,
                        "context_fingerprint": target.context_fingerprint,
                        "kind": target.kind,
                        "value": value,
                    })
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let request = serde_json::json!({
            "expected_origin": self.expected_origin,
            "expected_route": expected_route,
            "route_fingerprint": self.route_fingerprint,
            "capture_fingerprint": self.current_capture_fingerprint,
            "nonce": self.current_nonce,
            "restore_geometry": {
                "top_x": self.top_x,
                "top_y": self.top_y,
                "targets": targets,
                "media_targets": media_targets,
                "editable_targets": editable_targets,
            },
            "max_nodes": 4_096,
            "max_candidates": 64,
            "max_ms": 50,
            "include_controls": false,
            "include_media": include_media,
            "include_editable": include_editable,
        });
        let encoded = serde_json::to_string(&request).map_err(|_| SidecarError::Malformed)?;
        if encoded.len() > 32 * 1_024 {
            return Err(SidecarError::TooLarge);
        }
        Ok(encoded)
    }

    #[must_use]
    pub fn target_count(&self) -> usize {
        self.targets.len()
    }

    #[must_use]
    pub const fn skipped_ambiguous(&self) -> u16 {
        self.skipped_ambiguous
    }

    #[must_use]
    pub const fn skipped_changed(&self) -> u16 {
        self.skipped_changed
    }

    #[must_use]
    pub const fn skipped_unsupported(&self) -> u16 {
        self.skipped_unsupported
    }

    /// The existing ADR-0014 keyboard scroll owner is never restored.
    #[must_use]
    pub const fn keyboard_scroll_target_is_auto(&self) -> bool {
        true
    }
}

impl fmt::Debug for PageStateSidecar {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.summary().fmt(formatter)
    }
}

impl PageStateSidecar {
    /// Parses renderer-supplied JSON with byte and schema checks before any
    /// descriptor can be treated as an application observation.
    ///
    /// # Errors
    /// Refuses malformed, over-limit, future/legacy or inconsistent data.
    pub fn parse_json(bytes: &[u8]) -> Result<Self, SidecarError> {
        if bytes.len() > MAX_PAGE_SIDECAR_BYTES {
            return Err(SidecarError::TooLarge);
        }
        let probe: SchemaProbe =
            serde_json::from_slice(bytes).map_err(|_| SidecarError::Malformed)?;
        match probe.schema_version {
            1 => {}
            0 => return Err(SidecarError::LegacySchema(0)),
            version => return Err(SidecarError::NewerSchema(version)),
        }
        let raw: RawPageStateSidecar =
            serde_json::from_slice(bytes).map_err(|_| SidecarError::Malformed)?;
        let sidecar = Self {
            schema_version: raw.schema_version,
            helper_version: raw.helper_version,
            nonce: raw.nonce,
            capture_fingerprint: raw.capture_fingerprint,
            requested_categories: raw.requested_categories,
            frames: raw.frames,
            candidates: raw.candidates,
            positions: raw.positions,
            controls: raw.controls,
            coverage: raw.coverage,
            exclusions: raw.exclusions,
            sensitive_editable: raw.sensitive_editable,
        };
        sidecar.validate()?;
        Ok(sidecar)
    }

    #[must_use]
    pub fn summary(&self) -> SidecarSummary {
        let mut exclusions_by_reason = BTreeMap::new();
        for exclusion in &self.exclusions {
            *exclusions_by_reason.entry(exclusion.reason).or_insert(0) += 1;
        }
        SidecarSummary {
            schema_version: self.schema_version,
            helper_version: self.helper_version.clone(),
            requested_categories: self.requested_categories.clone(),
            frame_count: self.frames.len(),
            candidate_count: self.candidates.len(),
            position_count: self.positions.len(),
            control_count: self.controls.len(),
            editable_count: self.sensitive_editable.len(),
            exclusion_count: self.exclusions.len(),
            coverage: self.coverage.clone(),
            exclusions_by_reason,
        }
    }

    #[must_use]
    pub fn sensitive_editable_count(&self) -> usize {
        self.sensitive_editable.len()
    }

    /// Returns whether the stored top-frame identity exactly matches the
    /// independently supplied current URL. Sensitive values must not be
    /// previewed for a different origin or route.
    #[must_use]
    pub fn matches_url(&self, url: &str) -> bool {
        let Some(origin) = canonical_origin(url) else {
            return false;
        };
        let Some(route_fingerprint) = route_fingerprint_for_url(url) else {
            return false;
        };
        self.frames
            .iter()
            .find(|frame| frame.id == 0)
            .is_some_and(|top| top.origin == origin && top.route_fingerprint == route_fingerprint)
    }

    /// Returns a bounded, redacted-by-identity preview for the native
    /// one-use editable-recovery dialog. Values are sensitive and this method
    /// must not be used for logging, IPC, or ordinary commit projections.
    ///
    /// # Errors
    /// Returns an error if serialization fails or the encoded preview exceeds
    /// its defensive byte limit.
    pub fn editable_restore_preview_json(&self) -> Result<String, SidecarError> {
        let mut items = Vec::new();
        let mut used_bytes = 0_usize;
        let mut omitted = 0_usize;
        for saved in &self.sensitive_editable {
            if saved.frame_id != 0 {
                omitted += 1;
                continue;
            }
            let kind = self
                .candidates
                .iter()
                .find(|candidate| {
                    candidate.frame_id == 0 && candidate.candidate_id == saved.candidate_id
                })
                .map(|candidate| candidate.kind);
            let item = match (&saved.value, kind) {
                (EditableValue::Text(text), Some(CandidateKind::Text)) if !text.is_empty() => {
                    used_bytes = used_bytes.saturating_add(text.len());
                    (used_bytes <= 16 * 1024)
                        .then(|| serde_json::json!({"kind":"text", "value":text}))
                }
                (EditableValue::Text(text), Some(CandidateKind::Textarea)) if !text.is_empty() => {
                    used_bytes = used_bytes.saturating_add(text.len());
                    (used_bytes <= 16 * 1024)
                        .then(|| serde_json::json!({"kind":"textarea", "value":text}))
                }
                (EditableValue::Text(text), Some(CandidateKind::ContentEditable))
                    if !text.is_empty() =>
                {
                    used_bytes = used_bytes.saturating_add(text.len());
                    (used_bytes <= 16 * 1024)
                        .then(|| serde_json::json!({"kind":"content-editable", "value":text}))
                }
                (EditableValue::Checked(true), Some(CandidateKind::Checkbox)) => {
                    Some(serde_json::json!({"kind":"checkbox", "value":true}))
                }
                _ => None,
            };
            if let Some(item) = item {
                if items.len() < 64 {
                    items.push(item);
                } else {
                    omitted += 1;
                }
            } else {
                omitted += 1;
            }
        }
        let encoded = serde_json::to_string(&serde_json::json!({
            "items": items,
            "omitted": omitted,
        }))
        .map_err(|_| SidecarError::Malformed)?;
        if encoded.len() > 20 * 1024 {
            return Err(SidecarError::TooLarge);
        }
        Ok(encoded)
    }

    /// Candidate descriptors are usable only for the exact admitted origin,
    /// route and capture fingerprint supplied independently by the caller.
    #[must_use]
    pub fn qualifies_for(
        &self,
        expected_origin: &str,
        expected_route_fingerprint: &str,
        expected_capture_fingerprint: &str,
        expected_nonce: &str,
    ) -> bool {
        self.frames
            .iter()
            .find(|frame| frame.id == 0)
            .is_some_and(|top| {
                top.origin == expected_origin
                    && top.route_fingerprint == expected_route_fingerprint
                    && self.capture_fingerprint == expected_capture_fingerprint
                    && self.nonce == expected_nonce
            })
    }

    /// Plans geometry using the current live URL as independent route and
    /// origin evidence. Both sidecars must carry the canonical route
    /// fingerprint for that URL; values copied from the stored sidecar alone
    /// cannot qualify the live document.
    ///
    /// # Errors
    /// Refuses an unsupported URL, mismatched origin/route, or unqualified
    /// stored/current sidecar.
    pub fn plan_geometry_restore_for_url(
        &self,
        current: &Self,
        current_url: &str,
    ) -> Result<GeometryRestorePlan, GeometryPlanError> {
        let origin = canonical_origin(current_url).ok_or(GeometryPlanError::RouteChanged)?;
        let route_fingerprint =
            route_fingerprint_for_url(current_url).ok_or(GeometryPlanError::RouteChanged)?;
        let stored_proof = self
            .qualification_for(&origin, &route_fingerprint)
            .ok_or(GeometryPlanError::UnqualifiedStoredDocument)?;
        let current_proof = current
            .qualification_for(&origin, &route_fingerprint)
            .ok_or(GeometryPlanError::UnqualifiedCurrentDocument)?;
        self.plan_geometry_restore(current, &stored_proof, &current_proof)
    }

    fn qualification_for(
        &self,
        origin: &str,
        route_fingerprint: &str,
    ) -> Option<SidecarQualification<'_>> {
        let top = self.frames.iter().find(|frame| frame.id == 0)?;
        if top.origin != origin || top.route_fingerprint != route_fingerprint {
            return None;
        }
        Some(SidecarQualification {
            origin: &top.origin,
            route_fingerprint: &top.route_fingerprint,
            capture_fingerprint: &self.capture_fingerprint,
            nonce: &self.nonce,
        })
    }

    /// Compares one stored observation with a separately qualified current
    /// document. Only unique top-document scroll areas with an id/name and
    /// matching bounded context are candidates. Ambiguous/changed layout is
    /// skipped; nothing here mutates a live page or chooses a CSS selector.
    ///
    /// # Errors
    /// Refuses an unqualified observation or changed origin/route.
    pub fn plan_geometry_restore(
        &self,
        current: &Self,
        stored_proof: &SidecarQualification<'_>,
        current_proof: &SidecarQualification<'_>,
    ) -> Result<GeometryRestorePlan, GeometryPlanError> {
        if !self.qualifies_for(
            stored_proof.origin,
            stored_proof.route_fingerprint,
            stored_proof.capture_fingerprint,
            stored_proof.nonce,
        ) {
            return Err(GeometryPlanError::UnqualifiedStoredDocument);
        }
        if !current.qualifies_for(
            current_proof.origin,
            current_proof.route_fingerprint,
            current_proof.capture_fingerprint,
            current_proof.nonce,
        ) {
            return Err(GeometryPlanError::UnqualifiedCurrentDocument);
        }
        if stored_proof.origin != current_proof.origin
            || stored_proof.route_fingerprint != current_proof.route_fingerprint
        {
            return Err(GeometryPlanError::RouteChanged);
        }

        let (top_x, top_y, positions) = geometry_positions(self);
        let mut plan = GeometryRestorePlan {
            expected_origin: current_proof.origin.to_owned(),
            route_fingerprint: current_proof.route_fingerprint.to_owned(),
            current_capture_fingerprint: current_proof.capture_fingerprint.to_owned(),
            current_nonce: current_proof.nonce.to_owned(),
            top_x,
            top_y,
            targets: Vec::new(),
            media_targets: Vec::new(),
            editable_targets: Vec::new(),
            skipped_ambiguous: 0,
            skipped_changed: 0,
            skipped_unsupported: 0,
        };
        let geometry_lookup = CandidateLookup::new(
            &self.candidates,
            &current.candidates,
            geometry_key,
            geometry_identity_counts,
        );
        self.plan_scroll_targets(&positions, &geometry_lookup, &mut plan);

        let editable_lookup = CandidateLookup::new(
            &self.candidates,
            &current.candidates,
            editable_key,
            editable_identity_counts,
        );
        self.plan_editable_targets(current, &editable_lookup, &mut plan);

        let media_lookup = CandidateLookup::new(
            &self.candidates,
            &current.candidates,
            media_key,
            media_identity_counts,
        );
        self.plan_media_targets(&media_lookup, &mut plan)?;
        Ok(plan)
    }

    fn plan_scroll_targets(
        &self,
        positions: &GeometryPositionMap,
        lookup: &CandidateLookup<'_>,
        plan: &mut GeometryRestorePlan,
    ) {
        for candidate in &self.candidates {
            let Some((x, y, duplicate_position)) = positions.get(&candidate.candidate_id) else {
                continue;
            };
            if *duplicate_position {
                plan.skipped_ambiguous += 1;
                continue;
            }
            let Some(key) = geometry_key(candidate) else {
                plan.skipped_unsupported += 1;
                continue;
            };
            let current_candidate = match lookup.find(&key) {
                CandidateMatch::Unique(candidate) => candidate,
                CandidateMatch::Ambiguous => {
                    plan.skipped_ambiguous += 1;
                    continue;
                }
                CandidateMatch::Missing => {
                    plan.skipped_changed += 1;
                    continue;
                }
            };
            if plan.targets.len() >= 64 {
                plan.skipped_unsupported += 1;
                continue;
            }
            plan.targets.push(GeometryRestoreTarget {
                current_candidate_id: current_candidate.candidate_id,
                id_fingerprint: key.id,
                name_fingerprint: key.name,
                label_fingerprint: key.label,
                context_fingerprint: key.context,
                x: *x,
                y: *y,
            });
        }
    }

    fn plan_editable_targets(
        &self,
        current: &Self,
        lookup: &CandidateLookup<'_>,
        plan: &mut GeometryRestorePlan,
    ) {
        let mut planned_editable_bytes = 0_usize;
        for saved_value in &self.sensitive_editable {
            if saved_value.frame_id != 0 {
                plan.skipped_unsupported += 1;
                continue;
            }
            let Some(stored_candidate) = self.candidates.iter().find(|candidate| {
                candidate.frame_id == 0 && candidate.candidate_id == saved_value.candidate_id
            }) else {
                plan.skipped_changed += 1;
                continue;
            };
            let Some(key) = editable_key(stored_candidate) else {
                plan.skipped_unsupported += 1;
                continue;
            };
            let (target_kind, target_value, saved_text_bytes) =
                match (&saved_value.value, stored_candidate.kind) {
                    (EditableValue::Text(text), CandidateKind::Text) if !text.is_empty() => {
                        ("text", EditableRestoreValue::Text(text.clone()), text.len())
                    }
                    (EditableValue::Text(text), CandidateKind::Textarea) if !text.is_empty() => (
                        "textarea",
                        EditableRestoreValue::Text(text.clone()),
                        text.len(),
                    ),
                    (EditableValue::Checked(true), CandidateKind::Checkbox) => {
                        ("checkbox", EditableRestoreValue::Checked, 0)
                    }
                    _ => {
                        plan.skipped_unsupported += 1;
                        continue;
                    }
                };
            let current_candidate = match lookup.find(&key) {
                CandidateMatch::Unique(candidate) => candidate,
                CandidateMatch::Ambiguous => {
                    plan.skipped_ambiguous += 1;
                    continue;
                }
                CandidateMatch::Missing => {
                    plan.skipped_changed += 1;
                    continue;
                }
            };
            let current_value = current.sensitive_editable.iter().find(|value| {
                value.frame_id == 0 && value.candidate_id == current_candidate.candidate_id
            });
            let current_is_empty =
                match (&saved_value.value, current_value.map(|value| &value.value)) {
                    (EditableValue::Text(_), Some(EditableValue::Text(value))) => value.is_empty(),
                    (EditableValue::Checked(true), Some(EditableValue::Checked(false))) => true,
                    _ => false,
                };
            if !current_is_empty {
                plan.skipped_changed += 1;
                continue;
            }
            if plan.editable_targets.len() >= 64
                || planned_editable_bytes.saturating_add(saved_text_bytes) > 16 * 1024
            {
                plan.skipped_unsupported += 1;
                continue;
            }
            planned_editable_bytes += saved_text_bytes;
            plan.editable_targets.push(EditableRestoreTarget {
                current_candidate_id: current_candidate.candidate_id,
                id_fingerprint: key.id,
                name_fingerprint: key.name,
                label_fingerprint: key.label,
                context_fingerprint: key.context,
                kind: target_kind,
                value: target_value,
            });
        }
    }

    fn plan_media_targets(
        &self,
        lookup: &CandidateLookup<'_>,
        plan: &mut GeometryRestorePlan,
    ) -> Result<(), GeometryPlanError> {
        let mut media_positions: BTreeMap<u16, (f64, bool)> = BTreeMap::new();
        for position in &self.positions {
            if position.frame_id != 0
                || position.kind != PositionKind::MediaTime
                || position.paused != Some(true)
            {
                continue;
            }
            let Some(candidate_id) = position.candidate_id else {
                continue;
            };
            match media_positions.entry(candidate_id) {
                Entry::Vacant(entry) => {
                    entry.insert((position.value, false));
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().1 = true;
                }
            }
        }
        for candidate in &self.candidates {
            let Some((time_seconds, duplicate)) = media_positions.get(&candidate.candidate_id)
            else {
                continue;
            };
            if *duplicate {
                plan.skipped_ambiguous += 1;
                continue;
            }
            let Some(key) = media_key(candidate) else {
                plan.skipped_unsupported += 1;
                continue;
            };
            let current_candidate = match lookup.find(&key) {
                CandidateMatch::Unique(candidate) => candidate,
                CandidateMatch::Ambiguous => {
                    plan.skipped_ambiguous += 1;
                    continue;
                }
                CandidateMatch::Missing => {
                    plan.skipped_changed += 1;
                    continue;
                }
            };
            if plan.media_targets.len() >= 64 {
                plan.skipped_unsupported += 1;
                continue;
            }
            plan.media_targets.push(MediaRestoreTarget {
                current_candidate_id: current_candidate.candidate_id,
                id_fingerprint: key.id,
                name_fingerprint: key.name,
                label_fingerprint: key.label,
                context_fingerprint: key.context,
                source_fingerprint: key.source.ok_or(GeometryPlanError::RouteChanged)?,
                time_seconds: *time_seconds,
            });
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), SidecarError> {
        if !valid_version(&self.helper_version)
            || !valid_nonce(&self.nonce)
            || !valid_fingerprint(&self.capture_fingerprint)
        {
            return Err(SidecarError::Invalid(
                "version, nonce or capture fingerprint",
            ));
        }
        let requested: BTreeSet<_> = self.requested_categories.iter().copied().collect();
        if requested.is_empty() || requested.len() != self.requested_categories.len() {
            return Err(SidecarError::Invalid("requested categories"));
        }
        let frames = self.validate_frames()?;
        let candidates = self.validate_candidates(&frames)?;
        self.validate_positions(&requested, &candidates)?;
        self.validate_controls(&requested, &candidates)?;
        self.validate_editable(&requested, &candidates)?;
        self.validate_coverage(&requested)?;
        if self.exclusions.len() > MAX_EXCLUSIONS
            || self
                .exclusions
                .iter()
                .any(|item| item.frame_id.is_some_and(|id| !frames.contains_key(&id)))
        {
            return Err(SidecarError::Invalid("exclusions"));
        }
        Ok(())
    }

    fn validate_frames(&self) -> Result<BTreeMap<u16, &SidecarFrame>, SidecarError> {
        if self.frames.is_empty() || self.frames.len() > MAX_FRAMES {
            return Err(SidecarError::Invalid("frame count"));
        }
        let mut frames = BTreeMap::new();
        for frame in &self.frames {
            if canonical_origin(&frame.origin).as_deref() != Some(frame.origin.as_str())
                || !valid_fingerprint(&frame.route_fingerprint)
                || frames.insert(frame.id, frame).is_some()
            {
                return Err(SidecarError::Invalid("frame identity or origin"));
            }
        }
        let top = frames.get(&0).ok_or(SidecarError::Invalid("top frame"))?;
        if top.parent.is_some() {
            return Err(SidecarError::Invalid("top frame"));
        }
        for frame in &self.frames {
            if frame.origin != top.origin || (frame.id != 0 && frame.parent.is_none()) {
                return Err(SidecarError::Invalid("frame origin or parent"));
            }
            let mut next = frame.parent;
            for _ in 0..self.frames.len() {
                let Some(parent_id) = next else { break };
                if parent_id == frame.id {
                    return Err(SidecarError::Invalid("frame cycle"));
                }
                next = frames
                    .get(&parent_id)
                    .ok_or(SidecarError::Invalid("frame parent"))?
                    .parent;
            }
            if next.is_some() {
                return Err(SidecarError::Invalid("frame cycle"));
            }
        }
        Ok(frames)
    }

    fn validate_candidates(
        &self,
        frames: &BTreeMap<u16, &SidecarFrame>,
    ) -> Result<BTreeMap<(u16, u16), CandidateKind>, SidecarError> {
        if self.candidates.len() > MAX_CANDIDATES {
            return Err(SidecarError::Invalid("candidate count"));
        }
        let mut candidates = BTreeMap::new();
        for candidate in &self.candidates {
            let hints = [
                candidate.id_fingerprint.as_deref(),
                candidate.name_fingerprint.as_deref(),
                candidate.label_fingerprint.as_deref(),
                candidate.context_fingerprint.as_deref(),
            ];
            if !frames.contains_key(&candidate.frame_id)
                || hints.iter().all(Option::is_none)
                || hints
                    .into_iter()
                    .flatten()
                    .any(|hash| !valid_fingerprint(hash))
                || match candidate.kind {
                    CandidateKind::Media => {
                        candidate.source_fingerprint.as_deref().is_none_or(|hash| {
                            !valid_fingerprint(hash)
                                && media_source_fingerprint_versions(hash).is_none()
                        })
                    }
                    _ => candidate.source_fingerprint.is_some(),
                }
                || candidates
                    .insert((candidate.frame_id, candidate.candidate_id), candidate.kind)
                    .is_some()
            {
                return Err(SidecarError::Invalid("candidate identity"));
            }
        }
        Ok(candidates)
    }

    fn validate_positions(
        &self,
        requested: &BTreeSet<SidecarCategory>,
        candidates: &BTreeMap<(u16, u16), CandidateKind>,
    ) -> Result<(), SidecarError> {
        if self.positions.len() > MAX_POSITIONS {
            return Err(SidecarError::Invalid("position count"));
        }
        for position in &self.positions {
            if !position.value.is_finite() || position.value.abs() > MAX_POSITION {
                return Err(SidecarError::Invalid("position value"));
            }
            let candidate = position
                .candidate_id
                .and_then(|id| candidates.get(&(position.frame_id, id)));
            let valid = match position.kind {
                PositionKind::TopScrollX | PositionKind::TopScrollY => {
                    position.frame_id == 0
                        && position.candidate_id.is_none()
                        && position.paused.is_none()
                        && requested.contains(&SidecarCategory::Geometry)
                }
                PositionKind::ElementScrollX | PositionKind::ElementScrollY => {
                    candidate == Some(&CandidateKind::ScrollArea)
                        && position.paused.is_none()
                        && requested.contains(&SidecarCategory::Geometry)
                }
                PositionKind::SelectionStart | PositionKind::SelectionEnd => {
                    candidate.is_some_and(|kind| {
                        matches!(
                            kind,
                            CandidateKind::Text
                                | CandidateKind::Textarea
                                | CandidateKind::ContentEditable
                        )
                    }) && position.value >= 0.0
                        && position.value.fract() == 0.0
                        && position.paused.is_none()
                        && requested.contains(&SidecarCategory::Selection)
                }
                PositionKind::MediaTime => {
                    candidate == Some(&CandidateKind::Media)
                        && (0.0..=MAX_MEDIA_SECONDS).contains(&position.value)
                        && position.paused.is_some()
                        && requested.contains(&SidecarCategory::Media)
                }
            };
            if !valid {
                return Err(SidecarError::Invalid("position target or category"));
            }
        }
        Ok(())
    }

    fn validate_controls(
        &self,
        requested: &BTreeSet<SidecarCategory>,
        candidates: &BTreeMap<(u16, u16), CandidateKind>,
    ) -> Result<(), SidecarError> {
        if self.controls.len() > MAX_CONTROLS
            || (!self.controls.is_empty() && !requested.contains(&SidecarCategory::Controls))
        {
            return Err(SidecarError::Invalid("control count or category"));
        }
        let mut seen = BTreeSet::new();
        for control in &self.controls {
            let kind = candidates.get(&(control.frame_id, control.candidate_id));
            let valid = match &control.value {
                ControlValue::Details { open } => {
                    let _ = open;
                    kind == Some(&CandidateKind::Details)
                }
                ControlValue::Select {
                    option_fingerprints,
                } => {
                    kind == Some(&CandidateKind::Select)
                        && option_fingerprints.len() <= 16
                        && option_fingerprints
                            .iter()
                            .all(|item| valid_fingerprint(item))
                        && option_fingerprints.iter().collect::<BTreeSet<_>>().len()
                            == option_fingerprints.len()
                }
            };
            if !valid || !seen.insert((control.frame_id, control.candidate_id)) {
                return Err(SidecarError::Invalid("control target"));
            }
        }
        Ok(())
    }

    fn validate_editable(
        &self,
        requested: &BTreeSet<SidecarCategory>,
        candidates: &BTreeMap<(u16, u16), CandidateKind>,
    ) -> Result<(), SidecarError> {
        if self.sensitive_editable.len() > MAX_EDITABLE_VALUES
            || (!self.sensitive_editable.is_empty()
                && !requested.contains(&SidecarCategory::Editable))
        {
            return Err(SidecarError::Invalid("editable count or category"));
        }
        let mut seen = BTreeSet::new();
        let mut total = 0_usize;
        for item in &self.sensitive_editable {
            let kind = candidates.get(&(item.frame_id, item.candidate_id));
            let valid = match &item.value {
                EditableValue::Text(value) => {
                    total = total.saturating_add(value.len());
                    value.len() <= MAX_EDITABLE_VALUE_BYTES
                        && kind.is_some_and(|kind| {
                            matches!(
                                kind,
                                CandidateKind::Text
                                    | CandidateKind::Textarea
                                    | CandidateKind::ContentEditable
                            )
                        })
                }
                EditableValue::Checked(value) => {
                    let _ = value;
                    kind.is_some_and(|kind| {
                        matches!(kind, CandidateKind::Checkbox | CandidateKind::Radio)
                    })
                }
            };
            if !valid
                || total > MAX_EDITABLE_TOTAL_BYTES
                || !seen.insert((item.frame_id, item.candidate_id))
            {
                return Err(SidecarError::Invalid("editable target or size"));
            }
        }
        Ok(())
    }

    fn validate_coverage(&self, requested: &BTreeSet<SidecarCategory>) -> Result<(), SidecarError> {
        let mut seen = BTreeSet::new();
        for entry in &self.coverage {
            let observed_records = match entry.category {
                SidecarCategory::Geometry => self
                    .positions
                    .iter()
                    .filter(|position| {
                        matches!(
                            position.kind,
                            PositionKind::TopScrollX
                                | PositionKind::TopScrollY
                                | PositionKind::ElementScrollX
                                | PositionKind::ElementScrollY
                        )
                    })
                    .count(),
                SidecarCategory::Controls => self.controls.len(),
                SidecarCategory::Editable => self.sensitive_editable.len(),
                SidecarCategory::Selection => self
                    .positions
                    .iter()
                    .filter(|position| {
                        matches!(
                            position.kind,
                            PositionKind::SelectionStart | PositionKind::SelectionEnd
                        )
                    })
                    .count(),
                SidecarCategory::Media => self
                    .positions
                    .iter()
                    .filter(|position| matches!(position.kind, PositionKind::MediaTime))
                    .count(),
            };
            if !requested.contains(&entry.category)
                || entry.requested == 0
                || entry.captured > entry.requested
                || usize::from(entry.captured) > observed_records
                || !seen.insert(entry.category)
            {
                return Err(SidecarError::Invalid("category coverage"));
            }
        }
        if seen != *requested {
            return Err(SidecarError::Invalid("category coverage"));
        }
        Ok(())
    }
}

fn valid_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_fingerprint(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn media_source_fingerprint_versions(value: &str) -> Option<(u16, u16)> {
    if value.len() > 112 {
        return None;
    }
    let tagged = value.strip_prefix("hmac-sha256-v")?;
    let (format_version, keyed_epoch_and_digest) = tagged.split_once("-k")?;
    let (key_epoch, digest) = keyed_epoch_and_digest.split_once(':')?;
    let parse_version = |value: &str| {
        let number = value.parse::<u16>().ok()?;
        (number > 0 && value == number.to_string()).then_some(number)
    };
    Some((parse_version(format_version)?, parse_version(key_epoch)?))
        .filter(|_| valid_fingerprint(digest))
}

fn valid_nonce(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{
      "schema_version":1,
      "helper_version":"ferric-helper-1",
      "nonce":"11111111111111111111111111111111",
      "capture_fingerprint":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "requested_categories":["geometry","editable"],
      "frames":[{"id":0,"parent":null,"origin":"https://example.test","route_fingerprint":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}],
      "candidates":[{"frame_id":0,"candidate_id":1,"kind":"textarea","id_fingerprint":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","name_fingerprint":null,"label_fingerprint":null,"context_fingerprint":null}],
      "positions":[{"frame_id":0,"candidate_id":null,"kind":"top-scroll-y","value":12.5,"paused":null}],
      "controls":[],
      "coverage":[{"category":"geometry","requested":1,"captured":1,"truncated":false},{"category":"editable","requested":1,"captured":1,"truncated":false}],
      "exclusions":[],
      "sensitive_editable":[{"frame_id":0,"candidate_id":1,"value":{"kind":"text","value":"secret-draft-9"}}]
    }"#;

    #[test]
    fn private_values_never_enter_debug_or_safe_projection() {
        let sidecar = PageStateSidecar::parse_json(VALID.as_bytes()).unwrap();
        let debug = format!("{sidecar:?}");
        let summary = serde_json::to_string(&sidecar.summary()).unwrap();
        assert!(!debug.contains("secret-draft-9"));
        assert!(!summary.contains("secret-draft-9"));
        assert_eq!(sidecar.sensitive_editable_count(), 1);
    }

    #[test]
    fn capture_fingerprint_is_operation_specific_and_rejects_invalid_challenges() {
        let route = route_fingerprint_for_url("https://example.test/page?q=1").unwrap();
        let first = capture_fingerprint_for_request(&route, &"1".repeat(32)).unwrap();
        let second = capture_fingerprint_for_request(&route, &"2".repeat(32)).unwrap();
        assert_ne!(first, second);
        assert!(capture_fingerprint_for_request(&route, "not-a-nonce").is_none());
    }

    #[test]
    fn parser_rejects_new_schema_and_oversized_bytes() {
        let newer = VALID.replace("\"schema_version\":1", "\"schema_version\":2");
        assert!(matches!(
            PageStateSidecar::parse_json(newer.as_bytes()),
            Err(SidecarError::NewerSchema(2))
        ));
        assert!(matches!(
            PageStateSidecar::parse_json(&vec![b' '; 256 * 1024 + 1]),
            Err(SidecarError::TooLarge)
        ));
    }

    #[test]
    fn frame_tree_accepts_capture_local_same_origin_children_and_denied_frame_counts() {
        let mut document: serde_json::Value = serde_json::from_str(VALID).unwrap();
        document["frames"] = serde_json::json!([
            {"id": 0, "parent": null, "origin": "https://example.test",
             "route_fingerprint": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},
            {"id": 1, "parent": 0, "origin": "https://example.test",
             "route_fingerprint": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"}
        ]);
        document["exclusions"] = serde_json::json!([
            {"frame_id": null, "reason": "cross-origin"}
        ]);
        let sidecar = parsed_fixture(&document);
        let summary = sidecar.summary();
        assert_eq!(summary.frame_count, 2);
        assert_eq!(summary.exclusion_count, 1);
        let public_summary = serde_json::to_string(&summary).unwrap();
        let debug = format!("{sidecar:?}");
        let child_route = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
        assert!(!public_summary.contains(child_route));
        assert!(!debug.contains(child_route));
        assert_eq!(
            summary
                .exclusions_by_reason
                .get(&ExclusionReason::CrossOrigin),
            Some(&1)
        );

        document["frames"][1]["origin"] = serde_json::Value::String("https://other.test".into());
        assert!(PageStateSidecar::parse_json(&serde_json::to_vec(&document).unwrap()).is_err());
    }

    #[test]
    fn registered_component_without_public_shadow_root_is_a_typed_exclusion() {
        let mut document: serde_json::Value = serde_json::from_str(VALID).unwrap();
        document["exclusions"] = serde_json::json!([
            {"frame_id": 0, "reason": "uninspectable-component"}
        ]);
        let sidecar = parsed_fixture(&document);
        assert_eq!(
            sidecar
                .summary()
                .exclusions_by_reason
                .get(&ExclusionReason::UninspectableComponent),
            Some(&1)
        );
    }

    #[test]
    fn frame_ids_must_remain_a_parented_capture_local_tree() {
        let mut document: serde_json::Value = serde_json::from_str(VALID).unwrap();
        document["frames"] = serde_json::json!([
            {"id": 0, "parent": null, "origin": "https://example.test",
             "route_fingerprint": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},
            {"id": 1, "parent": 2, "origin": "https://example.test",
             "route_fingerprint": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"},
            {"id": 2, "parent": 1, "origin": "https://example.test",
             "route_fingerprint": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"}
        ]);
        assert!(PageStateSidecar::parse_json(&serde_json::to_vec(&document).unwrap()).is_err());
    }

    #[test]
    fn untrusted_descriptors_reject_invalid_origins_targets_counts_and_values() {
        let base: serde_json::Value = serde_json::from_str(VALID).unwrap();
        let cases = [
            (
                "origin path",
                "/frames/0/origin",
                serde_json::json!("https://example.test/private"),
            ),
            (
                "opaque origin",
                "/frames/0/origin",
                serde_json::json!("null"),
            ),
            (
                "bad helper",
                "/helper_version",
                serde_json::json!("helper\nsecret"),
            ),
            (
                "bad fingerprint",
                "/capture_fingerprint",
                serde_json::json!("short"),
            ),
            ("bad nonce", "/nonce", serde_json::json!("short")),
            (
                "missing candidate",
                "/sensitive_editable/0/candidate_id",
                serde_json::json!(42),
            ),
            (
                "wrong editable kind",
                "/sensitive_editable/0/value/kind",
                serde_json::json!("checked"),
            ),
            (
                "position overflow",
                "/positions/0/value",
                serde_json::json!(1_000_000_001_f64),
            ),
            (
                "wrong position category",
                "/positions/0/kind",
                serde_json::json!("media-time"),
            ),
            ("overclaim", "/coverage/0/captured", serde_json::json!(2)),
            (
                "empty request",
                "/coverage/0/requested",
                serde_json::json!(0),
            ),
            (
                "huge edit",
                "/sensitive_editable/0/value/value",
                serde_json::json!("x".repeat(4097)),
            ),
        ];
        for (name, pointer, replacement) in cases {
            let mut document = base.clone();
            *document.pointer_mut(pointer).unwrap() = replacement;
            let bytes = serde_json::to_vec(&document).unwrap();
            assert!(PageStateSidecar::parse_json(&bytes).is_err(), "{name}");
        }

        let mut duplicate = base.clone();
        duplicate["candidates"] = serde_json::json!([base["candidates"][0], base["candidates"][0]]);
        assert!(PageStateSidecar::parse_json(&serde_json::to_vec(&duplicate).unwrap()).is_err());

        let mut unknown = base.clone();
        unknown["native_command"] = serde_json::json!("open-file");
        assert!(PageStateSidecar::parse_json(&serde_json::to_vec(&unknown).unwrap()).is_err());

        let mut unknown_frame = base.clone();
        unknown_frame["frames"][0]["url"] =
            serde_json::json!("https://example.test/private-frame-url");
        assert!(
            PageStateSidecar::parse_json(&serde_json::to_vec(&unknown_frame).unwrap()).is_err()
        );

        let mut unknown_candidate = base.clone();
        unknown_candidate["candidates"][0]["native_command"] = serde_json::json!("open-file");
        assert!(
            PageStateSidecar::parse_json(&serde_json::to_vec(&unknown_candidate).unwrap()).is_err()
        );

        let mut unknown_editable_value = base.clone();
        unknown_editable_value["sensitive_editable"][0]["value"]["native_command"] =
            serde_json::json!("open-file");
        assert!(
            PageStateSidecar::parse_json(&serde_json::to_vec(&unknown_editable_value).unwrap())
                .is_err()
        );

        let mut fabricated_coverage = base.clone();
        fabricated_coverage["sensitive_editable"] = serde_json::json!([]);
        assert!(
            PageStateSidecar::parse_json(&serde_json::to_vec(&fabricated_coverage).unwrap())
                .is_err()
        );

        let non_finite = VALID.replace("12.5", "1e400");
        assert!(PageStateSidecar::parse_json(non_finite.as_bytes()).is_err());
    }

    #[test]
    fn descriptors_require_independent_exact_document_qualification() {
        let sidecar = PageStateSidecar::parse_json(VALID.as_bytes()).unwrap();
        let route = "b".repeat(64);
        let capture = "a".repeat(64);
        let nonce = "1".repeat(32);
        assert!(sidecar.qualifies_for("https://example.test", &route, &capture, &nonce));
        assert!(!sidecar.qualifies_for("https://foreign.test", &route, &capture, &nonce));
        assert!(!sidecar.qualifies_for("https://example.test", &"c".repeat(64), &capture, &nonce));
        assert!(!sidecar.qualifies_for("https://example.test", &route, &"d".repeat(64), &nonce));
        assert!(!sidecar.qualifies_for("https://example.test", &route, &capture, &"2".repeat(32)));
    }

    fn geometry_fixture() -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1,
            "helper_version": "ferric-page-state-5",
            "nonce": "11111111111111111111111111111111",
            "capture_fingerprint": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "requested_categories": ["geometry"],
            "frames": [{"id": 0, "parent": null, "origin": "https://example.test", "route_fingerprint": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}],
            "candidates": [
                {"frame_id": 0, "candidate_id": 1, "kind": "scroll-area", "id_fingerprint": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc", "name_fingerprint": null, "label_fingerprint": null, "context_fingerprint": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"},
                {"frame_id": 0, "candidate_id": 2, "kind": "scroll-area", "id_fingerprint": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee", "name_fingerprint": null, "label_fingerprint": null, "context_fingerprint": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"}
            ],
            "positions": [
                {"frame_id": 0, "candidate_id": null, "kind": "top-scroll-y", "value": 12.5, "paused": null},
                {"frame_id": 0, "candidate_id": 1, "kind": "element-scroll-y", "value": 40.0, "paused": null},
                {"frame_id": 0, "candidate_id": 2, "kind": "element-scroll-x", "value": 15.0, "paused": null}
            ],
            "controls": [],
            "coverage": [{"category": "geometry", "requested": 3, "captured": 3, "truncated": false}],
            "exclusions": [],
            "sensitive_editable": []
        })
    }

    fn parsed_fixture(value: &serde_json::Value) -> PageStateSidecar {
        PageStateSidecar::parse_json(&serde_json::to_vec(value).unwrap()).unwrap()
    }

    #[test]
    fn geometry_plan_matches_unique_nested_scrollers_but_never_replays_editable_state() {
        let stored = parsed_fixture(&geometry_fixture());
        let mut live_json = geometry_fixture();
        live_json["candidates"][0]["candidate_id"] = serde_json::json!(9);
        live_json["candidates"][1]["candidate_id"] = serde_json::json!(10);
        live_json["positions"] = serde_json::json!([]);
        live_json["coverage"][0]["requested"] = serde_json::json!(1);
        live_json["coverage"][0]["captured"] = serde_json::json!(0);
        let live = parsed_fixture(&live_json);
        let proof = SidecarQualification {
            origin: "https://example.test",
            route_fingerprint: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            capture_fingerprint: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            nonce: "11111111111111111111111111111111",
        };
        let plan = stored.plan_geometry_restore(&live, &proof, &proof).unwrap();
        assert_eq!(plan.target_count(), 2);
        assert_eq!(plan.targets[0].current_candidate_id, 9);
        assert_eq!(plan.targets[1].current_candidate_id, 10);
        assert_eq!(plan.top_y, Some(12.5));
        assert!(plan.keyboard_scroll_target_is_auto());
        let encoded = plan.script_request("/same-route", false, false).unwrap();
        assert!(!encoded.contains("secret-draft"));
        assert!(!format!("{plan:?}").contains("example.test"));
    }

    fn media_fixture(paused: bool, candidate_id: u16, seconds: f64) -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1,
            "helper_version": "ferric-page-state-5",
            "nonce": "11111111111111111111111111111111",
            "capture_fingerprint": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "requested_categories": ["geometry", "media"],
            "frames": [{"id": 0, "parent": null, "origin": "https://example.test", "route_fingerprint": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}],
            "candidates": [{"frame_id": 0, "candidate_id": candidate_id, "kind": "media", "id_fingerprint": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc", "name_fingerprint": null, "label_fingerprint": null, "context_fingerprint": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd", "source_fingerprint": "hmac-sha256-v1-k1:9999999999999999999999999999999999999999999999999999999999999999"}],
            "positions": [
                {"frame_id": 0, "candidate_id": null, "kind": "top-scroll-y", "value": 0.0, "paused": null},
                {"frame_id": 0, "candidate_id": candidate_id, "kind": "media-time", "value": seconds, "paused": paused}
            ],
            "controls": [],
            "coverage": [
                {"category": "geometry", "requested": 1, "captured": 1, "truncated": false},
                {"category": "media", "requested": 1, "captured": 1, "truncated": false}
            ],
            "exclusions": [],
            "sensitive_editable": []
        })
    }

    #[test]
    fn media_restore_requires_saved_pause_and_exact_unique_live_identity() {
        let stored = parsed_fixture(&media_fixture(true, 1, 35.0));
        let live = parsed_fixture(&media_fixture(true, 7, 10.0));
        let source_hint = format!("hmac-sha256-v1-k1:{}", "9".repeat(64));
        assert!(!format!("{stored:?}").contains(&source_hint));
        assert!(
            !serde_json::to_string(&stored.summary())
                .unwrap()
                .contains(&source_hint)
        );
        let proof = SidecarQualification {
            origin: "https://example.test",
            route_fingerprint: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            capture_fingerprint: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            nonce: "11111111111111111111111111111111",
        };
        let plan = stored.plan_geometry_restore(&live, &proof, &proof).unwrap();
        assert_eq!(plan.media_targets.len(), 1);
        let approved = plan.script_request("/same-route", true, false).unwrap();
        assert!(approved.contains("\"time_seconds\":35.0"));
        assert!(approved.contains("\"include_media\":true"));
        assert!(approved.contains(&source_hint));
        let declined = plan.script_request("/same-route", false, false).unwrap();
        assert!(!declined.contains("\"time_seconds\":35.0"));
        assert!(!declined.contains(&source_hint));
        assert!(declined.contains("\"media_targets\":[]"));

        let playing = parsed_fixture(&media_fixture(false, 1, 35.0));
        let plan = playing
            .plan_geometry_restore(&live, &proof, &proof)
            .unwrap();
        assert!(plan.media_targets.is_empty());
    }

    #[test]
    fn legacy_and_unknown_version_media_hints_never_authorize_a_seek() {
        let current = parsed_fixture(&media_fixture(true, 7, 10.0));
        let proof = SidecarQualification {
            origin: "https://example.test",
            route_fingerprint: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            capture_fingerprint: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            nonce: "11111111111111111111111111111111",
        };

        for source_hint in [
            "9".repeat(64),
            format!("hmac-sha256-v1-k2:{}", "8".repeat(64)),
            format!("hmac-sha256-v2-k1:{}", "7".repeat(64)),
        ] {
            let mut stored_value = media_fixture(true, 1, 35.0);
            stored_value["candidates"][0]["source_fingerprint"] = serde_json::json!(source_hint);
            let stored = parsed_fixture(&stored_value);
            let plan = stored
                .plan_geometry_restore(&current, &proof, &proof)
                .expect("unsupported media metadata does not invalidate geometry");
            assert!(plan.media_targets.is_empty());
            assert_eq!(plan.top_y, Some(0.0));
        }
    }

    #[test]
    fn media_restore_skips_replaced_or_ambiguous_live_elements() {
        let stored = parsed_fixture(&media_fixture(true, 1, 35.0));
        let mut changed = media_fixture(true, 7, 10.0);
        changed["candidates"][0]["source_fingerprint"] = serde_json::json!(
            "hmac-sha256-v1-k1:8888888888888888888888888888888888888888888888888888888888888888"
        );
        let changed = parsed_fixture(&changed);
        let proof = SidecarQualification {
            origin: "https://example.test",
            route_fingerprint: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            capture_fingerprint: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            nonce: "11111111111111111111111111111111",
        };
        let plan = stored
            .plan_geometry_restore(&changed, &proof, &proof)
            .unwrap();
        assert!(plan.media_targets.is_empty());
        assert_eq!(plan.skipped_changed(), 1);

        let mut ambiguous = media_fixture(true, 7, 10.0);
        let mut repeated = ambiguous["candidates"][0].clone();
        repeated["candidate_id"] = serde_json::json!(8);
        repeated["context_fingerprint"] =
            serde_json::json!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        ambiguous["candidates"]
            .as_array_mut()
            .unwrap()
            .push(repeated);
        ambiguous["coverage"][1]["requested"] = serde_json::json!(2);
        ambiguous["coverage"][1]["captured"] = serde_json::json!(2);
        ambiguous["positions"].as_array_mut().unwrap().push(serde_json::json!({"frame_id": 0, "candidate_id": 8, "kind": "media-time", "value": 20.0, "paused": true}));
        let ambiguous = parsed_fixture(&ambiguous);
        let plan = stored
            .plan_geometry_restore(&ambiguous, &proof, &proof)
            .unwrap();
        assert!(plan.media_targets.is_empty());
        assert_eq!(plan.skipped_ambiguous(), 1);
    }

    fn editable_fixture(text: &str, candidate_id: u16) -> serde_json::Value {
        serde_json::json!({
            "schema_version": 1,
            "helper_version": "ferric-page-state-5",
            "nonce": "11111111111111111111111111111111",
            "capture_fingerprint": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "requested_categories": ["geometry", "editable"],
            "frames": [{"id": 0, "parent": null, "origin": "https://example.test", "route_fingerprint": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}],
            "candidates": [{"frame_id": 0, "candidate_id": candidate_id, "kind": "text", "id_fingerprint": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc", "name_fingerprint": null, "label_fingerprint": null, "context_fingerprint": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"}],
            "positions": [{"frame_id": 0, "candidate_id": null, "kind": "top-scroll-y", "value": 0.0, "paused": null}],
            "controls": [],
            "coverage": [
                {"category": "geometry", "requested": 1, "captured": 1, "truncated": false},
                {"category": "editable", "requested": 1, "captured": 1, "truncated": false}
            ],
            "exclusions": [],
            "sensitive_editable": [{"frame_id": 0, "candidate_id": candidate_id, "value": {"kind": "text", "value": text}}]
        })
    }

    #[test]
    fn saved_editable_values_are_qualified_to_exact_origin_and_route() {
        let expected_url = "https://example.test/account/edit?draft=1";
        let mut fixture = editable_fixture("synthetic private draft", 1);
        fixture["frames"][0]["route_fingerprint"] =
            serde_json::json!(route_fingerprint_for_url(expected_url).expect("valid fixture URL"));
        let stored = parsed_fixture(&fixture);

        assert!(stored.matches_url(expected_url));
        assert!(stored.matches_url("https://example.test/account/edit?draft=1#section"));
        assert!(!stored.matches_url("https://example.test/account/edit?draft=2"));
        assert!(!stored.matches_url("https://other.test/account/edit?draft=1"));
    }

    #[test]
    fn editable_restore_requires_unique_matching_blank_native_control_and_opt_in() {
        let stored = parsed_fixture(&editable_fixture("synthetic saved draft", 1));
        let live = parsed_fixture(&editable_fixture("", 9));
        let proof = SidecarQualification {
            origin: "https://example.test",
            route_fingerprint: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            capture_fingerprint: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            nonce: "11111111111111111111111111111111",
        };
        let plan = stored.plan_geometry_restore(&live, &proof, &proof).unwrap();
        assert_eq!(plan.editable_targets.len(), 1);
        let declined = plan.script_request("/same-route", false, false).unwrap();
        assert!(!declined.contains("synthetic saved draft"));
        assert!(declined.contains("\"editable_targets\":[]"));
        let accepted = plan.script_request("/same-route", false, true).unwrap();
        assert!(accepted.contains("synthetic saved draft"));
        assert!(accepted.contains("\"include_editable\":true"));
        assert!(!format!("{plan:?}").contains("synthetic saved draft"));

        let occupied = parsed_fixture(&editable_fixture("app value", 9));
        let plan = stored
            .plan_geometry_restore(&occupied, &proof, &proof)
            .unwrap();
        assert!(plan.editable_targets.is_empty());
        assert_eq!(plan.skipped_changed(), 1);
    }

    #[test]
    fn editable_restore_preview_is_bounded_plain_value_json_without_identity() {
        let sensitive = "synthetic private draft value";
        let stored = parsed_fixture(&editable_fixture(sensitive, 1));
        let preview = stored
            .editable_restore_preview_json()
            .expect("bounded native review preview");
        let parsed: serde_json::Value = serde_json::from_str(&preview).expect("preview json");
        assert_eq!(parsed["items"].as_array().unwrap().len(), 1);
        assert_eq!(parsed["items"][0]["kind"], "text");
        assert_eq!(parsed["items"][0]["value"], sensitive);
        assert_eq!(parsed["omitted"], 0);
        assert!(!preview.contains("cccccccccccccccccccccccccccccccc"));
        assert!(!preview.contains("dddddddddddddddddddddddddddddddd"));
        assert!(!format!("{stored:?}").contains(sensitive));

        let mut foreign_frame = editable_fixture(sensitive, 1);
        foreign_frame["frames"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": 1,
                "parent": 0,
                "origin": "https://example.test",
                "route_fingerprint": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
            }));
        foreign_frame["candidates"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "frame_id": 1,
                "candidate_id": 2,
                "kind": "text",
                "id_fingerprint": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                "name_fingerprint": null,
                "label_fingerprint": null,
                "context_fingerprint": "9999999999999999999999999999999999999999999999999999999999999999"
            }));
        foreign_frame["sensitive_editable"][0]["frame_id"] = serde_json::json!(1);
        foreign_frame["sensitive_editable"][0]["candidate_id"] = serde_json::json!(2);
        let foreign_frame = parsed_fixture(&foreign_frame);
        let preview = foreign_frame
            .editable_restore_preview_json()
            .expect("foreign frame is omitted");
        let parsed: serde_json::Value = serde_json::from_str(&preview).expect("preview json");
        assert!(parsed["items"].as_array().unwrap().is_empty());
        assert_eq!(parsed["omitted"], 1);
        assert!(!preview.contains(sensitive));
    }

    #[test]
    fn contenteditable_is_a_plain_text_draft_and_never_a_refill_target() {
        let text = "<b>synthetic rich draft text</b>";
        let mut saved_fixture = editable_fixture(text, 1);
        saved_fixture["candidates"][0]["kind"] = serde_json::json!("content-editable");
        let saved = parsed_fixture(&saved_fixture);
        let preview = saved
            .editable_restore_preview_json()
            .expect("contenteditable is retained only as a bounded text preview");
        let parsed: serde_json::Value = serde_json::from_str(&preview).unwrap();
        assert_eq!(parsed["items"][0]["kind"], "content-editable");
        assert_eq!(parsed["items"][0]["value"], text);
        assert!(!preview.contains("cccccccccccccccccccccccccccccccc"));
        assert!(!preview.contains("dddddddddddddddddddddddddddddddd"));
        assert!(!format!("{saved:?}").contains(text));

        let mut live_fixture = editable_fixture("", 9);
        live_fixture["candidates"][0]["kind"] = serde_json::json!("content-editable");
        let live = parsed_fixture(&live_fixture);
        let proof = SidecarQualification {
            origin: "https://example.test",
            route_fingerprint: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            capture_fingerprint: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            nonce: "11111111111111111111111111111111",
        };
        let plan = saved
            .plan_geometry_restore(&live, &proof, &proof)
            .expect("contenteditable is not an eligible native refill target");
        assert!(plan.editable_targets.is_empty());
    }

    #[test]
    fn geometry_plan_skips_duplicate_identity_and_changed_layout() {
        let stored = parsed_fixture(&geometry_fixture());
        let mut live_json = geometry_fixture();
        let duplicate = live_json["candidates"][0].clone();
        let mut duplicate = duplicate;
        duplicate["candidate_id"] = serde_json::json!(20);
        duplicate["context_fingerprint"] =
            serde_json::json!("7777777777777777777777777777777777777777777777777777777777777777");
        live_json["candidates"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        live_json["candidates"][1]["context_fingerprint"] =
            serde_json::json!("9999999999999999999999999999999999999999999999999999999999999999");
        let live = parsed_fixture(&live_json);
        let proof = SidecarQualification {
            origin: "https://example.test",
            route_fingerprint: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            capture_fingerprint: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            nonce: "11111111111111111111111111111111",
        };
        let plan = stored.plan_geometry_restore(&live, &proof, &proof).unwrap();
        assert_eq!(plan.target_count(), 0);
        assert_eq!(plan.skipped_ambiguous(), 1);
        assert_eq!(plan.skipped_changed(), 1);
    }

    #[test]
    fn geometry_plan_refuses_route_or_document_change() {
        let stored = parsed_fixture(&geometry_fixture());
        let mut live_json = geometry_fixture();
        live_json["frames"][0]["route_fingerprint"] =
            serde_json::json!("9999999999999999999999999999999999999999999999999999999999999999");
        let live = parsed_fixture(&live_json);
        let stored_proof = SidecarQualification {
            origin: "https://example.test",
            route_fingerprint: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            capture_fingerprint: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            nonce: "11111111111111111111111111111111",
        };
        let current_proof = SidecarQualification {
            route_fingerprint: "9999999999999999999999999999999999999999999999999999999999999999",
            ..stored_proof
        };
        assert_eq!(
            stored
                .plan_geometry_restore(&live, &stored_proof, &current_proof)
                .unwrap_err(),
            GeometryPlanError::RouteChanged
        );
        let wrong_nonce = SidecarQualification {
            nonce: "22222222222222222222222222222222",
            ..current_proof
        };
        assert_eq!(
            stored
                .plan_geometry_restore(&live, &stored_proof, &wrong_nonce)
                .unwrap_err(),
            GeometryPlanError::UnqualifiedCurrentDocument
        );
    }

    #[test]
    fn frame_tree_and_nested_input_remain_bounded() {
        let mut document: serde_json::Value = serde_json::from_str(VALID).unwrap();
        let mut child = document["frames"][0].clone();
        child["id"] = serde_json::json!(1);
        child["parent"] = serde_json::json!(1);
        document["frames"].as_array_mut().unwrap().push(child);
        assert!(PageStateSidecar::parse_json(&serde_json::to_vec(&document).unwrap()).is_err());

        let mut document: serde_json::Value = serde_json::from_str(VALID).unwrap();
        let extra = document["candidates"][0].clone();
        document["candidates"] = serde_json::Value::Array(vec![extra; MAX_CANDIDATES + 1]);
        assert!(PageStateSidecar::parse_json(&serde_json::to_vec(&document).unwrap()).is_err());

        let deeply_nested = format!("{}0{}", "[".repeat(130), "]".repeat(130));
        assert!(PageStateSidecar::parse_json(deeply_nested.as_bytes()).is_err());
    }
}
