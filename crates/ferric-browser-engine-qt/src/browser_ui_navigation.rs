use super::browser_ui_hint_pointer::HintPointerAction;
use super::hint_payload::validate_hint_candidates_for_family;
use super::hint_policy::{
    HintOptions, LocalInteractionOperation, LocalInteractionOwner, parse_hint_options,
};
use super::{
    CommandRegistry, CoreHintAutoFollow, CxxQtType, Event, HintInteraction, HintInteractionInput,
    HintInteractionOutcome, HintSession, HintTarget, Mode, ParsedCommand, Pin, QString, Value,
    assign_labels_with_options, current_target, hint_json, parse_hint_payload, qobject,
};

use ferric_browser_config::{Config, HintSelectorFamily, resolve_hint_selector_policies};

pub(super) fn effective_hint_selector_snapshot(
    config_json: &str,
    url: &str,
    configuration_revision: u64,
    family: &str,
) -> Result<Value, String> {
    let selector_family = HintSelectorFamily::parse(family);
    if selector_family.is_none() && !matches!(family, "regions" | "surfaces" | "actions" | "focus")
    {
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
            this.interaction.hint_configuration_revision = this
                .interaction
                .hint_configuration_revision
                .saturating_add(1);
            this.interaction.hint_session = None;
            this.interaction.hint_interaction = None;
            this.interaction.pending_hint_scope_handoff = None;
            this.interaction.active_hint_scope = None;
            this.interaction.hint_consumed.clear();
            this.pending_hint_action = None;
            if let Some(action) = this.interaction.hint_pointer_action.as_mut() {
                action.terminate();
            }
            this.interaction.hint_pointer_action = None;
            this.interaction.hint_pointer_source = None;
            this.interaction.pending_hint_palette_permit = None;
            this.interaction.pending_grid_scope_permit = None;
            this.interaction
                .hint_local_interaction_authority
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

    pub(super) fn set_hint_options(
        mut self: Pin<&mut Self>,
        family: &str,
        rapid: bool,
        target: &str,
        script: Option<&str>,
        first: bool,
        index: usize,
    ) {
        self.as_mut().set_hint_target(QString::from(target));
        self.as_mut().set_target_search_active(false);
        self.as_mut().set_target_search_scope(QString::from("all"));
        self.as_mut().set_target_search_query(QString::default());
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
            this.interaction.hint_pointer_source = None;
            this.interaction.hint_pointer_action = None;
            this.hint_script = script.map(ToOwned::to_owned);
            this.hint_first = first;
            this.hint_index = index;
            this.hint_rapid_tabs_created = 0;
            this.interaction.hint_consumed.clear();
            this.interaction.hint_interaction = None;
            this.interaction.pending_hint_palette_permit = None;
            this.interaction.pending_grid_scope_permit = None;
            this.interaction
                .hint_local_interaction_authority
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
            .interaction
            .hint_pointer_source = Some(source);
        self.as_mut()
            .set_hint_pointer_state(QString::from("selecting"));
        true
    }

    pub(super) fn clear_hint_session_state(mut self: Pin<&mut Self>) {
        self.as_mut().set_target_search_active(false);
        self.as_mut().set_target_search_scope(QString::from("all"));
        self.as_mut().set_target_search_query(QString::default());
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.interaction.hint_session = None;
            this.interaction.hint_interaction = None;
            this.interaction.pending_hint_scope_handoff = None;
            this.interaction.active_hint_scope = None;
            this.interaction.hint_consumed.clear();
            this.hint_values = QString::default();
            this.pending_hint_action = None;
            if let Some(action) = this.interaction.hint_pointer_action.as_mut() {
                action.terminate();
            }
            this.interaction.hint_pointer_action = None;
            this.interaction.hint_pointer_source = None;
            this.interaction.pending_hint_palette_permit = None;
            this.interaction.pending_grid_scope_permit = None;
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
    }

    pub(super) fn begin_hint_session(
        mut self: Pin<&mut Self>,
        candidates_json: &QString,
    ) -> QString {
        if self.as_ref().rust().core_mode != Mode::Hint {
            self.as_mut().set_hint_mode();
        }
        let family = self.as_ref().rust().hint_family.to_string();
        let target_search_scope = self.as_ref().rust().target_search_scope.to_string();
        let target_search_active = self.as_ref().rust().target_search_active;
        let preserved_query = if target_search_active {
            self.as_ref().rust().target_search_query.to_string()
        } else {
            String::new()
        };
        let configuration_revision = self.as_ref().rust().interaction.hint_configuration_revision;
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
        let scope_is_authorized = self
            .as_ref()
            .rust()
            .interaction
            .active_hint_scope
            .as_ref()
            .map_or(requested_scope_lease_id.is_none(), |scope| {
                requested_scope_lease_id.as_deref() == Some(scope.lease_id.as_str())
                    && scope.owner.target.tab == target.tab
                    && scope.owner.target.generation == target.generation
                    && scope.owner.target.document == target.document
                    && scope.owner.configuration_revision == configuration_revision
                    && matches!(family.as_str(), "all" | "actions")
            });
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
        let candidate_validation = if target_search_active && family == "all" {
            let ordinary = candidates
                .iter()
                .filter(|candidate| {
                    !ferric_browser_core::hint_kind_is_region_or_surface(candidate.kind)
                })
                .cloned()
                .collect::<Vec<_>>();
            let regions = candidates
                .iter()
                .filter(|candidate| {
                    matches!(
                        candidate.kind,
                        ferric_browser_core::HintKind::Region
                            | ferric_browser_core::HintKind::ScrollableRegion
                    )
                })
                .cloned()
                .collect::<Vec<_>>();
            let surfaces = candidates
                .iter()
                .filter(|candidate| {
                    matches!(
                        candidate.kind,
                        ferric_browser_core::HintKind::Surface
                            | ferric_browser_core::HintKind::FocusableSurface
                            | ferric_browser_core::HintKind::Frame
                    )
                })
                .cloned()
                .collect::<Vec<_>>();
            validate_hint_candidates_for_family("all", &ordinary)
                .and_then(|()| validate_hint_candidates_for_family("regions", &regions))
                .and_then(|()| validate_hint_candidates_for_family("surfaces", &surfaces))
        } else {
            validate_hint_candidates_for_family(&family, &candidates)
        };
        if let Err(error) = candidate_validation {
            self.set_status_text(QString::from(format!("Hints rejected: {error}")));
            return QString::from(
                serde_json::json!({"error":"invalid-facet-candidates"}).to_string(),
            );
        }
        if target_search_active {
            if super::browser_ui_target_search::target_search_collection_family(
                &target_search_scope,
            ) != Some(family.as_str())
            {
                self.set_status_text(QString::from(
                    "Target Search rejected: collector scope changed",
                ));
                return QString::from(r#"{"error":"stale-search-scope"}"#);
            }
            candidates.retain(|candidate| {
                super::browser_ui_target_search::target_search_candidate_matches(
                    &target_search_scope,
                    candidate.kind,
                )
            });
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
            !self.as_ref().rust().interaction.hint_consumed.contains(&(
                candidate.frame_owner.clone(),
                candidate.frame_path.clone(),
                candidate.element_id,
            ))
        });
        let config =
            serde_json::from_str::<super::Config>(&self.as_ref().rust().config_json.to_string())
                .unwrap_or_default();
        let alphabet = config.hints.chars.clone();
        let minimum_width = usize::from(config.hints.min_chars);
        let mut ids = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut().get_mut().interaction.hint_ids.clone()
        };
        let auto_follow = if self.as_ref().rust().hint_pointer_mode || target_search_active {
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
        let previous = self
            .as_ref()
            .rust()
            .interaction
            .hint_session
            .as_ref()
            .filter(|session| {
                session.target.tab == target.tab
                    && session.target.generation == target.generation
                    && session.target.document == target.document
                    && session.configuration_revision == configuration_revision
                    && session.selector_policy_revision == selector_policy_revision
            })
            .and_then(|_| self.as_ref().rust().interaction.hint_interaction.clone());
        let refreshing = previous.is_some();
        let mut interaction = if let Some(mut previous) = previous {
            if let Err(error) = previous.refresh(candidates) {
                self.set_status_text(QString::from(format!("Hints rejected: {error}")));
                return QString::from(serde_json::json!({"error":error.to_string()}).to_string());
            }
            previous
        } else {
            let labeled = match assign_labels_with_options(candidates, &alphabet, minimum_width) {
                Ok(labeled) => labeled,
                Err(error) => {
                    self.set_status_text(QString::from(format!("Hints rejected: {error}")));
                    return QString::from(
                        serde_json::json!({"error":error.to_string()}).to_string(),
                    );
                }
            };
            HintInteraction::new(labeled, alphabet, auto_follow)
        };
        if target_search_active && !refreshing {
            interaction.handle(HintInteractionInput::SetText(preserved_query));
        }
        let labeled = interaction.hints().to_vec();
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
        let first_label = if target_search_active || refreshing {
            None
        } else if hint_first {
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
            let Some(source) = self.as_ref().rust().interaction.hint_pointer_source else {
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
                    .interaction
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
        let serialized_hints = {
            let binding = self.as_ref();
            let rust = binding.rust();
            hint_json(
                &labeled,
                rust.hint_rapid,
                if rust.hint_pointer_mode {
                    "pointer"
                } else {
                    &rust.hint_rapid_target
                },
            )
        };
        let result = serde_json::json!({
            "session_id": session.id.to_string(),
            "hints": serialized_hints,
            "state": hint_interaction_json(&interaction),
            "first_label": first_label,
        });
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if !this.hint_pointer_mode {
                this.interaction.hint_pointer_source = None;
            }
            this.interaction.hint_pointer_action = pointer_action;
            this.hint_pointer_pending = false;
            this.hint_pointer_state = QString::from(if this.hint_pointer_mode {
                "selecting"
            } else {
                "terminal"
            });
            this.interaction.hint_ids = ids;
            this.interaction.hint_session = Some(session);
            this.interaction.hint_interaction = Some(interaction);
            this.hint_values = QString::from(result["hints"].to_string());
            this.interaction.pending_hint_palette_permit = None;
            this.interaction.pending_grid_scope_permit = None;
            this.interaction
                .hint_local_interaction_authority
                .clear_session_requests();
        }
        self.as_mut().set_hint_visible(!hint_first);
        if target_search_active {
            self.set_status_text(QString::from(format!(
                "{} visible target(s) · Enter confirms",
                labeled.len()
            )));
        } else {
            self.set_status_text(QString::from(format!(
                "{} hint(s) · type a label or click",
                labeled.len()
            )));
        }
        QString::from(result.to_string())
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
            "set-query" if self.as_ref().rust().target_search_active => {
                Some(HintInteractionInput::SetText(text.clone()))
            }
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
            let Some(interaction) = this.interaction.hint_interaction.as_mut() else {
                return QString::from(r#"{"error":"no-session"}"#);
            };
            let outcome = interaction.handle(input);
            (outcome, hint_interaction_json(interaction))
        };
        if self.as_ref().rust().target_search_active {
            let query = state
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default();
            self.as_mut().set_target_search_query(QString::from(query));
        }
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
            if self.as_ref().rust().target_search_active {
                let remaining = state.get("remaining").and_then(Value::as_u64).unwrap_or(0);
                self.as_mut().set_status_text(QString::from(format!(
                    "{remaining} visible target(s) · Enter confirms"
                )));
                return QString::from(response.to_string());
            }
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
                rust.interaction.hint_configuration_revision,
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
        "refresh_required": snapshot.refresh_required,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ferric_browser_config::{HintSelectorMode, HintSelectorPolicy, SiteRule};
    use ferric_browser_core::{DocumentId, HintKind, HintSessionId, IdSource, TabId, WindowId};

    use super::super::browser_ui_scope_handoff::{
        HintScopeHandoffContext, hint_scope_handoff_is_valid,
    };
    use super::super::hint_kind_name;
    use super::super::hint_policy::HintScopePurpose;
    use super::*;

    fn scope_handoff_fixture() -> (
        super::super::hint_policy::HintScopeBinding,
        HintSession,
        Value,
        LocalInteractionOwner,
    ) {
        let mut ids = IdSource::default();
        let target = HintTarget {
            tab: TabId::from_display("tabid-31").expect("tab ID"),
            generation: 12,
            document: DocumentId::from_display("documentid-41").expect("document ID"),
        };
        let configuration_revision = 23;
        let selector_policy_revision = 0x0123_4567_89ab_cdef;
        let session = HintSession::new_with_selector_policy(
            &mut ids,
            target,
            Vec::new(),
            configuration_revision,
            selector_policy_revision,
            Vec::new(),
        );
        let owner = LocalInteractionOwner {
            window: WindowId::from_display("windowid-51").expect("window ID"),
            session: session.id,
            target,
            configuration_revision,
        };
        let binding = super::super::hint_policy::HintScopeBinding {
            purpose: super::super::hint_policy::HintScopePurpose::Hint,
            source_family: "regions".into(),
            lease_id: "hint-scope-current".into(),
            owner,
            element_id: 17,
            frame_path: "0.2.1".into(),
            kind: HintKind::ScrollableRegion,
            selector_policy_revision,
            parent_scope_lease_id: Some("hint-scope-parent".into()),
        };
        let receipt = serde_json::json!({
            "ok": true,
            "lease_id": binding.lease_id,
            "element_id": binding.element_id,
            "frame_path": binding.frame_path,
            "kind": "scrollable-region",
            "selector_revision": format!("{selector_policy_revision:016x}"),
            "configuration_revision": configuration_revision,
            "parent_scope_lease_id": binding.parent_scope_lease_id,
        });
        (binding, session, receipt, owner)
    }

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
        for family in ["actions", "focus"] {
            let filtered = effective_hint_selector_snapshot(
                &config_json,
                "https://example.test/page",
                7,
                family,
            )
            .unwrap();
            assert_eq!(filtered["family"], family);
            assert_eq!(filtered["policies"], all["policies"]);
            assert_ne!(filtered["selector_revision"], all["selector_revision"]);
        }
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

    #[test]
    fn scoped_handoff_requires_exact_receipt_and_current_owner_session() {
        let (binding, session, receipt, owner) = scope_handoff_fixture();
        let selector_revision = binding.selector_policy_revision;
        let parent_scope_id = binding.parent_scope_lease_id.as_deref();
        let context = HintScopeHandoffContext {
            owner: Some(owner),
            scope_lease_id: parent_scope_id,
            selector_policy_revision: Some(selector_revision),
            session: Some(&session),
            family: "regions",
            mode: Mode::Hint,
        };
        assert!(hint_scope_handoff_is_valid(
            &binding,
            &binding.lease_id,
            &receipt,
            context,
        ));

        let invalid_receipt_values = [
            ("ok", serde_json::json!(false)),
            ("lease_id", serde_json::json!("hint-scope-other")),
            ("element_id", serde_json::json!(18)),
            ("frame_path", serde_json::json!("0.2.2")),
            ("kind", serde_json::json!("region")),
            ("selector_revision", serde_json::json!("0000000000000001")),
            ("configuration_revision", serde_json::json!(24)),
            ("parent_scope_lease_id", Value::Null),
        ];
        for (field, value) in invalid_receipt_values {
            let mut invalid_receipt = receipt.clone();
            invalid_receipt[field] = value;
            assert!(
                !hint_scope_handoff_is_valid(
                    &binding,
                    &binding.lease_id,
                    &invalid_receipt,
                    context,
                ),
                "mismatched receipt field {field} must be rejected"
            );
        }
        let mut missing_field = receipt.clone();
        missing_field
            .as_object_mut()
            .expect("receipt object")
            .remove("kind");
        assert!(!hint_scope_handoff_is_valid(
            &binding,
            &binding.lease_id,
            &missing_field,
            context,
        ));
        let mut extra_field = receipt.clone();
        extra_field["unexpected"] = serde_json::json!("field");
        assert!(!hint_scope_handoff_is_valid(
            &binding,
            &binding.lease_id,
            &extra_field,
            context,
        ));
        assert!(!hint_scope_handoff_is_valid(
            &binding,
            "hint-scope-other",
            &receipt,
            context,
        ));

        let mut other_document_owner = owner;
        other_document_owner.target.document =
            DocumentId::from_display("documentid-42").expect("other document ID");
        let mut other_config_owner = owner;
        other_config_owner.configuration_revision += 1;
        let mut other_window_owner = owner;
        other_window_owner.window = WindowId::from_display("windowid-52").expect("other window ID");

        let mut other_session_id = session.clone();
        other_session_id.id =
            HintSessionId::from_display("hintsessionid-53").expect("other session ID");
        let mut other_session_target = session.clone();
        other_session_target.target.document =
            DocumentId::from_display("documentid-43").expect("other session document ID");
        let mut other_session_config = session.clone();
        other_session_config.configuration_revision += 1;
        let stale_contexts = [
            HintScopeHandoffContext {
                owner: Some(other_document_owner),
                ..context
            },
            HintScopeHandoffContext {
                owner: Some(other_config_owner),
                ..context
            },
            HintScopeHandoffContext {
                owner: Some(other_window_owner),
                ..context
            },
            HintScopeHandoffContext {
                scope_lease_id: Some("hint-scope-replaced"),
                ..context
            },
            HintScopeHandoffContext {
                selector_policy_revision: None,
                ..context
            },
            HintScopeHandoffContext {
                selector_policy_revision: Some(selector_revision + 1),
                ..context
            },
            HintScopeHandoffContext {
                session: None,
                ..context
            },
            HintScopeHandoffContext {
                session: Some(&other_session_id),
                ..context
            },
            HintScopeHandoffContext {
                session: Some(&other_session_target),
                ..context
            },
            HintScopeHandoffContext {
                session: Some(&other_session_config),
                ..context
            },
            HintScopeHandoffContext {
                family: "all",
                ..context
            },
            HintScopeHandoffContext {
                mode: Mode::Normal,
                ..context
            },
        ];
        for stale_context in stale_contexts {
            assert!(!hint_scope_handoff_is_valid(
                &binding,
                &binding.lease_id,
                &receipt,
                stale_context,
            ));
        }

        let mut top_level_binding = binding.clone();
        top_level_binding.parent_scope_lease_id = None;
        let mut top_level_receipt = receipt;
        top_level_receipt["parent_scope_lease_id"] = Value::Null;
        assert!(hint_scope_handoff_is_valid(
            &top_level_binding,
            &binding.lease_id,
            &top_level_receipt,
            HintScopeHandoffContext {
                scope_lease_id: None,
                ..context
            },
        ));
    }

    #[test]
    fn grid_scope_handoff_is_limited_to_region_and_surface_families() {
        let (mut binding, session, mut receipt, owner) = scope_handoff_fixture();
        binding.purpose = HintScopePurpose::Grid;
        binding.source_family = "surfaces".into();
        binding.kind = HintKind::Surface;
        binding.parent_scope_lease_id = None;
        receipt["kind"] = Value::String("surface".into());
        receipt["parent_scope_lease_id"] = Value::Null;
        let surface_context = HintScopeHandoffContext {
            owner: Some(owner),
            scope_lease_id: None,
            selector_policy_revision: Some(binding.selector_policy_revision),
            session: Some(&session),
            family: "surfaces",
            mode: Mode::Hint,
        };
        assert!(hint_scope_handoff_is_valid(
            &binding,
            &binding.lease_id,
            &receipt,
            surface_context,
        ));

        binding.source_family = "actions".into();
        let action_context = HintScopeHandoffContext {
            family: "actions",
            ..surface_context
        };
        assert!(!hint_scope_handoff_is_valid(
            &binding,
            &binding.lease_id,
            &receipt,
            action_context
        ));
        binding.kind = HintKind::Frame;
        receipt["kind"] = Value::String("frame".into());
        assert!(hint_scope_handoff_is_valid(
            &binding,
            &binding.lease_id,
            &receipt,
            action_context
        ));
        binding.source_family = "surfaces".into();
        binding.kind = HintKind::Button;
        receipt["kind"] = Value::String(hint_kind_name(binding.kind).into());
        assert!(!hint_scope_handoff_is_valid(
            &binding,
            &binding.lease_id,
            &receipt,
            surface_context,
        ));
    }

    #[test]
    fn search_within_handoff_accepts_target_search_all_and_rejects_family_drift() {
        let (mut binding, session, receipt, owner) = scope_handoff_fixture();
        binding.purpose = HintScopePurpose::Search;
        binding.source_family = "all".into();
        let context = HintScopeHandoffContext {
            owner: Some(owner),
            scope_lease_id: binding.parent_scope_lease_id.as_deref(),
            selector_policy_revision: Some(binding.selector_policy_revision),
            session: Some(&session),
            family: "all",
            mode: Mode::Hint,
        };
        assert!(hint_scope_handoff_is_valid(
            &binding,
            &binding.lease_id,
            &receipt,
            context,
        ));

        let wrong_family = HintScopeHandoffContext {
            family: "regions",
            ..context
        };
        assert!(!hint_scope_handoff_is_valid(
            &binding,
            &binding.lease_id,
            &receipt,
            wrong_family,
        ));
    }
}
