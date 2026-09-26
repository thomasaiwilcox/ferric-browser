//! Qt-independent state and geometry for keyboard-driven spatial navigation.
//!
//! The session in this module deliberately knows nothing about a renderer or
//! a pointer device.  It owns only the user intent (the selected rectangle),
//! the identity of the surface that intent belongs to, and one in-flight
//! dispatch transaction.  Browser/Qt adapters are responsible for observing
//! that surface and for delivering the resulting request.

use std::fmt;

use crate::{IdSource, SpatialRequestId, SpatialSessionId, Target, WindowId};

pub const MAX_SPATIAL_DEPTH: u8 = 8;
pub const SPATIAL_PRECISION_LIMIT: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalPoint {
    pub x: f64,
    pub y: f64,
}

impl LogicalPoint {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl LogicalRect {
    /// Creates a logical rectangle after validating all derived edges.
    ///
    /// Coordinates may be negative because the type is also useful for
    /// adapter-side mapping.  A root surface is required to use `(0, 0)`.
    #[must_use]
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Option<Self> {
        let rect = Self {
            x,
            y,
            width,
            height,
        };
        if !x.is_finite()
            || !y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
            || !(x + width).is_finite()
            || !(y + height).is_finite()
        {
            return None;
        }
        Some(rect)
    }

