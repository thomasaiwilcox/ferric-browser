use super::{
    CatalogReadState, CxxQtType, FeedbackLane, FeedbackState, JourneyWrite, MarkWrite, Pin,
    ProfileLibrarySnapshot, QString, StorageRequest, StorageTicket, Uuid, is_safe_history_url,
    qobject, safe_ipc_url, sanitize_untrusted_title, unix_timestamp,
};

impl qobject::BrowserUi {
    pub(super) fn queue_command_history_write(mut self: Pin<&mut Self>, command: String) {
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.queued_command_history.len() == 1_000 {
                this.queued_command_history.remove(0);
            }
            this.queued_command_history
                .push((command, unix_timestamp()));
        }
        self.as_mut().flush_command_history_writes();
    }

    pub(super) fn flush_command_history_writes(mut self: Pin<&mut Self>) {
        let next = {
            let binding = self.as_ref();
            let rust = binding.rust();
            if rust.command_history_write_pending {
                return;
            }
            rust.queued_command_history.first().cloned()
        };
        let Some((command, timestamp)) = next else {
            return;
        };
        if self
            .as_mut()
            .submit_storage(StorageRequest::CommandRecord { command, timestamp })
            .is_ok()
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.queued_command_history.remove(0);
            this.command_history_write_pending = true;
        }
    }

    pub(super) fn apply_mark_write(self: Pin<&mut Self>, write: MarkWrite) -> Result<(), String> {
        self.apply_mark_write_with_feedback(write, FeedbackLane::Mark)
    }

    fn apply_mark_write_with_feedback(
        mut self: Pin<&mut Self>,
        write: MarkWrite,
        lane: FeedbackLane,
    ) -> Result<(), String> {
        if !self.as_ref().storage_is_open() {
            return Err("profile metadata worker is unavailable".to_owned());
        }
        let ticket = self
            .as_mut()
            .submit_storage_with_ticket(StorageRequest::MarkBatch {
                writes: vec![write],
            })
            .map_err(|error| error.to_string())?;
        self.as_mut().publish_management_feedback(
            lane,
            FeedbackState::Pending,
            "Saving library change…",
            "",
        );
        let id = if matches!(lane, FeedbackLane::BookmarkTransfer) {
            self.as_ref().rust().bookmark_transfer_id.clone()
        } else {
            self.as_ref().rust().mark_action_id.clone()
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .library
            .mark_feedback_tickets
            .insert(ticket, (lane, id));
        self.set_status_text(QString::from("Saving library change…"));
        Ok(())
    }

    pub(super) fn publish_library_pending(self: Pin<&mut Self>) {
        let pending = self
            .as_ref()
            .rust()
            .library
            .storage_library_read_state
            .is_pending()
            || self.as_ref().rust().journey_read_pending;
        self.set_library_pending(pending);
    }

    pub(super) fn refresh_library(mut self: Pin<&mut Self>) {
        let private_history = self.as_ref().rust().library_kind.to_string() == "history"
            && self.as_ref().active_profile_is_transient();
        if private_history {
            let rows = self
                .as_ref()
                .get_ref()
                .saved_library_rows("history")
                .unwrap_or_default();
            self.as_mut().set_library_error(QString::default());
            self.set_library_values(QString::from(rows.join("\n")));
        } else {
            self.as_ref().rust().library.storage_library_dirty.set(true);
            self.request_storage_library();
        }
    }

    pub(super) fn apply_mark_write_result(
        mut self: Pin<&mut Self>,
        ticket: StorageTicket,
        result: Result<usize, String>,
    ) {
        let receipt = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .library
            .mark_feedback_tickets
            .remove(&ticket);
        let Some((lane, id)) = receipt else {
            return;
        };
        let current_receipt = if matches!(lane, FeedbackLane::BookmarkTransfer) {
            self.as_ref().rust().bookmark_transfer_id == id
        } else {
            self.as_ref().rust().mark_action_id == id
        };
        if result.is_ok() {
            self.as_ref().rust().library.storage_library_dirty.set(true);
            let pending = self
                .as_ref()
                .rust()
                .library
                .storage_library_read_state
                .is_pending();
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .library
                .storage_library_refresh_again |= pending;
            self.as_mut().request_storage_library();
            self.as_mut().refresh_start_page_metadata();
        }
        if !current_receipt {
            return;
        }
        match result {
            Ok(_) => {
                self.as_mut().publish_management_feedback(
                    lane,
                    FeedbackState::Succeeded,
                    "Library change saved",
                    "",
                );
                self.set_status_text(QString::from("Library change saved"));
            }
            Err(error) => {
                self.as_mut().publish_management_feedback(
                    lane,
                    FeedbackState::Failed,
                    "This change couldn't be saved. Review the details and try again.",
                    &error,
                );
                self.set_status_text(QString::from(
                    "The library change couldn't be saved. Review its details and try again.",
                ));
            }
        }
    }

    pub(super) fn apply_library_read_result(
        mut self: Pin<&mut Self>,
        result: Result<ProfileLibrarySnapshot, String>,
    ) {
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .library
            .storage_library_read_state = match &result {
            Ok(_) => CatalogReadState::Ready,
            Err(error) => CatalogReadState::Failed(error.clone()),
        };
        self.as_mut().publish_library_pending();
        if self.as_ref().rust().library.storage_library_refresh_again {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .library
                .storage_library_refresh_again = false;
            self.as_ref().rust().library.storage_library_dirty.set(true);
            self.request_storage_library();
            return;
        }
        match result {
            Ok(snapshot) => {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                for entry in &snapshot.command_history {
                    if !this.command_history.contains(entry) {
                        this.command_history.push(entry.clone());
                    }
                }
                let limit = this
                    .config
                    .get("history")
                    .and_then(|history| history.get("command_limit"))
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(1000)
                    .min(1000) as usize;
                this.command_history.truncate(limit);
                this.library.storage_library = Some(snapshot);
                this.storage_library_revision = this.storage_library_revision.saturating_add(1);
                this.library.storage_library_dirty.set(false);
                drop(rust);
                self.as_mut().set_library_error(QString::default());
                let kind = self.as_ref().rust().library_kind.to_string();
                if let Some(rows) = self.as_ref().get_ref().saved_library_rows(&kind) {
                    self.set_library_values(QString::from(rows.join("\n")));
                }
            }
            Err(error) => {
                // Keep the last readable rows and allow an explicit retry.
                self.as_mut().set_library_error(QString::from(&error));
                self.as_ref().rust().library.storage_library_dirty.set(true);
                self.set_status_text(QString::from(
                    "Saved items couldn't be loaded. Refresh the library to try again.",
                ));
            }
        }
    }

    pub(super) fn saved_library_rows(&self, kind: &str) -> Option<Vec<String>> {
        let rust = self.rust();
        if kind == "history" && self.active_profile_is_transient() {
            return Some(
                self.private_history_records()
                    .iter()
                    .map(|page| {
                        format!(
                            "{}\t{}\t{}\t{}",
                            page.id,
                            page.title.replace(['\t', '\n', '\r'], " "),
                            safe_ipc_url(&page.url),
                            page.last_visit
                        )
                    })
                    .collect(),
            );
        }
        let library = rust.library.storage_library.as_ref()?;
        match kind {
            "bookmarks" => Some(
                library
                    .bookmarks
                    .iter()
                    .map(|mark| {
                        format!(
                            "{}\t{}\t{}",
                            mark.id,
                            mark.title.replace(['\t', '\n', '\r'], " "),
                            safe_ipc_url(&mark.url)
                        )
                    })
                    .collect(),
            ),
            "quickmarks" => Some(
                library
                    .quickmarks
                    .iter()
                    .map(|mark| format!("{}\t{}", mark.name, safe_ipc_url(&mark.url)))
                    .collect(),
            ),
            "history" => Some(
                library
                    .history
                    .iter()
                    .map(|page| {
                        format!(
                            "{}\t{}\t{}\t{}",
                            page.id,
                            page.title.replace(['\t', '\n', '\r'], " "),
                            safe_ipc_url(&page.url),
                            page.last_visit
                        )
                    })
                    .collect(),
            ),
            _ => None,
        }
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
        if self.as_ref().rust().bookmark_transfer_state.to_string() == "pending" {
            self.set_status_text(QString::from(
                "Another bookmark is being saved. Wait for its result.",
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
        if let Err(error) = self.as_mut().apply_mark_write_with_feedback(
            MarkWrite::AddBookmark {
                id: Uuid::new_v4().to_string(),
                url,
                title,
                timestamp: unix_timestamp(),
            },
            FeedbackLane::BookmarkTransfer,
        ) {
            self.as_mut().publish_management_feedback(
                FeedbackLane::BookmarkTransfer,
                FeedbackState::Failed,
                "The bookmark couldn't be saved. Restore profile storage and retry.",
                &error,
            );
            self.set_status_text(QString::from("The bookmark couldn't be saved"));
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
                self.as_ref().rust().library.storage_library_dirty.set(true);
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

    pub(super) fn apply_journey_write(
        mut self: Pin<&mut Self>,
        write: JourneyWrite,
    ) -> Result<StorageTicket, String> {
        match self
            .as_mut()
            .submit_storage_with_ticket(StorageRequest::JourneyWrite { write })
        {
            Ok(ticket) => {
                self.as_ref().rust().library.storage_library_dirty.set(true);
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
                self.as_ref().rust().library.storage_library_dirty.set(true);
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
                self.as_ref().rust().library.storage_library_dirty.set(true);
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
                self.as_ref().rust().library.storage_library_dirty.set(true);
            }
            Err(error) => {
                self.as_mut().set_status_text(QString::from(format!(
                    "Download worker write failed: {error}"
                )));
            }
        }
    }
}
