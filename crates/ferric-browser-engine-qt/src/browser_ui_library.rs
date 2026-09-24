use super::{
    Arc, AtomicBool, BrowserApplication, CxxQtType, DownloadState, Event, JourneyReopenRecord,
    JourneyWrite, MarkWrite, Ordering, Pin, QString, StorageCompletion, StorageRequest,
    StorageTicket, SwitcherLibraryIndexResult, SwitcherQueryCache, Uuid,
    build_switcher_library_index, elapsed_ms, is_safe_history_url, qobject, safe_ipc_url,
    sanitize_untrusted_title, switcher_max_results, thread, unix_timestamp,
};

impl qobject::BrowserUi {
    #[allow(clippy::too_many_lines)]
    pub(super) fn switcher_query(
        mut self: Pin<&mut Self>,
        query: &QString,
        scope: &QString,
    ) -> bool {
        if self.as_ref().rust().storage_library.is_none()
            || self.as_ref().rust().storage_library_dirty.get()
        {
            self.as_mut().request_storage_library();
        }
        self.as_mut().request_session_names();
        let scope = scope.to_string();
        let max_results = switcher_max_results(&self.as_ref().rust().config);
        let params = serde_json::json!({
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
        if let Some(result) = self
            .as_ref()
            .rust()
            .switcher_cache
            .as_ref()
            .filter(|cache| cache.matches(self.as_ref().rust(), &params))
            .map(|cache| cache.result.clone())
        {
            return match self.as_mut().publish_switcher_rows(&result) {
                Ok(()) => true,
                Err(error) => {
                    self.set_status_text(QString::from(format!(
                        "Switcher presentation failed: {error}"
                    )));
                    false
                }
            };
        }
        match self.as_ref().get_ref().ipc_switcher_query(&params) {
            Ok(result) => {
                if let Err(error) = self.as_mut().publish_switcher_rows(&result) {
                    self.set_status_text(QString::from(format!(
                        "Switcher presentation failed: {error}"
                    )));
                    return false;
                }
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.switcher_cache = Some(SwitcherQueryCache {
                    params,
                    state_revision: this.state.as_ref().map_or(0, BrowserApplication::revision),
                    storage_library_revision: this.storage_library_revision,
                    session_names: this.session_names.clone(),
                    contexts_json: this.contexts_json.to_string(),
                    config_fingerprint: serde_json::to_string(&this.config).unwrap_or_default(),
                    profile_name: this.profile_name.clone(),
                    result,
                });
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!("Switcher query rejected: {error}")));
                false
            }
        }
    }

    pub(super) fn request_storage_library(mut self: Pin<&mut Self>) {
        self.as_mut().ensure_storage_worker();
        let should_request = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.storage_library.is_some() && !this.storage_library_dirty.get() {
                return;
            }
            true
        };
        let result = should_request.then(|| {
            self.as_mut()
                .submit_storage(StorageRequest::Library { limit: 1_000 })
        });
        if let Some(Err(error)) = result
            && !matches!(error, ferric_browser_storage::StorageWorkerError::Busy)
        {
            self.set_status_text(QString::from(format!(
                "Profile metadata query unavailable: {error}"
            )));
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
            if transient || this.storage_library_dirty.get() {
                return;
            }
            if this.switcher_library_index_building {
                if this.switcher_library_index_revision != this.storage_library_revision
                    && let Some(cancel) = this.switcher_library_index_cancel.as_ref()
                {
                    cancel.store(true, Ordering::Relaxed);
                }
                return;
            }
            if this.switcher_library_index.as_ref().is_some_and(|index| {
                this.switcher_library_index_revision == this.storage_library_revision
                    && index.profile_name == this.profile_name
            }) {
                return;
            }
            let Some(snapshot) = this.storage_library.clone() else {
                return;
            };
            this.switcher_library_index_building = true;
            let cancel = Arc::new(AtomicBool::new(false));
            this.switcher_library_index_cancel = Some(Arc::clone(&cancel));
            (
                snapshot,
                this.storage_library_revision,
                this.profile_name.clone(),
                Arc::clone(&this.switcher_library_index_result),
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
                .switcher_library_index_building = false;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .switcher_library_index_cancel = None;
        }
    }

