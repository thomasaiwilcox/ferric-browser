use super::feature_state::SessionRestoreIntent;
use super::{
    Arc, AtomicBool, BrowserApplication, CatalogReadState, CxxQtType, Event, FeedbackLane,
    FeedbackState, JourneyReopenRecord, Ordering, Pin, QString, StorageCompletion, StorageRequest,
    SwitcherLibraryIndexResult, SwitcherQueryCache, build_switcher_library_index, qobject,
    safe_ipc_url, switcher_max_results, thread,
};

impl qobject::BrowserUi {
    #[allow(clippy::too_many_lines)]
    pub(super) fn switcher_query(
        mut self: Pin<&mut Self>,
        query: &QString,
        scope: &QString,
    ) -> bool {
        let scope = scope.to_string();
        if !self.as_mut().prepare_switcher_sources(&scope) {
            self.set_status_text(QString::from(
                "Choose a valid Switcher scope and try again.",
            ));
            return false;
        }
        self.as_mut().publish_switcher_availability(&scope);
        let max_results = switcher_max_results(&self.as_ref().rust().config);
        let mut params = serde_json::json!({
            "query": query.to_string(),
            "scope": if scope.is_empty() { "all" } else { &scope },
            "limit": max_results,
            "include_private": self.as_ref().active_profile_is_transient(),
            "private_scope": self
                .as_ref()
                .active_profile_is_transient()
                .then(|| {
                    self.as_ref().rust().state.as_ref().and_then(|state| {
                        state
                            .active_tab()
                            .and_then(|tab| state.profiles().get(&tab.profile))
                            .map(|profile| profile.label.clone())
                    })
                })
                .flatten()
        });
        // Optional query fields are absent, rather than null, in the strict
        // switcher protocol. Private windows still supply their exact scope.
        if let Some(parameters) = params.as_object_mut()
            && parameters
                .get("private_scope")
                .is_some_and(serde_json::Value::is_null)
        {
            parameters.remove("private_scope");
        }
        if let Some(result) = self
            .as_ref()
            .rust()
            .library
            .switcher_cache
            .as_ref()
            .filter(|cache| cache.matches(self.as_ref().rust(), &params))
            .map(|cache| cache.result.clone())
        {
            return match self.as_mut().publish_switcher_rows(&result) {
                Ok(()) => true,
                Err(error) => self.reject_switcher_query(super::PublicError::new(
                    ferric_browser_ipc::ErrorCode::Engine,
                    "Search results couldn't be displayed. Close the search and try again.",
                    error,
                )),
            };
        }
        match self.as_ref().get_ref().native_switcher_query(&params) {
            Ok(result) => {
                if let Err(error) = self.as_mut().publish_switcher_rows(&result) {
                    return self.reject_switcher_query(super::PublicError::new(
                        ferric_browser_ipc::ErrorCode::Engine,
                        "Search results couldn't be displayed. Close the search and try again.",
                        error,
                    ));
                }
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.library.switcher_cache = Some(SwitcherQueryCache {
                    params,
                    state_revision: this.state.as_ref().map_or(0, BrowserApplication::revision),
                    storage_library_revision: this.storage_library_revision,
                    session_names: this.library.session_names.clone(),
                    contexts_json: this.contexts_json.to_string(),
                    configuration_revision: this
                        .state
                        .as_ref()
                        .map_or(0, BrowserApplication::configuration_revision),
                    profile_name: this.profile_name.clone(),
                    result,
                });
                true
            }
            Err(error) => self.reject_switcher_query(error),
        }
    }

    fn reject_switcher_query(mut self: Pin<&mut Self>, error: super::PublicError) -> bool {
        self.as_mut().set_switcher_sources_pending(false);
        self.as_mut().set_switcher_sources_failed(true);
        self.as_mut().set_switcher_sources_retryable(false);
        self.as_mut()
            .set_switcher_sources_notice(QString::from(error.user_message()));
        self.as_mut().set_switcher_sources_detail(QString::from(
            error
                .diagnostic_context()
                .chars()
                .take(4096)
                .collect::<String>(),
        ));
        self.set_status_text(QString::from(error.user_message()));
        false
    }

