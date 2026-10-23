//! Admission, snapshot capture and ticket-correlated completion for session saves.
//! The application storage scheduler owns durable work and serializes requests.

use super::{
    ClosedTabSnapshot, CxxQtType, ExistenceState, FeedbackLane, FeedbackState, PathBuf, Pin,
    QString, RestoreSafety, SessionSaveIntent, SessionSnapshot, SnapshotTabInput,
    SnapshotWindowInput, StorageRequest, StorageTicket, Uuid, configured_undo_limit, elapsed_ms,
    named_session_path, qobject, unix_timestamp,
};

impl qobject::BrowserUi {
    pub(super) fn build_session_snapshot(
        &self,
        name: &str,
        path: PathBuf,
    ) -> Option<(PathBuf, SessionSnapshot)> {
        let rust = self.rust();
        let (Some(state), Some(profile_id)) = (rust.state.as_ref(), rust.profile_id) else {
            return None;
        };
        let window_id = rust.window?;
        let window = state.windows().get(&window_id)?;
        let core_profile_id = window.profile;
        let mut tab_ids = std::collections::HashMap::new();
        for tab_id in &window.tabs {
            if let Some(tab) = state.tabs().get(tab_id)
                && tab.profile == core_profile_id
                && tab.existence == ExistenceState::Live
            {
                tab_ids.insert(*tab_id, Uuid::new_v4());
            }
        }
        let tabs = window
            .tabs
            .iter()
            .filter_map(|tab_id| {
                let durable_id = tab_ids.get(tab_id).copied()?;
                let tab = state.tabs().get(tab_id)?;
                let url = tab.url.clone().unwrap_or_else(|| "about:blank".into());
                Some(SnapshotTabInput {
                    id: durable_id,
                    url,
                    safety: RestoreSafety::SafeGet,
                    private: false,
                    pinned: tab.pinned,
                    muted: tab.muted,
                    zoom: tab.zoom,
                    scroll_position: rust.navigation.scroll_positions.get(tab_id).copied(),
                })
            })
            .collect::<Vec<_>>();
        let selected_tab = window
            .active_tab
            .and_then(|tab_id| tab_ids.get(&tab_id).copied());
        let Ok(mut snapshot) = SessionSnapshot::new(
            rust.session_id,
            name,
            profile_id,
            state.revision(),
            unix_timestamp().to_string(),
            vec![SnapshotWindowInput {
                selected_tab,
                workspace: None,
                tabs,
            }],
        ) else {
            return None;
        };
        snapshot.closed_tabs = rust
            .closed_tabs
            .iter()
            .filter(|closed| {
                !closed.private && super::url_safety::is_safe_tab_descriptor(&closed.url)
            })
            .take(configured_undo_limit(&rust.config))
            .map(|closed| ClosedTabSnapshot {
                id: closed.id,
                url: closed.url.clone(),
                title: closed.title.clone(),
                closed_at: closed.closed_at,
                group_index: closed.group_index,
                pinned: closed.pinned,
                muted: closed.muted,
                zoom_hundredths: closed.zoom_hundredths,
            })
            .collect();
        snapshot
            .recovered_session_ids
            .clone_from(&rust.recovered_session_ids);
        Some((path, snapshot))
    }

