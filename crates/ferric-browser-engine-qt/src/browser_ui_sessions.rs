use super::{
    CleanLinkResult, ClosedTabDescriptor, ClosedTabSnapshot, CxxQtType, Event, ExistenceState,
    JourneyEdgeKind, PathBuf, PendingExternalNavigation, Pin, PrivacyKind, ProfileId,
    ProfilePersistence, QString, QStringList, RestoreSafety, SessionSnapshot, SnapshotTabInput,
    SnapshotWindowInput, StorageRequest, TabId, Uuid, ValidatedUrl, configured_undo_limit,
    current_target, elapsed_ms, is_bounded_untrusted_text, is_safe_history_url,
    link_preview_presentation, named_session_path, qobject, restore_entry_line, unix_timestamp,
};

use std::{collections::HashSet, path::Path};

pub(super) const SESSION_ACCESS_REVOKED_PREVIEW: &str =
    "ERROR\tSession result is unavailable for this profile";

/// INVARIANT: session snapshots are only for a durable normal-profile owner
/// whose core profile is bound to the active session root and has no unresolved
/// child branches. Private/ephemeral owners and profiles without durable
/// storage fail closed even if a caller supplies a session path.
fn session_snapshot_policy_allows(
    persistence: ProfilePersistence,
    privacy: PrivacyKind,
    profile_matches_session_root: bool,
    unresolved_child_branch: bool,
) -> bool {
    persistence.is_durable()
        && privacy == PrivacyKind::Normal
        && profile_matches_session_root
        && !unresolved_child_branch
}

fn session_profile_binding_matches(
    session_core_profile_id: Option<ProfileId>,
    window_profile_id: ProfileId,
) -> bool {
    session_core_profile_id == Some(window_profile_id)
}

fn session_restore_operation_pending(
    preview_pending: bool,
    recovery_pending: bool,
    load_append: Option<bool>,
) -> bool {
    preview_pending || recovery_pending || load_append.is_some()
}

pub(super) fn session_ticket_value_matches(
    expected_ticket: Option<u64>,
    completion_ticket: u64,
) -> bool {
    expected_ticket == Some(completion_ticket)
}

/// INVARIANT: Session-list results are ticket-correlated, and only one catalog
/// request may be active for the current UI owner at a time.
pub(super) fn session_list_request_should_start(dirty: bool, pending: bool) -> bool {
    dirty && !pending
}

fn session_path_is_profile_scoped(root: &Path, profile_id: Uuid, path: &Path) -> bool {
    let expected_parent = root.join("sessions").join(profile_id.to_string());
    path.parent() == Some(expected_parent.as_path())
}

/// Keep session projection aligned with the core's authoritative visible-view
/// membership. Retained/transferring pages may remain live reducer records but
/// must not be recreated from an automatic normal-profile checkpoint.
fn visible_session_tabs(window_tabs: &[TabId], visible_tabs: &[TabId]) -> HashSet<TabId> {
    let window_tabs = window_tabs.iter().copied().collect::<HashSet<_>>();
    visible_tabs
        .iter()
        .copied()
        .filter(|tab| window_tabs.contains(tab))
        .collect()
}

impl qobject::BrowserUi {
    fn session_restore_request_pending(&self) -> bool {
        let rust = self.rust();
        session_restore_operation_pending(
            rust.session_restore_preview_pending,
            rust.session_restore_recovery_pending,
            rust.session_restore_load_append,
        )
    }

    pub(super) fn session_profile_access_allowed(&self) -> bool {
        let rust = self.rust();
        let (Some(state), Some(window_id)) = (rust.state.as_ref(), rust.window) else {
            return false;
        };
        if rust.profile_id.is_none() || rust.session_state_root.is_none() {
            return false;
        }
        let Some(window) = state.windows().get(&window_id) else {
            return false;
        };
        let Some(profile) = state.profiles().get(&window.profile) else {
            return false;
        };
        session_snapshot_policy_allows(
            rust.profile_persistence,
            profile.privacy,
            session_profile_binding_matches(rust.session_core_profile_id, window.profile),
            state.has_unresolved_child_branch_for_profile(window.profile),
        )
    }

