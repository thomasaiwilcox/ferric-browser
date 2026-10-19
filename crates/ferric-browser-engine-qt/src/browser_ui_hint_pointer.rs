//! Local native-pointer Hint request lifecycle.
//!
//! Page geometry and QML callback data are untrusted. This state is deliberately
//! local and non-serializable; the adapter request is the only serialized form.

use std::time::{Duration, Instant};

use ferric_browser_core::{CommandSource, HintTarget, Mode, ParsedCommand};
use serde_json::Value;

use super::{
    CxxQtType, Event, Pin, QString, current_target,
    hint_payload::parse_hint_candidate,
    hint_policy::hint_requests_pointer,
    hint_policy::{LocalInteractionOperation, LocalInteractionOwner, LocalInteractionPermit},
    qobject,
};

pub(super) const PREFLIGHT_LIFETIME: Duration = Duration::from_millis(250);
const PREFLIGHT_KEYS: &[&str] = &[
    "ok",
    "candidate",
    "x",
    "y",
    "css_x",
    "css_y",
    "viewport_width",
    "viewport_height",
    "view_width",
    "view_height",
];

/// Rejects local-only Hint interactions in an expanded external command chain.
/// Callers must check the entire chain before sending or executing any member.
///
/// # Errors
/// Returns a local-input requirement when a chain requests Pointer Hint or
/// Target Search; ordinary semantic Hint commands remain allowed.
pub fn reject_external_hint_interaction_chain(commands: &[ParsedCommand]) -> Result<(), String> {
    if commands
        .iter()
        .any(|command| command.name == "target-search")
    {
        Err("target-search is available only from local keyboard or UI input".into())
    } else if commands.iter().any(hint_requests_pointer) {
        Err("Pointer click is available only from local UI or keyboard input".into())
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HintPointerLifecycle {
    Selecting,
    AwaitingConfirmationRelease,
    Preflight,
    DispatchPending,
    Terminal,
}

pub(super) struct HintPointerAction {
    pub(super) owner: LocalInteractionOwner,
    pub(super) source: CommandSource,
    pub(super) permit: Option<LocalInteractionPermit>,
    pub(super) lifecycle: HintPointerLifecycle,
    pub(super) label: Option<String>,
    pub(super) request_id: String,
    pub(super) preflight_started: Option<Instant>,
    pub(super) surface_serial: u64,
    pub(super) surface_revision: u64,
    pub(super) view_width: f64,
    pub(super) view_height: f64,
}

impl HintPointerAction {
    pub(super) fn new(
        owner: LocalInteractionOwner,
        source: CommandSource,
        permit: LocalInteractionPermit,
    ) -> Self {
        Self {
            owner,
            source,
            request_id: permit.request_id().to_string(),
            permit: Some(permit),
            lifecycle: HintPointerLifecycle::Selecting,
            label: None,
            preflight_started: None,
            surface_serial: 0,
            surface_revision: 0,
            view_width: 0.0,
            view_height: 0.0,
        }
    }

    /// INVARIANT: only the matching physical Enter release starts the bounded
    /// preflight window; a press, repeat, or stale owner cannot advance it.
    pub(super) fn begin_preflight(
        &mut self,
        label: String,
        surface_serial: u64,
        surface_revision: u64,
        view_width: f64,
        view_height: f64,
    ) -> bool {
        if self.lifecycle != HintPointerLifecycle::AwaitingConfirmationRelease
            || label.is_empty()
            || surface_serial == 0
            || surface_revision == 0
            || !view_width.is_finite()
            || !view_height.is_finite()
            || view_width <= 0.0
            || view_height <= 0.0
        {
            return false;
        }
        self.label = Some(label);
        self.surface_serial = surface_serial;
        self.surface_revision = surface_revision;
        self.view_width = view_width;
        self.view_height = view_height;
        self.preflight_started = Some(Instant::now());
        self.lifecycle = HintPointerLifecycle::Preflight;
        true
    }

    pub(super) fn preflight_is_live(&self, request_id: &str) -> bool {
        self.lifecycle == HintPointerLifecycle::Preflight
            && self.request_id == request_id
            && self
                .preflight_started
                .is_some_and(|started| started.elapsed() <= PREFLIGHT_LIFETIME)
    }

    pub(super) fn mark_dispatch_pending(&mut self) -> bool {
        if self.lifecycle != HintPointerLifecycle::Preflight
            || self
                .preflight_started
                .is_none_or(|started| started.elapsed() > PREFLIGHT_LIFETIME)
        {
            return false;
        }
        self.lifecycle = HintPointerLifecycle::DispatchPending;
        self.preflight_started = None;
        true
    }

    pub(super) fn terminate(&mut self) {
        self.lifecycle = HintPointerLifecycle::Terminal;
        self.preflight_started = None;
        self.permit = None;
    }
}

impl qobject::BrowserUi {
    /// INVARIANT: generic, replay, and alias-expanded command chains cannot
    /// mint local Pointer authority or perform earlier chain effects.
    pub(super) fn reject_generic_pointer_hint_chain(
        self: Pin<&mut Self>,
        commands: &[ParsedCommand],
    ) -> bool {
        if !commands.iter().any(hint_requests_pointer) {
            return false;
        }
        self.set_status_text(QString::from(
            "Pointer click requires a direct local UI or keyboard command",
        ));
        true
    }

    pub(super) fn hint_pointer_confirmation_pressed(mut self: Pin<&mut Self>) -> QString {
        let Some(owner) = self
            .as_ref()
            .rust()
            .interaction
            .hint_pointer_action
            .as_ref()
            .map(|a| a.owner)
        else {
            return QString::from(r#"{"error":"no-pointer-session"}"#);
        };
        if !self.as_ref().rust().hint_pointer_mode
            || self.as_ref().rust().core_mode != Mode::Hint
            || !self.as_ref().hint_pointer_owner_is_current(owner)
        {
            self.as_mut()
                .finish_hint_pointer_action("Pointer Hint owner changed; no action was sent");
            return QString::from(r#"{"error":"stale-owner"}"#);
        }
        let label = {
            let binding = self.as_ref();
            binding
                .rust()
                .interaction
                .hint_interaction
                .as_ref()
                .and_then(|interaction| interaction.snapshot().active_label)
        };
        let Some(label) = label else {
            self.set_status_text(QString::from("Select a Hint target before confirming"));
            return QString::from(r#"{"error":"no-selected-hint"}"#);
        };
        let session_has_label = self
            .as_ref()
            .rust()
            .interaction
            .hint_session
            .as_ref()
            .is_some_and(|session| {
                session.id == owner.session && session.hints.iter().any(|hint| hint.label == label)
            });
        let accepted = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(action) = this.interaction.hint_pointer_action.as_mut() else {
                return QString::from(r#"{"error":"no-pointer-session"}"#);
            };
            if action.lifecycle != HintPointerLifecycle::Selecting || !session_has_label {
                false
            } else {
                action.label = Some(label.clone());
                action.lifecycle = HintPointerLifecycle::AwaitingConfirmationRelease;
                true
            }
        };
        if !accepted {
            return QString::from(r#"{"error":"invalid-pointer-state"}"#);
        }
        self.as_mut()
            .set_hint_pointer_state(QString::from("awaiting-confirmation-release"));
        QString::from(serde_json::json!({"ok":true,"label":label}).to_string())
    }

    pub(super) fn hint_pointer_confirmation_released(
        mut self: Pin<&mut Self>,
        surface_serial: &QString,
        surface_revision: &QString,
        view_width: f64,
        view_height: f64,
    ) -> QString {
        let serial = canonical_nonzero_u64(&surface_serial.to_string());
        let revision = canonical_nonzero_u64(&surface_revision.to_string());
        let (Some(serial), Some(revision)) = (serial, revision) else {
            self.as_mut()
                .finish_hint_pointer_action("Pointer surface is unavailable; no action was sent");
            return QString::from(r#"{"error":"invalid-surface"}"#);
        };
        let Some(owner) = self
            .as_ref()
            .rust()
            .interaction
            .hint_pointer_action
            .as_ref()
            .map(|a| a.owner)
        else {
            return QString::from(r#"{"error":"no-pointer-session"}"#);
        };
        if !self.as_ref().rust().hint_pointer_mode
            || self.as_ref().rust().core_mode != Mode::Hint
            || !self.as_ref().hint_pointer_owner_is_current(owner)
        {
            self.as_mut()
                .finish_hint_pointer_action("Pointer Hint owner changed; no action was sent");
            return QString::from(r#"{"error":"stale-owner"}"#);
        }
        let (label, accepted) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let action = this.interaction.hint_pointer_action.as_mut();
            let Some(action) = action else {
                return QString::from(r#"{"error":"no-pointer-session"}"#);
            };
            let Some(label) = action.label.clone() else {
                return QString::from(r#"{"error":"no-confirmed-label"}"#);
            };
            let accepted =
                action.begin_preflight(label.clone(), serial, revision, view_width, view_height);
            (label, accepted)
        };
        if !accepted {
            self.as_mut()
                .finish_hint_pointer_action("Pointer confirmation was stale; no action was sent");
            return QString::from(r#"{"error":"invalid-pointer-state"}"#);
        }
        let request_id = self
            .as_ref()
            .rust()
            .interaction
            .hint_pointer_action
            .as_ref()
            .map_or_else(String::new, |action| action.request_id.clone());
        self.as_mut().set_hint_pointer_pending(true);
        self.as_mut()
            .set_hint_pointer_state(QString::from("preflight"));
        self.set_status_text(QString::from("Checking the selected target"));
        QString::from(
            serde_json::json!({"ok":true,"request_id":request_id,"label":label}).to_string(),
        )
    }

    pub(super) fn complete_hint_pointer_preflight(
        mut self: Pin<&mut Self>,
        request_id: &QString,
        preflight_json: &QString,
        surface_serial: &QString,
        surface_revision: &QString,
        view_width: f64,
        view_height: f64,
    ) -> bool {
        let request_id = request_id.to_string();
        let (matches_current_request, request_is_live) = {
            let binding = self.as_ref();
            let Some(action) = binding.rust().interaction.hint_pointer_action.as_ref() else {
                return false;
            };
            (
                action.request_id == request_id,
                action.preflight_is_live(&request_id),
            )
        };
        // A delayed callback from an older request must not terminate a newer
        // pointer session that happens to be in preflight on the same view.
        if !matches_current_request {
            return false;
        }
        if !request_is_live {
            self.as_mut()
                .finish_hint_pointer_action("Pointer preflight expired; no action was sent");
            return false;
        }
        let serial_text = surface_serial.to_string();
        let revision_text = surface_revision.to_string();
        let Some(serial) = canonical_nonzero_u64(&serial_text) else {
            self.as_mut()
                .finish_hint_pointer_action("Pointer preflight was rejected; no action was sent");
            return false;
        };
        let Some(revision) = canonical_nonzero_u64(&revision_text) else {
            self.as_mut()
                .finish_hint_pointer_action("Pointer preflight was rejected; no action was sent");
            return false;
        };
        let parsed = serde_json::from_str::<Value>(&preflight_json.to_string());
        let Ok(preflight) = parsed else {
            self.as_mut().finish_hint_pointer_action(
                "Pointer preflight returned invalid data; no action was sent",
            );
            return false;
        };
        let Some(object) = preflight.as_object() else {
            self.as_mut().finish_hint_pointer_action(
                "Pointer preflight returned invalid data; no action was sent",
            );
            return false;
        };
        if object
            .keys()
            .any(|key| !PREFLIGHT_KEYS.contains(&key.as_str()))
            || object.get("ok") != Some(&Value::Bool(true))
        {
            self.as_mut()
                .finish_hint_pointer_action("Selected target is no longer pointer eligible");
            return false;
        }
        let Some(candidate_value) = object.get("candidate") else {
            self.as_mut().finish_hint_pointer_action(
                "Pointer preflight omitted its target; no action was sent",
            );
            return false;
        };
        let fresh = match parse_hint_candidate(candidate_value) {
            Ok(candidate) => candidate,
            Err(_) => {
                self.as_mut().finish_hint_pointer_action(
                    "Pointer preflight target was invalid; no action was sent",
                );
                return false;
            }
        };
        let number = |key: &str| object.get(key).and_then(Value::as_f64);
        let (
            Some(x),
            Some(y),
            Some(css_x),
            Some(css_y),
            Some(viewport_width),
            Some(viewport_height),
            Some(js_view_width),
            Some(js_view_height),
        ) = (
            number("x"),
            number("y"),
            number("css_x"),
            number("css_y"),
            number("viewport_width"),
            number("viewport_height"),
            number("view_width"),
            number("view_height"),
        )
        else {
            self.as_mut().finish_hint_pointer_action(
                "Pointer preflight geometry was invalid; no action was sent",
            );
            return false;
        };
        let valid_numbers = [
            x,
            y,
            css_x,
            css_y,
            viewport_width,
            viewport_height,
            js_view_width,
            js_view_height,
        ]
        .into_iter()
        .all(f64::is_finite);
        if !valid_numbers || viewport_width <= 0.0 || viewport_height <= 0.0 {
            self.as_mut().finish_hint_pointer_action(
                "Pointer preflight geometry was invalid; no action was sent",
            );
            return false;
        }
        let (
            owner,
            label,
            expected_serial,
            expected_revision,
            expected_width,
            expected_height,
            live,
        ) = {
            let binding = self.as_ref();
            let Some(action) = binding.rust().interaction.hint_pointer_action.as_ref() else {
                return false;
            };
            (
                action.owner,
                action.label.clone().unwrap_or_default(),
                action.surface_serial,
                action.surface_revision,
                action.view_width,
                action.view_height,
                action.preflight_is_live(&request_id),
            )
        };
        if !live
            || !self.as_ref().rust().hint_pointer_mode
            || self.as_ref().rust().core_mode != Mode::Hint
            || !self.as_ref().rust().hint_pointer_pending
            || serial != expected_serial
            || revision != expected_revision
            || (view_width - expected_width).abs() > 0.25
            || (view_height - expected_height).abs() > 0.25
            || (js_view_width - expected_width).abs() > 0.25
            || (js_view_height - expected_height).abs() > 0.25
            || (view_width / viewport_width) < 0.2
            || (view_width / viewport_width) > 5.0
            || (view_height / viewport_height) < 0.2
            || (view_height / viewport_height) > 5.0
            || x < 0.0
            || y < 0.0
            || x >= view_width
            || y >= view_height
            || css_x < fresh.geometry.x
            || css_y < fresh.geometry.y
            || css_x > fresh.geometry.x + fresh.geometry.width
            || css_y > fresh.geometry.y + fresh.geometry.height
            || (x - css_x * view_width / viewport_width).abs() > 0.75
            || (y - css_y * view_height / viewport_height).abs() > 0.75
            || !self.as_ref().hint_pointer_owner_is_current(owner)
        {
            self.as_mut().finish_hint_pointer_action(
                "Pointer preflight expired or its owner changed; no action was sent",
            );
            return false;
        }
        let Some(target) = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        ) else {
            self.as_mut()
                .finish_hint_pointer_action("Pointer target changed; no action was sent");
            return false;
        };
        let expected_target = HintTarget {
            tab: target.tab,
            generation: target.generation,
            document: target.document,
        };
        let selected_is_current = {
            let binding = self.as_ref();
            binding
                .rust()
                .interaction
                .hint_session
                .as_ref()
                .filter(|session| {
                    session.id == owner.session
                        && session.configuration_revision == owner.configuration_revision
                })
                .and_then(|session| {
                    session
                        .select(
                            &label,
                            expected_target,
                            &fresh.frame_path,
                            true,
                            fresh.geometry,
                        )
                        .ok()
                })
                .is_some_and(|candidate| {
                    candidate.frame_owner.is_empty()
                        && fresh.frame_owner.is_empty()
                        && candidate.element_id == fresh.element_id
                        && candidate.related_element_id == fresh.related_element_id
                        && candidate.kind == fresh.kind
                        && candidate.frame_path == fresh.frame_path
                        && candidate.text == fresh.text
                        && candidate.href == fresh.href
                })
        };
        if !selected_is_current {
            self.as_mut()
                .finish_hint_pointer_action("Selected Hint target changed; no action was sent");
            return false;
        }
        let dispatch_identity = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.interaction
                .hint_pointer_action
                .as_mut()
                .and_then(|action| {
                    let permit = action.permit.take()?;
                    let consumed = this.interaction.hint_local_interaction_authority.consume(
                        permit,
                        action.source,
                        action.owner,
                        LocalInteractionOperation::HintPointerDispatch,
                    );
                    (consumed && action.mark_dispatch_pending())
                        .then(|| (action.request_id.clone(), action.owner.session.to_string()))
                })
        };
        let Some((request_id, session_id)) = dispatch_identity else {
            self.as_mut().finish_hint_pointer_action(
                "Pointer authorization was rejected; no action was sent",
            );
            return false;
        };
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.hint_pointer_pending = true;
            this.hint_pointer_state = QString::from("dispatch-pending");
        }
        self.as_mut().set_hint_pointer_pending(true);
        self.as_mut()
            .set_hint_pointer_state(QString::from("dispatch-pending"));
        self.as_mut()
            .set_status_text(QString::from("Pointer click pending"));
        let payload = serde_json::json!({
            "request_id":request_id,
            "session_id":session_id,
            "serial":serial.to_string(),
            "revision":revision.to_string(),
            "x":x,
            "y":y,
            "action":"left",
        });
        // INVARIANT: the permit is consumed before this one-shot property
        // handoff; no callback can publish a second adapter request.
        self.as_mut()
            .set_hint_pointer_dispatch_request(QString::from(payload.to_string()));
        true
    }

    pub(super) fn hint_pointer_dispatch_ack(
        mut self: Pin<&mut Self>,
        request_id: &QString,
        session_id: &QString,
        surface_serial: &QString,
        surface_revision: &QString,
        outcome: &QString,
    ) {
        let request_id = request_id.to_string();
        let session_id = session_id.to_string();
        let serial = canonical_nonzero_u64(&surface_serial.to_string());
        let revision = canonical_nonzero_u64(&surface_revision.to_string());
        let action_is_current = {
            let binding = self.as_ref();
            let Some(action) = binding.rust().interaction.hint_pointer_action.as_ref() else {
                return;
            };
            action.lifecycle == HintPointerLifecycle::DispatchPending
                && action.request_id == request_id
                && action.owner.session.to_string() == session_id
                && Some(action.surface_serial) == serial
                && Some(action.surface_revision) == revision
        };
        if !action_is_current {
            return;
        }
        let status = match outcome.to_string().as_str() {
            "delivered" => "Pointer click dispatched; page response is not verified",
            "rejected" => "Pointer click was rejected; no retry or fallback was sent",
            _ => "Pointer click outcome is uncertain; no retry or fallback was sent",
        };
        self.as_mut().finish_hint_pointer_action(status);
    }

    pub(super) fn hint_pointer_invalidated(mut self: Pin<&mut Self>, reason: &QString) {
        let lifecycle = self
            .as_ref()
            .rust()
            .interaction
            .hint_pointer_action
            .as_ref()
            .map(|action| action.lifecycle);
        if lifecycle.is_none() {
            return;
        }
        let status = if lifecycle == Some(HintPointerLifecycle::DispatchPending)
            || reason.to_string() == "dispatch-uncertain"
        {
            "Pointer click outcome is uncertain; no retry or fallback was sent"
        } else {
            "Pointer target or surface changed; no action was sent"
        };
        self.as_mut().finish_hint_pointer_action(status);
    }

    pub(super) fn hint_pointer_timeout(mut self: Pin<&mut Self>) {
        let lifecycle = self
            .as_ref()
            .rust()
            .interaction
            .hint_pointer_action
            .as_ref()
            .map(|action| action.lifecycle);
        let status = match lifecycle {
            Some(HintPointerLifecycle::Preflight) => {
                "Pointer preflight expired; no action was sent"
            }
            Some(HintPointerLifecycle::DispatchPending) => {
                "Pointer click outcome is uncertain; no retry or fallback was sent"
            }
            _ => return,
        };
        self.as_mut().finish_hint_pointer_action(status);
    }

    fn hint_pointer_owner_is_current(&self, owner: LocalInteractionOwner) -> bool {
        let rust = self.rust();
        let Some(target) = current_target(rust.state.as_ref(), rust.tab) else {
            return false;
        };
        rust.window == Some(owner.window)
            && rust.interaction.hint_configuration_revision == owner.configuration_revision
            && rust
                .interaction
                .hint_session
                .as_ref()
                .is_some_and(|session| {
                    session.id == owner.session
                        && session.target == owner.target
                        && session.configuration_revision == owner.configuration_revision
                })
            && HintTarget {
                tab: target.tab,
                generation: target.generation,
                document: target.document,
            } == owner.target
    }

    fn finish_hint_pointer_action(mut self: Pin<&mut Self>, status: &str) {
        if let Some(action) = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .interaction
            .hint_pointer_action
            .as_mut()
        {
            action.terminate();
        }
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.interaction.hint_pointer_action = None;
            this.interaction.hint_pointer_source = None;
            this.interaction.hint_session = None;
            this.interaction.hint_interaction = None;
            this.interaction.hint_consumed.clear();
            this.hint_values = QString::default();
            this.pending_hint_action = None;
            this.interaction.pending_hint_palette_permit = None;
            this.interaction
                .hint_local_interaction_authority
                .clear_session_requests();
        }
        self.as_mut().set_hint_visible(false);
        self.as_mut().set_hint_pointer_pending(false);
        self.as_mut()
            .set_hint_pointer_dispatch_request(QString::default());
        self.as_mut().set_hint_pointer_mode(false);
        self.as_mut()
            .set_hint_pointer_state(QString::from("terminal"));
        if self.as_ref().rust().core_mode == Mode::Hint {
            let window = self.as_ref().rust().window;
            if let Some(window) = window {
                let _ = self.as_mut().reduce_event(Event::PopMode { window });
            }
            self.as_mut().set_core_mode(Mode::Normal);
            self.as_mut().set_mode(QString::from("normal"));
        }
        self.set_status_text(QString::from(status));
    }
}

fn canonical_nonzero_u64(value: &str) -> Option<u64> {
    let parsed = value.parse::<u64>().ok()?;
    (parsed > 0 && value == parsed.to_string()).then_some(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hint_policy::{LocalInteractionAuthority, LocalInteractionOperation};
    use ferric_browser_core::{
        DocumentId, HintSessionId, HintTarget, ParsedCommand, TabId, WindowId,
    };

    fn action() -> HintPointerAction {
        let owner = LocalInteractionOwner {
            window: WindowId::from_display("windowid-1").expect("window"),
            session: HintSessionId::from_display("hintsessionid-2").expect("session"),
            target: HintTarget {
                tab: TabId::from_display("tabid-3").expect("tab"),
                generation: 4,
                document: DocumentId::from_display("documentid-5").expect("document"),
            },
            configuration_revision: 6,
        };
        let permit = LocalInteractionAuthority::default()
            .mint(
                CommandSource::Keyboard,
                owner,
                LocalInteractionOperation::HintPointerDispatch,
            )
            .expect("keyboard permit");
        HintPointerAction::new(owner, CommandSource::Keyboard, permit)
    }

    #[test]
    fn preflight_requires_release_owner_facts_and_one_live_request() {
        let mut action = action();
        assert!(!action.begin_preflight("aa".into(), 1, 1, 640.0, 480.0));
        action.lifecycle = HintPointerLifecycle::AwaitingConfirmationRelease;
        assert!(!action.begin_preflight("aa".into(), 0, 1, 640.0, 480.0));
        assert!(action.begin_preflight("aa".into(), 1, 1, 640.0, 480.0));
        let request_id = action.request_id.clone();
        assert!(action.preflight_is_live(&request_id));
        assert!(!action.preflight_is_live("other"));
        assert!(action.mark_dispatch_pending());
        assert!(!action.preflight_is_live(&request_id));
        assert!(!action.mark_dispatch_pending());
    }

    #[test]
    fn terminal_clears_permit_and_preflight_authority() {
        let mut action = action();
        action.terminate();
        assert_eq!(action.lifecycle, HintPointerLifecycle::Terminal);
        assert!(action.permit.is_none());
        assert!(!action.preflight_is_live(&action.request_id));
    }

    #[test]
    fn preflight_timeout_is_terminal_and_never_becomes_dispatchable() {
        let mut action = action();
        action.lifecycle = HintPointerLifecycle::AwaitingConfirmationRelease;
        assert!(action.begin_preflight("aa".into(), 1, 1, 640.0, 480.0));
        action.preflight_started = Some(
            Instant::now()
                .checked_sub(PREFLIGHT_LIFETIME + Duration::from_millis(1))
                .expect("short test duration is representable"),
        );
        assert!(!action.preflight_is_live(&action.request_id));
        assert!(!action.mark_dispatch_pending());
    }

    #[test]
    fn external_hint_interaction_guard_rejects_target_search_after_expansion() {
        let search = ParsedCommand {
            name: "target-search".into(),
            arguments: vec!["links".into()],
        };
        assert_eq!(
            reject_external_hint_interaction_chain(&[search]),
            Err("target-search is available only from local keyboard or UI input".into())
        );

        let local_hint = ParsedCommand {
            name: "hint".into(),
            arguments: vec!["links".into()],
        };
        assert_eq!(
            reject_external_hint_interaction_chain(&[local_hint]),
            Ok(())
        );
    }
}
