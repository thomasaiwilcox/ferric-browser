use super::browser_ui_hint_pointer::HintPointerAction;
use super::browser_ui_hint_selection::local_hint_owner;
use super::hint_payload::validate_hint_candidates_for_family;
use super::hint_policy::{
    HintOptions, LocalInteractionOperation, LocalInteractionOwner, parse_hint_options,
};
use super::{
    CommandRegistry, CoreHintAutoFollow, CxxQtType, Event, HintInteraction, HintInteractionInput,
    HintInteractionOutcome, HintSession, HintTarget, Mode, ParsedCommand, Pin, QString, Value,
    assign_labels_with_options, current_target, hint_json, hint_kind_name, parse_hint_payload,
    qobject, refresh_labels,
};

use ferric_browser_config::{Config, HintSelectorFamily, resolve_hint_selector_policies};

pub(super) fn effective_hint_selector_snapshot(
    config_json: &str,
    url: &str,
    configuration_revision: u64,
    family: &str,
) -> Result<Value, String> {
    let selector_family = HintSelectorFamily::parse(family);
    if selector_family.is_none() && !matches!(family, "regions" | "surfaces") {
        return Err(format!("unsupported hint selector family: {family}"));
    }
    let config = serde_json::from_str::<Config>(config_json)
        .map_err(|error| format!("effective configuration is unavailable: {error}"))?;
    let policies = resolve_hint_selector_policies(&config, url)?;
    let revision_input = if let Some(selector_family) = selector_family {
        serde_json::to_vec(&(configuration_revision, url, selector_family, &policies))
    } else {
        serde_json::to_vec(&(configuration_revision, url, family, &policies))
    }
    .map_err(|error| format!("selector policy could not be serialized: {error}"))?;
    let selector_policy_revision = revision_input
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
        });
    Ok(serde_json::json!({
        "family": family,
        "configuration_revision": configuration_revision,
        "selector_revision": format!("{selector_policy_revision:016x}"),
        "policies": policies,
    }))
}