    pub(super) fn recover_session(mut self: Pin<&mut Self>) -> QString {
        let (root, profile_id) = {
            let pinned = self.as_ref();
            let rust = pinned.rust();
            match (rust.session_state_root.as_ref(), rust.profile_id) {
                (Some(root), Some(profile_id)) => (root.clone(), profile_id),
                _ => {
                    self.set_status_text(QString::from("No durable recovery profile"));
                    return QString::default();
                }
            }
        };
        if !self.as_ref().session_profile_access_allowed() {
            self.set_status_text(QString::from(
                "Session recovery is unavailable for this profile",
            ));
            return QString::default();
        }
        if self.as_ref().session_restore_request_pending() {
            self.set_status_text(QString::from(
                "A session restore is already pending for this profile",
            ));
            return QString::default();
        }
        let result = Some(
            self.as_mut()
                .submit_storage_with_ticket(StorageRequest::SessionRecovery { root, profile_id }),
        );
        match result {
            Some(Ok(ticket)) => {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.session_restore_recovery_pending = true;
                this.session_restore_ticket = Some(ticket);
                this.session_restore_values = QString::default();
                drop(rust);
                self.set_status_text(QString::from("Loading recovery session"));
                QString::from("QUEUED")
            }
            Some(Err(_)) => {
                self.set_status_text(QString::from("Recovery unavailable; details suppressed"));
                QString::default()
            }
            None => {
                self.set_status_text(QString::from("Recovery worker is unavailable"));
                QString::default()
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn restore_closed_tabs_from_snapshot(
        mut self: Pin<&mut Self>,
        closed_tabs: Vec<ClosedTabSnapshot>,
    ) {
        let restored = closed_tabs
            .into_iter()
            .filter(|closed| is_safe_history_url(&closed.url))
            .map(|closed| ClosedTabDescriptor {
                id: closed.id,
                url: closed.url,
                title: closed.title,
                profile: self.as_ref().rust().profile_name.clone(),
                private: false,
                closed_at: closed.closed_at,
            })
            .take(configured_undo_limit(&self.as_ref().rust().config))
            .collect::<Vec<_>>();
        let mut rust = self.as_mut().rust_mut();
        rust.as_mut().get_mut().closed_tabs = restored;
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn restore_session_from_entries(
        mut self: Pin<&mut Self>,
        entries: Vec<ferric_browser_storage::RestorePlanEntry>,
        append: bool,
    ) -> QString {
        if entries.is_empty() {
            self.set_status_text(QString::from("Saved session is empty"));
            return QString::default();
        }
        let Some(window) = self.as_ref().rust().window else {
            self.set_status_text(QString::from("No recovery window"));
            return QString::default();
        };
        let existing_tabs = self.as_ref().rust().tab_ids.clone();
        let first_restored_index = if append { existing_tabs.len() } else { 0 };
        let selected_tab = if append {
            if self
                .as_mut()
                .reduce_event(Event::OpenTab { window })
                .is_err()
            {
                self.set_status_text(QString::from("Recovery tab creation failed"));
                return QString::default();
            }
            let Some(tab) = self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .and_then(|state| state.windows().get(&window))
                .and_then(|window| window.active_tab)
            else {
                self.set_status_text(QString::from("Recovery tab state failed"));
                return QString::default();
            };
            tab
        } else {
            let Some(tab) = self.as_ref().rust().tab else {
                self.set_status_text(QString::from("No recovery tab"));
                return QString::default();
            };
            tab
        };
        let mut restored_tabs = if append { existing_tabs } else { Vec::new() };
        restored_tabs.push(selected_tab);
        for _ in entries.iter().skip(1) {
            if self
                .as_mut()
                .reduce_event(Event::OpenTab { window })
                .is_err()
            {
                self.set_status_text(QString::from("Recovery tab creation failed"));
                return QString::default();
            }
            let Some(tab) = self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .and_then(|state| state.windows().get(&window))
                .and_then(|window| window.active_tab)
            else {
                self.set_status_text(QString::from("Recovery tab state failed"));
                return QString::default();
            };
            restored_tabs.push(tab);
        }
        if self
            .as_mut()
            .reduce_event(Event::ActivateTab {
                window,
                tab: selected_tab,
            })
            .is_err()
        {
            self.set_status_text(QString::from("Recovery selection failed"));
            return QString::default();
        }
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.tab_ids = restored_tabs.clone();
            this.popup_tab_ids.clear();
            this.popup_journey_targets.clear();
            this.tab = Some(selected_tab);
            if !append {
                this.scroll_positions.clear();
            }
            for (tab, entry) in this.tab_ids[first_restored_index..]
                .iter()
                .zip(entries.iter())
            {
                if let Some(position) = entry.scroll_position {
                    this.scroll_positions.insert(*tab, position);
                } else {
                    this.scroll_positions.remove(tab);
                }
            }
            this.checkpoint.dirty = false;
            this.checkpoint.dirty_since_ms = None;
            this.checkpoint.restore_pending = None;
        }
        let restored_metadata = restored_tabs[first_restored_index..]
            .iter()
            .zip(entries.iter())
            .map(|(tab, entry)| (*tab, entry.pinned, entry.muted, entry.zoom))
            .collect::<Vec<_>>();
        for (tab, pinned, muted, zoom) in restored_metadata {
            if self
                .as_mut()
                .reduce_event(Event::SetTabPinned { tab, pinned })
                .and_then(|_| {
                    self.as_mut()
                        .reduce_event(Event::SetTabMuted { tab, muted })
                })
                .and_then(|_| {
                    self.as_mut().reduce_event(Event::SetTabZoom {
                        tab,
                        zoom_hundredths: (zoom * 100.0).round() as u32,
                    })
                })
                .is_err()
            {
                self.set_status_text(QString::from("Recovery tab metadata failed"));
                return QString::default();
            }
        }
        self.as_mut().sync_tab_order_from_core();
        let urls = entries.iter().map(restore_entry_line).collect::<Vec<_>>();
        let selected_url = &urls[0];
        let inactive_restore_tabs = restored_tabs
            .iter()
            .skip(first_restored_index.saturating_add(1))
            .copied()
            .collect::<Vec<_>>();
        for tab in inactive_restore_tabs {
            if let Some(target) = current_target(self.as_ref().rust().state.as_ref(), Some(tab)) {
                self.as_mut().mark_journey_transition_for_target(
                    target,
                    JourneyEdgeKind::SessionRestore,
                    "session-restore",
                );
            }
        }
        let selected_index = self.as_ref().rust().active_tab_index;
        self.as_mut()
            .set_active_tab_properties(selected_index, selected_tab);
        if selected_url != "about:blank"
            && let Ok(url) = ValidatedUrl::parse(selected_url)
            && let Some(target) = current_target(
                self.as_ref().rust().state.as_ref(),
                self.as_ref().rust().tab,
            )
            && let Ok(effects) = self.as_mut().reduce_event(Event::StartNavigation {
                target,
                url: url.clone(),
            })
        {
            self.as_mut().set_pending_engine_action(&effects);
            self.as_mut().mark_journey_transition(
                &effects,
                JourneyEdgeKind::SessionRestore,
                "session-restore",
            );
            self.as_mut().set_initial_url(QString::from(selected_url));
            self.as_mut()
                .update_current_url(QString::from(selected_url));
            self.as_mut().set_load_state(QString::from("provisional"));
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut().get_mut().checkpoint.restore_pending = Some(selected_tab);
        } else {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.checkpoint.restore_pending = None;
            this.checkpoint.dirty = true;
            this.checkpoint.dirty_since_ms = Some(elapsed_ms(this.binding_clock));
        }
        self.set_status_text(QString::from(format!("Recovered {} tab(s)", urls.len())));
        QString::from(urls.join("\n"))
    }

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
        if !self.session_profile_access_allowed() {
            return None;
        }
        let session_state_root = rust.session_state_root.as_deref()?;
        if !session_path_is_profile_scoped(session_state_root, profile_id, &path) {
            return None;
        }
        let visible_tabs = visible_session_tabs(
            &window.tabs,
            &state.visible_versioning_tabs_for_window(window_id)?,
        );
        let mut tab_ids = std::collections::HashMap::new();
        for tab_id in &window.tabs {
            if visible_tabs.contains(tab_id)
                && let Some(tab) = state.tabs().get(tab_id)
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
                if !visible_tabs.contains(tab_id) {
                    return None;
                }
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
                    scroll_position: rust.scroll_positions.get(tab_id).copied(),
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
            .filter(|closed| !closed.private && is_safe_history_url(&closed.url))
            .take(configured_undo_limit(&rust.config))
            .map(|closed| ClosedTabSnapshot {
                id: closed.id,
                url: closed.url.clone(),
                title: closed.title.clone(),
                closed_at: closed.closed_at,
            })
            .collect();
        Some((path, snapshot))
    }

    pub(super) fn request_session_snapshot_save(
        mut self: Pin<&mut Self>,
        name: &str,
        path: PathBuf,
        checkpoint: bool,
    ) -> bool {
        if self.as_ref().rust().session_save_pending {
            return false;
        }
        let Some((path, snapshot)) = self.as_ref().get_ref().build_session_snapshot(name, path)
        else {
            return false;
        };
        let result = Some(
            self.as_mut()
                .submit_storage_with_ticket(StorageRequest::SessionSave { path, snapshot }),
        );
        match result {
            Some(Ok(ticket)) => {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.session_save_pending = true;
                this.session_save_ticket = Some(ticket);
                this.session_save_checkpoint = checkpoint;
                true
            }
            _ => false,
        }
    }

    pub(super) fn named_session_path(&self, name: &str) -> Option<PathBuf> {
        let rust = self.rust();
        named_session_path(rust.session_state_root.as_ref()?, rust.profile_id?, name).ok()
    }

    pub(super) fn save_named_session(mut self: Pin<&mut Self>, name: &QString) -> bool {
        if !self.as_ref().session_profile_access_allowed() {
            self.set_status_text(QString::from(
                "Session saving is unavailable for this profile",
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
        self.set_status_text(QString::from(if saved {
            "Session save queued"
        } else {
            "Session save is busy or unavailable"
        }));
        saved
    }

    pub(super) fn request_named_session_load(
        mut self: Pin<&mut Self>,
        name: &QString,
        append: bool,
    ) -> bool {
        if self.as_ref().session_restore_request_pending() {
            self.set_status_text(QString::from(
                "A session restore is already pending for this profile",
            ));
            return false;
        }
        if !self.as_ref().session_profile_access_allowed() {
            self.set_status_text(QString::from(
                "Session loading is unavailable for this profile",
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
        let result = Some(
            self.as_mut()
                .submit_storage_with_ticket(StorageRequest::SessionRestore { paths: vec![path] }),
        );
        match result {
            Some(Ok(ticket)) => {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.session_restore_load_append = Some(append);
                this.session_restore_ticket = Some(ticket);
                this.session_restore_values = QString::default();
                this.session_preview = QString::default();
                drop(rust);
                self.set_status_text(QString::from("Loading validated session"));
                true
            }
            Some(Err(
                ferric_browser_storage::StorageWorkerError::Busy
                | ferric_browser_storage::StorageWorkerError::QueueFull,
            )) => {
                self.set_status_text(QString::from(
                    "Session restore is busy; retry after the current operation",
                ));
                false
            }
            Some(Err(_)) => {
                self.set_status_text(QString::from(
                    "Session restore unavailable; details suppressed",
                ));
                false
            }
            None => {
                self.set_status_text(QString::from("Session restore worker is unavailable"));
                false
            }
        }
    }

    pub(super) fn request_session_preview(mut self: Pin<&mut Self>, name: &QString) -> bool {
        if self.as_ref().session_restore_request_pending() {
            self.set_status_text(QString::from(
                "A session restore is already pending for this profile",
            ));
            return false;
        }
        if !self.as_ref().session_profile_access_allowed() {
            self.set_status_text(QString::from(
                "Session previews are unavailable for this profile",
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
        let result = Some(
            self.as_mut()
                .submit_storage_with_ticket(StorageRequest::SessionRestore { paths: vec![path] }),
        );
        match result {
            Some(Ok(ticket)) => {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.session_restore_preview_pending = true;
                this.session_restore_ticket = Some(ticket);
                this.session_preview = QString::default();
                drop(rust);
                self.set_status_text(QString::from("Loading session preview"));
                true
            }
            Some(Err(
                ferric_browser_storage::StorageWorkerError::Busy
                | ferric_browser_storage::StorageWorkerError::QueueFull,
            )) => {
                self.set_status_text(QString::from(
                    "Session preview is busy; retry after the current operation",
                ));
                false
            }
            Some(Err(_)) => {
                self.set_status_text(QString::from(
                    "Session preview unavailable; details suppressed",
                ));
                false
            }
            None => {
                self.set_status_text(QString::from("Session preview worker is unavailable"));
                false
            }
        }
    }

    pub(super) fn list_named_sessions(mut self: Pin<&mut Self>) -> QString {
        self.as_mut().request_session_names();
        QString::from(self.as_ref().rust().session_names.join("\n"))
    }

    pub(super) fn delete_named_session(
        mut self: Pin<&mut Self>,
        name: &QString,
        confirmed: bool,
    ) -> bool {
        if self.as_ref().rust().session_delete_ticket.is_some() {
            self.set_status_text(QString::from("Session deletion is already pending"));
            return false;
        }
        if !self.as_ref().session_profile_access_allowed() {
            self.set_status_text(QString::from(
                "Session deletion is unavailable for this profile",
            ));
            return false;
        }
        if !confirmed {
            self.set_status_text(QString::from("Session deletion requires confirmation"));
            return false;
        }
        let Some(path_root) = self.as_ref().rust().session_state_root.clone() else {
            self.set_status_text(QString::from("No durable session profile"));
            return false;
        };
        let Some(profile_id) = self.as_ref().rust().profile_id else {
            self.set_status_text(QString::from("No durable session profile"));
            return false;
        };
        let result = Some(self.as_mut().submit_storage_with_ticket(
            StorageRequest::SessionDelete {
                root: path_root,
                profile_id,
                name: name.to_string(),
            },
        ));
        match result {
            Some(Ok(ticket)) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .session_delete_ticket = Some(ticket);
                self.set_status_text(QString::from("Session deletion queued"));
                true
            }
            Some(Err(_)) => {
                self.set_status_text(QString::from(
                    "Session deletion unavailable; details suppressed",
                ));
                false
            }
            None => {
                self.set_status_text(QString::from("Session deletion worker is unavailable"));
                false
            }
        }
    }

    pub(super) fn take_session_restore_values(mut self: Pin<&mut Self>) -> QString {
        let access_allowed = self.as_ref().session_profile_access_allowed();
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if !access_allowed {
            let pending = this.session_restore_preview_pending
                || this.session_restore_recovery_pending
                || this.session_restore_load_append.is_some()
                || !this.session_restore_values.to_string().is_empty();
            this.session_restore_values = QString::default();
            this.session_restore_preview_pending = false;
            this.session_restore_recovery_pending = false;
            this.session_restore_load_append = None;
            this.session_preview = if pending {
                QString::from(SESSION_ACCESS_REVOKED_PREVIEW)
            } else {
                QString::default()
            };
            return QString::default();
        }
        std::mem::take(&mut this.session_restore_values)
    }

    pub(super) fn take_session_preview(mut self: Pin<&mut Self>) -> QString {
        let access_allowed = self.as_ref().session_profile_access_allowed();
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if !access_allowed {
            let report_revocation = this.session_restore_preview_pending
                || this.session_restore_recovery_pending
                || this.session_restore_load_append.is_some()
                || this.session_preview.to_string() == SESSION_ACCESS_REVOKED_PREVIEW;
            this.session_restore_preview_pending = false;
            this.session_restore_recovery_pending = false;
            this.session_restore_load_append = None;
            this.session_preview = QString::default();
            return if report_revocation {
                QString::from(SESSION_ACCESS_REVOKED_PREVIEW)
            } else {
                QString::default()
            };
        }
        std::mem::take(&mut this.session_preview)
    }

    pub(super) fn clear_link_preview(mut self: Pin<&mut Self>) {
        self.as_mut().clear_link_preview_presentation();
        self.as_mut().set_link_preview_visible(false);
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_link_navigation = None;
    }

    pub(super) fn clear_link_preview_presentation(mut self: Pin<&mut Self>) {
        self.as_mut().set_link_preview_command(QString::default());
        self.as_mut().set_link_preview_original(QString::default());
        self.as_mut().set_link_preview_cleaned(QString::default());
        self.as_mut()
            .set_link_preview_applied_rules(QStringList::default());
        self.as_mut()
            .set_link_preview_removed_parameters(QStringList::default());
        self.as_mut()
            .set_link_preview_retained_parameters(QStringList::default());
        self.as_mut()
            .set_link_preview_explanation(QString::default());
        self.as_mut().set_link_preview_requires_confirmation(false);
    }

    pub(super) fn publish_link_preview(
        mut self: Pin<&mut Self>,
        command: &str,
        result: &CleanLinkResult,
        requires_confirmation: bool,
    ) {
        let presentation = link_preview_presentation(command, result, requires_confirmation);
        self.as_mut()
            .set_link_preview_command(QString::from(presentation.command));
        self.as_mut()
            .set_link_preview_original(QString::from(presentation.original));
        self.as_mut()
            .set_link_preview_cleaned(QString::from(presentation.cleaned));
        self.as_mut().set_link_preview_applied_rules(
            presentation
                .applied_rules
                .into_iter()
                .map(QString::from)
                .collect(),
        );
        self.as_mut().set_link_preview_removed_parameters(
            presentation
                .removed_parameters
                .into_iter()
                .map(QString::from)
                .collect(),
        );
        self.as_mut().set_link_preview_retained_parameters(
            presentation
                .retained_parameters
                .into_iter()
                .map(QString::from)
                .collect(),
        );
        self.as_mut()
            .set_link_preview_explanation(QString::from(presentation.explanation));
        self.as_mut()
            .set_link_preview_requires_confirmation(presentation.requires_confirmation);
    }

    pub(super) fn request_external_navigation(
        mut self: Pin<&mut Self>,
        scheme: String,
        url: String,
    ) -> bool {
        if !matches!(scheme.as_str(), "mailto" | "tel" | "sms" | "geo")
            || !is_bounded_untrusted_text(&url)
        {
            self.set_status_text(QString::from("External URI rejected by scheme policy"));
            return false;
        }
        {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut().get_mut().pending_external_navigation =
                Some(PendingExternalNavigation { url: url.clone() });
        }
        self.as_mut()
            .set_external_navigation_uri(QString::from(url));
        self.as_mut()
            .set_external_navigation_scheme(QString::from(scheme));
        self.as_mut().set_external_navigation_visible(true);
        self.set_status_text(QString::from(
            "External URI requires confirmation before opening",
        ));
        true
    }

    pub(super) fn confirm_external_navigation(mut self: Pin<&mut Self>) -> bool {
        let Some(pending) = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_external_navigation
            .take()
        else {
            self.set_status_text(QString::from("No pending external URI"));
            return false;
        };
        if self.as_ref().rust().pending_engine_action.is_some() {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_external_navigation = Some(pending);
            self.set_status_text(QString::from("Another browser action is pending"));
            return false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("external-open\t{}", pending.url));
        self.as_mut()
            .set_external_navigation_uri(QString::default());
        self.as_mut()
            .set_external_navigation_scheme(QString::default());
        self.as_mut().set_external_navigation_visible(false);
        self.set_status_text(QString::from(
            "External URI confirmed; opening with the system handler",
        ));
        true
    }

    pub(super) fn cancel_external_navigation(mut self: Pin<&mut Self>) {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_external_navigation = None;
        self.as_mut()
            .set_external_navigation_uri(QString::default());
        self.as_mut()
            .set_external_navigation_scheme(QString::default());
        self.as_mut().set_external_navigation_visible(false);
        self.set_status_text(QString::from("External URI opening cancelled"));
    }

    pub(super) fn confirm_link_navigation(mut self: Pin<&mut Self>) -> bool {
        let Some(pending) = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_link_navigation
            .take()
        else {
            self.set_status_text(QString::from("No pending clean-link navigation"));
            return false;
        };
        let journey_parent = pending.journey_parent;
        if let Some(action) = pending.new_window_action {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some(action);
        } else if let Some(target) = pending.target {
            match self.as_mut().reduce_event(Event::StartNavigation {
                target,
                url: pending.url,
            }) {
                Ok(effects) => {
                    if let Some(parent) = journey_parent {
                        self.as_mut().mark_journey_parent(&effects, parent);
                    }
                    self.as_mut().set_pending_engine_action(&effects)
                }
                Err(_) => {
                    self.as_mut().set_status_text(QString::from(
                        "Clean-link navigation rejected; details suppressed",
                    ));
                    return false;
                }
            }
        } else {
            self.set_status_text(QString::from("Clean-link target is no longer available"));
            return false;
        }
        self.as_mut().set_link_preview_visible(false);
        self.as_mut().clear_link_preview_presentation();
        self.as_mut().sync_core_tabs();
        self.set_status_text(QString::from("Clean-link navigation confirmed"));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_session_checkpoint_requires_normal_durable_profile() {
        assert!(session_snapshot_policy_allows(
            ProfilePersistence::Durable,
            PrivacyKind::Normal,
            true,
            false
        ));
        assert!(!session_snapshot_policy_allows(
            ProfilePersistence::Durable,
            PrivacyKind::Normal,
            true,
            true
        ));
        assert!(!session_snapshot_policy_allows(
            ProfilePersistence::Durable,
            PrivacyKind::Normal,
            false,
            false
        ));
        for privacy in [PrivacyKind::Private, PrivacyKind::Ephemeral] {
            assert!(!session_snapshot_policy_allows(
                ProfilePersistence::Durable,
                privacy,
                true,
                false
            ));
        }
        assert!(!session_snapshot_policy_allows(
            ProfilePersistence::Transient,
            PrivacyKind::Normal,
            true,
            false
        ));
        assert!(!session_snapshot_policy_allows(
            ProfilePersistence::Unavailable,
            PrivacyKind::Normal,
            true,
            false
        ));
    }

    #[test]
    fn session_checkpoint_root_is_bound_to_its_core_profile() {
        let configured = ProfileId::from_display("profileid-41").expect("configured profile");
        let other = ProfileId::from_display("profileid-42").expect("other profile");

        assert!(session_profile_binding_matches(
            Some(configured),
            configured
        ));
        assert!(!session_profile_binding_matches(Some(configured), other));
        assert!(!session_profile_binding_matches(None, configured));
    }

    #[test]
    fn only_one_session_restore_operation_can_be_pending() {
        assert!(!session_restore_operation_pending(false, false, None));
        assert!(session_restore_operation_pending(true, false, None));
        assert!(session_restore_operation_pending(false, true, None));
        assert!(session_restore_operation_pending(false, false, Some(false)));
        assert!(session_restore_operation_pending(false, false, Some(true)));
    }

    #[test]
    fn session_catalog_requests_are_single_flight() {
        assert!(session_list_request_should_start(true, false));
        assert!(!session_list_request_should_start(false, false));
        assert!(!session_list_request_should_start(true, true));
        assert!(!session_list_request_should_start(false, true));
    }

    #[test]
    fn session_completions_require_the_active_request_ticket() {
        assert!(session_ticket_value_matches(Some(41), 41));
        assert!(!session_ticket_value_matches(Some(41), 42));
        assert!(!session_ticket_value_matches(None, 41));
    }

    #[test]
    fn automatic_session_checkpoint_path_is_bound_to_its_profile() {
        let root = Path::new("/tmp/ferric-session-policy");
        let owner = Uuid::new_v4();
        let other = Uuid::new_v4();
        let owner_path = root
            .join("sessions")
            .join(owner.to_string())
            .join("current.json");
        let other_path = root
            .join("sessions")
            .join(other.to_string())
            .join("current.json");

        assert!(session_path_is_profile_scoped(root, owner, &owner_path));
        assert!(!session_path_is_profile_scoped(root, owner, &other_path));
        assert!(!session_path_is_profile_scoped(
            root,
            owner,
            &root
                .join("sessions")
                .join("..")
                .join(owner.to_string())
                .join("current.json")
        ));
    }

    #[test]
    fn automatic_session_checkpoint_excludes_retained_branch_views() {
        let visible = TabId::from_display("tabid-1").expect("visible tab id");
        let retained_branch_view = TabId::from_display("tabid-2").expect("retained tab id");
        let transferring = TabId::from_display("tabid-3").expect("transferring tab id");
        let outside_window = TabId::from_display("tabid-4").expect("outside tab id");

        let eligible = visible_session_tabs(
            &[visible, retained_branch_view, transferring],
            &[visible, outside_window],
        );

        assert_eq!(eligible, HashSet::from([visible]));
        assert!(!eligible.contains(&retained_branch_view));
        assert!(!eligible.contains(&transferring));
        assert!(!eligible.contains(&outside_window));
    }
}
