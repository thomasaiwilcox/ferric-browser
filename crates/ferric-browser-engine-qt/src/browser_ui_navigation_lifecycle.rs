use super::{
    ClosedTabDescriptor, ConfigWatch, CxxQtType, Event, Ordering, Pin, ProfilePersistence, QString,
    TabId, Uuid, ValidatedUrl, canonical_engine_url, cleanup_editor_artifact,
    cleanup_print_artifact, cleanup_staged_downloads, configured_undo_limit, current_target,
    discard_config_edit, discard_editor_request, finish_pending_selection_operation, fs,
    journey_transition_after_load, qobject, safe_ipc_url, sanitize_untrusted_title, unix_timestamp,
};

impl qobject::BrowserUi {
    pub(super) fn clear_navigation_failure(mut self: Pin<&mut Self>) {
        self.as_mut()
            .set_navigation_failure_kind(QString::default());
        self.as_mut()
            .set_navigation_failure_requested_url(QString::default());
        self.as_mut().set_navigation_failure_url(QString::default());
        self.as_mut()
            .set_navigation_failure_detail(QString::default());
        self.as_mut().set_navigation_failure_visible(false);
    }

    pub(super) fn navigation_started(mut self: Pin<&mut Self>, url: QString) {
        let url = match canonical_engine_url(url) {
            Ok(url) => QString::from(url),
            Err(error) => {
                self.set_status_text(QString::from(format!("Navigation URL rejected: {error}")));
                return;
            }
        };
        let started_url = url.to_string();
        if let Some(target) = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        ) {
            self.as_mut().note_navigation_started(target, &started_url);
        }
        self.as_mut().clear_navigation_failure();
        self.as_mut()
            .set_navigation_failure_requested_url(QString::from(safe_ipc_url(&url.to_string())));
        let active_index = self.as_ref().rust().active_tab_index;
        self.as_mut().clear_focus_observation_state(active_index);
        self.as_mut().clear_hint_session_state();
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            finish_pending_selection_operation(this, "failed (document target invalidated)");
            this.pending_caret = None;
            this.pending_spawn = None;
            if let Some(pending) = this.pending_action_target.take() {
                this.operation_states.insert(
                    pending.operation_id,
                    "failed (document target invalidated)".into(),
                );
            }
            this.pending_download = None;
            if let Some(pending) = this.pending_userscript.take() {
                this.operation_states.insert(
                    pending.operation_id,
                    "failed (document target invalidated)".into(),
                );
            }
        }
        self.as_mut().update_current_url(url);
        self.as_mut().set_load_state(QString::from("provisional"));
        self.set_status_text(QString::from("Loading"));
    }

    pub(super) fn navigation_started_for(mut self: Pin<&mut Self>, index: i32, url: QString) {
        let url = match canonical_engine_url(url) {
            Ok(url) => QString::from(url),
            Err(error) => {
                if index == self.as_ref().rust().active_tab_index {
                    self.set_status_text(QString::from(format!(
                        "Navigation URL rejected: {error}"
                    )));
                }
                return;
            }
        };
        let started_url = url.to_string();
        if let Some(target) = self.as_ref().get_ref().target_for_index(index) {
            self.as_mut().note_navigation_started(target, &started_url);
        }
        self.as_mut().clear_focus_observation_state(index);
        self.as_mut().clear_hint_session_state();
        if index == self.as_ref().rust().active_tab_index {
            self.as_mut().clear_navigation_failure();
            self.as_mut()
                .set_navigation_failure_requested_url(QString::from(safe_ipc_url(
                    &url.to_string(),
                )));
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            finish_pending_selection_operation(this, "failed (document target invalidated)");
            this.pending_caret = None;
            this.pending_spawn = None;
            if let Some(pending) = this.pending_action_target.take() {
                this.operation_states.insert(
                    pending.operation_id,
                    "failed (document target invalidated)".into(),
                );
            }
            this.pending_download = None;
            if let Some(pending) = this.pending_userscript.take() {
                this.operation_states.insert(
                    pending.operation_id,
                    "failed (document target invalidated)".into(),
                );
            }
        }
        if index == self.as_ref().rust().active_tab_index {
            self.as_mut().update_current_url(url);
            self.as_mut().set_load_state(QString::from("provisional"));
            self.set_status_text(QString::from("Loading"));
        }
    }

    pub(super) fn navigation_url_changed(mut self: Pin<&mut Self>, url: QString) {
        let url = match canonical_engine_url(url) {
            Ok(url) => QString::from(url),
            Err(error) => {
                self.set_status_text(QString::from(format!("Engine URL rejected: {error}")));
                return;
            }
        };
        let active_index = self.as_ref().rust().active_tab_index;
        self.as_mut().clear_focus_observation_state(active_index);
        let Ok(parsed) = ValidatedUrl::parse(url.to_string()) else {
            self.set_status_text(QString::from("Engine returned an invalid URL"));
            return;
        };
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        );
        if let Some(target) = target {
            let record_history = self
                .as_ref()
                .should_record_same_document_visit(target, &url.to_string());
            self.as_mut()
                .note_navigation_url_change(target, &url.to_string());
            let changed = self
                .as_mut()
                .reduce_event(Event::SameDocumentNavigation {
                    target,
                    url: parsed,
                })
                .is_ok();
            if changed && record_history {
                self.as_mut()
                    .record_same_document_visit(target.tab, &url.to_string());
            }
        }
        self.as_mut().update_current_url(url);
        self.set_status_text(QString::from("URL changed"));
    }

    pub(super) fn navigation_url_changed_for(mut self: Pin<&mut Self>, index: i32, url: QString) {
        let url = match canonical_engine_url(url) {
            Ok(url) => QString::from(url),
            Err(error) => {
                if index == self.as_ref().rust().active_tab_index {
                    self.set_status_text(QString::from(format!("Engine URL rejected: {error}")));
                }
                return;
            }
        };
        self.as_mut().clear_focus_observation_state(index);
        let changed_experiment_target =
            self.as_ref()
                .get_ref()
                .tab_for_index(index)
                .is_some_and(|tab| {
                    self.as_ref()
                        .rust()
                        .active_site_experiment
                        .as_ref()
                        .is_some_and(|experiment| {
                            let target =
                                tab == experiment.tab || experiment.temporary_tab == Some(tab);
                            let placeholder = experiment.temporary_tab == Some(tab)
                                && url.to_string() == "about:blank";
                            target
                                && !placeholder
                                && safe_ipc_url(&url.to_string()) != experiment.url
                        })
                });
        if changed_experiment_target
            && let Some(id) = self
                .as_ref()
                .rust()
                .active_site_experiment
                .as_ref()
                .map(|experiment| experiment.id.clone())
        {
            let _ = self
                .as_mut()
                .finish_site_doctor_experiment(&QString::from(id), false);
        }
        let Ok(parsed) = ValidatedUrl::parse(url.to_string()) else {
            if index == self.as_ref().rust().active_tab_index {
                self.set_status_text(QString::from("Engine returned an invalid URL"));
            }
            return;
        };
        let target = self.as_ref().get_ref().target_for_index(index);
        if let Some(target) = target {
            let record_history = self
                .as_ref()
                .should_record_same_document_visit(target, &url.to_string());
            self.as_mut()
                .note_navigation_url_change(target, &url.to_string());
            let changed = self
                .as_mut()
                .reduce_event(Event::SameDocumentNavigation {
                    target,
                    url: parsed,
                })
                .is_ok();
            if changed && record_history {
                self.as_mut()
                    .record_same_document_visit(target.tab, &url.to_string());
            }
        }
        if index == self.as_ref().rust().active_tab_index {
            self.as_mut().update_current_url(url);
            self.set_status_text(QString::from("URL changed"));
        }
    }

    pub(super) fn navigation_committed(mut self: Pin<&mut Self>, url: QString, title: QString) {
        let canonical_url = match canonical_engine_url(url) {
            Ok(url) => url,
            Err(error) => {
                self.set_status_text(QString::from(format!("Engine URL rejected: {error}")));
                return;
            }
        };
        let raw_title = title.to_string();
        drop(title);
        let title = sanitize_untrusted_title(&raw_title);
        let Ok(parsed) = ValidatedUrl::parse(canonical_url.clone()) else {
            self.set_status_text(QString::from("Engine returned an invalid URL"));
            return;
        };
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        );
        if let Some(target) = target {
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
            let committed = self.as_mut().reduce_event(commit_event).is_ok();
            if committed {
                self.as_mut().record_committed_visit(
                    target.tab,
                    &canonical_url,
                    &title,
                    was_traversal,
                    journey_parent,
                );
            }
        }
        self.as_mut()
            .update_current_url(QString::from(canonical_url));
        self.as_mut().set_page_title(QString::from(title));
        self.as_mut().set_load_state(QString::from("committed"));
    }

    pub(super) fn navigation_committed_for(
        mut self: Pin<&mut Self>,
        index: i32,
        url: QString,
        title: QString,
    ) {
        let canonical_url = match canonical_engine_url(url) {
            Ok(url) => url,
            Err(error) => {
                if index == self.as_ref().rust().active_tab_index {
                    self.set_status_text(QString::from(format!("Engine URL rejected: {error}")));
                }
                return;
            }
        };
        let raw_title = title.to_string();
        drop(title);
        let title = sanitize_untrusted_title(&raw_title);
        let Ok(parsed) = ValidatedUrl::parse(canonical_url.clone()) else {
            if index == self.as_ref().rust().active_tab_index {
                self.set_status_text(QString::from("Engine returned an invalid URL"));
            }
            return;
        };
        let target = self.as_ref().get_ref().target_for_index(index);
        if let Some(target) = target {
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
            let committed = self.as_mut().reduce_event(commit_event).is_ok();
            if committed {
                self.as_mut().record_committed_visit(
                    target.tab,
                    &canonical_url,
                    &title,
                    was_traversal,
                    journey_parent,
                );
            }
        }
        if index == self.as_ref().rust().active_tab_index {
            self.as_mut()
                .update_current_url(QString::from(canonical_url));
            self.as_mut().set_page_title(QString::from(title));
            self.as_mut().set_load_state(QString::from("committed"));
        }
    }

    pub(super) fn release_transient_resources(mut self: Pin<&mut Self>) -> bool {
        let has_state = self.as_ref().rust().state.is_some();
        if has_state && !self.as_ref().active_profile_is_transient() {
            self.set_status_text(QString::from(
                "Durable profile resources remain owned until the application exits",
            ));
            return false;
        }
        if !has_state {
            return true;
        }

        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            finish_pending_selection_operation(this, "cancelled");
            this.pending_caret = None;
            this.pending_spawn = None;
            if let Some(pending) = this.pending_action_target.take() {
                this.operation_states
                    .insert(pending.operation_id, "cancelled".into());
            }
            if let Some(pending) = this.pending_userscript.take() {
                this.operation_states
                    .insert(pending.operation_id, "cancelled".into());
            }
            if let Some(editor) = this.pending_editor.take() {
                discard_editor_request(editor);
            }
            let old_print_worker = this.print_worker.take();
            drop(old_print_worker);
            if let Some(path) = this.pending_print.take() {
                let private_temporary = std::mem::take(&mut this.pending_print_private);
                cleanup_print_artifact(&path, private_temporary);
            }
            this.pending_save_page = None;
            let old_editor_write_worker = this.editor_write_worker.take();
            drop(old_editor_write_worker);
            if let Some((_, path, private_temporary)) = this.pending_editor_write.take() {
                cleanup_editor_artifact(&path, private_temporary);
            }
            if let Some(editor) = this.pending_config_edit.take() {
                discard_config_edit(editor);
            }
            this.pending_download = None;
            cleanup_staged_downloads(&mut this.staged_downloads);
            this.pending_link_navigation = None;
            this.pending_external_navigation = None;
            this.pending_context_route = None;
            this.pending_engine_action = None;
            // A transient profile can be released while its asynchronous
            // bootstrap or profile-scoped probes are still in flight. Drop
            // the pending handoff before dropping workers so a late result
            // cannot recreate state after the final owner has gone away.
            this.pending_profile_configuration = None;
            this.config_reload_worker = None;
            this.config_write_worker = None;
            this.profile_delete_worker = None;
            this.profile_list_worker = None;
            this.profile_preview_worker = None;
            this.network_policy_worker = None;
            this.hyprland_worker = None;
            this.portal_probe_worker = None;
            this.reduced_motion_probe_worker = None;
            this.font_scale_probe_worker = None;
            this.pending_journey_transitions.clear();
            this.pending_journey_parent = None;
            this.pending_journey_mappings.clear();
            this.pending_history_clear = None;
            this.pending_journey_traversal = None;
            this.pending_journey_reopen = None;
            this.pending_navigation_urls.clear();
            this.pending_redirect_tabs.clear();
            this.pending_redirect_hops.clear();
            this.popup_journey_targets.clear();
            this.tab_ids.clear();
            this.popup_tab_ids.clear();
            this.journey_durable_ids.borrow_mut().clear();
            this.journey_export_payload = None;
            this.journey_export_preview_text = QString::default();
            this.session_permissions.clear();
            this.operation_states.clear();
            this.operation_stderr.clear();
            this.action_audit.clear();
            this.request_resolution_counts.clear();
            this.closed_tabs.clear();
            this.macro_registers.clear();
            this.recording_macro = None;
            this.macro_key_prefix = None;
            this.macro_key_started_ms = None;
            this.last_repeatable = None;
            this.macro_expanded_commands = 0;
            this.active_site_experiment = None;
            this.last_site_doctor_result = None;
            this.contexts = None;
            this.userscript_manager_worker = None;
            this.storage_library = None;
            this.storage_library_revision = 0;
            this.storage_library_dirty.set(false);
            this.pending_permission_reset_session = None;
            this.request_resolution_counts.clear();
            this.profile_persistence = ProfilePersistence::Unavailable;
            this.private_history.clear();
            this.private_history_next_id = -1;
            this.structured_log = None;
            this.storage_roots = None;
            this.profile_registry_roots = None;
            this.userscript_roots = None;
            if let Some(path) = this.spawn_working_directory.take() {
                let _ = fs::remove_dir_all(path);
            }
            this.profile_id = None;
            this.session_path = None;
            this.session_state_root = None;
            this.state = None;
            this.window = None;
            this.core_window_id = QString::default();
            this.live_window_registry.clear();
            this.tab = None;
            this.bindings = None;
            this.learning_mode = false;
            this.config_watch = ConfigWatch::default();
            this.theme_watch = ConfigWatch::default();
            this.editor_completions
                .lock()
                .map(|mut completions| completions.clear())
                .ok();
            this.spawn_completions
                .lock()
                .map(|mut mutations| mutations.clear())
                .ok();
            this.userscript_completions
                .lock()
                .map(|mut mutations| mutations.clear())
                .ok();
            if let Ok(mut cancellations) = this.userscript_cancellations.lock() {
                for cancellation in cancellations.values() {
                    cancellation.store(true, Ordering::Release);
                }
                cancellations.clear();
            }
        }
        self.as_mut().sync_macro_status_text();
        self.as_mut().set_active_tab_index(0);
        self.as_mut().set_tab_count(0);
        self.as_mut().set_current_url(QString::from("about:blank"));
        self.as_mut().set_display_url(QString::from("about:blank"));
        self.as_mut().set_page_title(QString::default());
        self.as_mut().set_view_alive(false);
        self.as_mut().set_load_state(QString::from("released"));
        self.set_status_text(QString::from("Transient profile resources released"));
        true
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn view_closed_for(mut self: Pin<&mut Self>, index: i32) -> bool {
        let Some(tab) = self.as_ref().get_ref().tab_for_index(index) else {
            self.set_status_text(QString::from("Unknown tab"));
            return false;
        };
        let Some(generation) = self
            .as_ref()
            .get_ref()
            .rust()
            .state
            .as_ref()
            .and_then(|state| state.tabs().get(&tab).map(|tab| tab.generation))
        else {
            self.set_status_text(QString::from("Unknown tab"));
            return false;
        };
        self.as_mut().finish_tab_close(index, tab, generation)
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn finish_tab_close(
        mut self: Pin<&mut Self>,
        index: i32,
        tab: TabId,
        generation: u64,
    ) -> bool {
        self.as_mut().clear_focus_observation_state(index);
        self.as_mut().clear_hint_session_state();
        let experiment_closing =
            self.as_ref()
                .rust()
                .active_site_experiment
                .as_ref()
                .map(|experiment| {
                    (
                        experiment.id.clone(),
                        experiment.temporary_tab == Some(tab),
                        experiment.tab == tab,
                        experiment.temporary_tab,
                    )
                });
        if let Some((id, temporary_closed, original_closed, _)) = experiment_closing.clone()
            && (temporary_closed || original_closed)
        {
            let _ = self
                .as_mut()
                .finish_site_doctor_experiment(&QString::from(id), false);
            if temporary_closed {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_engine_action = None;
            }
        }
        let closed = {
            let binding = self.as_ref();
            let state = binding.rust().state.as_ref();
            state.and_then(|state| {
                let tab_state = state.tabs().get(&tab)?;
                let profile = state.profiles().get(&tab_state.profile)?;
                Some(ClosedTabDescriptor {
                    id: Uuid::new_v4(),
                    url: tab_state
                        .url
                        .as_deref()
                        .map_or_else(|| "about:blank".into(), safe_ipc_url),
                    title: tab_state.title.clone(),
                    profile: profile.label.clone(),
                    private: profile.privacy.is_transient(),
                    closed_at: unix_timestamp(),
                })
            })
        };
        if let Err(error) = self
            .as_mut()
            .reduce_event(Event::ViewClosed { tab, generation })
        {
            self.set_status_text(QString::from(format!(
                "Tab close acknowledgement rejected: {error}"
            )));
            return false;
        }
        if let Some(closed) = closed {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.closed_tabs.insert(0, closed);
            this.closed_tabs
                .truncate(configured_undo_limit(&this.config));
        }
        let tab_count = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.tab_ids.retain(|candidate| *candidate != tab);
            if let Some((_, _, original_closed, Some(temporary_tab))) = experiment_closing {
                if original_closed {
                    if let Some(temporary_index) = this
                        .tab_ids
                        .iter()
                        .position(|candidate| *candidate == temporary_tab)
                    {
                        this.pending_engine_action =
                            Some(format!("site-doctor-close\t{temporary_index}"));
                    }
                }
            }
            let active_tab = this.window.and_then(|window| {
                this.state
                    .as_ref()
                    .and_then(|state| state.windows().get(&window))
                    .and_then(|window| window.active_tab)
            });
            if let Some(active_tab) = active_tab
                && !this.popup_tab_ids.contains(&active_tab)
                && !this.tab_ids.contains(&active_tab)
            {
                this.tab_ids.push(active_tab);
            }
            i32::try_from(this.tab_ids.len()).unwrap_or(i32::MAX)
        };
        self.as_mut().set_tab_count(tab_count);
        self.as_mut().sync_active_tab_properties();
        self.set_status_text(QString::from("Tab closed"));
        true
    }
}