    pub(super) fn request_session_snapshot_save(
        mut self: Pin<&mut Self>,
        name: &str,
        path: PathBuf,
        checkpoint: bool,
    ) -> bool {
        // The application storage scheduler already serializes writes. Allow
        // one explicit save behind an automatic checkpoint, while bounding
        // repeated explicit requests and avoiding redundant checkpoints.
        let blocked = {
            let owner = self.as_ref();
            let pending = &owner.rust().library.session_saves;
            (checkpoint && !pending.is_empty())
                || (!checkpoint
                    && pending
                        .values()
                        .any(|intent| matches!(intent, SessionSaveIntent::Named)))
        };
        if blocked {
            return false;
        }
        let Some((path, snapshot)) = self.as_ref().get_ref().build_session_snapshot(name, path)
        else {
            if !checkpoint {
                self.as_mut().publish_management_feedback(
                    FeedbackLane::SessionSave,
                    FeedbackState::Failed,
                    "The session couldn't be saved. Your open tabs are kept.",
                    "The window's session state is unavailable.",
                );
            }
            return false;
        };
        let intent = if checkpoint {
            SessionSaveIntent::Checkpoint {
                generation: self.as_ref().rust().checkpoint.generation,
            }
        } else {
            SessionSaveIntent::Named
        };
        let result = self
            .as_mut()
            .submit_storage_with_ticket(StorageRequest::SessionSave { path, snapshot });
        match result {
            Ok(ticket) => {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.library.session_saves.insert(ticket, intent);
                true
            }
            Err(error) => {
                if !checkpoint {
                    self.as_mut().publish_management_feedback(
                        FeedbackLane::SessionSave,
                        FeedbackState::Failed,
                        "The session couldn't be saved. Your open tabs are kept.",
                        &error.to_string(),
                    );
                }
                false
            }
        }
    }

    pub(super) fn named_session_path(&self, name: &str) -> Option<PathBuf> {
        let rust = self.rust();
        named_session_path(rust.session_state_root.as_ref()?, rust.profile_id?, name).ok()
    }

    pub(super) fn save_named_session(mut self: Pin<&mut Self>, name: &QString) -> bool {
        if self.as_ref().active_profile_is_transient() {
            self.set_status_text(QString::from(
                "Ephemeral and private profiles cannot save durable sessions",
            ));
            return false;
        }
        let Some(path) = self
            .as_ref()
            .get_ref()
            .named_session_path(&name.to_string())
        else {
            self.set_status_text(QString::from("Invalid session name or profile"));
            return false;
        };
        let saved = self
            .as_mut()
            .request_session_snapshot_save(&name.to_string(), path, false);
        if saved {
            self.as_mut().publish_management_feedback(
                FeedbackLane::SessionSave,
                FeedbackState::Pending,
                "Saving session…",
                "",
            );
        }
        let pending_named = self
            .as_ref()
            .rust()
            .library
            .session_saves
            .values()
            .any(|intent| matches!(intent, SessionSaveIntent::Named));
        self.set_status_text(QString::from(if saved {
            "Saving session…"
        } else if pending_named {
            "A session save is already in progress"
        } else {
            "The session couldn't be saved. Your open tabs are kept."
        }));
        saved
    }

    pub(super) fn complete_session_save(
        mut self: Pin<&mut Self>,
        ticket: StorageTicket,
        result: Result<(), String>,
    ) -> bool {
        let intent = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.library.session_saves.remove(&ticket)
        };
        let Some(intent) = intent else { return false };
        let checkpoint_save = matches!(intent, SessionSaveIntent::Checkpoint { .. });
        match result {
            Ok(()) => {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                if let SessionSaveIntent::Checkpoint { generation } = intent {
                    this.checkpoint
                        .saved(generation, elapsed_ms(this.binding_clock));
                }
                drop(rust);
                self.as_mut().invalidate_session_catalog();
                if !checkpoint_save {
                    self.as_mut().request_session_names();
                    self.as_mut().publish_management_feedback(
                        FeedbackLane::SessionSave,
                        FeedbackState::Succeeded,
                        "Session saved",
                        "",
                    );
                    self.as_mut()
                        .set_status_text(QString::from("Session saved"));
                }
            }
            Err(error) => {
                if !checkpoint_save {
                    self.as_mut().publish_management_feedback(
                        FeedbackLane::SessionSave,
                        FeedbackState::Failed,
                        "The session couldn't be saved. Your open tabs are kept.",
                        &error,
                    );
                }
                self.as_mut().set_status_text(QString::from(format!(
                    "Session snapshot failed; changes retained: {error}"
                )));
            }
        }
        true
    }
}
