use super::{
    CxxQtType, Effect, EngineEffect, Event, JourneyEdgeKind, Path, Pin, QString, StorageRequest,
    Target, Uuid, ValidatedUrl, Value, atomic_write_private, canonical_engine_url, current_target,
    journey_transition_after_load, qobject, safe_ipc_url, sanitize_untrusted_title,
};

impl qobject::BrowserUi {
    pub(super) fn popup_allowed(
        mut self: Pin<&mut Self>,
        requested_url: &QString,
        user_initiated: bool,
    ) -> bool {
        let index = self.as_ref().rust().active_tab_index;
        self.as_mut()
            .popup_allowed_for(index, requested_url, user_initiated)
    }

    pub(super) fn popup_allowed_for(
        mut self: Pin<&mut Self>,
        index: i32,
        requested_url: &QString,
        user_initiated: bool,
    ) -> bool {
        let Ok(url) = ValidatedUrl::parse(requested_url.to_string()) else {
            self.set_status_text(QString::from("Popup rejected: invalid URL"));
            return false;
        };
        let target = self.as_ref().get_ref().target_for_index(index);
        let Some(target) = target else {
            self.set_status_text(QString::from("Popup rejected: stale opener"));
            return false;
        };
        let parent = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(|state| state.journey().current_node(target.tab));
        let requested_url = url.as_str().to_owned();
        let effects = self.as_mut().reduce_event(Event::OpenPopup {
            opener: target,
            url,
            user_gesture: user_initiated,
        });
        let accepted = effects.as_ref().is_ok_and(|effects| {
            effects
                .iter()
                .any(|effect| matches!(effect, Effect::Engine(EngineEffect::Navigate { .. })))
        });
        if accepted {
            let effects = effects.expect("accepted popup has effects");
            self.as_mut()
                .mark_journey_transition(&effects, JourneyEdgeKind::Popup, "popup");
            if let Some(parent) = parent {
                self.as_mut().mark_journey_parent(&effects, parent);
            }
            self.as_mut().set_pending_engine_action(&effects);
            let known = {
                let rust = self.as_ref().get_ref().rust();
                rust.tab_ids
                    .iter()
                    .chain(rust.popup_tab_ids.iter())
                    .copied()
                    .collect::<Vec<_>>()
            };
            let new_popup = {
                let rust = self.as_ref().get_ref().rust();
                rust.state.as_ref().and_then(|state| {
                    state
                        .windows()
                        .values()
                        .flat_map(|window| window.tabs.iter().copied())
                        .find(|tab| !known.contains(tab))
                })
            };
            if let Some(tab) = new_popup {
                if let Some(popup_target) = self
                    .as_ref()
                    .rust()
                    .state
                    .as_ref()
                    .and_then(|state| state.capture_target(tab))
                {
                    let token = Uuid::new_v4().to_string();
                    let mut rust = self.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    this.popup_journey_targets
                        .push((token, popup_target, requested_url, false));
                    if this.popup_journey_targets.len() > 256 {
                        this.popup_journey_targets.remove(0);
                    }
                }
                let mut rust = self.as_mut().rust_mut();
                rust.as_mut().get_mut().popup_tab_ids.push(tab);
            }
        }
        self.set_status_text(QString::from(if accepted {
            "Popup accepted"
        } else {
            "Popup blocked"
        }));
        accepted
    }

    pub(super) fn popup_target_for_token(self: Pin<&mut Self>, token: &str) -> Option<Target> {
        self.as_ref()
            .rust()
            .popup_journey_targets
            .iter()
            .find(|(candidate, _, _, _)| candidate == token)
            .map(|(_, target, _, _)| *target)
    }

    pub(super) fn update_popup_target(mut self: Pin<&mut Self>, token: &str, target: Target) {
        let mut rust = self.as_mut().rust_mut();
        if let Some((_, pending, _, _)) = rust
            .as_mut()
            .get_mut()
            .popup_journey_targets
            .iter_mut()
            .find(|(candidate, _, _, _)| candidate == token)
        {
            *pending = target;
        }
    }

    pub(super) fn remove_popup_journey_token(mut self: Pin<&mut Self>, token: &str) {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .popup_journey_targets
            .retain(|(candidate, _, _, _)| candidate != token);
    }

    pub(super) fn take_popup_journey_token(
        mut self: Pin<&mut Self>,
        requested_url: &QString,
    ) -> QString {
        let Ok(requested_url) = ValidatedUrl::parse(requested_url.to_string()) else {
            return QString::default();
        };
        let requested_url = requested_url.as_str();
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let Some((token, _, _, claimed)) = this
            .popup_journey_targets
            .iter_mut()
            .rev()
            .find(|(_, _, candidate_url, claimed)| !*claimed && candidate_url == requested_url)
        else {
            return QString::default();
        };
        *claimed = true;
        QString::from(token.clone())
    }

