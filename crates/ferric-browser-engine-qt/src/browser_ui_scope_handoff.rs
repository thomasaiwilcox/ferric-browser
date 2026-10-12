use super::browser_ui_hint_selection::local_hint_owner;
use super::browser_ui_navigation::effective_hint_selector_snapshot;
use super::hint_policy::{HintScopePurpose, LocalInteractionOperation, LocalInteractionOwner};
use super::{
    CxxQtType, Event, HintSession, LogicalRect, Mode, Pin, QString, SpatialOwner, SpatialScope,
    SpatialSession, SpatialTarget, SurfaceStamp, current_target, hint_kind_name,
    map_css_rect_to_view_local, qobject,
};
use ferric_browser_core::HintCandidate;
use serde_json::Value;

pub(super) fn hint_capabilities_json(kind: ferric_browser_core::HintKind) -> Value {
    Value::Array(
        ferric_browser_core::hint_capabilities(kind)
            .iter()
            .map(|capability| {
                serde_json::json!({
                    "id": capability.id(),
                    "label": capability.label(),
                    "enabled": matches!(
                        capability,
                        ferric_browser_core::HintCapability::SetScrollTarget
                            | ferric_browser_core::HintCapability::Focus
                            | ferric_browser_core::HintCapability::HintWithin
                            | ferric_browser_core::HintCapability::GridWithin
                    )
                })
            })
            .collect(),
    )
}

pub(super) fn known_hint_capability_id(id: &str) -> bool {
    use ferric_browser_core::HintKind::{FocusableSurface, Region, ScrollableRegion, Surface};

    [Region, ScrollableRegion, Surface, FocusableSurface]
        .into_iter()
        .any(|kind| {
            ferric_browser_core::hint_capabilities(kind)
                .iter()
                .any(|capability| capability.id() == id)
        })
}

#[derive(Clone, Copy)]
pub(super) struct HintScopeHandoffContext<'a> {
    pub(super) current_owner: Option<LocalInteractionOwner>,
    pub(super) current_scope_lease_id: Option<&'a str>,
    pub(super) current_selector_policy_revision: Option<u64>,
    pub(super) current_session: Option<&'a HintSession>,
    pub(super) current_family: &'a str,
    pub(super) current_mode: Mode,
}

pub(super) fn hint_scope_handoff_is_valid(
    binding: &super::hint_policy::HintScopeBinding,
    lease_id: &str,
    receipt: &Value,
    context: HintScopeHandoffContext<'_>,
) -> bool {
    let expected_family = match (binding.purpose, binding.kind) {
        (
            super::hint_policy::HintScopePurpose::HintWithin,
            ferric_browser_core::HintKind::Region | ferric_browser_core::HintKind::ScrollableRegion,
        ) => Some("regions"),
        (
            super::hint_policy::HintScopePurpose::GridWithin,
            ferric_browser_core::HintKind::Region | ferric_browser_core::HintKind::ScrollableRegion,
        ) => Some("regions"),
        (
            super::hint_policy::HintScopePurpose::GridWithin,
            ferric_browser_core::HintKind::Surface
            | ferric_browser_core::HintKind::FocusableSurface,
        ) => Some("surfaces"),
        _ => None,
    };
    let parent_scope_id = binding.parent_scope_lease_id.as_deref();
    let receipt_parent_matches = match (parent_scope_id, receipt.get("parent_scope_lease_id")) {
        (None, Some(Value::Null)) => true,
        (Some(expected), Some(Value::String(actual))) => expected == actual,
        _ => false,
    };
    let receipt_fields_match = receipt.as_object().is_some_and(|object| {
        const RECEIPT_FIELDS: &[&str] = &[
            "ok",
            "lease_id",
            "element_id",
            "frame_path",
            "kind",
            "selector_revision",
            "configuration_revision",
            "parent_scope_lease_id",
        ];
        object.len() == RECEIPT_FIELDS.len()
            && object
                .keys()
                .all(|key| RECEIPT_FIELDS.contains(&key.as_str()))
            && receipt.get("ok") == Some(&Value::Bool(true))
            && receipt.get("lease_id").and_then(Value::as_str) == Some(lease_id)
            && receipt.get("element_id").and_then(Value::as_u64)
                == Some(u64::from(binding.element_id))
            && receipt.get("frame_path").and_then(Value::as_str)
                == Some(binding.frame_path.as_str())
            && receipt.get("kind").and_then(Value::as_str) == Some(hint_kind_name(binding.kind))
            && receipt.get("selector_revision").and_then(Value::as_str)
                == Some(format!("{:016x}", binding.selector_policy_revision).as_str())
            && receipt
                .get("configuration_revision")
                .and_then(Value::as_u64)
                == Some(binding.owner.configuration_revision)
            && receipt_parent_matches
    });
    let current_scope_matches = context.current_scope_lease_id == parent_scope_id;
    let source_session_matches = context.current_session.is_some_and(|session| {
        session.id == binding.owner.session
            && session.target == binding.owner.target
            && session.configuration_revision == binding.owner.configuration_revision
            && session.selector_policy_revision == binding.selector_policy_revision
    });

    expected_family.is_some()
        && receipt_fields_match
        && context.current_owner == Some(binding.owner)
        && current_scope_matches
        && context.current_selector_policy_revision == Some(binding.selector_policy_revision)
        && source_session_matches
        && expected_family == Some(context.current_family)
        && context.current_mode == Mode::Hint
        && lease_id == binding.lease_id
}

