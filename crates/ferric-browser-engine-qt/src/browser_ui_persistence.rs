use super::{
    BTreeMap, CxxQtType, DownloadUpdate, Duration, Event, HistoryRecord, Instant, JourneyNodeId,
    JourneyWrite, MAX_PRIVATE_HISTORY, PermissionRule, Pin, QString, StorageRequest, TabId, Uuid,
    Value, VisitInput, canonical_origin, is_safe_history_url, publish_ipc_event, qobject, thread,
    unix_timestamp,
};

impl qobject::BrowserUi {
    pub(super) fn queue_download_update(mut self: Pin<&mut Self>, update: DownloadUpdate) -> bool {
        let event_payload = serde_json::json!({
            "download_id": update.id.clone(),
            "state": update.state.as_str(),
            "bytes_received": update.bytes_received
        });
        let accepted = match self.as_mut().submit_storage(StorageRequest::DownloadBatch {
            updates: vec![update],
        }) {
            Ok(()) => {
                self.as_ref().rust().storage_library_dirty.set(true);
                true
            }
            Err(error) => {
                self.as_mut().set_status_text(QString::from(format!(
                    "Download update could not be scheduled: {error}"
                )));
                false
            }
        };
        if accepted {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            publish_ipc_event(&mut this.ipc_sequence, "download.changed", event_payload);
        }
        accepted
    }

    pub(super) fn flush_download_writes(self: Pin<&mut Self>) {
        let _ = self;
    }

    pub(super) fn flush_mark_writes(self: Pin<&mut Self>) {
        let _ = self;
    }

    pub(super) fn queue_permission_writes(
        mut self: Pin<&mut Self>,
        rules: Vec<PermissionRule>,
    ) -> bool {
        if rules.is_empty() {
            self.set_status_text(QString::from("Permission write batch is empty"));
            return false;
        }
        match self
            .as_mut()
            .submit_storage(StorageRequest::PermissionBatch { rules })
        {
            Ok(()) => {
                self.as_ref().rust().storage_library_dirty.set(true);
                true
            }
            Err(error) => {
                self.set_status_text(QString::from(format!(
                    "Permission write could not be scheduled: {error}"
                )));
                false
            }
        }
    }

    pub(super) fn flush_permission_writes(self: Pin<&mut Self>) {
        let _ = self;
    }

    pub(super) fn queue_history_visit(mut self: Pin<&mut Self>, visit: VisitInput) {
        match self.as_mut().submit_storage(StorageRequest::HistoryBatch {
            visits: vec![visit],
        }) {
            Ok(()) => self.as_ref().rust().storage_library_dirty.set(true),
            Err(error) => self.set_status_text(QString::from(format!(
                "History write could not be scheduled: {error}"
            ))),
        }
    }

    pub(super) fn flush_history_writes(self: Pin<&mut Self>) {
        let _ = self;
    }

    pub(super) fn wait_for_history_writes(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().poll_storage_library();
        self.as_ref().storage_is_open()
    }

    pub(super) fn wait_for_permission_writes(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().poll_storage_library();
        self.as_ref().storage_is_open()
    }

    pub(super) fn wait_for_download_writes(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().poll_storage_library();
        self.as_ref().storage_is_open()
    }

    pub(super) fn wait_for_mark_writes(mut self: Pin<&mut Self>) -> bool {
        self.as_mut().poll_storage_library();
        self.as_ref().storage_is_open()
    }