    pub(super) fn popup_navigation_started(
        mut self: Pin<&mut Self>,
        token: &QString,
        url: QString,
    ) {
        let Ok(url) = canonical_engine_url(url) else {
            return;
        };
        let Some(target) = self.as_mut().popup_target_for_token(&token.to_string()) else {
            return;
        };
        self.as_mut().note_navigation_started(target, &url);
    }

    pub(super) fn popup_navigation_url_changed(
        mut self: Pin<&mut Self>,
        token: &QString,
        url: QString,
    ) {
        let Ok(url) = canonical_engine_url(url) else {
            return;
        };
        let Ok(parsed) = ValidatedUrl::parse(url.clone()) else {
            return;
        };
        let token = token.to_string();
        let Some(target) = self.as_mut().popup_target_for_token(&token) else {
            return;
        };
        let record_history = self
            .as_ref()
            .should_record_same_document_visit(target, &url);
        self.as_mut().note_navigation_url_change(target, &url);
        let changed = self
            .as_mut()
            .reduce_event(Event::SameDocumentNavigation {
                target,
                url: parsed,
            })
            .is_ok();
        if changed && record_history {
            self.as_mut().record_same_document_visit(target.tab, &url);
        }
    }

    pub(super) fn popup_navigation_committed(
        mut self: Pin<&mut Self>,
        token: &QString,
        url: QString,
        title: QString,
    ) {
        let Ok(canonical_url) = canonical_engine_url(url) else {
            return;
        };
        let Ok(parsed) = ValidatedUrl::parse(canonical_url.clone()) else {
            return;
        };
        let title = sanitize_untrusted_title(&title.to_string());
        let token = token.to_string();
        let Some(target) = self.as_mut().popup_target_for_token(&token) else {
            return;
        };
        let was_traversal = self.as_mut().take_journey_traversal(target);
        let journey_parent = self.as_mut().take_journey_parent(target);
        let redirect_observed = self.as_mut().take_journey_redirect(target);
        let journey_transition = journey_transition_after_load(
            self.as_mut().take_journey_transition(target),
            was_traversal,
            redirect_observed,
        );
        let commit_event = journey_transition.map_or_else(
            || Event::CommitNavigation {
                target,
                url: parsed.clone(),
                title: title.clone(),
            },
            |(transition, source)| Event::CommitNavigationWithTransition {
                target,
                url: parsed.clone(),
                title: title.clone(),
                transition,
                source: Some(source),
                parent: journey_parent,
            },
        );
        if self.as_mut().reduce_event(commit_event).is_ok() {
            self.as_mut().record_committed_visit(
                target.tab,
                &canonical_url,
                &title,
                was_traversal,
                journey_parent,
            );
            if let Some(next_target) =
                current_target(self.as_ref().rust().state.as_ref(), Some(target.tab))
            {
                self.as_mut().update_popup_target(&token, next_target);
            }
        }
    }

    pub(super) fn popup_navigation_completed(mut self: Pin<&mut Self>, token: &QString) {
        let Some(target) = self.as_mut().popup_target_for_token(&token.to_string()) else {
            return;
        };
        let _ = self
            .as_mut()
            .reduce_event(Event::CompleteNavigation { target });
    }

    pub(super) fn popup_navigation_failed(mut self: Pin<&mut Self>, token: &QString) {
        let token = token.to_string();
        let Some(target) = self.as_mut().popup_target_for_token(&token) else {
            return;
        };
        let _ = self.as_mut().take_journey_redirect(target);
        let _ = self.as_mut().take_journey_transition(target);
        let _ = self.as_mut().take_journey_parent(target);
        let _ = self.as_mut().reduce_event(Event::FailNavigation { target });
    }

    pub(super) fn release_popup_journey_token(mut self: Pin<&mut Self>, token: &QString) {
        self.as_mut().remove_popup_journey_token(&token.to_string());
    }

    pub(super) fn close_popup_tab(mut self: Pin<&mut Self>, token: &QString) -> bool {
        let token = token.to_string();
        if token.is_empty() || token.len() > 128 || token.chars().any(char::is_control) {
            self.set_status_text(QString::from("Popup close token is invalid"));
            return false;
        }
        let Some(target) = self.as_mut().popup_target_for_token(&token) else {
            self.set_status_text(QString::from("Popup close target is already gone"));
            return true;
        };
        let Some(generation) = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(|state| state.tabs().get(&target.tab).map(|tab| tab.generation))
        else {
            self.as_mut().remove_popup_journey_token(&token);
            self.set_status_text(QString::from("Popup close target is already gone"));
            return true;
        };
        if self
            .as_mut()
            .reduce_event(Event::CloseTab { tab: target.tab })
            .is_err()
        {
            self.set_status_text(QString::from("Popup close was rejected"));
            return false;
        }
        if self
            .as_mut()
            .reduce_event(Event::ViewClosed {
                tab: target.tab,
                generation,
            })
            .is_err()
        {
            self.set_status_text(QString::from("Popup close acknowledgement failed"));
            return false;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.popup_tab_ids
                .retain(|candidate| *candidate != target.tab);
        }
        self.as_mut().remove_popup_journey_token(&token);
        self.set_status_text(QString::from("Popup closed"));
        true
    }