impl qobject::BrowserUi {
    pub(super) fn dispatch_hint_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        registry: &CommandRegistry,
    ) -> bool {
        if let Err(error) = registry.validate(command, self.as_ref().rust().core_mode) {
            self.set_status_text(QString::from(error.to_string()));
            return false;
        }
        let options = match parse_hint_options(command) {
            Ok(options) => options,
            Err(error) => {
                self.set_status_text(QString::from(error));
                return false;
            }
        };
        self.as_mut().set_hint_options(
            &options.family,
            options.rapid,
            &options.target,
            options.script.as_deref(),
            options.first,
            options.index,
        );
        true
    }

    pub(super) fn note_hint_configuration_change(mut self: Pin<&mut Self>) {
        let hint_mode_was_active = self.as_ref().rust().core_mode == Mode::Hint;
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.hint_configuration_revision = this.hint_configuration_revision.saturating_add(1);
            this.hint_session = None;
            this.hint_interaction = None;
            this.pending_hint_scope_handoff = None;
            this.active_hint_scope = None;
            this.hint_consumed.clear();
            this.pending_hint_action = None;
            if let Some(action) = this.hint_pointer_action.as_mut() {
                action.terminate();
            }
            this.hint_pointer_action = None;
            this.hint_pointer_source = None;
            this.pending_hint_palette_permit = None;
            this.hint_local_interaction_authority
                .clear_session_requests();
        }
        self.as_mut().set_hint_pointer_pending(false);
        self.as_mut()
            .set_hint_pointer_dispatch_request(QString::default());
        self.as_mut().set_hint_pointer_mode(false);
        self.as_mut()
            .set_hint_pointer_state(QString::from("terminal"));
        if hint_mode_was_active {
            self.as_mut().cancel_hints();
        } else {
            self.set_hint_visible(false);
        }
    }

    pub(super) fn set_hint_mode(mut self: Pin<&mut Self>) {
        self.as_mut()
            .set_hint_options("all", false, "current", None, false, 1);
    }

    pub(super) fn set_hint_mode_kind(mut self: Pin<&mut Self>, links_only: bool) {
        self.as_mut().set_hint_options(
            if links_only { "links" } else { "all" },
            false,
            "current",
            None,
            false,
            1,
        );
    }

    pub(super) fn set_hint_options(
        mut self: Pin<&mut Self>,
        family: &str,
        rapid: bool,
        target: &str,
        script: Option<&str>,
        first: bool,
        index: usize,
    ) {
        self.as_mut().set_hint_pointer_pending(false);
        self.as_mut()
            .set_hint_pointer_state(QString::from("terminal"));
        self.as_mut()
            .set_hint_pointer_dispatch_request(QString::default());
        self.as_mut().set_hint_pointer_mode(target == "pointer");
        self.as_mut().set_hint_links_only(family == "links");
        self.as_mut().set_hint_family(QString::from(family));
        self.as_mut().set_hint_rapid(rapid);
        if let Ok(config) =
            serde_json::from_str::<super::Config>(&self.as_ref().rust().config_json.to_string())
        {
            self.as_mut().update_hint_preferences(&config);
        }
        target.clone_into(
            &mut self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .hint_rapid_target,
        );
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.hint_pointer_source = None;
            this.hint_pointer_action = None;
            this.hint_script = script.map(ToOwned::to_owned);
            this.hint_first = first;
            this.hint_index = index;
            this.hint_rapid_tabs_created = 0;
            this.hint_consumed.clear();
            this.hint_interaction = None;
            this.pending_hint_palette_permit = None;
            this.hint_local_interaction_authority
                .clear_session_requests();
        }
        let window = self.as_ref().rust().window;
        if let Some(window) = window
            && self
                .as_mut()
                .reduce_event(Event::PushMode {
                    window,
                    mode: Mode::Hint,
                })
                .is_err()
        {
            self.set_status_text(QString::from("Hint mode rejected"));
            return;
        }
        self.as_mut().set_core_mode(Mode::Hint);
        self.as_mut().set_mode(QString::from("hint"));
        self.set_status_text(QString::from("Select a page element"));
    }

    pub(super) fn set_local_pointer_hint_options(
        mut self: Pin<&mut Self>,
        options: &HintOptions,
        source: super::CommandSource,
    ) -> bool {
        if options.target != "pointer"
            || !matches!(
                source,
                super::CommandSource::Ui | super::CommandSource::Keyboard
            )
            || self.as_ref().rust().recording_macro.is_some()
        {
            self.set_status_text(QString::from(
                "Pointer click requires a direct local UI or keyboard command",
            ));
            return false;
        }
        self.as_mut()
            .set_hint_options(&options.family, false, "pointer", None, false, 1);
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .hint_pointer_source = Some(source);
        self.as_mut()
            .set_hint_pointer_state(QString::from("selecting"));
        true
    }

    pub(super) fn clear_hint_session_state(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.hint_session = None;
            this.hint_interaction = None;
            this.pending_hint_scope_handoff = None;
            this.active_hint_scope = None;
            this.hint_consumed.clear();
            this.hint_values = QString::default();
            this.pending_hint_action = None;
            if let Some(action) = this.hint_pointer_action.as_mut() {
                action.terminate();
            }
            this.hint_pointer_action = None;
            this.hint_pointer_source = None;
            this.pending_hint_palette_permit = None;
            this.hint_local_interaction_authority
                .clear_session_requests();
        }
        self.as_mut().set_hint_visible(false);
        self.as_mut().set_hint_pointer_pending(false);
        self.as_mut()
            .set_hint_pointer_dispatch_request(QString::default());
        self.as_mut().set_hint_pointer_mode(false);
        self.as_mut()
            .set_hint_pointer_state(QString::from("terminal"));
    }

    pub(super) fn begin_hint_session(
        mut self: Pin<&mut Self>,
        candidates_json: &QString,
    ) -> QString {
        if self.as_ref().rust().core_mode != Mode::Hint {
            self.as_mut().set_hint_mode();
        }
        let family = self.as_ref().rust().hint_family.to_string();
        let configuration_revision = self.as_ref().rust().hint_configuration_revision;
        let requested_scope_lease_id = serde_json::from_str::<Value>(&candidates_json.to_string())
            .ok()
            .and_then(|payload| {
                payload
                    .get("scope_lease_id")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            });
        let snapshot = match effective_hint_selector_snapshot(
            &self.as_ref().rust().config_json.to_string(),
            &self.as_ref().rust().current_url.to_string(),
            configuration_revision,
            &family,
        ) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Hint selectors unavailable: {error}"
                )));
                return QString::from(
                    serde_json::json!({"error":"selector-policy-unavailable"}).to_string(),
                );
            }
        };
        let expected_selector_revision = snapshot
            .get("selector_revision")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some(selector_policy_revision) =
            u64::from_str_radix(expected_selector_revision, 16).ok()
        else {
            self.set_status_text(QString::from(
                "Hints rejected: invalid selector policy revision",
            ));
            return QString::from(
                serde_json::json!({"error":"invalid-selector-policy-revision"}).to_string(),
            );
        };
        let supplied_selector_revision =
            serde_json::from_str::<Value>(&candidates_json.to_string())
                .ok()
                .and_then(|payload| payload.get("selector_revision").cloned())
                .and_then(|value| value.as_str().map(ToOwned::to_owned));
        if supplied_selector_revision.as_deref() != Some(expected_selector_revision) {
            self.set_status_text(QString::from(
                "Hints rejected: selector policy changed during collection",
            ));
            return QString::from(serde_json::json!({"error":"stale-selector-policy"}).to_string());
        }
        let Some(target) = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        ) else {
            self.set_status_text(QString::from("Hints unavailable: tab is no longer live"));
            return QString::from(r#"{"error":"stale-target"}"#);
        };
        let scope_is_authorized = self.as_ref().rust().active_hint_scope.as_ref().map_or(
            requested_scope_lease_id.is_none(),
            |scope| {
                requested_scope_lease_id.as_deref() == Some(scope.lease_id.as_str())
                    && scope.owner.target.tab == target.tab
                    && scope.owner.target.generation == target.generation
                    && scope.owner.target.document == target.document
                    && scope.owner.configuration_revision == configuration_revision
                    && family == "all"
            },
        );
        if !scope_is_authorized {
            self.set_status_text(QString::from(
                "Scoped Hint collection rejected: scope lease is missing or stale",
            ));
            return QString::from(r#"{"error":"stale-scope"}"#);
        }
        let mut candidates = match parse_hint_payload(&candidates_json.to_string()) {
            Ok(candidates) => candidates,
            Err(error) => {
                self.set_status_text(QString::from(format!("Hints rejected: {error}")));
                return QString::from(serde_json::json!({"error": error}).to_string());
            }
        };
        if let Err(error) = validate_hint_candidates_for_family(&family, &candidates) {
            self.set_status_text(QString::from(format!("Hints rejected: {error}")));
            return QString::from(
                serde_json::json!({"error":"invalid-facet-candidates"}).to_string(),
            );
        }
        if !self.as_ref().rust().hint_proxy_targets_enabled
            && candidates.iter().any(|candidate| {
                matches!(
                    candidate.kind,
                    ferric_browser_core::HintKind::EditorFocus
                        | ferric_browser_core::HintKind::LabelControl
                )
            })
        {
            self.set_status_text(QString::from(
                "Hints rejected: validated proxy targets are disabled",
            ));
            return QString::from(r#"{"error":"proxy-targets-disabled"}"#);
        }
        let normally_usable_control_ids = candidates
            .iter()
            .filter(|candidate| candidate.kind != ferric_browser_core::HintKind::LabelControl)
            .map(|candidate| candidate.element_id)
            .collect::<std::collections::BTreeSet<_>>();
        candidates.retain(|candidate| {
            candidate.kind != ferric_browser_core::HintKind::LabelControl
                || !candidate
                    .related_element_id
                    .is_some_and(|control_id| normally_usable_control_ids.contains(&control_id))
        });
        candidates.retain(|candidate| {
            !self
                .as_ref()
                .rust()
                .hint_consumed
                .contains(&(candidate.frame_path.clone(), candidate.element_id))
        });
        let config =
            serde_json::from_str::<super::Config>(&self.as_ref().rust().config_json.to_string())
                .unwrap_or_default();
        let alphabet = config.hints.chars.clone();
        let minimum_width = usize::from(config.hints.min_chars);
        let mut ids = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut().get_mut().hint_ids.clone()
        };
        let binding = self.as_ref();
        let previous = binding
            .rust()
            .hint_session
            .as_ref()
            .filter(|session| {
                session.target.tab == target.tab && session.target.document == target.document
            })
            .map_or(&[][..], |session| session.hints.as_slice());
        let labeled = match if previous.is_empty() {
            assign_labels_with_options(candidates, &alphabet, minimum_width)
        } else {
            refresh_labels(candidates, previous, &alphabet, minimum_width)
        } {
            Ok(labeled) => labeled,
            Err(error) => {
                self.set_status_text(QString::from(format!("Hints rejected: {error}")));
                return QString::from(serde_json::json!({"error": error.to_string()}).to_string());
            }
        };
        let hint_first = self.as_ref().rust().hint_first;
        let hint_index = self.as_ref().rust().hint_index;
        if hint_first && hint_index > labeled.len() {
            self.set_status_text(QString::from(format!(
                "Hint index {hint_index} is unavailable; only {} candidate(s)",
                labeled.len()
            )));
            return QString::from(
                serde_json::json!({"error":"index-unavailable", "available":labeled.len()})
                    .to_string(),
            );
        }
        let auto_follow = if self.as_ref().rust().hint_pointer_mode {
            CoreHintAutoFollow::Never
        } else {
            match config.hints.auto_follow {
                ferric_browser_config::HintAutoFollow::Always => CoreHintAutoFollow::Always,
                ferric_browser_config::HintAutoFollow::UniqueMatch => {
                    CoreHintAutoFollow::UniqueMatch
                }
                ferric_browser_config::HintAutoFollow::FullMatch => CoreHintAutoFollow::FullMatch,
                ferric_browser_config::HintAutoFollow::Never => CoreHintAutoFollow::Never,
            }
        };
        let interaction = HintInteraction::new(labeled.clone(), alphabet, auto_follow);
        let first_label = if hint_first {
            labeled.get(hint_index - 1).map(|hint| hint.label.clone())
        } else {
            interaction.initial_activation().map(ToOwned::to_owned)
        };
        let discovery_provenance = snapshot
            .get("policies")
            .and_then(|policies| policies.get(family.as_str()))
            .and_then(|entry| entry.get("provenance"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect();
        let session = HintSession::new_with_selector_policy(
            &mut ids,
            HintTarget {
                tab: target.tab,
                generation: target.generation,
                document: target.document,
            },
            labeled.clone(),
            configuration_revision,
            selector_policy_revision,
            discovery_provenance,
        );
        let pointer_action = if self.as_ref().rust().hint_pointer_mode {
            let Some(source) = self.as_ref().rust().hint_pointer_source else {
                self.set_status_text(QString::from("Pointer Hint has no local authority"));
                return QString::from(r#"{"error":"local-authority-required"}"#);
            };
            let Some(window) = self.as_ref().rust().window else {
                self.set_status_text(QString::from("Pointer Hint has no owning window"));
                return QString::from(r#"{"error":"window-unavailable"}"#);
            };
            let owner = LocalInteractionOwner {
                window,
                session: session.id,
                target: session.target,
                configuration_revision: session.configuration_revision,
            };
            let permit = {
                let mut rust = self.as_mut().rust_mut();
                rust.as_mut()
                    .get_mut()
                    .hint_local_interaction_authority
                    .mint(
                        source,
                        owner,
                        LocalInteractionOperation::HintPointerDispatch,
                    )
            };
            let Some(permit) = permit else {
                self.set_status_text(QString::from("Pointer Hint authority was rejected"));
                return QString::from(r#"{"error":"local-authority-rejected"}"#);
            };
            Some(HintPointerAction::new(owner, source, permit))
        } else {
            None
        };
        let result = serde_json::json!({
            "session_id": session.id.to_string(),
            "hints": hint_json(&labeled),
            "state": hint_interaction_json(&interaction),
            "first_label": first_label,
        });
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if !this.hint_pointer_mode {
                this.hint_pointer_source = None;
            }
            this.hint_pointer_action = pointer_action;
            this.hint_pointer_pending = false;
            this.hint_pointer_state = QString::from(if this.hint_pointer_mode {
                "selecting"
            } else {
                "terminal"
            });
            this.hint_ids = ids;
            this.hint_session = Some(session);
            this.hint_interaction = Some(interaction);
            this.hint_values = QString::from(result["hints"].to_string());
            this.pending_hint_palette_permit = None;
            this.hint_local_interaction_authority
                .clear_session_requests();
        }
        self.as_mut().set_hint_visible(!hint_first);
        self.set_status_text(QString::from(format!(
            "{} hint(s) · type a label or click",
            labeled.len()
        )));
        QString::from(result.to_string())
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
                && receipt.get("lease_id").and_then(Value::as_str) == Some(lease_id.as_str())
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
        let current_owner_matches = local_hint_owner(self.as_ref().rust()) == Some(binding.owner);
        let current_scope_matches = self
            .as_ref()
            .rust()
            .active_hint_scope
            .as_ref()
            .map(|scope| scope.lease_id.as_str())
            == parent_scope_id;
        let selector_revision_matches = {
            let browser_binding = self.as_ref();
            let rust = browser_binding.rust();
            super::browser_ui_navigation::effective_hint_selector_snapshot(
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
            }) == Some(binding.selector_policy_revision)
        };
        let source_session_matches =
            self.as_ref()
                .rust()
                .hint_session
                .as_ref()
                .is_some_and(|session| {
                    session.id == binding.owner.session
                        && session.target == binding.owner.target
                        && session.configuration_revision == binding.owner.configuration_revision
                        && session.selector_policy_revision == binding.selector_policy_revision
                });
        if !receipt_fields_match
            || !current_owner_matches
            || !current_scope_matches
            || !selector_revision_matches
            || !source_session_matches
            || self.as_ref().rust().hint_family.to_string() != "regions"
            || self.as_ref().rust().core_mode != Mode::Hint
            || lease_id != binding.lease_id
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

    pub(super) fn update_hint_interaction(
        mut self: Pin<&mut Self>,
        action: &QString,
        text: &QString,
    ) -> QString {
        let action = action.to_string();
        let text = text.to_string();
        let input = match action.as_str() {
            "character" => text.chars().next().map(HintInteractionInput::Character),
            "text-mode" => Some(HintInteractionInput::EnterTextMode),
            "backspace" => Some(HintInteractionInput::Backspace),
            "clear" => Some(HintInteractionInput::Clear),
            "next" => Some(HintInteractionInput::Next),
            "previous" => Some(HintInteractionInput::Previous),
            "activate" => Some(HintInteractionInput::Activate),
            "rotate" => Some(HintInteractionInput::RotateCollision),
            "select" => Some(HintInteractionInput::SelectLabel(text.clone())),
            "cancel" => Some(HintInteractionInput::Cancel),
            _ => None,
        };
        let Some(input) = input else {
            return QString::from(r#"{"error":"invalid-interaction"}"#);
        };
        let (outcome, state) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(interaction) = this.hint_interaction.as_mut() else {
                return QString::from(r#"{"error":"no-session"}"#);
            };
            let outcome = interaction.handle(input);
            (outcome, hint_interaction_json(interaction))
        };
        let update_status = matches!(
            &outcome,
            HintInteractionOutcome::Updated | HintInteractionOutcome::RotateCollision
        );
        let mut response = serde_json::json!({"state": state});
        match outcome {
            HintInteractionOutcome::Updated => {
                response["outcome"] = Value::String("updated".into());
            }
            HintInteractionOutcome::Activate(label) => {
                response["outcome"] = Value::String("activate".into());
                response["label"] = Value::String(label);
            }
            HintInteractionOutcome::InvalidCharacter(character) => {
                response["outcome"] = Value::String("invalid".into());
                response["character"] = Value::String(character.to_string());
                self.as_mut()
                    .set_status_text(QString::from("Key is not in the hint alphabet"));
            }
            HintInteractionOutcome::NoMatches => {
                response["outcome"] = Value::String("no-matches".into());
                self.as_mut()
                    .set_status_text(QString::from("No hints match that input"));
            }
            HintInteractionOutcome::RotateCollision => {
                response["outcome"] = Value::String("rotate".into());
            }
            HintInteractionOutcome::Cancel => {
                response["outcome"] = Value::String("cancel".into());
            }
        }
        if update_status {
            let state = &response["state"];
            let typed = if state.get("mode").and_then(Value::as_str) == Some("text") {
                format!(
                    "/{}",
                    state
                        .get("query")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                )
            } else {
                state
                    .get("prefix")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            };
            let remaining = state.get("remaining").and_then(Value::as_u64).unwrap_or(0);
            let total = state.get("total").and_then(Value::as_u64).unwrap_or(0);
            let target = self.as_ref().rust().hint_rapid_target.clone();
            self.as_mut().set_status_text(QString::from(format!(
                "{target} · {typed} · {remaining}/{total}"
            )));
        }
        QString::from(response.to_string())
    }

    pub(super) fn hint_selector_policy(self: Pin<&mut Self>, family: &QString) -> QString {
        let result = {
            let binding = self.as_ref();
            let rust = binding.rust();
            effective_hint_selector_snapshot(
                &rust.config_json.to_string(),
                &rust.current_url.to_string(),
                rust.hint_configuration_revision,
                &family.to_string(),
            )
        };
        match result {
            Ok(snapshot) => QString::from(snapshot.to_string()),
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Hint selectors unavailable: {error}"
                )));
                QString::default()
            }
        }
    }

    pub(super) fn confirm_rapid_hint_tabs(mut self: Pin<&mut Self>) -> bool {
        let binding = self.as_ref();
        let rust = binding.rust();
        if rust.core_mode != Mode::Hint
            || !rust.hint_rapid
            || rust.hint_rapid_target != "tab-bg"
            || rust.hint_rapid_tabs_created < 20
        {
            self.set_status_text(QString::from("No rapid hint tab confirmation is pending"));
            return false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .hint_rapid_tabs_created = 0;
        self.set_status_text(QString::from(
            "Rapid hint tab limit confirmed for another 20 tabs",
        ));
        true
    }

    pub(super) fn cancel_hints(mut self: Pin<&mut Self>) {
        self.as_mut().clear_hint_session_state();
        if self.as_ref().rust().core_mode == Mode::Hint {
            let window = self.as_ref().rust().window;
            if let Some(window) = window {
                let _ = self.as_mut().reduce_event(Event::PopMode { window });
            }
            self.as_mut().set_core_mode(Mode::Normal);
            self.as_mut().set_mode(QString::from("normal"));
        }
        self.set_status_text(QString::from("Hints cancelled"));
    }
}

fn hint_interaction_json(interaction: &HintInteraction) -> Value {
    let snapshot = interaction.snapshot();
    serde_json::json!({
        "mode": match snapshot.mode {
            ferric_browser_core::HintInputMode::Label => "label",
            ferric_browser_core::HintInputMode::Text => "text",
        },
        "prefix": snapshot.prefix,
        "query": snapshot.query,
        "matching_labels": snapshot.matching_labels,
        "active_label": snapshot.active_label,
        "remaining": snapshot.remaining,
        "total": snapshot.total,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ferric_browser_config::{HintSelectorMode, HintSelectorPolicy, SiteRule};

    use super::*;

    #[test]
    fn selector_snapshot_binds_configuration_family_and_site_policy() {
        let config = Config::default();
        let config_json = serde_json::to_string(&config).expect("serialize default config");
        let all =
            effective_hint_selector_snapshot(&config_json, "https://example.test/page", 7, "all")
                .expect("default selector snapshot");
        let repeated =
            effective_hint_selector_snapshot(&config_json, "https://example.test/page", 7, "all")
                .expect("repeat selector snapshot");
        let buttons = effective_hint_selector_snapshot(
            &config_json,
            "https://example.test/page",
            7,
            "buttons",
        )
        .expect("button selector snapshot");
        let next_configuration =
            effective_hint_selector_snapshot(&config_json, "https://example.test/page", 8, "all")
                .expect("new config revision snapshot");
        assert_eq!(all["selector_revision"], repeated["selector_revision"]);
        assert_ne!(all["selector_revision"], buttons["selector_revision"]);
        assert_ne!(
            all["selector_revision"],
            next_configuration["selector_revision"]
        );

        let mut site_config = config;
        site_config.site_rules.push(SiteRule {
            id: "custom-buttons".into(),
            pattern: "https://example.test/*".into(),
            priority: 1,
            set: BTreeMap::from([(
                "hints.selectors.buttons".into(),
                toml::Value::try_from(HintSelectorPolicy {
                    mode: HintSelectorMode::Replace,
                    selectors: vec![".custom-button".into()],
                })
                .expect("serialize site policy"),
            )]),
        });
        let site_json = serde_json::to_string(&site_config).expect("serialize site config");
        let scoped =
            effective_hint_selector_snapshot(&site_json, "https://example.test/page", 7, "buttons")
                .expect("site selector snapshot");
        assert_ne!(buttons["selector_revision"], scoped["selector_revision"]);
        assert_eq!(
            scoped["policies"]["buttons"]["policy"]["selectors"][0],
            ".custom-button"
        );
        assert_eq!(
            scoped["policies"]["buttons"]["provenance"][0],
            "site-rule:custom-buttons"
        );
    }
}
