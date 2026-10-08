use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use serde::Serialize;

use crate::{ApplicationState, TabId, Target};

use super::{CapturedScope, LogicalPageId, LogicalWindowId};

/// Observed native retention, not an immutable memory snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Retention {
    LiveFrozen,
    LiveActive,
    None,
    Lost,
}

/// Representations may coexist in one commit or retained working state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Representation {
    Structural,
    Archive,
    SemanticPage,
}

/// A projection must not call an unknown count empty or complete.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Coverage(CoverageValue);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
enum CoverageValue {
    Unknown,
    NotRequested,
    Unavailable { requested: u16 },
    Partial { requested: u16, captured: u16 },
    CompleteForRequest { requested: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoverageKind {
    Unknown,
    NotRequested,
    Unavailable,
    Partial,
    CompleteForRequest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageError {
    CapturedExceedsRequest,
}

impl fmt::Display for CoverageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("captured count exceeds the declared request")
    }
}

impl std::error::Error for CoverageError {}

impl Coverage {
    #[must_use]
    pub const fn unknown() -> Self {
        Self(CoverageValue::Unknown)
    }

    /// Classifies bounded request-relative counts. Missing information stays
    /// unknown, including a zero observation with no declared request.
    ///
    /// # Errors
    ///
    /// Returns an error when a captured count exceeds the requested count.
    pub const fn from_counts(
        requested: Option<u16>,
        captured: Option<u16>,
    ) -> Result<Self, CoverageError> {
        let value = match (requested, captured) {
            (Some(requested), Some(captured)) if captured > requested => {
                return Err(CoverageError::CapturedExceedsRequest);
            }
            (Some(0), Some(0)) => CoverageValue::NotRequested,
            (Some(requested), Some(0)) => CoverageValue::Unavailable { requested },
            (Some(requested), Some(captured)) if captured == requested => {
                CoverageValue::CompleteForRequest { requested }
            }
            (Some(requested), Some(captured)) => CoverageValue::Partial {
                requested,
                captured,
            },
            _ => CoverageValue::Unknown,
        };
        Ok(Self(value))
    }

