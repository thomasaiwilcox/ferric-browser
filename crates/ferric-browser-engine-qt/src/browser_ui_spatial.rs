//! Browser-window owner for the ephemeral spatial-navigation session.
//!
//! QML receives only immutable projection values and a bounded dispatch
//! envelope.  The session, target freshness, and cancellation policy remain
//! in Rust; the native pointer adapter acknowledges delivery independently.

use super::{
    BindingResolver, CxxQtType, Event, GridCell, LogicalRect, Mode, Pin, PointerButton, QString,
    SpatialAction, SpatialCancelReason, SpatialDispatchOutcome, SpatialOwner, SpatialSession,
    SpatialTarget, SurfaceStamp, current_target, qobject,
};
use ferric_browser_core::{SpatialRequestId, SpatialSessionId};
use serde_json::json;

impl qobject::BrowserUi {
    pub(super) fn clear_spatial_session_state(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.spatial_session = None;
        }
        self.as_mut().set_spatial_visible(false);
        self.as_mut().set_spatial_session_id(QString::default());
        self.as_mut()
            .set_spatial_dispatch_request(QString::default());
        self.as_mut().set_spatial_labels(QString::from(
            "[\"1\",\"2\",\"3\",\"4\",\"5\",\"6\",\"7\",\"8\",\"9\"]",
        ));
        self.as_mut().set_spatial_root_x(0.0);
        self.as_mut().set_spatial_root_y(0.0);
        self.as_mut().set_spatial_root_width(0.0);
        self.as_mut().set_spatial_root_height(0.0);
        self.as_mut().set_spatial_current_x(0.0);
        self.as_mut().set_spatial_current_y(0.0);
        self.as_mut().set_spatial_current_width(0.0);
        self.as_mut().set_spatial_current_height(0.0);
        self.as_mut().set_spatial_crosshair_x(0.0);
        self.as_mut().set_spatial_crosshair_y(0.0);
        self.as_mut().set_spatial_depth(0);
        self.as_mut().set_spatial_help_visible(false);
    }

    fn publish_spatial_projection(mut self: Pin<&mut Self>) {
        let definitions = self
            .as_ref()
            .rust()
            .bindings
            .as_ref()
            .map(BindingResolver::definitions)
            .unwrap_or_default();
        let mut labels = vec![String::from("—"); 9];
        for index in 1..=9 {
            let command = format!("grid-refine {index}");
            let mut candidates = definitions
                .iter()
                .filter(|definition| definition.mode == Mode::Grid && definition.command == command)
                .map(|definition| {
                    let joined = definition.keys.join(" ");
                    (definition.keys.len(), joined)
                })
                .collect::<Vec<_>>();
            candidates.sort();
            if let Some((_, label)) = candidates.into_iter().next() {
                labels[index - 1] = label;
            }
        }
        self.as_mut().set_spatial_labels(QString::from(
            serde_json::to_string(&labels).unwrap_or_else(|_| {
                "[\"—\",\"—\",\"—\",\"—\",\"—\",\"—\",\"—\",\"—\",\"—\"]".into()
            }),
        ));
        let projection = self
            .as_ref()
            .rust()
            .spatial_session
            .as_ref()
            .and_then(SpatialSession::projection);
        let Some(projection) = projection else {
            self.as_mut().clear_spatial_session_state();
            return;
        };
        self.as_mut().set_spatial_visible(true);
        self.as_mut()
            .set_spatial_session_id(QString::from(projection.session_id.to_string()));
        self.as_mut().set_spatial_root_x(projection.root_rect.x);
        self.as_mut().set_spatial_root_y(projection.root_rect.y);
        self.as_mut()
            .set_spatial_root_width(projection.root_rect.width);
        self.as_mut()
            .set_spatial_root_height(projection.root_rect.height);
        self.as_mut()
            .set_spatial_current_x(projection.current_rect.x);
        self.as_mut()
            .set_spatial_current_y(projection.current_rect.y);
        self.as_mut()
            .set_spatial_current_width(projection.current_rect.width);
        self.as_mut()
            .set_spatial_current_height(projection.current_rect.height);
        self.as_mut()
            .set_spatial_crosshair_x(projection.crosshair.x);
        self.as_mut()
            .set_spatial_crosshair_y(projection.crosshair.y);
        self.as_mut().set_spatial_depth(i32::from(projection.depth));
        self.as_mut()
            .set_spatial_help_visible(projection.help_visible);
    }

    pub(super) fn begin_spatial_navigation(mut self: Pin<&mut Self>) -> bool {
        if self.as_ref().rust().core_mode == Mode::Grid
            && self.as_ref().rust().spatial_session.is_some()
        {
            return true;
        }
        if !matches!(self.as_ref().rust().core_mode, Mode::Normal | Mode::Command) {
            self.set_status_text(QString::from("Grid mode is available from Normal mode"));
            return false;
        }
        let (Some(window), Some(target)) = (
            self.as_ref().rust().window,
            current_target(
                self.as_ref().rust().state.as_ref(),
                self.as_ref().rust().tab,
            ),
        ) else {
            self.set_status_text(QString::from("Grid unavailable: no active page view"));
            return false;
        };
        if self
            .as_mut()
            .reduce_event(Event::PushMode {
                window,
                mode: Mode::Grid,
            })
            .is_err()
        {
            self.set_status_text(QString::from("Grid mode rejected"));
            return false;
        }
        let spatial_target = SpatialTarget::new(window, target);
        let session = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            SpatialSession::new(&mut this.spatial_ids, spatial_target)
        };
        let session_id = session.id().to_string();
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.spatial_session = Some(session);
        }
        self.as_mut().set_core_mode(Mode::Grid);
        self.as_mut().set_mode(QString::from("grid"));
        self.as_mut()
            .set_spatial_session_id(QString::from(session_id));
        self.as_mut()
            .set_spatial_dispatch_request(QString::default());
        self.as_mut()
            .set_status_text(QString::from("Grid mode: waiting for page surface"));
        true
    }

    pub(super) fn cancel_spatial_navigation(mut self: Pin<&mut Self>, reason: SpatialCancelReason) {
        let was_grid = self.as_ref().rust().core_mode == Mode::Grid;
        if let Some(session) = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .spatial_session
            .as_mut()
        {
            session.cancel(reason);
        }
        self.as_mut().clear_spatial_session_state();
        if was_grid {
            if let Some(window) = self.as_ref().rust().window {
                let _ = self.as_mut().reduce_event(Event::Escape { window });
            }
            self.as_mut().set_core_mode(Mode::Normal);
            self.as_mut().set_mode(QString::from("normal"));
        }
        let status = match reason {
            SpatialCancelReason::User => "Grid cancelled",
            SpatialCancelReason::WindowInactive | SpatialCancelReason::PhysicalPointer => {
                "Grid cancelled: pointer or window took control"
            }
            SpatialCancelReason::PromptOwnedInput => "Grid cancelled: browser prompt took control",
            SpatialCancelReason::DispatchRejected => "Grid action rejected",
            SpatialCancelReason::DispatchUncertain => "Grid action outcome uncertain",
            _ => "Grid cancelled: page view changed",
        };
        self.set_status_text(QString::from(status));
    }

    pub(super) fn spatial_surface_ready(
        mut self: Pin<&mut Self>,
        width: f64,
        height: f64,
        serial: &QString,
        revision: &QString,
    ) -> bool {
        if self.as_ref().rust().core_mode != Mode::Grid {
            return false;
        }
        let (Some(serial), Some(revision)) = (
            serial.to_string().parse::<u64>().ok(),
            revision.to_string().parse::<u64>().ok(),
        ) else {
            self.as_mut()
                .cancel_spatial_navigation(SpatialCancelReason::SurfaceUnavailable);
            return false;
        };
        let Some(surface) = SurfaceStamp::new(serial, revision) else {
            self.as_mut()
                .cancel_spatial_navigation(SpatialCancelReason::SurfaceUnavailable);
            return false;
        };
        let Some(root) = LogicalRect::new(0.0, 0.0, width, height) else {
            self.as_mut()
                .cancel_spatial_navigation(SpatialCancelReason::GeometryChanged);
            return false;
        };
        let result = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(session) = this.spatial_session.as_mut() else {
                return false;
            };
            let owner = SpatialOwner::new(session.target(), surface);
            session.observe_surface(owner, root)
        };
        match result {
            Ok(_) => {
                self.as_mut().publish_spatial_projection();
                true
            }
            Err(_) => {
                self.as_mut()
                    .cancel_spatial_navigation(SpatialCancelReason::ViewChanged);
                false
            }
        }
    }

    fn spatial_dispatch_action(mut self: Pin<&mut Self>, action: SpatialAction) -> bool {
        let request = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(session) = this.spatial_session.as_mut() else {
                return false;
            };
            session.request(&mut this.spatial_ids, action)
        };
        let Ok(request) = request else {
            self.set_status_text(QString::from("Grid action rejected"));
            return false;
        };
        let action_name = match request.action {
            SpatialAction::Hover => "hover",
            SpatialAction::Click(PointerButton::Left) => "left",
            SpatialAction::Click(PointerButton::Right) => "right",
            SpatialAction::Click(PointerButton::Middle) => "middle",
        };
        let payload = json!({
            "request_id": request.request_id.to_string(),
            "session_id": request.session_id.to_string(),
            "serial": request.owner.surface.serial.to_string(),
            "revision": request.owner.surface.revision.to_string(),
            "selection_revision": request.selection_revision,
            "x": request.point.x,
            "y": request.point.y,
            "action": action_name,
        });
        self.as_mut()
            .set_spatial_dispatch_request(QString::from(payload.to_string()));
        self.as_mut().publish_spatial_projection();
        true
    }

    pub(super) fn handle_spatial_command(
        mut self: Pin<&mut Self>,
        name: &str,
        arguments: &[String],
    ) -> Option<bool> {
        if name == "grid" {
            if !arguments.is_empty() {
                self.set_status_text(QString::from("grid does not accept arguments"));
                return Some(false);
            }
            return Some(self.as_mut().begin_spatial_navigation());
        }
        if !name.starts_with("grid-") {
            return None;
        }
        if self.as_ref().rust().core_mode != Mode::Grid {
            self.set_status_text(QString::from("Grid command requires Grid mode"));
            return Some(false);
        }
        match name {
            "grid-refine" => {
                let [digit] = arguments else {
                    self.set_status_text(QString::from("grid-refine requires one cell 1-9"));
                    return Some(false);
                };
                let Some(cell) = digit.chars().next().and_then(GridCell::from_digit) else {
                    self.set_status_text(QString::from("grid-refine requires one cell 1-9"));
                    return Some(false);
                };
                if digit.len() != 1 {
                    self.set_status_text(QString::from("grid-refine requires one cell 1-9"));
                    return Some(false);
                }
                let result = self
                    .as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .spatial_session
                    .as_mut()
                    .and_then(|session| session.refine(cell).ok());
                if result.is_none() {
                    self.set_status_text(QString::from("Grid precision or depth limit reached"));
                    return Some(false);
                }
                self.as_mut().publish_spatial_projection();
                Some(true)
            }
            "grid-click" => {
                let [button] = arguments else {
                    self.set_status_text(QString::from(
                        "grid-click requires left, right, or middle",
                    ));
                    return Some(false);
                };
                let button = match button.as_str() {
                    "left" => PointerButton::Left,
                    "right" => PointerButton::Right,
                    "middle" => PointerButton::Middle,
                    _ => {
                        self.set_status_text(QString::from(
                            "grid-click requires left, right, or middle",
                        ));
                        return Some(false);
                    }
                };
                if arguments.len() != 1 {
                    return Some(false);
                }
                Some(
                    self.as_mut()
                        .spatial_dispatch_action(SpatialAction::Click(button)),
                )
            }
            "grid-hover" => {
                if !arguments.is_empty() {
                    return Some(false);
                }
                Some(self.as_mut().spatial_dispatch_action(SpatialAction::Hover))
            }
            "grid-back" => {
                if !arguments.is_empty() {
                    return Some(false);
                }
                let _ = self
                    .as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .spatial_session
                    .as_mut()
                    .and_then(|session| session.back().ok());
                self.as_mut().publish_spatial_projection();
                Some(true)
            }
            "grid-reset" => {
                if !arguments.is_empty() {
                    return Some(false);
                }
                let _ = self
                    .as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .spatial_session
                    .as_mut()
                    .and_then(|session| session.reset().ok());
                self.as_mut().publish_spatial_projection();
                Some(true)
            }
            "grid-help" => {
                if !arguments.is_empty() {
                    return Some(false);
                }
                let _ = self
                    .as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .spatial_session
                    .as_mut()
                    .and_then(|session| session.toggle_help().ok());
                self.as_mut().publish_spatial_projection();
                Some(true)
            }
            "grid-cancel" => {
                if !arguments.is_empty() {
                    return Some(false);
                }
                self.as_mut()
                    .cancel_spatial_navigation(SpatialCancelReason::User);
                Some(true)
            }
            _ => None,
        }
    }

    pub(super) fn spatial_dispatch_ack(
        mut self: Pin<&mut Self>,
        request_id: &QString,
        session_id: &QString,
        serial: &QString,
        revision: &QString,
        outcome: &QString,
    ) -> bool {
        let Some(request_id) = SpatialRequestId::from_display(&request_id.to_string()) else {
            return false;
        };
        let Some(session_id) = SpatialSessionId::from_display(&session_id.to_string()) else {
            return false;
        };
        let (Some(serial), Some(revision)) = (
            serial.to_string().parse::<u64>().ok(),
            revision.to_string().parse::<u64>().ok(),
        ) else {
            return false;
        };
        let Some(surface) = SurfaceStamp::new(serial, revision) else {
            return false;
        };
        let result = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(session) = this.spatial_session.as_mut() else {
                return false;
            };
            if session.id() != session_id {
                return false;
            }
            let owner = SpatialOwner::new(session.target(), surface);
            let outcome = match outcome.to_string().as_str() {
                "delivered" => SpatialDispatchOutcome::Delivered,
                "rejected" => SpatialDispatchOutcome::Rejected,
                _ => return false,
            };
            session.acknowledge(request_id, owner, outcome)
        };
        if matches!(result, ferric_browser_core::SpatialAck::Ignored) {
            return false;
        }
        self.as_mut()
            .set_spatial_dispatch_request(QString::default());
        if matches!(result, ferric_browser_core::SpatialAck::Inactive) {
            self.as_mut()
                .cancel_spatial_navigation(if outcome.to_string() == "rejected" {
                    SpatialCancelReason::DispatchRejected
                } else {
                    SpatialCancelReason::User
                });
        } else {
            self.as_mut().publish_spatial_projection();
        }
        true
    }

    pub(super) fn spatial_invalidated(mut self: Pin<&mut Self>, reason: &QString) {
        let reason = match reason.to_string().as_str() {
            "surface-unavailable" => SpatialCancelReason::SurfaceUnavailable,
            "target-changed" => SpatialCancelReason::TargetChanged,
            "geometry-changed" => SpatialCancelReason::GeometryChanged,
            "window-inactive" => SpatialCancelReason::WindowInactive,
            "prompt-owned-input" => SpatialCancelReason::PromptOwnedInput,
            "renderer-unavailable" => SpatialCancelReason::RendererUnavailable,
            "physical-pointer" => SpatialCancelReason::PhysicalPointer,
            "dispatch-uncertain" => SpatialCancelReason::DispatchUncertain,
            _ => SpatialCancelReason::ViewChanged,
        };
        self.as_mut().cancel_spatial_navigation(reason);
    }
}