    #[must_use]
    pub fn center(self) -> LogicalPoint {
        LogicalPoint::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    #[must_use]
    pub fn contains(self, point: LogicalPoint) -> bool {
        point.is_finite()
            && point.x >= self.x
            && point.y >= self.y
            && point.x < self.x + self.width
            && point.y < self.y + self.height
    }

    #[must_use]
    pub fn is_at_precision_limit(self) -> bool {
        self.width <= SPATIAL_PRECISION_LIMIT && self.height <= SPATIAL_PRECISION_LIMIT
    }

    fn child(self, cell: GridCell) -> Self {
        let (column, row) = cell.column_row();
        let width = self.width / 3.0;
        let height = self.height / 3.0;
        // A validated positive finite rectangle remains valid for the bounded
        // eight-level subdivision on supported view sizes.
        Self {
            x: self.x + width * f64::from(column),
            y: self.y + height * f64::from(row),
            width,
            height,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GridCell {
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
}

impl GridCell {
    #[must_use]
    pub const fn from_digit(digit: char) -> Option<Self> {
        Some(match digit {
            '1' => Self::One,
            '2' => Self::Two,
            '3' => Self::Three,
            '4' => Self::Four,
            '5' => Self::Five,
            '6' => Self::Six,
            '7' => Self::Seven,
            '8' => Self::Eight,
            '9' => Self::Nine,
            _ => return None,
        })
    }

    #[must_use]
    pub const fn digit(self) -> char {
        match self {
            Self::One => '1',
            Self::Two => '2',
            Self::Three => '3',
            Self::Four => '4',
            Self::Five => '5',
            Self::Six => '6',
            Self::Seven => '7',
            Self::Eight => '8',
            Self::Nine => '9',
        }
    }

    #[must_use]
    pub const fn column_row(self) -> (u8, u8) {
        match self {
            Self::One => (0, 0),
            Self::Two => (1, 0),
            Self::Three => (2, 0),
            Self::Four => (0, 1),
            Self::Five => (1, 1),
            Self::Six => (2, 1),
            Self::Seven => (0, 2),
            Self::Eight => (1, 2),
            Self::Nine => (2, 2),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SpatialAction {
    Hover,
    Click(PointerButton),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SpatialLifecycle {
    AwaitingSurface,
    Selecting,
    DispatchPending,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SpatialCancelReason {
    User,
    ActionDelivered,
    SurfaceUnavailable,
    TargetChanged,
    ViewChanged,
    GeometryChanged,
    WindowInactive,
    PromptOwnedInput,
    RendererUnavailable,
    PhysicalPointer,
    DispatchRejected,
    DispatchUncertain,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SurfaceStamp {
    pub serial: u64,
    pub revision: u64,
}

impl SurfaceStamp {
    #[must_use]
    pub const fn new(serial: u64, revision: u64) -> Option<Self> {
        if serial == 0 || revision == 0 {
            None
        } else {
            Some(Self { serial, revision })
        }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.serial != 0 && self.revision != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpatialTarget {
    pub window: WindowId,
    pub target: Target,
}

impl SpatialTarget {
    #[must_use]
    pub const fn new(window: WindowId, target: Target) -> Self {
        Self { window, target }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpatialOwner {
    pub target: SpatialTarget,
    pub surface: SurfaceStamp,
}

impl SpatialOwner {
    #[must_use]
    pub const fn new(target: SpatialTarget, surface: SurfaceStamp) -> Self {
        Self { target, surface }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialProjection {
    pub session_id: SpatialSessionId,
    pub selection_revision: u64,
    pub lifecycle: SpatialLifecycle,
    pub root_rect: LogicalRect,
    pub current_rect: LogicalRect,
    pub crosshair: LogicalPoint,
    pub depth: u8,
    pub help_visible: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialDispatchRequest {
    pub request_id: SpatialRequestId,
    pub session_id: SpatialSessionId,
    pub owner: SpatialOwner,
    pub selection_revision: u64,
    pub point: LogicalPoint,
    pub action: SpatialAction,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SpatialDispatchOutcome {
    Delivered,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SpatialAck {
    /// The acknowledgement belongs to an old request/session and was ignored.
    Ignored,
    /// The request ended this session for the supplied terminal reason.
    Inactive(SpatialCancelReason),
    /// A successful hover returned to root selection.
    HoverReset,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SpatialError {
    InvalidGeometry,
    InvalidSurface,
    TargetMismatch,
    SurfaceMismatch,
    NoSurface,
    Inactive,
    DispatchPending,
    NoDispatchPending,
    InvalidRequestPoint,
    AtDepthLimit,
    PrecisionLimit,
    RevisionExhausted,
}

impl fmt::Display for SpatialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidGeometry => "invalid spatial geometry",
            Self::InvalidSurface => "invalid spatial surface stamp",
            Self::TargetMismatch => "spatial target does not match the session",
            Self::SurfaceMismatch => "spatial surface does not match the session",
            Self::NoSurface => "spatial surface geometry is not ready",
            Self::Inactive => "spatial session is inactive",
            Self::DispatchPending => "a spatial dispatch is already pending",
            Self::NoDispatchPending => "no spatial dispatch is pending",
            Self::InvalidRequestPoint => "spatial dispatch point is outside the surface",
            Self::AtDepthLimit => "spatial refinement depth limit reached",
            Self::PrecisionLimit => "spatial precision limit reached",
            Self::RevisionExhausted => "spatial selection revision exhausted",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for SpatialError {}

#[derive(Clone, Debug, PartialEq)]
enum SessionState {
    AwaitingSurface {
        target: SpatialTarget,
    },
    Selecting {
        owner: SpatialOwner,
        root: LogicalRect,
        current: LogicalRect,
        history: Vec<LogicalRect>,
        revision: u64,
        help_visible: bool,
    },
    DispatchPending {
        owner: SpatialOwner,
        root: LogicalRect,
        current: LogicalRect,
        history: Vec<LogicalRect>,
        revision: u64,
        help_visible: bool,
        request: SpatialDispatchRequest,
    },
    Inactive {
        reason: SpatialCancelReason,
    },
}

/// A bounded, view-bound spatial navigation session.
///
/// The fields are private by design.  Adapters consume projections and
/// dispatch requests rather than mutating selection state directly.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialSession {
    id: SpatialSessionId,
    target: SpatialTarget,
    state: SessionState,
}

impl SpatialSession {
    #[must_use]
    pub fn new(ids: &mut IdSource, target: SpatialTarget) -> Self {
        Self {
            id: ids.spatial_session(),
            target,
            state: SessionState::AwaitingSurface { target },
        }
    }

    #[must_use]
    pub const fn id(&self) -> SpatialSessionId {
        self.id
    }

    #[must_use]
    pub const fn target(&self) -> SpatialTarget {
        self.target
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        !matches!(self.state, SessionState::Inactive { .. })
    }

    #[must_use]
    pub fn lifecycle(&self) -> Option<SpatialLifecycle> {
        match self.state {
            SessionState::AwaitingSurface { .. } => Some(SpatialLifecycle::AwaitingSurface),
            SessionState::Selecting { .. } => Some(SpatialLifecycle::Selecting),
            SessionState::DispatchPending { .. } => Some(SpatialLifecycle::DispatchPending),
            SessionState::Inactive { .. } => None,
        }
    }

    #[must_use]
    pub fn cancel_reason(&self) -> Option<SpatialCancelReason> {
        match self.state {
            SessionState::Inactive { reason } => Some(reason),
            _ => None,
        }
    }

    /// Installs or validates the current visible view surface.
    ///
    /// # Errors
    ///
    /// Returns an error when the surface stamp, geometry, or captured target
    /// does not match the session, or when the session is already inactive.
    ///
    /// # Panics
    ///
    /// Panics only if an internal state transition fails to install the
    /// selecting projection after matching an awaiting surface.
    pub fn observe_surface(
        &mut self,
        owner: SpatialOwner,
        root: LogicalRect,
    ) -> Result<SpatialProjection, SpatialError> {
        if !owner.surface.is_valid() {
            return Err(SpatialError::InvalidSurface);
        }
        if root.x != 0.0
            || root.y != 0.0
            || LogicalRect::new(root.x, root.y, root.width, root.height).is_none()
        {
            return Err(SpatialError::InvalidGeometry);
        }
        match &self.state {
            SessionState::AwaitingSurface { target } if *target == owner.target => {
                self.state = SessionState::Selecting {
                    owner,
                    root,
                    current: root,
                    history: Vec::new(),
                    revision: 1,
                    help_visible: false,
                };
                Ok(self.projection().expect("selecting projection installed"))
            }
            SessionState::AwaitingSurface { .. } => Err(SpatialError::TargetMismatch),
            SessionState::Selecting {
                owner: current_owner,
                root: current_root,
                ..
            }
            | SessionState::DispatchPending {
                owner: current_owner,
                root: current_root,
                ..
            } => {
                if *current_owner != owner {
                    return Err(SpatialError::SurfaceMismatch);
                }
                if *current_root != root {
                    return Err(SpatialError::SurfaceMismatch);
                }
                self.projection().ok_or(SpatialError::NoSurface)
            }
            SessionState::Inactive { .. } => Err(SpatialError::Inactive),
        }
    }

    #[must_use]
    pub fn projection(&self) -> Option<SpatialProjection> {
        let (root, current, revision, help_visible, lifecycle, depth) = match &self.state {
            SessionState::Selecting {
                root,
                current,
                revision,
                help_visible,
                history,
                ..
            } => (
                *root,
                *current,
                *revision,
                *help_visible,
                SpatialLifecycle::Selecting,
                u8::try_from(history.len()).unwrap_or(MAX_SPATIAL_DEPTH),
            ),
            SessionState::DispatchPending {
                root,
                current,
                revision,
                help_visible,
                history,
                ..
            } => (
                *root,
                *current,
                *revision,
                *help_visible,
                SpatialLifecycle::DispatchPending,
                u8::try_from(history.len()).unwrap_or(MAX_SPATIAL_DEPTH),
            ),
            _ => return None,
        };
        Some(SpatialProjection {
            session_id: self.id,
            selection_revision: revision,
            lifecycle,
            root_rect: root,
            current_rect: current,
            crosshair: current.center(),
            depth,
            help_visible,
        })
    }

    ///
    /// # Errors
    ///
    /// Returns an error when the session has no surface, a dispatch is
    /// pending, the depth or precision bound is reached, or the next geometry
    /// or revision cannot be represented safely.
    ///
    /// # Panics
    ///
    /// Panics only if an internal state transition fails to retain a selecting
    /// projection after a successful refinement.
    pub fn refine(&mut self, cell: GridCell) -> Result<SpatialProjection, SpatialError> {
        let SessionState::Selecting {
            current,
            history,
            revision,
            ..
        } = &mut self.state
        else {
            return match self.state {
                SessionState::AwaitingSurface { .. } => Err(SpatialError::NoSurface),
                SessionState::DispatchPending { .. } => Err(SpatialError::DispatchPending),
                SessionState::Inactive { .. } => Err(SpatialError::Inactive),
                SessionState::Selecting { .. } => unreachable!(),
            };
        };
        if history.len() >= usize::from(MAX_SPATIAL_DEPTH) {
            return Err(SpatialError::AtDepthLimit);
        }
        if current.is_at_precision_limit() {
            return Err(SpatialError::PrecisionLimit);
        }
        let next = current.child(cell);
        if LogicalRect::new(next.x, next.y, next.width, next.height).is_none() {
            return Err(SpatialError::InvalidGeometry);
        }
        let next_revision = revision
            .checked_add(1)
            .ok_or(SpatialError::RevisionExhausted)?;
        history.push(*current);
        *current = next;
        *revision = next_revision;
        Ok(self
            .projection()
            .expect("selecting projection remains available"))
    }

    ///
    /// # Errors
    ///
    /// Returns an error when the session has no usable surface, a dispatch is
    /// pending, or the session is inactive.
    pub fn back(&mut self) -> Result<SpatialProjection, SpatialError> {
        let SessionState::Selecting {
            current,
            history,
            revision,
            ..
        } = &mut self.state
        else {
            return match self.state {
                SessionState::AwaitingSurface { .. } => Err(SpatialError::NoSurface),
                SessionState::DispatchPending { .. } => Err(SpatialError::DispatchPending),
                SessionState::Inactive { .. } => Err(SpatialError::Inactive),
                SessionState::Selecting { .. } => unreachable!(),
            };
        };
        if let Some(parent) = history.pop() {
            *current = parent;
            *revision = revision
                .checked_add(1)
                .ok_or(SpatialError::RevisionExhausted)?;
        }
        self.projection().ok_or(SpatialError::NoSurface)
    }

    ///
    /// # Errors
    ///
    /// Returns an error when the session has no usable surface, a dispatch is
    /// pending, or the session is inactive.
    pub fn reset(&mut self) -> Result<SpatialProjection, SpatialError> {
        let SessionState::Selecting {
            root,
            current,
            history,
            revision,
            ..
        } = &mut self.state
        else {
            return match self.state {
                SessionState::AwaitingSurface { .. } => Err(SpatialError::NoSurface),
                SessionState::DispatchPending { .. } => Err(SpatialError::DispatchPending),
                SessionState::Inactive { .. } => Err(SpatialError::Inactive),
                SessionState::Selecting { .. } => unreachable!(),
            };
        };
        if !history.is_empty() {
            history.clear();
            *current = *root;
            *revision = revision
                .checked_add(1)
                .ok_or(SpatialError::RevisionExhausted)?;
        }
        self.projection().ok_or(SpatialError::NoSurface)
    }

    ///
    /// # Errors
    ///
    /// Returns an error when the session has no usable surface, a dispatch is
    /// pending, the session is inactive, or the revision counter is exhausted.
    pub fn toggle_help(&mut self) -> Result<SpatialProjection, SpatialError> {
        let SessionState::Selecting {
            help_visible,
            revision,
            ..
        } = &mut self.state
        else {
            return match self.state {
                SessionState::AwaitingSurface { .. } => Err(SpatialError::NoSurface),
                SessionState::DispatchPending { .. } => Err(SpatialError::DispatchPending),
                SessionState::Inactive { .. } => Err(SpatialError::Inactive),
                SessionState::Selecting { .. } => unreachable!(),
            };
        };
        *help_visible = !*help_visible;
        *revision = revision
            .checked_add(1)
            .ok_or(SpatialError::RevisionExhausted)?;
        self.projection().ok_or(SpatialError::NoSurface)
    }

    ///
    /// # Errors
    ///
    /// Returns an error when the surface is unavailable, another request is
    /// pending, the session is inactive, or the crosshair is outside the root.
    pub fn request(
        &mut self,
        ids: &mut IdSource,
        action: SpatialAction,
    ) -> Result<SpatialDispatchRequest, SpatialError> {
        let (owner, root, current, history, revision, help_visible) = match &self.state {
            SessionState::Selecting {
                owner,
                root,
                current,
                history,
                revision,
                help_visible,
            } => (
                *owner,
                *root,
                *current,
                history.clone(),
                *revision,
                *help_visible,
            ),
            SessionState::AwaitingSurface { .. } => return Err(SpatialError::NoSurface),
            SessionState::DispatchPending { .. } => return Err(SpatialError::DispatchPending),
            SessionState::Inactive { .. } => return Err(SpatialError::Inactive),
        };
        let point = current.center();
        if !root.contains(point) {
            return Err(SpatialError::InvalidRequestPoint);
        }
        let request = SpatialDispatchRequest {
            request_id: ids.spatial_request(),
            session_id: self.id,
            owner,
            selection_revision: revision,
            point,
            action,
        };
        self.state = SessionState::DispatchPending {
            owner,
            root,
            current,
            history,
            revision,
            help_visible,
            request,
        };
        Ok(request)
    }

    #[must_use]
    pub fn pending_request(&self) -> Option<SpatialDispatchRequest> {
        match self.state {
            SessionState::DispatchPending { request, .. } => Some(request),
            _ => None,
        }
    }

    pub fn acknowledge(
        &mut self,
        request_id: SpatialRequestId,
        owner: SpatialOwner,
        outcome: SpatialDispatchOutcome,
    ) -> SpatialAck {
        let SessionState::DispatchPending { request, .. } = self.state.clone() else {
            return SpatialAck::Ignored;
        };
        if request.request_id != request_id
            || request.session_id != self.id
            || request.owner != owner
        {
            return SpatialAck::Ignored;
        }
        if let (SpatialAction::Hover, SpatialDispatchOutcome::Delivered) = (request.action, outcome)
        {
            let SessionState::DispatchPending { root, .. } = self.state else {
                unreachable!();
            };
            let Some(next_revision) = request.selection_revision.checked_add(1) else {
                self.state = SessionState::Inactive {
                    reason: SpatialCancelReason::DispatchUncertain,
                };
                return SpatialAck::Inactive(SpatialCancelReason::DispatchUncertain);
            };
            self.state = SessionState::Selecting {
                owner,
                root,
                current: root,
                history: Vec::new(),
                revision: next_revision,
                help_visible: false,
            };
            SpatialAck::HoverReset
        } else {
            let reason = if outcome == SpatialDispatchOutcome::Rejected {
                SpatialCancelReason::DispatchRejected
            } else {
                SpatialCancelReason::ActionDelivered
            };
            self.state = SessionState::Inactive { reason };
            SpatialAck::Inactive(reason)
        }
    }

    pub fn expire_dispatch(&mut self) {
        if matches!(self.state, SessionState::DispatchPending { .. }) {
            self.state = SessionState::Inactive {
                reason: SpatialCancelReason::DispatchUncertain,
            };
        }
    }

    pub fn cancel(&mut self, reason: SpatialCancelReason) {
        if self.is_active() {
            self.state = SessionState::Inactive { reason };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(ids: &mut IdSource) -> SpatialTarget {
        SpatialTarget::new(
            ids.window(),
            Target {
                tab: ids.tab(),
                generation: 1,
                document: ids.document(),
            },
        )
    }

    fn ready(ids: &mut IdSource) -> SpatialSession {
        let target = target(ids);
        let mut session = SpatialSession::new(ids, target);
        let stamp = SurfaceStamp::new(1, 1).expect("valid stamp");
        session
            .observe_surface(
                SpatialOwner::new(target, stamp),
                LogicalRect::new(0.0, 0.0, 1920.0, 1080.0).expect("valid rect"),
            )
            .expect("surface ready");
        session
    }

    fn assert_rect_close(actual: LogicalRect, expected: LogicalRect) {
        let tolerance = 1.0e-12;
        assert!((actual.x - expected.x).abs() <= tolerance);
        assert!((actual.y - expected.y).abs() <= tolerance);
        assert!((actual.width - expected.width).abs() <= tolerance);
        assert!((actual.height - expected.height).abs() <= tolerance);
    }

    #[test]
    fn every_cell_uses_row_major_geometry() {
        let expected = [
            (GridCell::One, 0.0, 0.0),
            (GridCell::Two, 30.0, 0.0),
            (GridCell::Three, 60.0, 0.0),
            (GridCell::Four, 0.0, 20.0),
            (GridCell::Five, 30.0, 20.0),
            (GridCell::Six, 60.0, 20.0),
            (GridCell::Seven, 0.0, 40.0),
            (GridCell::Eight, 30.0, 40.0),
            (GridCell::Nine, 60.0, 40.0),
        ];
        for (cell, x, y) in expected {
            let root = LogicalRect::new(0.0, 0.0, 90.0, 60.0).expect("valid root");
            assert_rect_close(
                root.child(cell),
                LogicalRect::new(x, y, 30.0, 20.0).expect("valid child"),
            );
        }
    }

    #[test]
    fn rectangles_reject_non_finite_and_non_positive_geometry() {
        for rect in [
            LogicalRect::new(0.0, 0.0, 0.0, 1.0),
            LogicalRect::new(0.0, 0.0, -1.0, 1.0),
            LogicalRect::new(0.0, 0.0, f64::NAN, 1.0),
            LogicalRect::new(0.0, 0.0, 1.0, f64::INFINITY),
            LogicalRect::new(f64::MAX, 0.0, f64::MAX, 1.0),
        ] {
            assert!(rect.is_none());
        }
        assert!(LogicalRect::new(-1.5, -2.5, 0.25, 0.5).is_some());
    }

    #[test]
    fn row_major_cells_subdivide_without_rounding() {
        let mut ids = IdSource::new();
        let mut session = ready(&mut ids);
        let projection = session.refine(GridCell::Eight).expect("refine");
        assert!((projection.current_rect.x - 640.0).abs() < f64::EPSILON);
        assert!((projection.current_rect.y - 720.0).abs() < f64::EPSILON);
        assert!((projection.current_rect.width - 640.0).abs() < f64::EPSILON);
        assert!((projection.current_rect.height - 360.0).abs() < f64::EPSILON);
        assert_eq!(projection.crosshair, LogicalPoint::new(960.0, 900.0));
    }

    #[test]
    fn depth_and_precision_stops_are_non_dispatching() {
        let mut ids = IdSource::new();
        let mut session = ready(&mut ids);
        let mut stopped = None;
        for _ in 0..=MAX_SPATIAL_DEPTH {
            match session.refine(GridCell::Five) {
                Ok(_) => {}
                Err(error @ (SpatialError::PrecisionLimit | SpatialError::AtDepthLimit)) => {
                    stopped = Some(error);
                    break;
                }
                Err(error) => panic!("unexpected refinement error: {error}"),
            }
        }
        assert!(stopped.is_some());
        assert!(session.pending_request().is_none());
    }

    #[test]
    fn back_reset_and_hover_return_to_root() {
        let mut ids = IdSource::new();
        let mut session = ready(&mut ids);
        let SessionState::Selecting { owner, .. } = session.state else {
            unreachable!();
        };
        session.refine(GridCell::Nine).expect("refine");
        session.back().expect("back");
        assert_eq!(session.projection().expect("projection").depth, 0);
        session.refine(GridCell::One).expect("refine");
        session.reset().expect("reset");
        let request = session
            .request(&mut ids, SpatialAction::Hover)
            .expect("hover request");
        assert_eq!(
            session.acknowledge(request.request_id, owner, SpatialDispatchOutcome::Delivered),
            SpatialAck::HoverReset
        );
        assert_eq!(session.projection().expect("projection").depth, 0);
    }

    #[test]
    fn stale_ack_is_ignored_and_click_ends_session() {
        let mut ids = IdSource::new();
        let mut session = ready(&mut ids);
        let SessionState::Selecting { owner, .. } = session.state else {
            unreachable!();
        };
        let request = session
            .request(&mut ids, SpatialAction::Click(PointerButton::Left))
            .expect("click request");
        assert_eq!(
            session.acknowledge(
                SpatialRequestId::from_display("spatialrequestid-999").expect("opaque id"),
                owner,
                SpatialDispatchOutcome::Delivered
            ),
            SpatialAck::Ignored
        );
        assert_eq!(
            session.acknowledge(request.request_id, owner, SpatialDispatchOutcome::Delivered),
            SpatialAck::Inactive(SpatialCancelReason::ActionDelivered)
        );
        assert_eq!(
            session.cancel_reason(),
            Some(SpatialCancelReason::ActionDelivered)
        );
        assert!(!session.is_active());
    }

    #[test]
    fn pending_dispatch_blocks_selection_and_duplicate_commits() {
        let mut ids = IdSource::new();
        let mut session = ready(&mut ids);
        session
            .request(&mut ids, SpatialAction::Click(PointerButton::Right))
            .expect("first request");
        assert_eq!(
            session.request(&mut ids, SpatialAction::Hover),
            Err(SpatialError::DispatchPending)
        );
        assert!(matches!(
            session.refine(GridCell::One),
            Err(SpatialError::DispatchPending)
        ));
        assert!(matches!(session.back(), Err(SpatialError::DispatchPending)));
        assert!(matches!(
            session.reset(),
            Err(SpatialError::DispatchPending)
        ));
        assert!(matches!(
            session.toggle_help(),
            Err(SpatialError::DispatchPending)
        ));
    }

    #[test]
    fn wrong_owner_ack_is_ignored_and_rejection_is_terminal() {
        let mut ids = IdSource::new();
        let mut session = ready(&mut ids);
        let request = session
            .request(&mut ids, SpatialAction::Hover)
            .expect("hover request");
        let wrong_owner = SpatialOwner::new(
            session.target(),
            SurfaceStamp::new(1, 2).expect("valid different stamp"),
        );
        assert_eq!(
            session.acknowledge(
                request.request_id,
                wrong_owner,
                SpatialDispatchOutcome::Delivered
            ),
            SpatialAck::Ignored
        );
        assert_eq!(session.pending_request(), Some(request));
        assert_eq!(
            session.acknowledge(
                request.request_id,
                request.owner,
                SpatialDispatchOutcome::Rejected
            ),
            SpatialAck::Inactive(SpatialCancelReason::DispatchRejected)
        );
        assert_eq!(
            session.cancel_reason(),
            Some(SpatialCancelReason::DispatchRejected)
        );
    }

    #[test]
    fn expiring_a_pending_dispatch_is_uncertain_and_never_revivable() {
        let mut ids = IdSource::new();
        let mut session = ready(&mut ids);
        let request = session
            .request(&mut ids, SpatialAction::Click(PointerButton::Middle))
            .expect("click request");
        session.expire_dispatch();
        assert_eq!(
            session.cancel_reason(),
            Some(SpatialCancelReason::DispatchUncertain)
        );
        assert_eq!(
            session.acknowledge(
                request.request_id,
                request.owner,
                SpatialDispatchOutcome::Delivered
            ),
            SpatialAck::Ignored
        );
        assert!(!session.is_active());
    }
}