    pub(super) fn journey_export_json(self: Pin<&mut Self>) -> Result<String, String> {
        let binding = self.as_ref();
        let rust = binding.rust();
        if rust.profile_persistence.is_durable() {
            return rust
                .journey_export_payload
                .clone()
                .ok_or_else(|| "durable journey export is still loading".to_owned());
        }
        let state = rust
            .state
            .as_ref()
            .ok_or_else(|| "core state unavailable".to_owned())?;
        let nodes = state
            .journey()
            .nodes()
            .take(50_000)
            .map(|node| {
                serde_json::json!({
                    "id": node.id.to_string(),
                    "profile": node.profile.to_string(),
                    "url": safe_ipc_url(&node.url),
                    "title": node.title,
                    "committed_at": node.committed_at,
                    "transition": node.transition.name(),
                    "source": node.source,
                })
            })
            .collect::<Vec<_>>();
        let edges = state
            .journey()
            .edges()
            .iter()
            .take(100_000)
            .map(|edge| {
                serde_json::json!({
                    "source": edge.source.to_string(),
                    "target": edge.target.to_string(),
                    "transition": edge.kind.name(),
                    "created_at": edge.created_at,
                })
            })
            .collect::<Vec<_>>();
        let durability = "memory-only";
        let profile = state
            .profiles()
            .values()
            .next()
            .map(|profile| profile.id.to_string())
            .unwrap_or_default();
        serde_json::to_string(&serde_json::json!({
            "format": "ferric-browser-journey-v1",
            "durability": durability,
            "profile": profile,
            "nodes": nodes,
            "edges": edges,
        }))
        .map_err(|error| error.to_string())
    }

    pub(super) fn journey_export_preview(mut self: Pin<&mut Self>) -> QString {
        let durable_without_payload = {
            let binding = self.as_ref();
            let rust = binding.rust();
            rust.profile_persistence.is_durable() && rust.journey_export_payload.is_none()
        };
        if durable_without_payload {
            let result = self
                .as_mut()
                .submit_storage(StorageRequest::JourneyExport)
                .map_err(|error| error.to_string());
            if let Err(error) = result {
                self.as_mut().set_status_text(QString::from(format!(
                    "Journey export is already loading or unavailable: {error}"
                )));
            } else {
                self.as_mut()
                    .set_status_text(QString::from("Loading journey export preview"));
            }
            return QString::default();
        }
        let payload = match self.as_mut().journey_export_json() {
            Ok(payload) => payload,
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Journey export unavailable: {error}"
                )));
                return QString::default();
            }
        };
        Self::format_journey_export_preview(&payload)
    }

    pub(super) fn format_journey_export_preview(payload: &str) -> QString {
        let Ok(value) = serde_json::from_str::<Value>(payload) else {
            return QString::default();
        };
        let nodes = value
            .get("nodes")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let edges = value
            .get("edges")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let durability = value
            .get("durability")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let warning = if durability == "memory-only" {
            "Warning: this export contains private or ephemeral in-memory journey data and writes it to the selected external file."
        } else {
            "The export contains sanitized journey URLs, titles, typed relationships, and timestamps."
        };
        QString::from(format!(
            "Journey export preview\n{nodes} node(s), {edges} relationship(s)\n{warning}"
        ))
    }

    pub(super) fn export_journey(mut self: Pin<&mut Self>, path: &QString) -> bool {
        let path = path.to_string();
        let path_ref = Path::new(&path);
        if path.is_empty()
            || path.len() > 4096
            || path.chars().any(char::is_control)
            || !path_ref.is_absolute()
        {
            self.set_status_text(QString::from(
                "Journey export requires an absolute local file path",
            ));
            return false;
        }
        if path_ref.exists() {
            self.set_status_text(QString::from(
                "Journey export refuses to overwrite an existing file",
            ));
            return false;
        }
        let payload = match self.as_mut().journey_export_json() {
            Ok(payload) => payload,
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Journey export unavailable: {error}"
                )));
                return false;
            }
        };
        match atomic_write_private(path_ref, payload.as_bytes()) {
            Ok(()) => {
                self.set_status_text(QString::from("Journey export written"));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Journey export failed: {error}")));
                false
            }
        }
    }
}
