use super::feature_state::SessionRestoreIntent;
use super::{
    CatalogReadState, CleanLinkResult, ClosedTabDescriptor, ClosedTabSnapshot, ContextMember,
    CxxQtType, Event, FeedbackLane, FeedbackState, JourneyEdgeKind, PendingExternalNavigation, Pin,
    QString, QStringList, StorageRequest, ValidatedUrl, configured_undo_limit, current_target,
    elapsed_ms, is_bounded_untrusted_text, link_preview_presentation, qobject, restore_entry_line,
    sanitize_untrusted_title,
};

impl qobject::BrowserUi {
    pub(super) fn request_session_names(mut self: Pin<&mut Self>) {
        if !self.as_ref().rust().library.session_names_dirty.get()
            || self
                .as_ref()
                .rust()
                .session_catalog_read_state
                .is_in_flight()
        {
            return;
        }
        let context = {
            let pinned = self.as_ref();
            let rust = pinned.rust();
            let transient = rust.state.as_ref().and_then(|state| {
                let tab = state.active_tab()?;
                state
                    .profiles()
                    .get(&tab.profile)
                    .map(|profile| profile.privacy.is_transient())
            }) == Some(true);
            if transient {
                None
            } else {
                rust.session_state_root.clone().zip(rust.profile_id)
            }
        };
        let Some((root, profile_id)) = context else {
            let transient = self.as_ref().active_profile_is_transient();
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.library.session_names.clear();
            this.library.session_names_dirty.set(false);
            this.session_catalog_read_state = if transient {
                CatalogReadState::Ready
            } else {
                CatalogReadState::Failed(
                    "The saved-session location is unavailable. Reopen the profile to try again."
                        .into(),
                )
            };
            drop(rust);
            self.as_mut()
                .set_session_names_values(super::QStringList::default());
            self.as_mut().set_session_names_pending(false);
            return;
        };
        match self
            .as_mut()
            .submit_storage(StorageRequest::SessionList { root, profile_id })
        {
            Ok(()) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .library
                    .session_names_refresh_again = false;
                self.as_mut().set_session_names_error(QString::default());
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .session_catalog_read_state = CatalogReadState::Pending;
                self.as_mut().set_session_names_pending(true);
            }
            Err(
                ferric_browser_storage::StorageWorkerError::Busy
                | ferric_browser_storage::StorageWorkerError::QueueFull,
            ) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .session_catalog_read_state = CatalogReadState::Queued;
                self.as_mut().set_session_names_pending(true);
            }
            Err(error) => {
                let detail = error.to_string();
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .session_catalog_read_state = CatalogReadState::Failed(detail.clone());
                self.as_mut().set_session_names_pending(false);
                self.as_mut().set_session_names_error(QString::from(detail));
                self.as_mut().set_status_text(QString::from(
                    "Saved sessions couldn't be loaded. Refresh the session list to try again.",
                ));
            }
        }
    }

    pub(super) fn recover_session(mut self: Pin<&mut Self>) -> QString {
        if self.as_ref().rust().session_recovery_pending {
            return QString::from("QUEUED");
        }
        self.as_mut().set_session_recovery_error(QString::default());
        let paths = {
            let pinned = self.as_ref();
            let rust = pinned.rust();
            match (rust.session_state_root.as_ref(), rust.profile_id) {
                (Some(_), Some(_)) => rust.recovery_paths.clone(),
                _ => {
                    self.as_mut().set_session_recovery_error(QString::from(
                        "Recovery isn't available in this profile. You can start without recovery.",
                    ));
                    self.set_status_text(QString::from("No durable recovery profile"));
                    return QString::default();
                }
            }
        };
        if paths.is_empty() {
            self.as_mut().set_session_recovery_error(QString::from(
                "There are no saved tabs to recover. You can start without recovery.",
            ));
            self.set_status_text(QString::from("Recovery session is empty"));
            return QString::default();
        }
        match self
            .as_mut()
            .submit_storage_with_ticket(StorageRequest::SessionRestore { paths })
        {
            Ok(ticket) => {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.library
                    .session_restores
                    .insert(ticket, SessionRestoreIntent::Recovery);
                this.session_restore_values = QString::default();
                drop(rust);
                self.as_mut().set_session_recovery_pending(true);
                self.set_status_text(QString::from("Loading recovery session"));
                QString::from("QUEUED")
            }
            Err(error) => {
                self.as_mut().set_session_recovery_error(QString::from(
                    "Saved tabs couldn't be read. Try again, or start without recovery.",
                ));
                self.set_status_text(QString::from(format!("Recovery unavailable: {error}")));
                QString::default()
            }
        }
    }

    pub(super) fn remember_recovery_placeholder(mut self: Pin<&mut Self>) {
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        );
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .recovery_placeholder = target;
    }

    pub(super) fn can_reuse_recovery_placeholder(&self) -> bool {
        let rust = self.rust();
        let Some(target) = rust.recovery_placeholder else {
            return false;
        };
        let Some(state) = rust.state.as_ref() else {
            return false;
        };
        let Some(tab) = state.tabs().get(&target.tab) else {
            return false;
        };
        current_target(Some(state), Some(target.tab)) == Some(target)
            && Some(tab.window) == rust.window
            && rust.tab == Some(target.tab)
            && rust.tab_ids == [target.tab]
            && state
                .windows()
                .get(&tab.window)
                .is_some_and(|window| window.tabs == [target.tab])
            && matches!(
                tab.url.as_deref(),
                Some("rb://start" | "rb://blank" | "about:blank")
            )
            && matches!(
                tab.loading,
                super::LoadingState::Idle | super::LoadingState::Complete
            )
            && !tab.pinned
            && !tab.muted
            && (tab.zoom - 1.0).abs() < f64::EPSILON
            && rust.closed_tabs.is_empty()
    }

    pub(super) fn cancel_session_recovery(mut self: Pin<&mut Self>) {
        // The worker may already be reading. Revoke its publication intent so
        // its eventual completion cannot restore tabs or become a preview.
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .library
            .session_restores
            .retain(|_, intent| *intent != SessionRestoreIntent::Recovery);
        self.as_mut().set_session_recovery_pending(false);
        self.as_mut().set_session_recovery_error(QString::default());
        self.set_status_text(QString::from("Started without recovery"));
    }

    /// Materializes the safe portion of a saved context membership into the
    /// current profile window. Runtime tab IDs are deliberately not reused;
    /// only the bounded descriptors are replayed, and inactive tabs remain
    /// unloaded in QML until selected.
    pub(super) fn restore_context_membership(
        mut self: Pin<&mut Self>,
        member: &ContextMember,
    ) -> Result<usize, String> {
        let descriptors = member
            .tab_descriptors
            .iter()
            .filter(|descriptor| {
                descriptor
                    .safe_restore_url
                    .as_deref()
                    .is_none_or(super::url_safety::is_safe_tab_descriptor)
            })
            .cloned()
            .collect::<Vec<_>>();
        if descriptors.is_empty() {
            self.set_session_restore_values(QString::default());
            return Ok(0);
        }
        let Some(window) = self.as_ref().rust().window else {
            return Err("No context restore window".into());
        };
        let (existing_tabs, existing_active) = {
            let binding = self.as_ref();
            let rust = binding.rust();
            let Some(state) = rust.state.as_ref() else {
                return Err("core state unavailable".into());
            };
            let Some(window_state) = state.windows().get(&window) else {
                return Err("context window is unavailable".into());
            };
            (window_state.tabs.clone(), window_state.active_tab)
        };
        let append = existing_tabs.len() > 1;
        let selected_index = member
            .selected_index
            .filter(|index| *index < descriptors.len())
            .unwrap_or(0);
        let mut tab_ids = if append {
            existing_tabs
        } else {
            vec![existing_active.ok_or_else(|| "context window has no active tab".to_owned())?]
        };
        let mut selected_tab = None;
        let mut selected_effects = None;
        let mut restore_lines = Vec::with_capacity(descriptors.len());

        for (index, descriptor) in descriptors.iter().enumerate() {
            let tab = if !append && index == 0 {
                tab_ids[0]
            } else {
                self.as_mut()
                    .reduce_event(Event::OpenTab { window })
                    .map_err(|error| error.to_string())?;
                let tab = self
                    .as_ref()
                    .rust()
                    .state
                    .as_ref()
                    .and_then(|state| state.windows().get(&window))
                    .and_then(|window| window.active_tab)
                    .ok_or_else(|| "context restore tab creation failed".to_owned())?;
                tab_ids.push(tab);
                tab
            };

            self.as_mut()
                .reduce_event(Event::SetTabPinned {
                    tab,
                    pinned: descriptor.pinned,
                })
                .map_err(|error| error.to_string())?;
            self.as_mut()
                .reduce_event(Event::SetTabMuted {
                    tab,
                    muted: descriptor.muted,
                })
                .map_err(|error| error.to_string())?;
            self.as_mut()
                .reduce_event(Event::SetTabZoom {
                    tab,
                    zoom_hundredths: (descriptor.zoom * 100.0).round() as u32,
                })
                .map_err(|error| error.to_string())?;

            let url = descriptor
                .safe_restore_url
                .as_deref()
                .unwrap_or("about:blank");
            if let Ok(url) = ValidatedUrl::parse(url)
                && let Some(target) = current_target(self.as_ref().rust().state.as_ref(), Some(tab))
            {
                let effects = self
                    .as_mut()
                    .reduce_event(Event::StartNavigation { target, url })
                    .map_err(|error| error.to_string())?;
                self.as_mut().mark_journey_transition(
                    &effects,
                    JourneyEdgeKind::SessionRestore,
                    "session-restore",
                );
                if index == selected_index {
                    selected_effects = Some(effects);
                }
            }
            if index == selected_index {
                selected_tab = Some(tab);
            }
            restore_lines.push(format!(
                "{}\t{}\t{}\t{}\t{}",
                url,
                descriptor.pinned,
                descriptor.muted,
                descriptor.zoom,
                sanitize_untrusted_title(&descriptor.title)
            ));
        }

        let selected_tab =
            selected_tab.ok_or_else(|| "context restore selection failed".to_owned())?;
        self.as_mut()
            .reduce_event(Event::ActivateTab {
                window,
                tab: selected_tab,
            })
            .map_err(|error| error.to_string())?;
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.tab_ids = tab_ids;
            this.popup_tab_ids.clear();
            this.navigation.popup_journey_targets.clear();
            this.navigation.page_document_identities.clear();
            this.navigation.pending_same_document_pass_through = None;
            this.tab = Some(selected_tab);
            this.pending_engine_action = None;
            this.pending_engine_actions.clear();
            this.navigation.engine_navigation_kinds.clear();
        }
        self.as_mut()
            .set_session_restore_values(QString::from(format!(
                "{}\n{}",
                if append { "append" } else { "replace" },
                restore_lines.join("\n")
            )));
        self.as_mut().sync_tab_order_from_core();
        if let Some(effects) = selected_effects {
            self.as_mut().mark_journey_transition(
                &effects,
                JourneyEdgeKind::SessionRestore,
                "session-restore",
            );
            self.as_mut().set_pending_engine_action(&effects);
        }
        Ok(descriptors.len())
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn restore_closed_tabs_from_snapshot(
        mut self: Pin<&mut Self>,
        closed_tabs: Vec<ClosedTabSnapshot>,
    ) {
        let restored = closed_tabs
            .into_iter()
            .filter(|closed| super::url_safety::is_safe_tab_descriptor(&closed.url))
            .map(|closed| ClosedTabDescriptor {
                id: closed.id,
                url: closed.url,
                title: closed.title,
                profile: self.as_ref().rust().profile_name.clone(),
                private: false,
                closed_at: closed.closed_at,
                group_index: closed.group_index,
                pinned: closed.pinned,
                muted: closed.muted,
                zoom_hundredths: closed.zoom_hundredths,
            })
            .take(configured_undo_limit(&self.as_ref().rust().config))
            .collect::<Vec<_>>();
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let mut seen = this
            .closed_tabs
            .iter()
            .map(|tab| tab.id)
            .collect::<super::BTreeSet<_>>();
        this.closed_tabs
            .extend(restored.into_iter().filter(|tab| seen.insert(tab.id)));
        this.closed_tabs
            .truncate(configured_undo_limit(&this.config));
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
            if !append {
                this.popup_tab_ids.clear();
                this.navigation.popup_journey_targets.clear();
                this.navigation.page_document_identities.clear();
                this.navigation.pending_same_document_pass_through = None;
            }
            this.tab = Some(selected_tab);
            if !append {
                this.navigation.scroll_positions.clear();
            }
            for (tab, entry) in this.tab_ids[first_restored_index..]
                .iter()
                .zip(entries.iter())
            {
                if let Some(position) = entry.scroll_position {
                    this.navigation.scroll_positions.insert(*tab, position);
                } else {
                    this.navigation.scroll_positions.remove(tab);
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
        for (tab, entry) in restored_tabs[first_restored_index..].iter().zip(&entries) {
            if let Some(target) = current_target(self.as_ref().rust().state.as_ref(), Some(*tab))
                && let Ok(url) = ValidatedUrl::parse(entry.url.as_deref().unwrap_or("about:blank"))
            {
                let _ = self
                    .as_mut()
                    .reduce_event(Event::RestoreTabAddress { target, url });
            }
        }
        self.as_mut().sync_tab_order_from_core();
        // Pinning can reorder the Core tabs independently of the selected-first
        // restore plan. Keep each row attached to its new runtime identity.
        let restore_rows = entries
            .iter()
            .zip(&restored_tabs[first_restored_index..])
            .map(|(entry, tab)| format!("{}\t{tab}", restore_entry_line(entry)))
            .collect::<Vec<_>>();
        let selected_url = entries[0].url.as_deref().unwrap_or("about:blank");
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
            this.checkpoint.mark_dirty(elapsed_ms(this.binding_clock));
        }
        self.set_status_text(QString::from(format!(
            "Recovered {} tab(s)",
            restore_rows.len()
        )));
        QString::from(restore_rows.join("\n"))
    }

    pub(super) fn request_named_session_load(
        mut self: Pin<&mut Self>,
        name: &QString,
        append: bool,
    ) -> bool {
        if self.as_ref().active_profile_is_transient() {
            self.set_status_text(QString::from(
                "Ephemeral and private profiles cannot load durable sessions",
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
                this.library
                    .session_restores
                    .insert(ticket, SessionRestoreIntent::Load { append });
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
            Some(Err(error)) => {
                self.set_status_text(QString::from(format!(
                    "Session restore unavailable: {error}"
                )));
                false
            }
            None => {
                self.set_status_text(QString::from("Session restore is unavailable in this window. Reopen the window and try again."));
                false
            }
        }
    }

    pub(super) fn clear_session_preview_rows(mut self: Pin<&mut Self>) {
        self.as_mut()
            .set_session_preview_labels(QStringList::default());
        self.as_mut()
            .set_session_preview_urls(QStringList::default());
        self.as_mut()
            .set_session_preview_pinned(QStringList::default());
        self.as_mut()
            .set_session_preview_muted(QStringList::default());
    }

    pub(super) fn publish_session_preview_rows(
        mut self: Pin<&mut Self>,
        entries: &[ferric_browser_storage::RestorePlanEntry],
    ) {
        self.as_mut().set_session_preview_labels(
            entries
                .iter()
                .map(|entry| {
                    QString::from(
                        entry
                            .url
                            .as_deref()
                            .and_then(super::safe_site_origin)
                            .and_then(|origin| super::safe_site_host(&origin))
                            .unwrap_or_else(|| match entry.url.as_deref() {
                                Some("rb://start") => "Start page".into(),
                                Some("rb://blank" | "about:blank") => "Blank tab".into(),
                                _ => "Saved tab".into(),
                            }),
                    )
                })
                .collect(),
        );
        self.as_mut().set_session_preview_urls(
            entries
                .iter()
                .map(|entry| QString::from(entry.url.as_deref().unwrap_or_default()))
                .collect(),
        );
        self.as_mut().set_session_preview_pinned(
            entries
                .iter()
                .map(|entry| QString::from(if entry.pinned { "true" } else { "false" }))
                .collect(),
        );
        self.as_mut().set_session_preview_muted(
            entries
                .iter()
                .map(|entry| QString::from(if entry.muted { "true" } else { "false" }))
                .collect(),
        );
    }

    pub(super) fn request_session_preview(mut self: Pin<&mut Self>, name: &QString) -> bool {
        if self.as_ref().active_profile_is_transient() {
            self.set_status_text(QString::from(
                "Ephemeral and private profiles cannot preview durable sessions",
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
                this.library
                    .session_restores
                    .insert(ticket, SessionRestoreIntent::Preview);
                this.session_preview = QString::default();
                drop(rust);
                self.as_mut().clear_session_preview_rows();
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
            Some(Err(error)) => {
                self.set_status_text(QString::from(format!(
                    "Session preview unavailable: {error}"
                )));
                false
            }
            None => {
                self.set_status_text(QString::from("Session preview is unavailable in this window. Reopen the window and try again."));
                false
            }
        }
    }

    pub(super) fn list_named_sessions(mut self: Pin<&mut Self>) -> QString {
        self.as_ref().rust().library.session_names_dirty.set(true);
        self.as_mut().request_session_names();
        QString::from(self.as_ref().rust().library.session_names.join("\n"))
    }

    pub(super) fn delete_named_session(
        mut self: Pin<&mut Self>,
        name: &QString,
        confirmed: bool,
    ) -> bool {
        if self.as_ref().active_profile_is_transient() {
            self.set_status_text(QString::from(
                "Ephemeral and private profiles cannot delete durable sessions",
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
        let result = Some(self.as_mut().submit_storage(StorageRequest::SessionDelete {
            root: path_root,
            profile_id,
            name: name.to_string(),
        }));
        match result {
            Some(Ok(())) => {
                self.as_mut().publish_management_feedback(
                    FeedbackLane::SessionDelete,
                    FeedbackState::Pending,
                    "Deleting session…",
                    "",
                );
                self.set_status_text(QString::from("Session deletion queued"));
                true
            }
            Some(Err(error)) => {
                self.set_status_text(QString::from(format!(
                    "Session deletion unavailable: {error}"
                )));
                false
            }
            None => {
                self.set_status_text(QString::from("Session deletion is unavailable in this window. Reopen the window and try again."));
                false
            }
        }
    }

    pub(super) fn take_session_restore_values(mut self: Pin<&mut Self>) -> QString {
        let mut rust = self.as_mut().rust_mut();
        std::mem::take(&mut rust.as_mut().get_mut().session_restore_values)
    }

    pub(super) fn take_session_preview(mut self: Pin<&mut Self>) -> QString {
        let mut rust = self.as_mut().rust_mut();
        std::mem::take(&mut rust.as_mut().get_mut().session_preview)
    }

    pub(super) fn take_session_preview_request(mut self: Pin<&mut Self>) -> QStringList {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .session_preview_request
            .take()
            .map_or_else(QStringList::default, |(name, append)| {
                [
                    QString::from(name),
                    QString::from(if append { "true" } else { "false" }),
                ]
                .into_iter()
                .collect()
            })
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
            rust.as_mut().get_mut().pending_external_navigation = Some(PendingExternalNavigation {
                scheme: scheme.clone(),
                url: url.clone(),
            });
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
        self.set_status_text(QString::from(format!(
            "{} URI confirmed; opening with the system handler",
            pending.scheme
        )));
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
        if let Some(route) = pending.repeat_route.as_ref()
            && let Err(error) = self.as_ref().validate_ipc_route_public(route)
        {
            self.set_status_text(QString::from(error.to_string()));
            return false;
        }
        let repeated = pending.repeat_route.map(|route| super::RepeatableCommand {
            command: super::ParsedCommand {
                name: "open".into(),
                arguments: vec![pending.url.to_string()],
            },
            count: 1,
            route: Some(route),
        });
        let journey_parent = pending.journey_parent;
        if let Some(action) = pending.new_window_action {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some(action);
        } else if let Some(route) = pending.new_tab_route {
            if let Err(error) =
                self.as_mut()
                    .dispatch_new_tab_open(pending.url.as_str(), &route, None)
            {
                self.set_status_text(QString::from(format!(
                    "Clean-link navigation rejected: {error}"
                )));
                return false;
            }
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
                Err(error) => {
                    self.as_mut().set_status_text(QString::from(format!(
                        "Clean-link navigation rejected: {error}"
                    )));
                    return false;
                }
            }
        } else {
            self.set_status_text(QString::from("Clean-link target is no longer available"));
            return false;
        }
        if let Some(recorded) = repeated {
            self.as_mut().note_repeatable_invocation(
                &recorded.command,
                recorded.route.as_ref(),
                recorded.count,
            );
        }
        self.as_mut().set_link_preview_visible(false);
        self.as_mut().clear_link_preview_presentation();
        self.as_mut().sync_core_tabs();
        self.set_status_text(QString::from("Clean-link navigation confirmed"));
        true
    }
}