    #[must_use]
    pub const fn kind(self) -> CoverageKind {
        match self.0 {
            CoverageValue::Unknown => CoverageKind::Unknown,
            CoverageValue::NotRequested => CoverageKind::NotRequested,
            CoverageValue::Unavailable { .. } => CoverageKind::Unavailable,
            CoverageValue::Partial { .. } => CoverageKind::Partial,
            CoverageValue::CompleteForRequest { .. } => CoverageKind::CompleteForRequest,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CoverageBreakdown {
    pages: Coverage,
    forms: Coverage,
}

impl CoverageBreakdown {
    #[must_use]
    pub const fn new(pages: Coverage, forms: Coverage) -> Self {
        Self { pages, forms }
    }

    #[must_use]
    pub const fn pages(self) -> Coverage {
        self.pages
    }

    #[must_use]
    pub const fn forms(self) -> Coverage {
        self.forms
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Consistency {
    OwnerConsistent,
    CaptureInterval,
    ObservedOnly,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Identity {
    NotApplicable,
    Clean,
    SnapshotVerified,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Persistence {
    Durable,
    MemoryOnly,
}

/// Redacted, user-facing headlines derived only from the structured facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FidelityLabel {
    Live,
    Lost,
    Archived,
    Reconstructable,
    Structural,
    Partial,
    Unavailable,
    UnknownCoverage,
}

impl FidelityLabel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Live => "LIVE",
            Self::Lost => "LOST",
            Self::Archived => "ARCHIVED",
            Self::Reconstructable => "RECONSTRUCTABLE",
            Self::Structural => "STRUCTURAL",
            Self::Partial => "PARTIAL",
            Self::Unavailable => "UNAVAILABLE",
            Self::UnknownCoverage => "UNKNOWN COVERAGE",
        }
    }
}

/// Independent fidelity facts. This contains no page text or cookie values.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Fidelity {
    retention: Retention,
    representations: BTreeSet<Representation>,
    coverage: CoverageBreakdown,
    consistency: Consistency,
    identity: Identity,
    persistence: Persistence,
}

impl Fidelity {
    #[must_use]
    pub fn new(
        retention: Retention,
        representations: impl IntoIterator<Item = Representation>,
        coverage: CoverageBreakdown,
        consistency: Consistency,
        identity: Identity,
        persistence: Persistence,
    ) -> Self {
        Self {
            retention,
            representations: representations.into_iter().collect(),
            coverage,
            consistency,
            identity,
            persistence,
        }
    }

    #[must_use]
    pub const fn retention(&self) -> Retention {
        self.retention
    }

    #[must_use]
    pub const fn coverage(&self) -> CoverageBreakdown {
        self.coverage
    }

    #[must_use]
    pub fn labels(&self) -> Vec<FidelityLabel> {
        let mut labels = Vec::with_capacity(8);
        match self.retention {
            Retention::LiveActive | Retention::LiveFrozen => labels.push(FidelityLabel::Live),
            Retention::Lost => labels.push(FidelityLabel::Lost),
            Retention::None => {}
        }
        if self.representations.contains(&Representation::Archive) {
            labels.push(FidelityLabel::Archived);
        }
        if self.representations.contains(&Representation::SemanticPage) {
            labels.push(FidelityLabel::Reconstructable);
        }
        if self.representations.contains(&Representation::Structural) {
            labels.push(FidelityLabel::Structural);
        }
        let pages = self.coverage.pages().kind();
        let forms = self.coverage.forms().kind();
        if matches!(pages, CoverageKind::Partial) || matches!(forms, CoverageKind::Partial) {
            labels.push(FidelityLabel::Partial);
        }
        if matches!(pages, CoverageKind::Unavailable) || matches!(forms, CoverageKind::Unavailable)
        {
            labels.push(FidelityLabel::Unavailable);
        }
        if matches!(pages, CoverageKind::Unknown) || matches!(forms, CoverageKind::Unknown) {
            labels.push(FidelityLabel::UnknownCoverage);
        }
        labels
    }
}

/// Missing evidence is not an observed absence. This distinction is retained
/// through capture, diff, and eventual manifest serialization.
#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(tag = "state", content = "value", rename_all = "kebab-case")]
pub enum CaptureField<T> {
    Known(T),
    Absent,
    Unavailable(CaptureUnavailable),
}

impl<T> fmt::Debug for CaptureField<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Known(_) => formatter.write_str("Known([redacted])"),
            Self::Absent => formatter.write_str("Absent"),
            Self::Unavailable(reason) => {
                formatter.debug_tuple("Unavailable").field(reason).finish()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureUnavailable {
    NotObserved,
    Unsupported,
    UnsafeNavigation,
}

/// Only a URL independently proven to be a safe GET may become replayable.
/// The string is bounded and canonicalized; its `Debug` output is redacted.
#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SafePageUrl(SafePageUrlValue);

#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
enum SafePageUrlValue {
    Replayable(String),
    Placeholder(CaptureUnavailable),
}

impl fmt::Debug for SafePageUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            SafePageUrlValue::Replayable(_) => formatter.write_str("Replayable([redacted])"),
            SafePageUrlValue::Placeholder(reason) => {
                formatter.debug_tuple("Placeholder").field(reason).finish()
            }
        }
    }
}

impl SafePageUrl {
    #[must_use]
    pub const fn placeholder(reason: CaptureUnavailable) -> Self {
        Self(SafePageUrlValue::Placeholder(reason))
    }

    #[must_use]
    pub fn from_observed(url: Option<&str>, proven_safe_get: bool) -> Self {
        let Some(url) = url else {
            return Self::placeholder(CaptureUnavailable::NotObserved);
        };
        if !proven_safe_get {
            return Self::placeholder(CaptureUnavailable::UnsafeNavigation);
        }
        let Ok(canonical) = crate::canonicalize(url) else {
            return Self::placeholder(CaptureUnavailable::UnsafeNavigation);
        };
        let Some((scheme, remainder)) = canonical.split_once(':') else {
            return Self::placeholder(CaptureUnavailable::UnsafeNavigation);
        };
        let lower = canonical.to_ascii_lowercase();
        let authority = remainder
            .strip_prefix("//")
            .and_then(|rest| rest.split(['/', '?', '#']).next());
        if !matches!(scheme, "http" | "https" | "file" | "about")
            || (remainder.starts_with("//") && remainder.len() == 2)
            || authority.is_some_and(|value| value.contains('@'))
            || [
                "code=",
                "state=",
                "token=",
                "auth=",
                "session=",
                "samlresponse=",
                "sso=",
            ]
            .iter()
            .any(|marker| lower.contains(marker))
        {
            return Self::placeholder(CaptureUnavailable::UnsafeNavigation);
        }
        let without_fragment = canonical.split('#').next().unwrap_or(&canonical);
        Self(SafePageUrlValue::Replayable(without_fragment.to_owned()))
    }