impl qobject::BrowserUi {
    pub(super) fn begin_grid_scope_handoff(
        mut self: Pin<&mut Self>,
        selected: &HintCandidate,
        fresh: &HintCandidate,
    ) -> Result<Value, QString> {
        let binding = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(owner) = local_hint_owner(this) else {
                self.set_status_text(QString::from(
                    "Scoped Grid rejected: local session is unavailable",
                ));
                return Err(QString::from(r#"{"error":"local-session-unavailable"}"#));
            };
            let Some(session) = this.hint_session.as_ref() else {
                return Err(QString::from(r#"{"error":"no-session"}"#));
            };
            if this.active_hint_scope.as_ref().is_some_and(|parent| {
                parent.owner.target != owner.target
                    || parent.owner.configuration_revision != owner.configuration_revision
            }) {
                self.set_status_text(QString::from("Scoped Grid rejected: parent scope is stale"));
                return Err(QString::from(r#"{"error":"stale-scope"}"#));
            }
            let Some(next_id) = this.next_hint_scope_lease_id.checked_add(1) else {
                self.set_status_text(QString::from(
                    "Scoped Grid rejected: lease capacity exhausted",
                ));
                return Err(QString::from(r#"{"error":"scope-lease-capacity"}"#));
            };
            this.next_hint_scope_lease_id = next_id;
            let selector_policy_revision = session.selector_policy_revision;
            let Some(permit) = this.hint_local_interaction_authority.mint(
                super::CommandSource::Ui,
                owner,
                LocalInteractionOperation::GridWithinHandoff,
            ) else {
                self.set_status_text(QString::from(
                    "Scoped Grid rejected: local authority unavailable",
                ));
                return Err(QString::from(r#"{"error":"local-authority-unavailable"}"#));
            };
            this.pending_grid_scope_permit = Some(permit);
            let binding = super::hint_policy::HintScopeBinding {
                purpose: HintScopePurpose::GridWithin,
                lease_id: format!("hint-scope-{next_id}"),
                owner,
                element_id: selected.element_id,
                frame_path: fresh.frame_path.clone(),
                kind: selected.kind,
                selector_policy_revision,
                parent_scope_lease_id: this
                    .active_hint_scope
                    .as_ref()
                    .map(|scope| scope.lease_id.clone()),
            };
            this.pending_hint_scope_handoff = Some(binding.clone());
            binding
        };
        Ok(serde_json::json!({
            "action":"grid-within",
            "kind":hint_kind_name(selected.kind),
            "scope_lease":{
                "lease_id":binding.lease_id,
                "element_id":binding.element_id,
                "frame_path":binding.frame_path,
                "kind":hint_kind_name(binding.kind),
                "selector_revision":format!("{:016x}", binding.selector_policy_revision),
                "configuration_revision":binding.owner.configuration_revision,
                "parent_scope_lease_id":binding.parent_scope_lease_id,
            }
        }))
    }

    pub(super) fn complete_hint_scope_handoff(
        mut self: Pin<&mut Self>,
        lease_id: &QString,
        receipt_json: &QString,
    ) -> bool {
        let lease_id = lease_id.to_string();
        let receipt = match serde_json::from_str::<Value>(&receipt_json.to_string()) {
            Ok(Value::Object(receipt)) => Value::Object(receipt),
            _ => {
                self.set_status_text(QString::from("Scoped Hint rejected: invalid lease receipt"));
                return false;
            }
        };
        let Some(binding) = self
            .as_ref()
            .rust()
            .pending_hint_scope_handoff
            .as_ref()
            .cloned()
        else {
            self.set_status_text(QString::from(
                "Scoped Hint rejected: no pending local handoff",
            ));
            return false;
        };
        let browser_binding = self.as_ref();
        let rust = browser_binding.rust();
        let current_selector_policy_revision = {
            let browser_binding = self.as_ref();
            let rust = browser_binding.rust();
            effective_hint_selector_snapshot(
                &rust.config_json.to_string(),
                &rust.current_url.to_string(),
                rust.hint_configuration_revision,
                &rust.hint_family.to_string(),
            )
            .ok()
            .and_then(|snapshot| {
                snapshot
                    .get("selector_revision")
                    .and_then(Value::as_str)
                    .and_then(|revision| u64::from_str_radix(revision, 16).ok())
            })
        };
        let current_scope_lease_id = rust
            .active_hint_scope
            .as_ref()
            .map(|scope| scope.lease_id.as_str());
        let current_family = rust.hint_family.to_string();
        let context = HintScopeHandoffContext {
            current_owner: local_hint_owner(rust),
            current_scope_lease_id,
            current_selector_policy_revision,
            current_session: rust.hint_session.as_ref(),
            current_family: &current_family,
            current_mode: rust.core_mode,
        };
        if binding.purpose != HintScopePurpose::HintWithin
            || !hint_scope_handoff_is_valid(&binding, &lease_id, &receipt, context)
        {
            self.set_status_text(QString::from("Scoped Hint rejected: lease became stale"));
            return false;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.pending_hint_scope_handoff = None;
            this.active_hint_scope = Some(binding);
            this.hint_session = None;
            this.hint_interaction = None;
            this.hint_consumed.clear();
            this.hint_values = QString::default();
            this.pending_hint_action = None;
            this.pending_hint_palette_permit = None;
            this.pending_grid_scope_permit = None;
            this.hint_local_interaction_authority
                .clear_session_requests();
        }
        self.as_mut()
            .set_hint_options("all", false, "current", None, false, 1);
        self.as_mut().set_hint_visible(false);
        self.as_mut()
            .set_status_text(QString::from("Select a target within the region"));
        true
    }

    pub(super) fn complete_grid_scope_handoff(
        mut self: Pin<&mut Self>,
        lease_id: &QString,
        receipt_json: &QString,
        geometry_json: &QString,
        serial: &QString,
        revision: &QString,
        view_width: f64,
        view_height: f64,
        zoom_factor: f64,
    ) -> bool {
        let lease_id = lease_id.to_string();
        let receipt = match serde_json::from_str::<Value>(&receipt_json.to_string()) {
            Ok(Value::Object(receipt)) => Value::Object(receipt),
            _ => {
                self.set_status_text(QString::from("Scoped Grid rejected: invalid lease receipt"));
                return false;
            }
        };
        let geometry = match serde_json::from_str::<Value>(&geometry_json.to_string()) {
            Ok(Value::Object(geometry)) => Value::Object(geometry),
            _ => {
                self.set_status_text(QString::from("Scoped Grid rejected: invalid geometry"));
                return false;
            }
        };
        let Some(binding) = self
            .as_ref()
            .rust()
            .pending_hint_scope_handoff
            .as_ref()
            .cloned()
        else {
            self.set_status_text(QString::from("Scoped Grid rejected: no local handoff"));
            return false;
        };
        let current_selector_policy_revision = {
            let binding = self.as_ref();
            let rust = binding.rust();
            effective_hint_selector_snapshot(
                &rust.config_json.to_string(),
                &rust.current_url.to_string(),
                rust.hint_configuration_revision,
                &rust.hint_family.to_string(),
            )
            .ok()
            .and_then(|snapshot| {
                snapshot
                    .get("selector_revision")
                    .and_then(Value::as_str)
                    .and_then(|revision| u64::from_str_radix(revision, 16).ok())
            })
        };
        let binding_ref = self.as_ref();
        let rust = binding_ref.rust();
        let current_scope_lease_id = rust
            .active_hint_scope
            .as_ref()
            .map(|scope| scope.lease_id.as_str());
        let current_family = rust.hint_family.to_string();
        let context = HintScopeHandoffContext {
            current_owner: local_hint_owner(rust),
            current_scope_lease_id,
            current_selector_policy_revision,
            current_session: rust.hint_session.as_ref(),
            current_family: &current_family,
            current_mode: rust.core_mode,
        };
        if binding.purpose != HintScopePurpose::GridWithin
            || !hint_scope_handoff_is_valid(&binding, &lease_id, &receipt, context)
        {
            self.set_status_text(QString::from("Scoped Grid rejected: lease became stale"));
            return false;
        }
        let permit_valid = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.pending_grid_scope_permit.take().is_some_and(|permit| {
                this.hint_local_interaction_authority.consume(
                    permit,
                    super::CommandSource::Ui,
                    binding.owner,
                    LocalInteractionOperation::GridWithinHandoff,
                )
            })
        };
        if !permit_valid {
            self.set_status_text(QString::from("Scoped Grid rejected: local permit is stale"));
            return false;
        }

        let field = |name: &str| geometry.get(name);
        let metadata_matches = field("ok") == Some(&Value::Bool(true))
            && field("lease_id").and_then(Value::as_str) == Some(binding.lease_id.as_str())
            && field("element_id").and_then(Value::as_u64) == Some(u64::from(binding.element_id))
            && field("frame_path").and_then(Value::as_str) == Some(binding.frame_path.as_str())
            && field("kind").and_then(Value::as_str) == Some(hint_kind_name(binding.kind))
            && field("selector_revision").and_then(Value::as_str)
                == Some(format!("{:016x}", binding.selector_policy_revision).as_str())
            && field("configuration_revision").and_then(Value::as_u64)
                == Some(binding.owner.configuration_revision)
            && field("scroll_revision").and_then(Value::as_u64) == Some(0);
        let Some(css_viewport_width) = field("viewport_width").and_then(Value::as_f64) else {
            self.set_status_text(QString::from(
                "Scoped Grid rejected: viewport is unavailable",
            ));
            return false;
        };
        let Some(css_viewport_height) = field("viewport_height").and_then(Value::as_f64) else {
            self.set_status_text(QString::from(
                "Scoped Grid rejected: viewport is unavailable",
            ));
            return false;
        };
        let Some(css_rect) = field("geometry").and_then(|value| {
            Some(LogicalRect::new(
                value.get("x")?.as_f64()?,
                value.get("y")?.as_f64()?,
                value.get("width")?.as_f64()?,
                value.get("height")?.as_f64()?,
            )?)
        }) else {
            self.set_status_text(QString::from(
                "Scoped Grid rejected: scope geometry is invalid",
            ));
            return false;
        };
        if !metadata_matches
            || !view_width.is_finite()
            || !view_height.is_finite()
            || !zoom_factor.is_finite()
            || zoom_factor <= 0.0
        {
            self.set_status_text(QString::from(
                "Scoped Grid rejected: scope geometry is stale",
            ));
            return false;
        }
        let Some(interaction) = map_css_rect_to_view_local(
            css_rect,
            css_viewport_width,
            css_viewport_height,
            view_width,
            view_height,
        ) else {
            self.set_status_text(QString::from("Scoped Grid rejected: unsupported geometry"));
            return false;
        };
        let (Some(serial), Some(revision)) = (
            serial.to_string().parse::<u64>().ok(),
            revision.to_string().parse::<u64>().ok(),
        ) else {
            self.set_status_text(QString::from("Scoped Grid rejected: page surface is stale"));
            return false;
        };
        let Some(surface) = SurfaceStamp::new(serial, revision) else {
            self.set_status_text(QString::from("Scoped Grid rejected: page surface is stale"));
            return false;
        };
        let scope = SpatialScope {
            lease_id: binding.lease_id.clone(),
            element_id: binding.element_id,
            frame_path: binding.frame_path.clone(),
            kind: binding.kind,
            configuration_revision: binding.owner.configuration_revision,
            selector_policy_revision: binding.selector_policy_revision,
            scroll_revision: 0,
            css_viewport_width_bits: css_viewport_width.to_bits(),
            css_viewport_height_bits: css_viewport_height.to_bits(),
            zoom_factor_bits: zoom_factor.to_bits(),
        };
        if !scope.is_valid() {
            self.set_status_text(QString::from(
                "Scoped Grid rejected: invalid scope identity",
            ));
            return false;
        }
        let viewport = match LogicalRect::new(0.0, 0.0, view_width, view_height) {
            Some(viewport) => viewport,
            None => {
                self.set_status_text(QString::from("Scoped Grid rejected: invalid view geometry"));
                return false;
            }
        };
        let target = SpatialTarget::new(
            binding.owner.window,
            super::Target {
                tab: binding.owner.target.tab,
                generation: binding.owner.target.generation,
                document: binding.owner.target.document,
            },
        );
        let owner = SpatialOwner::new(target, surface);
        let current_owner_matches = {
            let ui_binding = self.as_ref();
            let rust = ui_binding.rust();
            rust.spatial_session.is_none()
                && rust.window == Some(binding.owner.window)
                && current_target(rust.state.as_ref(), rust.tab) == Some(target.target)
        };
        if !current_owner_matches {
            self.set_status_text(QString::from("Scoped Grid rejected: page owner changed"));
            return false;
        }
        let session = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let mut session = SpatialSession::new_scoped(&mut this.spatial_ids, target, scope);
            if session
                .observe_scoped_surface(owner, viewport, interaction)
                .is_err()
            {
                None
            } else {
                Some(session)
            }
        };
        let Some(session) = session else {
            self.set_status_text(QString::from(
                "Scoped Grid rejected: invalid scope geometry",
            ));
            return false;
        };

        if self
            .as_mut()
            .reduce_event(Event::Escape {
                window: binding.owner.window,
            })
            .is_err()
            || self
                .as_mut()
                .reduce_event(Event::PushMode {
                    window: binding.owner.window,
                    mode: Mode::Grid,
                })
                .is_err()
        {
            self.set_status_text(QString::from(
                "Scoped Grid rejected: mode transition failed",
            ));
            return false;
        }
        let session_id = session.id().to_string();
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.pending_hint_scope_handoff = None;
            this.active_hint_scope = None;
            this.hint_session = None;
            this.hint_interaction = None;
            this.hint_consumed.clear();
            this.hint_values = QString::default();
            this.pending_hint_action = None;
            this.pending_hint_palette_permit = None;
            this.pending_grid_scope_permit = None;
            this.hint_local_interaction_authority
                .clear_session_requests();
            this.spatial_session = Some(session);
        }
        self.as_mut()
            .set_hint_options("all", false, "current", None, false, 1);
        self.as_mut().set_hint_visible(false);
        self.as_mut().set_core_mode(Mode::Grid);
        self.as_mut().set_mode(QString::from("grid"));
        self.as_mut()
            .set_spatial_session_id(QString::from(session_id));
        self.as_mut()
            .set_spatial_scope_lease_id(QString::from(binding.lease_id));
        self.as_mut()
            .set_spatial_dispatch_request(QString::default());
        self.as_mut().set_spatial_selecting(false);
        self.as_mut().publish_spatial_projection();
        self.as_mut().set_spatial_visible(true);
        self.as_mut()
            .set_status_text(QString::from("Grid within ready"));
        true
    }
}
