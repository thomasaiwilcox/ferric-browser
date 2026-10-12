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
pub const SPATIAL_SCOPE_GEOMETRY_TOLERANCE: f64 = 0.5;

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

/// Maps a top-document CSS rectangle into WebEngineView-local Qt logical
/// coordinates using the measured viewport scale.  DPR and window placement
/// are deliberately handled later by the native adapter.
#[must_use]
pub fn map_css_rect_to_view_local(
    css_rect: LogicalRect,
    css_viewport_width: f64,
    css_viewport_height: f64,
    view_local_width: f64,
    view_local_height: f64,
) -> Option<LogicalRect> {
    let css_rect = LogicalRect::new(css_rect.x, css_rect.y, css_rect.width, css_rect.height)?;
    let css_viewport = LogicalRect::new(0.0, 0.0, css_viewport_width, css_viewport_height)?;
    let view_local = LogicalRect::new(0.0, 0.0, view_local_width, view_local_height)?;
    if css_rect.x < 0.0
        || css_rect.y < 0.0
        || css_rect.x + css_rect.width > css_viewport.width
        || css_rect.y + css_rect.height > css_viewport.height
    {
        return None;
    }
    let scale_x = view_local.width / css_viewport.width;
    let scale_y = view_local.height / css_viewport.height;
    if !scale_x.is_finite()
        || !scale_y.is_finite()
        || !(0.2..=5.0).contains(&scale_x)
        || !(0.2..=5.0).contains(&scale_y)
    {
        return None;
    }
    let mapped = LogicalRect::new(
        css_rect.x * scale_x,
        css_rect.y * scale_y,
        css_rect.width * scale_x,
        css_rect.height * scale_y,
    )?;
    (mapped.x + mapped.width <= view_local.width && mapped.y + mapped.height <= view_local.height)
        .then_some(mapped)
}

fn rect_is_within(rect: LogicalRect, bounds: LogicalRect) -> bool {
    LogicalRect::new(rect.x, rect.y, rect.width, rect.height).is_some()
        && LogicalRect::new(bounds.x, bounds.y, bounds.width, bounds.height).is_some()
        && rect.x >= bounds.x
        && rect.y >= bounds.y
        && rect.x + rect.width <= bounds.x + bounds.width
        && rect.y + rect.height <= bounds.y + bounds.height
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
    /// The full captured `WebEngineView` surface, always rooted at `(0, 0)`.
    pub viewport_rect: LogicalRect,
    /// The selected interaction area; equal to `viewport_rect` for legacy Grid.
    pub interaction_rect: LogicalRect,
    /// Compatibility alias for `interaction_rect`.
    pub root_rect: LogicalRect,
    pub current_rect: LogicalRect,
    pub crosshair: LogicalPoint,
    pub depth: u8,
    pub help_visible: bool,
}

/// The exact weak page-scope lease identity that authorized scoped Grid.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpatialScope {
    pub lease_id: String,
    pub element_id: u32,
    pub frame_path: String,
    pub kind: crate::HintKind,
    pub configuration_revision: u64,
    pub selector_policy_revision: u64,
    pub scroll_revision: u64,
    pub css_viewport_width_bits: u64,
    pub css_viewport_height_bits: u64,
    pub zoom_factor_bits: u64,
}

impl SpatialScope {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.lease_id.is_empty()
            && self.lease_id.len() <= 128
            && self.element_id > 0
            && !self.frame_path.is_empty()
            && self.frame_path.len() <= 256
            && matches!(
                self.kind,
                crate::HintKind::Region
                    | crate::HintKind::ScrollableRegion
                    | crate::HintKind::Surface
                    | crate::HintKind::FocusableSurface
            )
            && f64::from_bits(self.css_viewport_width_bits).is_finite()
            && f64::from_bits(self.css_viewport_height_bits).is_finite()
            && f64::from_bits(self.css_viewport_width_bits) > 0.0
            && f64::from_bits(self.css_viewport_height_bits) > 0.0
            && f64::from_bits(self.zoom_factor_bits).is_finite()
            && f64::from_bits(self.zoom_factor_bits) > 0.0
    }
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
    InvalidScope,
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
            Self::InvalidScope => "spatial scope identity or geometry is invalid",
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
        viewport: LogicalRect,
        interaction: LogicalRect,
        current: LogicalRect,
        history: Vec<LogicalRect>,
        revision: u64,
        help_visible: bool,
    },
    DispatchPending {
        owner: SpatialOwner,
        viewport: LogicalRect,
        interaction: LogicalRect,
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
    scope: Option<SpatialScope>,
    state: SessionState,
}

impl SpatialSession {
    #[must_use]
    pub fn new(ids: &mut IdSource, target: SpatialTarget) -> Self {
        Self {
            id: ids.spatial_session(),
            target,
            scope: None,
            state: SessionState::AwaitingSurface { target },
        }
    }