    pub(super) fn poll_switcher_library_index(mut self: Pin<&mut Self>) -> bool {
        let result = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.switcher_library_index_result
                .lock()
                .ok()
                .and_then(|mut pending| pending.take())
        };
        let Some(result) = result else {
            return false;
        };
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.switcher_library_index_building = false;
        this.switcher_library_index_cancel = None;
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
        this.switcher_library_index_revision = result.revision;
        this.switcher_library_index = Some(index);
        this.switcher_cache = None;
        true
    }

    pub(super) fn ensure_storage_worker(mut self: Pin<&mut Self>) -> bool {
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
                self.set_status_text(QString::from("Profile metadata worker restarted"));
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Profile metadata worker restart unavailable: {error}"
                )));
                false
            }
        }
    }

    pub(super) fn request_session_names(mut self: Pin<&mut Self>) {
        let request = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if !this.session_names_dirty.get() {
                return;
            }
            // Private and ephemeral sessions have no durable session catalog.
            // Keep this explicit so a future in-memory session implementation
            // cannot accidentally route transient metadata to disk.
            let transient_profile = this.state.as_ref().and_then(|state| {
                let tab = state.active_tab()?;
                state
                    .profiles()
                    .get(&tab.profile)
                    .map(|profile| profile.privacy.is_transient())
            }) == Some(true);
            if transient_profile {
                this.session_names.clear();
                this.session_names_dirty.set(false);
                return;
            }
            let Some(root) = this.session_state_root.clone() else {
                this.session_names_dirty.set(false);
                return;
            };
            let Some(profile_id) = this.profile_id else {
                this.session_names_dirty.set(false);
                return;
            };
            drop(rust);
            Some(
                self.as_mut()
                    .submit_storage(StorageRequest::SessionList { root, profile_id }),
            )
        };
        if let Some(Err(error)) = request
            && !matches!(
                error,
                ferric_browser_storage::StorageWorkerError::Busy
                    | ferric_browser_storage::StorageWorkerError::QueueFull
            )
        {
            self.set_status_text(QString::from(format!(
                "Session catalog unavailable: {error}"
            )));
        }
    }

    pub(super) fn poll_storage_library(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().ensure_storage_worker();
        let mut consumed = self.as_mut().poll_switcher_library_index();
        let effects = self.as_mut().poll_storage_effects();
        let mut journey_ticket = None;
        let mut library_result = None;
        let mut session_result = None;
        let mut session_restore_result = None;
        let mut session_save_result = None;
        let mut session_delete_result = None;
        let mut session_checkpoint_clear_result = None;
        let mut history_result = None;
        let mut permission_result = None;
        let mut permission_reset_result = None;
        let mut download_result = None;
        let mut download_destination_result = None;
        let mut download_create_result = None;
        let mut journey_result = None;
        let mut journey_node_result = None;
        let mut journey_query_result = None;
        let mut journey_export_result = None;
        let mut mark_result = None;
        let mut history_clear_result = None;
        let mut flush_result = None;
        for effect in effects {
            match effect.completion {
                StorageCompletion::Library(result) => {
                    library_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::SessionList(result) => {
                    session_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::SessionRestore(result) => {
                    session_restore_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::SessionSave(result) => {
                    session_save_result = Some(result.map_err(|error| error.message));
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
                StorageCompletion::PermissionBatch(result) => {
                    permission_result = Some(result.map_err(|error| error.message));
                }
                StorageCompletion::PermissionReset(result) => {
                    permission_reset_result = Some(result.map_err(|error| error.message));
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
                    mark_result = Some(result.map_err(|error| error.message));
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
            this.storage_flush_pending = false;
            this.storage_flush_error = result.err();
        }
        if let Some(result) = session_checkpoint_clear_result {
            consumed = true;
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.session_checkpoint_clear_pending = false;
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
                    self.as_ref().rust().session_names_dirty.set(true);
                    self.as_mut().request_session_names();
                    self.as_mut()
                        .set_status_text(QString::from("Session deleted"));
                }
                Err(error) => {
                    self.as_mut().set_status_text(QString::from(format!(
                        "Session deletion failed: {error}"
                    )));
                }
            }
        }
        if let Some(result) = session_save_result {
            consumed = true;
            let checkpoint_save = {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.session_save_pending = false;
                this.session_save_checkpoint
            };
            match result {
                Ok(()) => {
                    let mut rust = self.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    this.session_names_dirty.set(true);
                    if checkpoint_save {
                        this.checkpoint.dirty = false;
                        this.checkpoint.dirty_since_ms = None;
                        this.checkpoint.restore_pending = None;
                        this.checkpoint.last_saved_ms = elapsed_ms(this.binding_clock);
                    }
                    drop(rust);
                    self.as_mut()
                        .set_status_text(QString::from(if checkpoint_save {
                            "Session checkpoint saved"
                        } else {
                            "Session saved"
                        }));
                }
                Err(error) => {
                    self.as_mut().set_status_text(QString::from(format!(
                        "Session snapshot failed; changes retained: {error}"
                    )));
                }
            }
        }
        if let Some(result) = session_restore_result {
            consumed = true;
            let recovery_pending = {
                let mut rust = self.as_mut().rust_mut();
                rust.as_mut().get_mut().session_restore_recovery_pending
            };
            let restore_load_append = {
                let mut rust = self.as_mut().rust_mut();
                rust.as_mut().get_mut().session_restore_load_append.take()
            };
            {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                this.session_restore_preview_pending = false;
                this.session_restore_recovery_pending = false;
            }
            if recovery_pending {
                match result {
                    Ok(snapshot)
                        if !snapshot.entries.is_empty() || !snapshot.closed_tabs.is_empty() =>
                    {
                        self.as_mut()
                            .restore_closed_tabs_from_snapshot(snapshot.closed_tabs);
                        let restored = self
                            .as_mut()
                            .restore_session_from_entries(snapshot.entries, false);
                        if restored.is_empty() {
                            self.as_mut()
                                .set_status_text(QString::from("Recovery could not be applied"));
                        } else {
                            self.as_mut()
                                .set_session_restore_values(QString::from(format!(
                                    "replace\n{restored}"
                                )));
                        }
                    }
                    Ok(_) => self
                        .as_mut()
                        .set_status_text(QString::from("Recovery session is empty")),
                    Err(error) => self
                        .as_mut()
                        .set_status_text(QString::from(format!("Recovery rejected: {error}"))),
                }
            } else if let Some(append) = restore_load_append {
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
                            .set_status_text(QString::from("Saved session is empty"));
                    }
                    Err(error) => {
                        self.as_mut().set_status_text(QString::from(format!(
                            "Session preview rejected: {error}"
                        )));
                    }
                }
            }
        }
        if let Some(result) = session_result {
            consumed = true;
            match result {
                Ok(names) => {
                    let mut rust = self.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    this.session_names = names;
                    this.session_names_dirty.set(false);
                }
                Err(error) => {
                    self.as_mut().set_status_text(QString::from(format!(
                        "Session catalog query failed: {error}"
                    )));
                }
            }
        }
        if let Some(result) = library_result {
            consumed = true;
            match result {
                Ok(snapshot) => {
                    let mut rust = self.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    this.storage_library = Some(snapshot);
                    this.storage_library_revision = this.storage_library_revision.saturating_add(1);
                    this.storage_library_dirty.set(false);
                }
                Err(error) => {
                    {
                        let mut rust = self.as_mut().rust_mut();
                        let this = rust.as_mut().get_mut();
                        this.storage_library = None;
                        this.storage_library_dirty.set(false);
                    }
                    self.as_mut().close_storage();
                    self.as_mut().set_status_text(QString::from(format!(
                        "Profile metadata query failed: {error}"
                    )));
                }
            }
        }
        self.as_mut().request_switcher_library_index();
        if let Some(result) = journey_node_result {
            consumed = true;
            let pending = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_journey_reopen
                .take();
            match (pending, result) {
                (Some((node_id, target_kind, current_tab, profile_id)), Ok(Some(node))) => {
                    if node.id != node_id || node.profile_id != profile_id {
                        self.as_mut().set_status_text(QString::from(
                            "Journey node is stale or belongs to another profile",
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
                (Some(_), Ok(None)) => self
                    .as_mut()
                    .set_status_text(QString::from("Journey node is stale or unavailable")),
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
            match result {
                Ok(snapshot) => {
                    self.as_mut().apply_journey_query_snapshot(snapshot);
                    self.as_mut()
                        .set_status_text(QString::from("Journey query ready"));
                }
                Err(error) => self
                    .as_mut()
                    .set_status_text(QString::from(format!("Journey query failed: {error}"))),
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
                    self.as_ref().rust().storage_library_dirty.set(true);
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
        if let Some(result) = mark_result {
            consumed = true;
            match result {
                Ok(_) => {
                    self.as_ref().rust().storage_library_dirty.set(true);
                    self.as_mut()
                        .set_status_text(QString::from("Bookmark/quickmark write committed"));
                }
                Err(error) => {
                    self.as_mut().set_status_text(QString::from(format!(
                        "Bookmark/quickmark worker write failed: {error}"
                    )));
                }
            }
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
                                    .journey_durable_ids
                                    .borrow()
                                    .iter()
                                    .filter(|(node, _)| state.journey().node(**node).is_some())
                                    .map(|(node, durable)| (*node, durable.clone()))
                                    .collect();
                                this.journey_durable_ids.replace(retained);
                            }
                        }
                    }
                    self.as_ref().rust().storage_library_dirty.set(true);
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
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .download_destination_pending = false;
            match result {
                Ok(()) => {
                    self.as_ref().rust().storage_library_dirty.set(true);
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
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .download_create_pending = false;
            match result {
                Ok(()) => {
                    self.as_ref().rust().storage_library_dirty.set(true);
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
                    .pending_journey_mappings
                    .remove(&ticket)
            });
            match result {
                Ok(true) => {
                    if let Some((core_id, durable_id)) = mapping {
                        self.as_ref()
                            .rust()
                            .journey_durable_ids
                            .borrow_mut()
                            .insert(core_id, durable_id);
                    }
                    self.as_ref().rust().storage_library_dirty.set(true);
                    let mut rust = self.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    this.journey_export_payload = None;
                    this.journey_export_preview_text = QString::default();
                }
                Ok(false) => {
                    self.as_mut().set_status_text(QString::from(
                        "Journey worker did not find the requested current node",
                    ));
                }
                Err(error) => {
                    self.as_mut().set_status_text(QString::from(format!(
                        "Journey worker write failed: {error}"
                    )));
                }
            }
        }
        self.as_mut().flush_history_writes();
        self.as_mut().flush_permission_writes();
        self.as_mut().flush_mark_writes();
        self.as_mut().flush_download_writes();
        consumed
    }

    pub(super) fn apply_mark_write(
        mut self: Pin<&mut Self>,
        write: MarkWrite,
    ) -> Result<(), String> {
        if !self.as_ref().storage_is_open() {
            return Err("profile metadata worker is unavailable".to_owned());
        }
        self.as_mut()
            .submit_storage(StorageRequest::MarkBatch {
                writes: vec![write],
            })
            .map_err(|error| error.to_string())?;
        self.as_ref().rust().storage_library_dirty.set(true);
        self.set_status_text(QString::from("Bookmark/quickmark write queued"));
        Ok(())
    }

    pub(super) fn queue_bookmark_transfer(
        mut self: Pin<&mut Self>,
        url: &QString,
        title: &QString,
    ) -> bool {
        if self.as_ref().active_profile_is_transient() {
            self.as_mut().set_status_text(QString::from(
                "Private and ephemeral profiles cannot receive durable bookmarks",
            ));
            return false;
        }
        let url = url.to_string();
        if !is_safe_history_url(&url) {
            self.as_mut()
                .set_status_text(QString::from("Bookmark transfer URL is not eligible"));
            return false;
        }
        let title = sanitize_untrusted_title(&title.to_string());
        if self
            .as_mut()
            .apply_mark_write(MarkWrite::AddBookmark {
                id: Uuid::new_v4().to_string(),
                url,
                title,
                timestamp: unix_timestamp(),
            })
            .is_err()
        {
            return false;
        }
        true
    }

    pub(super) fn clear_history_via_worker(
        mut self: Pin<&mut Self>,
        since: Option<i64>,
        origin: Option<String>,
    ) -> Result<(), String> {
        let result = Some(self.as_mut().submit_storage(StorageRequest::HistoryClear {
            since,
            origin: origin.clone(),
        }));
        match result {
            Some(Ok(())) => {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_history_clear = Some((since, origin.clone()));
                self.as_ref().rust().storage_library_dirty.set(true);
                self.as_mut()
                    .set_status_text(QString::from("History clear queued"));
                Ok(())
            }
            Some(Err(
                ferric_browser_storage::StorageWorkerError::Busy
                | ferric_browser_storage::StorageWorkerError::QueueFull,
            )) => Err("profile metadata worker is busy; retry history clear".into()),
            Some(Err(error)) => Err(error.to_string()),
            None => Err("profile metadata worker is unavailable".to_owned()),
        }
    }

    pub(super) fn set_download_destination_via_worker(
        mut self: Pin<&mut Self>,
        id: String,
        destination: String,
    ) -> Result<(), String> {
        let result = Some(
            self.as_mut()
                .submit_storage(StorageRequest::DownloadDestination {
                    id: id.clone(),
                    destination: destination.clone(),
                }),
        );
        match result {
            Some(Ok(())) => {
                self.as_ref().rust().storage_library_dirty.set(true);
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .download_destination_pending = true;
                self.as_mut()
                    .set_status_text(QString::from("Download destination update queued"));
                Ok(())
            }
            Some(Err(
                ferric_browser_storage::StorageWorkerError::Busy
                | ferric_browser_storage::StorageWorkerError::QueueFull,
            )) => Err("profile metadata worker is busy; retry download destination".into()),
            Some(Err(error)) => Err(error.to_string()),
            None => Err("profile metadata worker is unavailable".to_owned()),
        }
    }

    pub(super) fn create_download_via_worker(
        mut self: Pin<&mut Self>,
        id: String,
        source_url: String,
        destination: String,
        state: DownloadState,
        created_at: i64,
    ) -> Result<(), String> {
        let result = Some(
            self.as_mut()
                .submit_storage(StorageRequest::DownloadCreate {
                    id: id.clone(),
                    source_url: source_url.clone(),
                    destination: destination.clone(),
                    state,
                    created_at,
                }),
        );
        match result {
            Some(Ok(())) => {
                self.as_ref().rust().storage_library_dirty.set(true);
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .download_create_pending = true;
                self.as_mut()
                    .set_status_text(QString::from("Download index creation queued"));
                Ok(())
            }
            Some(Err(
                ferric_browser_storage::StorageWorkerError::Busy
                | ferric_browser_storage::StorageWorkerError::QueueFull,
            )) => Err("profile metadata worker is busy; retry download index creation".into()),
            Some(Err(error)) => Err(error.to_string()),
            None => Err("profile metadata worker is unavailable".to_owned()),
        }
    }

    pub(super) fn apply_journey_write(
        mut self: Pin<&mut Self>,
        write: JourneyWrite,
    ) -> Result<StorageTicket, String> {
        match self
            .as_mut()
            .submit_storage_with_ticket(StorageRequest::JourneyWrite { write })
        {
            Ok(ticket) => {
                self.as_ref().rust().storage_library_dirty.set(true);
                self.as_mut()
                    .set_status_text(QString::from("Journey write queued"));
                Ok(ticket)
            }
            Err(error) => Err(error.to_string()),
        }
    }

    pub(super) fn apply_history_batch_result(
        mut self: Pin<&mut Self>,
        result: Result<usize, String>,
    ) {
        match result {
            Ok(_) => {
                self.as_ref().rust().storage_library_dirty.set(true);
            }
            Err(error) => {
                self.as_mut().set_status_text(QString::from(format!(
                    "History worker write failed: {error}"
                )));
            }
        }
    }

    pub(super) fn apply_permission_batch_result(
        mut self: Pin<&mut Self>,
        result: Result<usize, String>,
    ) {
        match result {
            Ok(_) => {
                self.as_ref().rust().storage_library_dirty.set(true);
            }
            Err(error) => {
                self.as_mut().set_status_text(QString::from(format!(
                    "Permission worker write failed: {error}"
                )));
            }
        }
    }

    pub(super) fn apply_download_batch_result(
        mut self: Pin<&mut Self>,
        result: Result<usize, String>,
    ) {
        match result {
            Ok(_) => {
                self.as_ref().rust().storage_library_dirty.set(true);
            }
            Err(error) => {
                self.as_mut().set_status_text(QString::from(format!(
                    "Download worker write failed: {error}"
                )));
            }
        }
    }
}