    pub(super) fn request_storage_library(mut self: Pin<&mut Self>) {
        if self.as_ref().active_profile_is_transient() {
            return;
        }
        self.as_mut().ensure_storage_worker();
        if self
            .as_ref()
            .rust()
            .library
            .storage_library_read_state
            .is_in_flight()
            || (self.as_ref().rust().library.storage_library.is_some()
                && !self.as_ref().rust().library.storage_library_dirty.get())
        {
            return;
        }
        match self
            .as_mut()
            .submit_storage(StorageRequest::Library { limit: 1_000 })
        {
            Ok(()) => {
                self.as_mut().set_library_error(QString::default());
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .library
                    .storage_library_read_state = CatalogReadState::Pending;
                self.publish_library_pending();
            }
            Err(
                ferric_browser_storage::StorageWorkerError::Busy
                | ferric_browser_storage::StorageWorkerError::QueueFull,
            ) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .library
                    .storage_library_read_state = CatalogReadState::Queued;
                self.publish_library_pending();
            }
            Err(error) => {
                let detail = error.to_string();
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .library
                    .storage_library_read_state = CatalogReadState::Failed(detail.clone());
                self.as_mut().set_library_error(QString::from(detail));
                self.as_mut().set_status_text(QString::from(
                    "Saved items couldn't be loaded. Refresh the library to try again.",
                ));
                self.publish_library_pending();
            }
        }
    }

    /// Build the durable switcher catalog away from the Qt thread. The
    /// snapshot is immutable and profile-local, so the worker can safely
    /// prepare the query-facing rows without touching Qt or SQLite.
    pub(super) fn request_switcher_library_index(mut self: Pin<&mut Self>) {
        let (snapshot, revision, profile_name, result_slot, cancel) = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let transient = this.state.as_ref().and_then(|state| {
                state
                    .active_tab()
                    .and_then(|tab| state.profiles().get(&tab.profile))
                    .map(|profile| profile.privacy.is_transient())
            }) == Some(true);
            if transient || this.library.storage_library_dirty.get() {
                return;
            }
            if this.library.switcher_library_index_building {
                if this.library.switcher_library_index_revision != this.storage_library_revision
                    && let Some(cancel) = this.library.switcher_library_index_cancel.as_ref()
                {
                    cancel.store(true, Ordering::Relaxed);
                }
                return;
            }
            if this
                .library
                .switcher_library_index
                .as_ref()
                .is_some_and(|index| {
                    this.library.switcher_library_index_revision == this.storage_library_revision
                        && index.profile_name == this.profile_name
                })
            {
                return;
            }
            let Some(snapshot) = this.library.storage_library.clone() else {
                return;
            };
            this.library.switcher_library_index_building = true;
            let cancel = Arc::new(AtomicBool::new(false));
            this.library.switcher_library_index_cancel = Some(Arc::clone(&cancel));
            (
                snapshot,
                this.storage_library_revision,
                this.profile_name.clone(),
                Arc::clone(&this.library.switcher_library_index_result),
                cancel,
            )
        };
        let spawned = thread::Builder::new()
            .name("ferric-browser-switcher-index".into())
            .spawn(move || {
                let index = build_switcher_library_index(&snapshot, profile_name, &cancel);
                if let Ok(mut pending) = result_slot.lock() {
                    *pending = Some(SwitcherLibraryIndexResult { revision, index });
                }
            });
        if spawned.is_err() {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .library
                .switcher_library_index_building = false;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .library
                .switcher_library_index_cancel = None;
        }
    }

    pub(super) fn poll_switcher_library_index(mut self: Pin<&mut Self>) -> bool {
        let result = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.library
                .switcher_library_index_result
                .lock()
                .ok()
                .and_then(|mut pending| pending.take())
        };
        let Some(result) = result else {
            return false;
        };
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.library.switcher_library_index_building = false;
        this.library.switcher_library_index_cancel = None;
        let Some(index) = result.index else {
            return true;
        };
        let transient = this.state.as_ref().and_then(|state| {
            state
                .active_tab()
                .and_then(|tab| state.profiles().get(&tab.profile))
                .map(|profile| profile.privacy.is_transient())
        }) == Some(true);
        if result.revision != this.storage_library_revision
            || index.profile_name != this.profile_name
            || transient
        {
            return false;
        }
        this.library.switcher_library_index_revision = result.revision;
        this.library.switcher_library_index = Some(index);
        this.library.switcher_cache = None;
        true
    }

    pub(super) fn ensure_storage_worker(mut self: Pin<&mut Self>) -> bool {
        // Transient and unavailable profiles have no durable worker to restart.
        if self.as_ref().active_profile_is_transient()
            || self
                .as_ref()
                .rust()
                .profile_persistence
                .lacks_durable_storage()
        {
            return false;
        }
        if self.as_ref().storage_is_open() {
            return true;
        }
        let restart = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .state
                .as_mut()
                .ok_or_else(|| "profile application is unavailable".to_owned())
                .and_then(|application| {
                    application
                        .restart_storage()
                        .map_err(|error| error.to_string())
                })
        };
        match restart {
            Ok(()) => {
                self.set_status_text(QString::from("Retrying profile storage…"));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Profile storage could not be reconnected: {error}"
                )));
                false
            }
        }
    }

    pub(super) fn poll_storage_library(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().ensure_storage_worker();
        self.as_mut().flush_start_page_refresh();
        let mut consumed = self.as_mut().poll_switcher_library_index();
        consumed |= self.as_mut().retry_queued_switcher_sources();
        let effects = self.as_mut().poll_storage_effects();
        let mut journey_ticket = None;
        let mut library_result = None;
        let mut session_result = None;
        let mut session_restore_results = Vec::new();
        let mut session_save_results = Vec::new();
        let mut session_delete_result = None;
        let mut session_checkpoint_clear_result = None;
        let mut history_result = None;
        let mut command_record_result = None;
        let mut permission_result = None;
        let mut permission_reset_result = None;
        let mut download_result = None;
        let mut download_destination_result = None;
        let mut download_create_result = None;
        let mut journey_result = None;
        let mut journey_node_result = None;
        let mut journey_query_result = None;
        let mut journey_export_result = None;
        let mut mark_results = Vec::new();
        let mut history_clear_result = None;
        let mut flush_result = None;
        for effect in effects {
            match effect.completion {
                StorageCompletion::StartPage(result) => {
                    self.as_mut()
                        .complete_start_page_read(effect.ticket, result.map(|snapshot| *snapshot));
                    consumed = true;
                }
                StorageCompletion::StartPageWrite(result) => {
                    self.as_mut()
                        .complete_start_page_write(effect.ticket, result);
                    consumed = true;
                }
                StorageCompletion::Library(result) => {
                    library_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::SessionList(result) => {
                    session_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::SessionRestore(result) => {
                    session_restore_results
                        .push((effect.ticket, result.map_err(|error| error.message)));
                }
                StorageCompletion::SessionSave(result) => {
                    session_save_results
                        .push((effect.ticket, result.map_err(|error| error.message)));
                }
                StorageCompletion::SessionDelete(result) => {
                    session_delete_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::SessionCheckpointClear(result) => {
                    session_checkpoint_clear_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::HistoryBatch(result) => {
                    history_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::CommandRecord(result) => {
                    command_record_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::PermissionLookup(_) => consumed = true,
                StorageCompletion::PermissionBatch(result) => {
                    permission_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::PermissionReset(result) => {
                    permission_reset_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::DownloadSave(result) => {
                    if result.is_ok() {
                        self.as_ref().rust().library.storage_library_dirty.set(true);
                    }
                    consumed = true;
                }
                StorageCompletion::DownloadBatch(result) => {
                    download_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::DownloadDestination(result) => {
                    download_destination_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::DownloadCreate(result) => {
                    download_create_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::JourneyWrite(result) => {
                    journey_ticket = Some(effect.ticket);
                    journey_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::JourneyNode(result) => {
                    journey_node_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::JourneyQuery(result) => {
                    journey_query_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::JourneyExport(result) => {
                    journey_export_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::MarkWrite(result) => {
                    mark_results.push((effect.ticket, result.map_err(|error| error.message)));
                }
                StorageCompletion::HistoryClear(result) => {
                    history_clear_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::Flush(result) => {
                    flush_result = Some(result.map_err(|error| error.message));
                }
            }
        }
        if let Some(result) = flush_result {
            consumed = true;
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.library.storage_flush_pending = false;
            this.library.storage_flush_error = result.err();
        }
        if let Some(result) = session_checkpoint_clear_result {
            consumed = true;
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.library.session_checkpoint_clear_pending = false;
            drop(rust);
            if let Err(error) = result {
                self.as_mut().set_status_text(QString::from(format!(
                    "Stale session checkpoint cleanup failed; recovery data was retained: {error}"
                )));
            }
        }
        if let Some(result) = session_delete_result {
            consumed = true;
            match result {
                Ok(()) => {
                    self.as_mut().invalidate_session_catalog();
                    self.as_mut().request_session_names();
                    self.as_mut().publish_management_feedback(
                        FeedbackLane::SessionDelete,
                        FeedbackState::Succeeded,
                        "Saved session deleted. Open tabs are kept.",
                        "",
                    );
                    self.as_mut()
                        .set_status_text(QString::from("Session deleted"));
                }
                Err(error) => {
                    self.as_mut().publish_management_feedback(
                        FeedbackLane::SessionDelete,
                        FeedbackState::Failed,
                        "The saved session couldn't be deleted. Review the details and try again.",
                        &error,
                    );
                    self.as_mut().set_status_text(QString::from(format!(
                        "Session deletion failed: {error}"
                    )));
                }
            }
        }
        for (ticket, result) in session_save_results {
            consumed |= self.as_mut().complete_session_save(ticket, result);
        }
        for (ticket, result) in session_restore_results {
            consumed = true;
            let intent = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .library
                .session_restores
                .remove(&ticket);
            // Cancelled requests and old profile owners have no authority to
            // publish. In particular they must not fall through to preview.
            let Some(intent) = intent else { continue };
            if intent == SessionRestoreIntent::Recovery {
                match result {
                    Ok(snapshot)
                        if !snapshot.entries.is_empty() || !snapshot.closed_tabs.is_empty() =>
                    {
                        let append = !self.as_ref().can_reuse_recovery_placeholder();
                        let restored = if snapshot.entries.is_empty() {
                            // A closed-tab-only checkpoint still recovers Undo,
                            // without replacing or adding any live page.
                            self.as_mut()
                                .set_status_text(QString::from("Recovered recently closed tabs"));
                            QString::from("append\n")
                        } else {
                            let rows = self
                                .as_mut()
                                .restore_session_from_entries(snapshot.entries, append);
                            if rows.is_empty() {
                                QString::default()
                            } else {
                                QString::from(format!(
                                    "{}\n{rows}",
                                    if append { "append" } else { "replace" }
                                ))
                            }
                        };
                        if restored.is_empty() {
                            self.as_mut().set_session_recovery_error(QString::from(
                                "Saved tabs couldn't be restored. Try again, or start without recovery.",
                            ));
                            self.as_mut()
                                .set_status_text(QString::from("Recovery could not be applied"));
                        } else {
                            self.as_mut()
                                .restore_closed_tabs_from_snapshot(snapshot.closed_tabs);
                            {
                                let mut rust = self.as_mut().rust_mut();
                                let this = rust.as_mut().get_mut();
                                let current = this.session_id;
                                this.recovered_session_ids.extend(
                                    snapshot
                                        .source_session_ids
                                        .into_iter()
                                        .filter(|id| *id != current),
                                );
                                this.recovered_session_ids.sort_unstable();
                                this.recovered_session_ids.dedup();
                                this.checkpoint
                                    .mark_dirty(super::elapsed_ms(this.binding_clock));
                                this.recovery_paths.clear();
                                this.recovery_placeholder = None;
                            }
                            self.as_mut().set_session_restore_values(restored);
                        }
                    }
                    Ok(_) => {
                        self.as_mut().set_session_recovery_error(QString::from(
                            "There are no saved tabs to recover. You can start without recovery.",
                        ));
                        self.as_mut()
                            .set_status_text(QString::from("Recovery session is empty"));
                    }
                    Err(error) => {
                        self.as_mut().set_session_recovery_error(QString::from(
                            "Saved tabs couldn't be read. Try again, or start without recovery.",
                        ));
                        self.as_mut()
                            .set_status_text(QString::from(format!("Recovery rejected: {error}")));
                    }
                }
                self.as_mut().set_session_recovery_pending(false);
            } else if let SessionRestoreIntent::Load { append } = intent {
                match result {
                    Ok(snapshot) if !snapshot.entries.is_empty() => {
                        let restored = self
                            .as_mut()
                            .restore_session_from_entries(snapshot.entries, append);
                        if restored.is_empty() {
                            self.as_mut().set_session_preview(QString::from(
                                "ERROR\tSession restore could not be applied",
                            ));
                        } else {
                            self.as_mut()
                                .set_session_restore_values(QString::from(format!(
                                    "{}\n{restored}",
                                    if append { "append" } else { "replace" }
                                )));
                            self.as_mut()
                                .set_status_text(QString::from("Session loaded"));
                        }
                    }
                    Ok(_) => {
                        self.as_mut()
                            .set_session_preview(QString::from("ERROR\tSaved session is empty"));
                    }
                    Err(error) => {
                        self.as_mut().set_session_preview(QString::from(format!(
                            "ERROR\tSession restore rejected: {error}"
                        )));
                    }
                }
            } else {
                match result {
                    Ok(snapshot) if !snapshot.entries.is_empty() => {
                        self.as_mut()
                            .publish_session_preview_rows(&snapshot.entries);
                        let contents = snapshot
                            .entries
                            .iter()
                            .map(|entry| {
                                entry
                                    .url
                                    .as_deref()
                                    .unwrap_or("[placeholder: navigation will not be replayed]")
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        self.as_mut().set_session_preview(QString::from(format!(
                            "{} tab(s)\n{contents}",
                            snapshot.entries.len()
                        )));
                        self.as_mut()
                            .set_status_text(QString::from("Session preview ready"));
                    }
                    Ok(_) => {
                        self.as_mut()
                            .set_session_preview(QString::from("ERROR\tSaved session is empty"));
                        self.as_mut()
                            .set_status_text(QString::from("Saved session is empty"));
                    }
                    Err(error) => {
                        self.as_mut()
                            .set_session_preview(QString::from(format!("ERROR\t{error}")));
                        self.as_mut().set_status_text(QString::from(format!(
                            "Session preview rejected: {error}"
                        )));
                    }
                }
            }
        }
        if let Some(result) = session_result {
            consumed = true;
            let refresh_again = self.as_ref().rust().library.session_names_refresh_again;
            self.as_mut().set_session_names_pending(false);
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .session_catalog_read_state = match &result {
                Ok(_) => CatalogReadState::Ready,
                Err(error) => CatalogReadState::Failed(error.clone()),
            };
            if refresh_again {
                // The read started before a successful mutation. Never publish
                // that older catalog over the mutation's newer state.
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .library
                    .session_names_refresh_again = false;
                self.as_mut().request_session_names();
            } else {
                match result {
                    Ok(names) => {
                        let values = names
                            .iter()
                            .map(|name| QString::from(name.as_str()))
                            .collect();
                        let mut rust = self.as_mut().rust_mut();
                        let this = rust.as_mut().get_mut();
                        this.library.session_names = names;
                        this.library.session_names_dirty.set(false);
                        drop(rust);
                        self.as_mut().set_session_names_error(QString::default());
                        self.as_mut().set_session_names_values(values);
                    }
                    Err(error) => {
                        self.as_mut().set_session_names_error(QString::from(&error));
                        self.as_mut().set_status_text(QString::from(
                            "Saved sessions couldn't be loaded. Refresh the session list to try again.",
                        ));
                    }
                }
            }
        }
        for (ticket, result) in mark_results {
            consumed = true;
            self.as_mut().apply_mark_write_result(ticket, result);
        }
        if let Some(result) = library_result {
            consumed = true;
            self.as_mut().apply_library_read_result(result);
        }
        if let Some(result) = command_record_result {
            consumed = true;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .command_history_write_pending = false;
            if let Err(error) = result {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .command_history_write_error = Some(error.clone());
                self.as_mut().set_status_text(QString::from(format!(
                    "Command history write failed: {error}"
                )));
            }
        }
        self.as_mut().flush_command_history_writes();
        self.as_mut().request_switcher_library_index();
        if let Some(result) = journey_node_result {
            consumed = true;
            let pending = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .navigation
                .pending_journey_reopen
                .take();
            match (pending, result) {
                (Some((node_id, target_kind, current_tab, profile_id)), Ok(Some(node))) => {
                    if node.id != node_id || node.profile_id != profile_id {
                        self.as_mut().set_status_text(QString::from(
                            "That journey entry is unavailable in this profile. Refresh the journey and try again.",
                        ));
                    } else {
                        let record = JourneyReopenRecord {
                            id: node.id,
                            url: node.url,
                        };
                        let reopen_result = self.as_mut().finish_journey_reopen(
                            record,
                            &target_kind,
                            current_tab,
                            Some(profile_id),
                        );
                        match reopen_result {
                            Ok(_) => self
                                .as_mut()
                                .set_status_text(QString::from("Journey reopened")),
                            Err(error) => self.as_mut().set_status_text(QString::from(error)),
                        }
                    }
                }
                (Some(_), Ok(None)) => self.as_mut().set_status_text(QString::from(
                    "That journey entry is no longer available. Refresh the journey and try again.",
                )),
                (Some(_), Err(error)) => self.as_mut().set_status_text(QString::from(format!(
                    "Journey node lookup failed: {error}"
                ))),
                (None, _) => self
                    .as_mut()
                    .set_status_text(QString::from("Journey reopen request expired")),
            }
        }
        if let Some(result) = journey_query_result {
            consumed = true;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .journey_read_pending = false;
            self.as_mut().publish_library_pending();
            if self.as_ref().rust().library_kind.to_string() == "journey" {
                match result {
                    Ok(snapshot) => {
                        self.as_mut().set_library_error(QString::default());
                        self.as_mut().apply_journey_query_snapshot(snapshot);
                        self.as_mut()
                            .set_status_text(QString::from("Journey ready"));
                    }
                    Err(error) => {
                        self.as_mut().set_library_error(QString::from(&error));
                        self.as_mut().set_status_text(QString::from(format!(
                            "Journey couldn't be loaded: {error}"
                        )));
                    }
                }
            }
        }
        if let Some(result) = journey_export_result {
            consumed = true;
            match result {
                Ok(snapshot) => {
                    let profile = self
                        .as_ref()
                        .rust()
                        .profile_id
                        .map_or_else(String::new, |id| id.to_string());
                    let payload = serde_json::to_string(&serde_json::json!({
                        "format": "ferric-browser-journey-v1",
                        "durability": "durable",
                        "profile": profile,
                        "nodes": snapshot.nodes.into_iter().map(|node| serde_json::json!({
                            "id": node.id,
                            "profile": node.profile_id,
                            "tab": node.tab_id,
                            "url": safe_ipc_url(&node.url),
                            "title": node.title,
                            "committed_at": node.committed_at,
                            "transition": node.transition,
                            "source": node.source,
                        })).collect::<Vec<_>>(),
                        "edges": snapshot.edges.into_iter().map(|edge| serde_json::json!({
                            "source": edge.source_id,
                            "target": edge.target_id,
                            "transition": edge.transition,
                            "created_at": edge.created_at,
                        })).collect::<Vec<_>>(),
                    }))
                    .unwrap_or_default();
                    let preview = Self::format_journey_export_preview(&payload);
                    let mut rust = self.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    this.journey_export_payload = Some(payload);
                    drop(rust);
                    self.as_mut().set_journey_export_preview_text(preview);
                    self.as_mut()
                        .set_status_text(QString::from("Journey export preview ready"));
                }
                Err(error) => self.as_mut().set_status_text(QString::from(format!(
                    "Journey export unavailable: {error}"
                ))),
            }
        }
        if let Some(result) = history_result {
            consumed = true;
            self.as_mut().apply_history_batch_result(result);
        }
        if let Some(result) = permission_result {
            consumed = true;
            self.as_mut().apply_permission_batch_result(result);
        }
        if let Some(result) = permission_reset_result {
            consumed = true;
            let session_removed = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_permission_reset_session
                .take()
                .unwrap_or(false);
            match result {
                Ok(true) => {
                    self.as_ref().rust().library.storage_library_dirty.set(true);
                    self.as_mut()
                        .set_status_text(QString::from(if session_removed {
                            "Permission decision reset"
                        } else {
                            "Permission rule reset"
                        }));
                }
                Ok(false) if session_removed => {
                    self.as_mut()
                        .set_status_text(QString::from("Permission decision reset"));
                }
                Ok(false) => self
                    .as_mut()
                    .set_status_text(QString::from("Permission rule was not found")),
                Err(error) => self.as_mut().set_status_text(QString::from(format!(
                    "Permission reset failed; retry is available: {error}"
                ))),
            }
        }
        if let Some(result) = download_result {
            consumed = true;
            self.as_mut().apply_download_batch_result(result);
        }
        if let Some(result) = history_clear_result {
            consumed = true;
            let clear_filter = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_history_clear
                .take();
            match result {
                Ok(deleted) => {
                    self.as_mut().refresh_start_page_metadata();
                    if let Some((since, origin)) = clear_filter {
                        let cutoff = since.and_then(|value| u64::try_from(value).ok());
                        if self
                            .as_mut()
                            .reduce_event(Event::ClearJourney {
                                since: cutoff,
                                origin,
                            })
                            .is_ok()
                        {
                            let mut rust = self.as_mut().rust_mut();
                            let this = rust.as_mut().get_mut();
                            if let Some(state) = this.state.as_ref() {
                                let retained = this
                                    .navigation
                                    .journey_durable_ids
                                    .borrow()
                                    .iter()
                                    .filter(|(node, _)| state.journey().node(**node).is_some())
                                    .map(|(node, durable)| (*node, durable.clone()))
                                    .collect();
                                this.navigation.journey_durable_ids.replace(retained);
                            }
                        }
                    }
                    self.as_ref().rust().library.storage_library_dirty.set(true);
                    self.as_mut().set_status_text(QString::from(format!(
                        "History cleared ({deleted} entries deleted)"
                    )));
                }
                Err(error) => {
                    self.as_mut()
                        .set_status_text(QString::from(format!("History clear failed: {error}")));
                }
            }
        }
        if let Some(result) = download_destination_result {
            consumed = true;
            match result {
                Ok(()) => {
                    self.as_ref().rust().library.storage_library_dirty.set(true);
                }
                Err(error) => {
                    self.as_mut().set_status_text(QString::from(format!(
                        "Download destination update failed: {error}"
                    )));
                }
            }
        }
        if let Some(result) = download_create_result {
            consumed = true;
            match result {
                Ok(()) => {
                    self.as_ref().rust().library.storage_library_dirty.set(true);
                }
                Err(error) => {
                    self.as_mut().set_status_text(QString::from(format!(
                        "Download index creation failed: {error}"
                    )));
                }
            }
        }
        if let Some(result) = journey_result {
            consumed = true;
            let mapping = journey_ticket.and_then(|ticket| {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .navigation
                    .pending_journey_mappings
                    .remove(&ticket)
            });
            match result {
                Ok(true) => {
                    if let Some((core_id, durable_id)) = mapping {
                        self.as_ref()
                            .rust()
                            .navigation
                            .journey_durable_ids
                            .borrow_mut()
                            .insert(core_id, durable_id);
                    }
                    self.as_ref().rust().library.storage_library_dirty.set(true);
                    let mut rust = self.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    this.journey_export_payload = None;
                    this.journey_export_preview_text = QString::default();
                }
                Ok(false) => {
                    self.as_mut().set_status_text(QString::from(
                        "The current page is no longer in this journey. Refresh the journey and try again.",
                    ));
                }
                Err(error) => {
                    self.as_mut().set_status_text(QString::from(format!(
                        "Journey changes could not be saved: {error}"
                    )));
                }
            }
        }
        self.as_mut().flush_history_writes();
        self.as_mut().flush_permission_writes();
        self.as_mut().flush_mark_writes();
        consumed
    }
}