    pub(super) fn wait_for_history_clear(mut self: Pin<&mut Self>) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            self.as_mut().poll_storage_library();
            let pending = self.as_ref().rust().pending_history_clear.is_some();
            let journey_pending = !self.as_ref().rust().pending_journey_mappings.is_empty();
            if !pending && !journey_pending {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                self.set_status_text(QString::from(
                    "History clear did not finish before shutdown",
                ));
                return false;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub(super) fn wait_for_download_destination(mut self: Pin<&mut Self>) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            self.as_mut().poll_storage_library();
            let pending = self.as_ref().rust().download_destination_pending;
            if !pending {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                self.set_status_text(QString::from(
                    "Download destination update did not finish before shutdown",
                ));
                return false;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub(super) fn wait_for_download_create(mut self: Pin<&mut Self>) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            self.as_mut().poll_storage_library();
            let pending = self.as_ref().rust().download_create_pending;
            if !pending {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                self.set_status_text(QString::from(
                    "Download index creation did not finish before shutdown",
                ));
                return false;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub(super) fn wait_for_journey_write(mut self: Pin<&mut Self>) -> bool {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            self.as_mut().poll_storage_library();
            if self.as_ref().rust().pending_journey_mappings.is_empty() {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                self.set_status_text(QString::from(
                    "Journey write did not finish before shutdown",
                ));
                return false;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub(super) fn private_history_records(&self) -> Vec<HistoryRecord> {
        self.rust().private_history.clone()
    }

    pub(super) fn upsert_private_history(
        records: &mut Vec<HistoryRecord>,
        next_id: &mut i64,
        url: &str,
        title: &str,
        timestamp: i64,
    ) {
        if let Some(record) = records.iter_mut().find(|record| record.url == url) {
            record.title = title.to_owned();
            record.visit_count = record.visit_count.saturating_add(1);
            record.last_visit = timestamp;
        } else {
            let id = *next_id;
            *next_id = next_id.saturating_sub(1);
            records.push(HistoryRecord {
                id,
                url: url.to_owned(),
                title: title.to_owned(),
                visit_count: 1,
                last_visit: timestamp,
            });
        }
        records.sort_by(|left, right| {
            right
                .last_visit
                .cmp(&left.last_visit)
                .then_with(|| right.id.cmp(&left.id))
        });
        records.truncate(MAX_PRIVATE_HISTORY);
    }

    pub(super) fn record_private_history_visit(
        mut self: Pin<&mut Self>,
        url: &str,
        title: &str,
        timestamp: i64,
    ) {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        Self::upsert_private_history(
            &mut this.private_history,
            &mut this.private_history_next_id,
            url,
            title,
            timestamp,
        );
    }

    pub(super) fn clear_private_history_records(
        records: &mut Vec<HistoryRecord>,
        since: Option<i64>,
        origin: Option<&str>,
    ) -> usize {
        let before = records.len();
        records.retain(|record| {
            let matches_since = since.is_none_or(|cutoff| record.last_visit < cutoff);
            let matches_origin = origin.is_none_or(|candidate| {
                canonical_origin(&record.url).as_deref() == Some(candidate)
            });
            !(matches_since && matches_origin)
        });
        before.saturating_sub(records.len())
    }

    pub(super) fn clear_private_history(
        mut self: Pin<&mut Self>,
        since: Option<i64>,
        origin: Option<&str>,
    ) -> usize {
        let deleted = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            Self::clear_private_history_records(&mut this.private_history, since, origin)
        };
        let cutoff = since.and_then(|value| u64::try_from(value).ok());
        let _ = self.as_mut().reduce_event(Event::ClearJourney {
            since: cutoff,
            origin: origin.map(str::to_owned),
        });
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .journey_durable_ids
            .replace(BTreeMap::new());
        deleted
    }

    pub(super) fn record_committed_visit(
        mut self: Pin<&mut Self>,
        tab: TabId,
        url: &str,
        title: &str,
        was_traversal: bool,
        journey_parent: Option<JourneyNodeId>,
    ) {
        if !is_safe_history_url(url) {
            return;
        }
        if self
            .as_ref()
            .rust()
            .profile_persistence
            .lacks_durable_storage()
            && self.as_ref().active_profile_is_transient()
        {
            self.as_mut()
                .record_private_history_visit(url, title, unix_timestamp());
            return;
        }
        let (retention_days, profile_id, journey_write) = {
            let binding = self.as_ref();
            let rust = binding.rust();
            let retention_days = rust
                .config
                .get("history")
                .and_then(|history| history.get("retention_days"))
                .and_then(Value::as_u64)
                .and_then(|days| u32::try_from(days).ok())
                .unwrap_or(90);
            if retention_days == 0 {
                return;
            }
            let Some(profile_id) = rust.profile_id else {
                return;
            };
            if was_traversal {
                let write = if let Some(state) = rust.state.as_ref()
                    && let Some(core_node_id) = state.journey().current_node(tab)
                    && let Some(durable_node_id) = rust
                        .journey_durable_ids
                        .borrow()
                        .get(&core_node_id)
                        .cloned()
                {
                    JourneyWrite::SetCurrentById {
                        tab_id: tab.to_string(),
                        node_id: durable_node_id,
                    }
                } else {
                    JourneyWrite::SetCurrentForUrl {
                        tab_id: tab.to_string(),
                        url: url.to_owned(),
                    }
                };
                (retention_days, profile_id, Some((write, None)))
            } else if let Some(state) = rust.state.as_ref()
                && let Some(node_id) = state.journey().current_node(tab)
                && let Some(node) = state.journey().node(node_id)
            {
                let timestamp = unix_timestamp();
                let durable_node_id = Uuid::new_v4().to_string();
                let durable_parent_id = journey_parent
                    .and_then(|parent| rust.journey_durable_ids.borrow().get(&parent).cloned());
                let write = JourneyWrite::RecordNode {
                    id: durable_node_id.clone(),
                    profile_id: profile_id.to_string(),
                    tab_id: tab.to_string(),
                    url: node.url.clone(),
                    title: node.title.clone(),
                    committed_at: timestamp,
                    transition: node.transition.name().into(),
                    source: node.source.clone(),
                    retention_before: Some(
                        timestamp
                            .saturating_sub(i64::from(retention_days).saturating_mul(24 * 60 * 60)),
                    ),
                    parent_id: durable_parent_id,
                };
                (
                    retention_days,
                    profile_id,
                    Some((write, Some((node.id, durable_node_id)))),
                )
            } else {
                (retention_days, profile_id, None)
            }
        };
        let _ = (retention_days, profile_id);
        if let Some((write, mapping)) = journey_write {
            match self.as_mut().apply_journey_write(write) {
                Ok(ticket) => {
                    if let Some(mapping) = mapping {
                        self.as_mut()
                            .rust_mut()
                            .as_mut()
                            .get_mut()
                            .pending_journey_mappings
                            .insert(ticket, mapping);
                    }
                }
                Err(error) => {
                    self.as_mut().set_status_text(QString::from(format!(
                        "Journey write could not be scheduled; history retained: {error}"
                    )));
                }
            }
        }
        if !was_traversal {
            let timestamp = unix_timestamp();
            self.as_mut().queue_history_visit(VisitInput {
                url: url.to_owned(),
                title: title.to_owned(),
                transition: "committed".into(),
                timestamp,
                retention_before: Some(
                    timestamp
                        .saturating_sub(i64::from(retention_days).saturating_mul(24 * 60 * 60)),
                ),
            });
        }
    }

    pub(super) fn record_same_document_visit(mut self: Pin<&mut Self>, tab: TabId, url: &str) {
        if !is_safe_history_url(url) {
            return;
        }
        let (title, retention_days, durable) = {
            let pinned = self.as_ref();
            let rust = pinned.rust();
            let title = rust
                .state
                .as_ref()
                .and_then(|state| state.tabs().get(&tab))
                .map(|tab| tab.title.clone())
                .unwrap_or_default();
            let retention_days = rust
                .config
                .get("history")
                .and_then(|history| history.get("retention_days"))
                .and_then(Value::as_u64)
                .and_then(|days| u32::try_from(days).ok())
                .unwrap_or(90);
            (
                title,
                retention_days,
                rust.profile_persistence.is_durable() && rust.profile_id.is_some(),
            )
        };
        if retention_days == 0 {
            return;
        }
        let timestamp = unix_timestamp();
        if !durable && self.as_ref().active_profile_is_transient() {
            self.as_mut()
                .record_private_history_visit(url, &title, timestamp);
            return;
        }
        self.as_mut().queue_history_visit(VisitInput {
            url: url.to_owned(),
            title,
            transition: "same-document".into(),
            timestamp,
            retention_before: Some(
                timestamp.saturating_sub(i64::from(retention_days).saturating_mul(24 * 60 * 60)),
            ),
        });
    }
}