    #[must_use]
    pub fn replay_url(&self) -> Option<&str> {
        match &self.0 {
            SafePageUrlValue::Replayable(url) => Some(url),
            SafePageUrlValue::Placeholder(_) => None,
        }
    }

    #[must_use]
    pub const fn unavailable_reason(&self) -> Option<CaptureUnavailable> {
        match &self.0 {
            SafePageUrlValue::Replayable(_) => None,
            SafePageUrlValue::Placeholder(reason) => Some(*reason),
        }
    }
}

/// Scroll coordinates use hundredths of a CSS pixel and exclude negative,
/// non-finite, or excessive values before they enter an immutable capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ScrollPoint {
    pub x_hundredths: u32,
    pub y_hundredths: u32,
}

impl ScrollPoint {
    /// # Errors
    /// Rejects non-finite, negative or greater-than-one-million positions.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn new(x: f64, y: f64) -> Result<Self, StructuralCaptureError> {
        fn coordinate(value: f64) -> Option<u32> {
            (value.is_finite() && (0.0..=1_000_000.0).contains(&value))
                .then(|| (value * 100.0).round() as u32)
        }
        Ok(Self {
            x_hundredths: coordinate(x).ok_or(StructuralCaptureError::InvalidScroll)?,
            y_hundredths: coordinate(y).ok_or(StructuralCaptureError::InvalidScroll)?,
        })
    }
}

/// Trusted admission supplies conceptual identity and observation proof. A
/// runtime `TabId` is only a lookup key and never enters the stored page DTO.
#[derive(Clone, Eq, PartialEq)]
pub struct PageCaptureInput {
    pub logical_id: LogicalPageId,
    pub target: Target,
    pub lineage_parent: Option<LogicalPageId>,
    pub proven_safe_get: bool,
    pub scroll: CaptureField<ScrollPoint>,
    pub page_coverage: Coverage,
    pub cookie_coverage: Coverage,
}

impl fmt::Debug for PageCaptureInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PageCaptureInput")
            .field("logical_id", &"[redacted]")
            .field("target", &"[redacted]")
            .field("has_lineage_parent", &self.lineage_parent.is_some())
            .field("proven_safe_get", &self.proven_safe_get)
            .field(
                "has_scroll",
                &matches!(&self.scroll, CaptureField::Known(_)),
            )
            .field("page_coverage", &self.page_coverage)
            .field("cookie_coverage", &self.cookie_coverage)
            .finish()
    }
}

impl PageCaptureInput {
    #[must_use]
    pub fn structural_only(logical_id: LogicalPageId, target: Target) -> Self {
        Self {
            logical_id,
            target,
            lineage_parent: None,
            proven_safe_get: false,
            scroll: CaptureField::Unavailable(CaptureUnavailable::NotObserved),
            page_coverage: Coverage::unknown(),
            cookie_coverage: Coverage::unknown(),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct StructuralPage {
    pub logical_id: LogicalPageId,
    pub lineage_parent: Option<LogicalPageId>,
    pub url: SafePageUrl,
    pub pinned: bool,
    pub muted: bool,
    pub zoom_hundredths: u16,
    pub scroll: CaptureField<ScrollPoint>,
    pub page_coverage: Coverage,
    pub cookie_coverage: Coverage,
}

impl fmt::Debug for StructuralPage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StructuralPage")
            .field("logical_id", &"[redacted]")
            .field("has_lineage_parent", &self.lineage_parent.is_some())
            .field("url", &self.url)
            .field("pinned", &self.pinned)
            .field("muted", &self.muted)
            .field("zoom_hundredths", &self.zoom_hundredths)
            .field(
                "has_scroll",
                &matches!(&self.scroll, CaptureField::Known(_)),
            )
            .field("page_coverage", &self.page_coverage)
            .field("cookie_coverage", &self.cookie_coverage)
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct StructuralWindow {
    pub logical_id: LogicalWindowId,
    pub context_ref: Option<String>,
    pub selected_page: Option<LogicalPageId>,
    pub pages: Vec<StructuralPage>,
}

impl fmt::Debug for StructuralWindow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StructuralWindow")
            .field("logical_id", &"[redacted]")
            .field("has_context_ref", &self.context_ref.is_some())
            .field("has_selected_page", &self.selected_page.is_some())
            .field("page_count", &self.pages.len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq, Serialize)]
pub struct StructuralSnapshot {
    pub application_revision: u64,
    pub windows: Vec<StructuralWindow>,
}

impl fmt::Debug for StructuralSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StructuralSnapshot")
            .field("application_revision", &self.application_revision)
            .field("window_count", &self.windows.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuralCaptureError {
    StaleRevision,
    MissingWindow,
    MissingPage,
    MissingLogicalId,
    DuplicateLogicalId,
    ObservationTargetMismatch,
    InvalidLineage,
    UnexpectedInput,
    InvalidZoom,
    InvalidScroll,
}

impl fmt::Display for StructuralCaptureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "structural capture rejected: {self:?}")
    }
}