    #[must_use]
    pub fn new_scoped(ids: &mut IdSource, target: SpatialTarget, scope: SpatialScope) -> Self {
        Self {
            id: ids.spatial_session(),
            target,
            scope: Some(scope),
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
    pub fn scope(&self) -> Option<&SpatialScope> {
        self.scope.as_ref()
    }

    #[must_use]
    pub fn surface_owner(&self) -> Option<SpatialOwner> {
        match self.state {
            SessionState::Selecting { owner, .. } | SessionState::DispatchPending { owner, .. } => {
                Some(owner)
            }
            SessionState::AwaitingSurface { .. } | SessionState::Inactive { .. } => None,
        }
    }

    #[must_use]
    pub fn scope_geometry_matches(&self, actual: LogicalRect, edge_tolerance: f64) -> bool {
        if self.scope.is_none()
            || !edge_tolerance.is_finite()
            || edge_tolerance < 0.0
            || LogicalRect::new(actual.x, actual.y, actual.width, actual.height).is_none()
        {
            return false;
        }
        let Some(expected) = self
            .projection()
            .map(|projection| projection.interaction_rect)
        else {
            return false;
        };
        (expected.x - actual.x).abs() <= edge_tolerance
            && (expected.y - actual.y).abs() <= edge_tolerance
            && (expected.x + expected.width - actual.x - actual.width).abs() <= edge_tolerance
            && (expected.y + expected.height - actual.y - actual.height).abs() <= edge_tolerance
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
        let interaction = if self.scope.is_some() {
            self.projection()
                .ok_or(SpatialError::InvalidScope)?
                .interaction_rect
        } else {
            root
        };
        self.observe_surface_in_scope(owner, root, interaction)
    }

    /// Installs a scoped interaction rectangle inside the full view viewport.
    /// The scope identity is fixed when the session is created.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::InvalidScope`] if the session scope is invalid or the
    /// interaction rectangle is outside the viewport.
    pub fn observe_scoped_surface(
        &mut self,
        owner: SpatialOwner,
        viewport: LogicalRect,
        interaction: LogicalRect,
    ) -> Result<SpatialProjection, SpatialError> {
        if !self.scope.as_ref().is_some_and(SpatialScope::is_valid)
            || !rect_is_within(interaction, viewport)
        {
            return Err(SpatialError::InvalidScope);
        }
        self.observe_surface_in_scope(owner, viewport, interaction)
    }

    fn observe_surface_in_scope(
        &mut self,
        owner: SpatialOwner,
        viewport: LogicalRect,
        interaction: LogicalRect,
    ) -> Result<SpatialProjection, SpatialError> {
        if !owner.surface.is_valid() {
            return Err(SpatialError::InvalidSurface);
        }
        if viewport.x != 0.0
            || viewport.y != 0.0
            || LogicalRect::new(viewport.x, viewport.y, viewport.width, viewport.height).is_none()
            || !rect_is_within(interaction, viewport)
        {
            return Err(SpatialError::InvalidGeometry);
        }
        match &self.state {
            SessionState::AwaitingSurface { target } if *target == owner.target => {
                self.state = SessionState::Selecting {
                    owner,
                    viewport,
                    interaction,
                    current: interaction,
                    history: Vec::new(),
                    revision: 1,
                    help_visible: false,
                };
                Ok(self.projection().expect("selecting projection installed"))
            }
            SessionState::AwaitingSurface { .. } => Err(SpatialError::TargetMismatch),
            SessionState::Selecting {
                owner: current_owner,
                viewport: current_viewport,
                interaction: current_interaction,
                ..
            }
            | SessionState::DispatchPending {
                owner: current_owner,
                viewport: current_viewport,
                interaction: current_interaction,
                ..
            } => {
                if *current_owner != owner {
                    return Err(SpatialError::SurfaceMismatch);
                }
                if *current_viewport != viewport || *current_interaction != interaction {
                    return Err(SpatialError::SurfaceMismatch);
                }
                self.projection().ok_or(SpatialError::NoSurface)
            }
            SessionState::Inactive { .. } => Err(SpatialError::Inactive),
        }
    }

    #[must_use]
    pub fn projection(&self) -> Option<SpatialProjection> {
        let (viewport, interaction, current, revision, help_visible, lifecycle, depth) =
            match &self.state {
                SessionState::Selecting {
                    viewport,
                    interaction,
                    current,
                    revision,
                    help_visible,
                    history,
                    ..
                } => (
                    *viewport,
                    *interaction,
                    *current,
                    *revision,
                    *help_visible,
                    SpatialLifecycle::Selecting,
                    u8::try_from(history.len()).unwrap_or(MAX_SPATIAL_DEPTH),
                ),
                SessionState::DispatchPending {
                    viewport,
                    interaction,
                    current,
                    revision,
                    help_visible,
                    history,
                    ..
                } => (
                    *viewport,
                    *interaction,
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
            viewport_rect: viewport,
            interaction_rect: interaction,
            root_rect: interaction,
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
            interaction,
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
            *current = *interaction;
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
        let (owner, viewport, interaction, current, history, revision, help_visible) =
            match &self.state {
                SessionState::Selecting {
                    owner,
                    viewport,
                    interaction,
                    current,
                    history,
                    revision,
                    help_visible,
                } => (
                    *owner,
                    *viewport,
                    *interaction,
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
        if !viewport.contains(point) || !interaction.contains(point) {
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
            viewport,
            interaction,
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
            let SessionState::DispatchPending {
                viewport,
                interaction,
                ..
            } = self.state
            else {
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
                viewport,
                interaction,
                current: interaction,
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

    fn region_scope() -> SpatialScope {
        SpatialScope {
            lease_id: "hint-scope-41".to_owned(),
            element_id: 7,
            frame_path: "0".to_owned(),
            kind: crate::HintKind::Region,
            configuration_revision: 3,
            selector_policy_revision: 9,
            scroll_revision: 0,
            css_viewport_width_bits: 1000.0_f64.to_bits(),
            css_viewport_height_bits: 700.0_f64.to_bits(),
            zoom_factor_bits: 1.0_f64.to_bits(),
        }
    }

    #[test]
    fn measured_css_mapping_scales_once_into_view_local_coordinates() {
        let css = LogicalRect::new(100.0, 40.0, 200.0, 120.0).expect("CSS rect");
        assert_eq!(
            map_css_rect_to_view_local(css, 800.0, 600.0, 1000.0, 750.0),
            Some(LogicalRect::new(125.0, 50.0, 250.0, 150.0).expect("view rect"))
        );
        assert_eq!(
            map_css_rect_to_view_local(css, 800.0, 600.0, 640.0, 480.0),
            Some(LogicalRect::new(80.0, 32.0, 160.0, 96.0).expect("zoomed view rect"))
        );
    }

    #[test]
    fn measured_css_mapping_rejects_unbounded_or_outside_geometry() {
        let outside = LogicalRect::new(90.0, 0.0, 20.0, 10.0).expect("outside rect");
        assert_eq!(
            map_css_rect_to_view_local(outside, 100.0, 100.0, 100.0, 100.0),
            None
        );
        let valid = LogicalRect::new(10.0, 10.0, 20.0, 20.0).expect("valid rect");
        assert_eq!(
            map_css_rect_to_view_local(valid, 100.0, 100.0, 600.0, 600.0),
            None
        );
        assert_eq!(
            map_css_rect_to_view_local(valid, 100.0, 100.0, f64::NAN, 100.0),
            None
        );
    }

    #[test]
    fn scoped_grid_keeps_viewport_separate_and_resets_to_nonzero_region() {
        let mut ids = IdSource::new();
        let target = target(&mut ids);
        let mut session = SpatialSession::new_scoped(&mut ids, target, region_scope());
        let viewport = LogicalRect::new(0.0, 0.0, 1000.0, 700.0).expect("viewport");
        let interaction = LogicalRect::new(100.0, 80.0, 600.0, 450.0).expect("region");
        let owner = SpatialOwner::new(target, SurfaceStamp::new(2, 5).expect("surface stamp"));
        let projection = session
            .observe_scoped_surface(owner, viewport, interaction)
            .expect("scoped surface");
        assert_eq!(projection.viewport_rect, viewport);
        assert_eq!(projection.interaction_rect, interaction);
        assert_eq!(projection.root_rect, interaction);
        assert_eq!(projection.current_rect, interaction);
        assert!(session.scope_geometry_matches(
            LogicalRect::new(100.25, 79.75, 599.75, 450.25).expect("tolerated region"),
            SPATIAL_SCOPE_GEOMETRY_TOLERANCE,
        ));
        assert!(!session.scope_geometry_matches(
            LogicalRect::new(100.51, 80.0, 600.0, 450.0).expect("moved region"),
            SPATIAL_SCOPE_GEOMETRY_TOLERANCE,
        ));

        session.refine(GridCell::Nine).expect("refine");
        session.back().expect("back");
        session.refine(GridCell::One).expect("refine again");
        let reset = session.reset().expect("reset");
        assert_eq!(reset.current_rect, interaction);
        let request = session
            .request(&mut ids, SpatialAction::Hover)
            .expect("scope hover");
        assert_eq!(request.point, interaction.center());
        assert_eq!(
            session.acknowledge(request.request_id, owner, SpatialDispatchOutcome::Delivered),
            SpatialAck::HoverReset
        );
        let hovered = session.projection().expect("projection after hover");
        assert_eq!(hovered.viewport_rect, viewport);
        assert_eq!(hovered.interaction_rect, interaction);
        assert_eq!(hovered.current_rect, interaction);
    }

    #[test]
    fn scoped_grid_rejects_invalid_lease_and_out_of_view_region() {
        let mut ids = IdSource::new();
        let target = target(&mut ids);
        let mut invalid_scope = region_scope();
        invalid_scope.lease_id.clear();
        let mut session = SpatialSession::new_scoped(&mut ids, target, invalid_scope);
        let viewport = LogicalRect::new(0.0, 0.0, 100.0, 100.0).expect("viewport");
        let outside = LogicalRect::new(90.0, 20.0, 20.0, 30.0).expect("outside");
        let owner = SpatialOwner::new(target, SurfaceStamp::new(1, 1).expect("surface stamp"));
        assert_eq!(
            session.observe_scoped_surface(owner, viewport, outside),
            Err(SpatialError::InvalidScope)
        );
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