impl std::error::Error for StructuralCaptureError {}

/// Materializes exactly the admitted revision and ordered scope. The returned
/// DTO contains no runtime IDs, document tokens, current-focus lookup, page
/// body, form value or cookie material.
///
/// # Errors
/// Rejects stale/missing owners, incomplete identity input, duplicate logical
/// pages and invalid numeric state without producing a partial snapshot.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn project_structural_scope(
    state: &ApplicationState,
    scope: &CapturedScope,
    logical_window: LogicalWindowId,
    inputs: &BTreeMap<TabId, PageCaptureInput>,
) -> Result<StructuralSnapshot, StructuralCaptureError> {
    if state.revision() != scope.revision() {
        return Err(StructuralCaptureError::StaleRevision);
    }
    let window = state
        .windows()
        .get(&scope.window())
        .ok_or(StructuralCaptureError::MissingWindow)?;
    if inputs.len() != scope.members().len() {
        return Err(StructuralCaptureError::UnexpectedInput);
    }
    let mut seen = BTreeSet::new();
    let mut pages = Vec::with_capacity(scope.members().len());
    let mut selected_page = None;
    for member in scope.members() {
        let target = member.target();
        let tab = state
            .tabs()
            .get(&target.tab)
            .ok_or(StructuralCaptureError::MissingPage)?;
        if tab.window != scope.window()
            || tab.document != target.document
            || tab.generation != target.generation
        {
            return Err(StructuralCaptureError::StaleRevision);
        }
        let input = inputs
            .get(&target.tab)
            .ok_or(StructuralCaptureError::MissingLogicalId)?;
        if input.target != target {
            return Err(StructuralCaptureError::ObservationTargetMismatch);
        }
        if !seen.insert(input.logical_id) {
            return Err(StructuralCaptureError::DuplicateLogicalId);
        }
        if input.lineage_parent == Some(input.logical_id) {
            return Err(StructuralCaptureError::InvalidLineage);
        }
        if scope.selected_tab() == Some(target.tab) {
            selected_page = Some(input.logical_id);
        }
        if !tab.zoom.is_finite() || !(0.25..=5.0).contains(&tab.zoom) {
            return Err(StructuralCaptureError::InvalidZoom);
        }
        pages.push(StructuralPage {
            logical_id: input.logical_id,
            lineage_parent: input.lineage_parent,
            url: SafePageUrl::from_observed(tab.url.as_deref(), input.proven_safe_get),
            pinned: tab.pinned,
            muted: tab.muted,
            zoom_hundredths: (tab.zoom * 100.0).round() as u16,
            scroll: input.scroll.clone(),
            page_coverage: input.page_coverage,
            cookie_coverage: input.cookie_coverage,
        });
    }
    Ok(StructuralSnapshot {
        application_revision: scope.revision(),
        windows: vec![StructuralWindow {
            logical_id: logical_window,
            context_ref: window.context.clone(),
            selected_page,
            pages,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ApplicationOwnerId, Event, NativeProfileOwner, PrivacyKind, ProfileInstanceId,
        ScopeSelector, ValidatedUrl, reduce, resolve_scope,
    };

    fn two_page_state_with_offset(
        extra_profile: bool,
    ) -> (ApplicationState, CapturedScope, [TabId; 2]) {
        let mut state = ApplicationState::new();
        if extra_profile {
            reduce(
                &mut state,
                Event::CreateProfile {
                    label: "unrelated".into(),
                    privacy: PrivacyKind::Normal,
                },
            )
            .unwrap();
        }
        reduce(
            &mut state,
            Event::CreateProfile {
                label: "normal".into(),
                privacy: PrivacyKind::Normal,
            },
        )
        .unwrap();
        let profile = *state.profiles().keys().last().unwrap();
        reduce(&mut state, Event::CreateWindow { profile }).unwrap();
        let window = *state.windows().keys().next().unwrap();
        let url = ValidatedUrl::parse("https://example.test/shared#fragment").unwrap();
        for _ in 0..2 {
            reduce(
                &mut state,
                Event::OpenTabWithNavigation {
                    window,
                    url: url.clone(),
                    background: false,
                },
            )
            .unwrap();
            let tab = state.windows()[&window].active_tab.unwrap();
            let target = state.capture_target(tab).unwrap();
            reduce(
                &mut state,
                Event::CommitNavigation {
                    target,
                    url: url.clone(),
                    title: "Private title".into(),
                },
            )
            .unwrap();
        }
        let tabs = [
            state.windows()[&window].tabs[0],
            state.windows()[&window].tabs[1],
        ];
        reduce(
            &mut state,
            Event::SetTabPinned {
                tab: tabs[0],
                pinned: true,
            },
        )
        .unwrap();
        reduce(
            &mut state,
            Event::SetTabMuted {
                tab: tabs[1],
                muted: true,
            },
        )
        .unwrap();
        reduce(
            &mut state,
            Event::SetTabZoom {
                tab: tabs[1],
                zoom_hundredths: 125,
            },
        )
        .unwrap();
        let native = NativeProfileOwner::new(
            ApplicationOwnerId::fresh(),
            profile,
            ProfileInstanceId::fresh(),
        );
        let scope = resolve_scope(&state, ScopeSelector::Window { window }, native).unwrap();
        (state, scope, tabs)
    }

    fn two_page_state() -> (ApplicationState, CapturedScope, [TabId; 2]) {
        two_page_state_with_offset(false)
    }

    fn inputs_for(
        scope: &CapturedScope,
        ids: [LogicalPageId; 2],
    ) -> BTreeMap<TabId, PageCaptureInput> {
        scope
            .members()
            .iter()
            .zip(ids)
            .map(|(member, logical_id)| {
                let target = member.target();
                (
                    target.tab,
                    PageCaptureInput::structural_only(logical_id, target),
                )
            })
            .collect()
    }

    #[test]
    fn structural_projection_keeps_logical_identity_order_and_safe_fields() {
        let (state, scope, tabs) = two_page_state();
        let ids = [LogicalPageId::fresh(), LogicalPageId::fresh()];
        let mut inputs = inputs_for(&scope, ids);
        for input in inputs.values_mut() {
            input.proven_safe_get = true;
        }
        inputs.get_mut(&tabs[1]).unwrap().scroll =
            CaptureField::Known(ScrollPoint::new(12.5, 99.0).unwrap());
        let snapshot =
            project_structural_scope(&state, &scope, LogicalWindowId::fresh(), &inputs).unwrap();
        let window = &snapshot.windows[0];
        assert_eq!(
            window
                .pages
                .iter()
                .map(|page| page.logical_id)
                .collect::<Vec<_>>(),
            ids
        );
        assert_eq!(window.selected_page, Some(ids[1]));
        assert_eq!(window.pages[0].url, window.pages[1].url);
        assert_ne!(window.pages[0].logical_id, window.pages[1].logical_id);
        assert_eq!(
            window.pages[1].url.replay_url(),
            Some("https://example.test/shared")
        );
        assert_eq!(window.pages[1].zoom_hundredths, 125);
        assert_eq!(
            window.pages[1].scroll,
            CaptureField::Known(ScrollPoint {
                x_hundredths: 1250,
                y_hundredths: 9900
            })
        );
        assert!(window.pages[0].pinned);
        assert!(window.pages[1].muted);
        assert_eq!(window.pages[0].page_coverage.kind(), CoverageKind::Unknown);
        assert_eq!(
            window.pages[0].cookie_coverage.kind(),
            CoverageKind::Unknown
        );
        let serialized = serde_json::to_string(&snapshot).unwrap();
        assert!(!serialized.contains("Private title"));
        assert!(!format!("{snapshot:?}").contains("example.test"));
    }

    #[test]
    fn structural_capture_debug_redacts_urls_context_and_page_identity() {
        let sentinel = "private-structural-sentinel-e642";
        let page = StructuralPage {
            logical_id: LogicalPageId::fresh(),
            lineage_parent: Some(LogicalPageId::fresh()),
            url: SafePageUrl::from_observed(
                Some(&format!("https://example.test/?private={sentinel}")),
                true,
            ),
            pinned: false,
            muted: false,
            zoom_hundredths: 100,
            scroll: CaptureField::Known(ScrollPoint::new(1.0, 2.0).expect("point")),
            page_coverage: Coverage::unknown(),
            cookie_coverage: Coverage::unknown(),
        };
        let window = StructuralWindow {
            logical_id: LogicalWindowId::fresh(),
            context_ref: Some(sentinel.into()),
            selected_page: Some(page.logical_id),
            pages: vec![page],
        };
        let snapshot = StructuralSnapshot {
            application_revision: 1,
            windows: vec![window],
        };

        for projection in [
            format!("{:?}", snapshot.windows[0].pages[0]),
            format!("{:?}", snapshot.windows[0]),
            format!("{snapshot:?}"),
        ] {
            assert!(!projection.contains(sentinel), "{projection}");
        }
    }

    #[test]
    fn capture_field_debug_never_formats_known_payloads() {
        let sentinel = "private-capture-field-sentinel-b941";
        let field = CaptureField::Known(sentinel.to_owned());

        assert!(!format!("{field:?}").contains(sentinel));
    }

    #[test]
    fn page_capture_input_debug_redacts_logical_and_runtime_identity() {
        let mut ids = crate::IdSource::new();
        let logical_id = LogicalPageId::fresh();
        let target = crate::Target {
            tab: ids.tab(),
            generation: 1,
            document: ids.document(),
        };
        let input = PageCaptureInput::structural_only(logical_id, target);
        let debug = format!("{input:?}");
        let logical_id = logical_id.to_string();
        let tab_id = target.tab.to_string();
        let document_id = target.document.to_string();

        assert!(!debug.contains(logical_id.as_str()));
        assert!(!debug.contains(tab_id.as_str()));
        assert!(!debug.contains(document_id.as_str()));
    }

    #[test]
    fn structural_projection_rejects_stale_or_duplicate_identity_input() {
        let (mut state, scope, tabs) = two_page_state();
        let id = LogicalPageId::fresh();
        let mut inputs = inputs_for(&scope, [id, id]);
        assert_eq!(
            project_structural_scope(&state, &scope, LogicalWindowId::fresh(), &inputs),
            Err(StructuralCaptureError::DuplicateLogicalId)
        );
        inputs.get_mut(&tabs[1]).unwrap().logical_id = LogicalPageId::fresh();
        let logical_window = LogicalWindowId::fresh();
        let second_target = inputs[&tabs[1]].target;
        let first_target = inputs[&tabs[0]].target;
        inputs.get_mut(&tabs[1]).unwrap().target = first_target;
        assert_eq!(
            project_structural_scope(&state, &scope, logical_window, &inputs),
            Err(StructuralCaptureError::ObservationTargetMismatch)
        );
        inputs.get_mut(&tabs[1]).unwrap().target = second_target;
        let snapshot = project_structural_scope(&state, &scope, logical_window, &inputs).unwrap();
        reduce(
            &mut state,
            Event::SetTabMuted {
                tab: tabs[0],
                muted: true,
            },
        )
        .unwrap();
        assert_eq!(
            project_structural_scope(&state, &scope, logical_window, &inputs),
            Err(StructuralCaptureError::StaleRevision)
        );
        assert_eq!(snapshot.windows[0].pages.len(), 2);
    }

    #[test]
    fn runtime_id_churn_does_not_change_the_structural_diff() {
        let (first_state, first_scope, first_tabs) = two_page_state();
        let (second_state, second_scope, second_tabs) = two_page_state_with_offset(true);
        assert_ne!(first_tabs, second_tabs);
        let ids = [LogicalPageId::fresh(), LogicalPageId::fresh()];
        let logical_window = LogicalWindowId::fresh();
        let first = project_structural_scope(
            &first_state,
            &first_scope,
            logical_window,
            &inputs_for(&first_scope, ids),
        )
        .unwrap();
        let second = project_structural_scope(
            &second_state,
            &second_scope,
            logical_window,
            &inputs_for(&second_scope, ids),
        )
        .unwrap();
        let diff = crate::diff_structural(&first, &second, 0, 100).unwrap();
        assert_eq!(diff.total_events, 0);
        assert!(diff.events.is_empty());
    }

    #[test]
    fn unsafe_urls_and_numeric_values_fail_closed() {
        assert_eq!(
            SafePageUrl::from_observed(Some("https://example.test/"), false),
            SafePageUrl::placeholder(CaptureUnavailable::UnsafeNavigation)
        );
        assert_eq!(
            SafePageUrl::from_observed(None, true),
            SafePageUrl::placeholder(CaptureUnavailable::NotObserved)
        );
        assert_eq!(
            SafePageUrl::from_observed(Some("https://example.test/?token=secret"), true),
            SafePageUrl::placeholder(CaptureUnavailable::UnsafeNavigation)
        );
        assert_eq!(
            SafePageUrl::from_observed(Some("https://user:pass@example.test/"), true),
            SafePageUrl::placeholder(CaptureUnavailable::UnsafeNavigation)
        );
        for value in [f64::NAN, f64::INFINITY, -1.0, 1_000_001.0] {
            assert_eq!(
                ScrollPoint::new(value, 1.0),
                Err(StructuralCaptureError::InvalidScroll)
            );
        }
    }

    #[test]
    fn live_and_partial_are_independent_facts() {
        let report = Fidelity::new(
            Retention::LiveFrozen,
            [Representation::Structural],
            CoverageBreakdown::new(
                Coverage::from_counts(Some(9), Some(7)).unwrap(),
                Coverage::from_counts(Some(0), Some(0)).unwrap(),
            ),
            Consistency::OwnerConsistent,
            Identity::NotApplicable,
            Persistence::MemoryOnly,
        );
        assert_eq!(
            report.labels(),
            vec![
                FidelityLabel::Live,
                FidelityLabel::Structural,
                FidelityLabel::Partial
            ]
        );
        assert_eq!(report.coverage().pages().kind(), CoverageKind::Partial);
        assert_eq!(report.retention(), Retention::LiveFrozen);
    }

    #[test]
    fn archive_does_not_imply_form_coverage() {
        let report = Fidelity::new(
            Retention::None,
            [Representation::Structural, Representation::Archive],
            CoverageBreakdown::new(
                Coverage::from_counts(Some(1), Some(1)).unwrap(),
                Coverage::from_counts(Some(1), Some(0)).unwrap(),
            ),
            Consistency::CaptureInterval,
            Identity::NotApplicable,
            Persistence::Durable,
        );
        assert_eq!(report.coverage().forms().kind(), CoverageKind::Unavailable);
        assert_eq!(
            report.labels(),
            vec![
                FidelityLabel::Archived,
                FidelityLabel::Structural,
                FidelityLabel::Unavailable,
            ]
        );
    }

    #[test]
    fn lost_live_view_can_have_a_durable_fallback_without_becoming_live() {
        let report = Fidelity::new(
            Retention::Lost,
            [Representation::Structural, Representation::Archive],
            CoverageBreakdown::new(
                Coverage::from_counts(Some(1), Some(1)).unwrap(),
                Coverage::from_counts(Some(0), Some(0)).unwrap(),
            ),
            Consistency::CaptureInterval,
            Identity::Unsupported,
            Persistence::Durable,
        );
        assert_eq!(
            report.labels(),
            vec![
                FidelityLabel::Lost,
                FidelityLabel::Archived,
                FidelityLabel::Structural
            ]
        );
        assert!(!report.labels().contains(&FidelityLabel::Live));
    }

    #[test]
    fn unknown_and_incomplete_never_serialize_as_complete_or_empty() {
        let unknown = Coverage::from_counts(None, Some(0)).unwrap();
        let unavailable = Coverage::from_counts(Some(4), Some(0)).unwrap();
        let partial = Coverage::from_counts(Some(4), Some(2)).unwrap();
        let not_requested = Coverage::from_counts(Some(0), Some(0)).unwrap();
        assert_eq!(serde_json::to_value(unknown).unwrap()["state"], "unknown");
        assert_eq!(
            serde_json::to_value(unavailable).unwrap()["state"],
            "unavailable"
        );
        assert_eq!(serde_json::to_value(partial).unwrap()["state"], "partial");
        assert_eq!(
            serde_json::to_value(not_requested).unwrap()["state"],
            "not-requested"
        );
        assert!(Coverage::from_counts(Some(4), Some(5)).is_err());
    }
}
