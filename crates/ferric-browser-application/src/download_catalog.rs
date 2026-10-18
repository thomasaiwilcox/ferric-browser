//! Native-session download state. Saved history is a projection, never live authority.

use ferric_browser_storage::{DownloadRecord, DownloadState};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DownloadSessionError {
    ProfileUnavailable,
    Unavailable,
    InvalidDestination,
    DestinationAlreadySelected,
    Capacity,
    AlreadyExists,
    UnknownDownload,
    WrongOwner,
    InvalidProgress,
    FileNotFinalized,
    RevisionExhausted,
    RetryPending,
}

impl std::fmt::Display for DownloadSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::ProfileUnavailable => "The window profile is not ready.",
            Self::Unavailable => "Downloads are unavailable. Reopen this window and try again.",
            Self::InvalidDestination => "Choose a valid absolute download destination.",
            Self::DestinationAlreadySelected => "This download already has a save location.",
            Self::Capacity => {
                "Downloads is full. Finish a transfer or refresh its history, then try again."
            }
            Self::AlreadyExists => "This download has already been offered.",
            Self::UnknownDownload => "This download is no longer available.",
            Self::WrongOwner => "This window does not own that download request.",
            Self::InvalidProgress => "The download reported invalid progress.",
            Self::FileNotFinalized => "The downloaded file is still being saved.",
            Self::RetryPending => {
                "This download is already being retried. Check its progress in Downloads."
            }
            Self::RevisionExhausted => "Download tracking cannot continue. Reopen this window.",
        })
    }
}
impl std::error::Error for DownloadSessionError {}

#[derive(Clone, Debug)]
pub(crate) struct SessionDownload {
    record: DownloadRecord,
    display_name: String,
    owner: Uuid,
    finished: bool,
    transport_finished: bool,
    finalized: bool,
    total_bytes: i64,
    failure: Option<DownloadFailure>,
    revision: u64,
    saved_revision: u64,
}

/// Retry preparation is distinct from an admitted native transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DownloadRetryState {
    Unavailable,
    Idle,
    Preparing,
    Running,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadRow {
    pub record: DownloadRecord,
    pub display_name: String,
    pub active: bool,
    pub finishing: bool,
    pub file_ready: bool,
    pub retry: DownloadRetryState,
    pub total_bytes: i64,
    pub failure: Option<DownloadFailure>,
}

/// Session-only detail captured before a native request is released.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DownloadFailure {
    Native { code: i32, message: String },
    Save(DownloadSaveFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DownloadSaveFailure {
    AlreadyExists,
    DestinationChanged,
    PermissionDenied,
    StorageFull,
    SourceUnavailable,
    Unavailable,
}

impl DownloadFailure {
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Native { message, .. } => message,
            Self::Save(kind) => match kind {
                DownloadSaveFailure::AlreadyExists => {
                    "A file already exists at this location. Retry to save another copy."
                }
                DownloadSaveFailure::DestinationChanged => {
                    "The save folder changed. Choose a save location and retry the download."
                }
                DownloadSaveFailure::PermissionDenied => {
                    "The save folder isn't writable. Choose another folder and retry the download."
                }
                DownloadSaveFailure::StorageFull => {
                    "There isn't enough space to save this file. Free some space or choose another folder, then retry."
                }
                DownloadSaveFailure::SourceUnavailable => {
                    "The downloaded data is no longer available. Retry the download."
                }
                DownloadSaveFailure::Unavailable => {
                    "The file couldn't be saved. Check the save folder's permissions and free space, then retry."
                }
            },
        }
    }
}

struct Retry {
    token: Uuid,
    owner: Uuid,
    download: Option<String>,
}

#[derive(Default)]
pub(crate) struct DownloadCatalog {
    entries: BTreeMap<String, SessionDownload>,
    retries: BTreeMap<String, Retry>,
    durable_history: bool,
    pending_write: bool,
    write_cursor: String,
    history_failed: bool,
}

impl DownloadCatalog {
    // Active requests are never evicted. Terminal session history is bounded.
    pub(crate) const CAPACITY: usize = 1_000;

    pub(crate) fn durable() -> Self {
        Self {
            durable_history: true,
            ..Self::default()
        }
    }

    pub(crate) fn offer(
        &mut self,
        record: DownloadRecord,
        display_name: String,
        owner: Uuid,
    ) -> Result<(), DownloadSessionError> {
        if self.entries.contains_key(&record.id) {
            return Err(DownloadSessionError::AlreadyExists);
        }
        if self.entries.len() >= Self::CAPACITY {
            let oldest = self
                .entries
                .iter()
                .filter(|(_, entry)| {
                    entry.finished
                        && (!self.durable_history || entry.saved_revision == entry.revision)
                })
                .min_by_key(|(_, entry)| entry.record.created_at)
                .map(|(id, _)| id.clone());
            let Some(oldest) = oldest else {
                return Err(DownloadSessionError::Capacity);
            };
            self.entries.remove(&oldest);
        }
        self.entries.insert(
            record.id.clone(),
            SessionDownload {
                record,
                display_name,
                owner,
                finished: false,
                transport_finished: false,
                finalized: false,
                total_bytes: -1,
                failure: None,
                revision: 1,
                saved_revision: 0,
            },
        );
        Ok(())
    }

    fn owned(
        &mut self,
        id: &str,
        owner: Uuid,
    ) -> Result<&mut SessionDownload, DownloadSessionError> {
        let entry = self
            .entries
            .get_mut(id)
            .ok_or(DownloadSessionError::UnknownDownload)?;
        if entry.owner != owner {
            return Err(DownloadSessionError::WrongOwner);
        }
        Ok(entry)
    }

    pub(crate) fn owns_offer(&self, id: &str, owner: Uuid) -> bool {
        self.entries.get(id).is_some_and(|entry| {
            entry.owner == owner && !entry.finished && entry.record.state == DownloadState::Offered
        })
    }

    pub(crate) fn reserve_retry(
        &mut self,
        source: &str,
        token: Uuid,
        owner: Uuid,
    ) -> Result<(), DownloadSessionError> {
        let entries = &self.entries;
        self.retries.retain(|_, retry| {
            retry
                .download
                .as_ref()
                .is_none_or(|id| entries.get(id).is_some_and(|entry| !entry.finished))
        });
        if self.retries.contains_key(source) {
            return Err(DownloadSessionError::RetryPending);
        }
        if self.retries.len() >= Self::CAPACITY {
            return Err(DownloadSessionError::Capacity);
        }
        self.retries.insert(
            source.to_owned(),
            Retry {
                token,
                owner,
                download: None,
            },
        );
        Ok(())
    }

    pub(crate) fn bind_retry(
        &mut self,
        source: &str,
        token: Uuid,
        owner: Uuid,
        id: &str,
    ) -> Result<(), DownloadSessionError> {
        if !self.owns_offer(id, owner) {
            return Err(DownloadSessionError::WrongOwner);
        }
        let retry = self
            .retries
            .get_mut(source)
            .ok_or(DownloadSessionError::UnknownDownload)?;
        if retry.token != token || retry.owner != owner || retry.download.is_some() {
            return Err(DownloadSessionError::WrongOwner);
        }
        retry.download = Some(id.to_owned());
        Ok(())
    }

    pub(crate) fn release_retry(&mut self, source: &str, token: Uuid) {
        if self
            .retries
            .get(source)
            .is_some_and(|retry| retry.token == token && retry.download.is_none())
        {
            self.retries.remove(source);
        }
    }

    fn retry_state(&self, record: &DownloadRecord) -> DownloadRetryState {
        if !matches!(
            record.state,
            DownloadState::Cancelled | DownloadState::Interrupted
        ) || !ferric_browser_storage::is_safe_history_url(&record.source_url)
            || !(record.source_url.starts_with("http://")
                || record.source_url.starts_with("https://"))
        {
            return DownloadRetryState::Unavailable;
        }
        let source = record.id.as_str();
        match self
            .retries
            .get(source)
            .and_then(|retry| retry.download.as_deref())
        {
            Some(id) if self.entries.get(id).is_some_and(|entry| !entry.finished) => {
                DownloadRetryState::Running
            }
            None if self.retries.contains_key(source) => DownloadRetryState::Preparing,
            _ => DownloadRetryState::Idle,
        }
    }

    pub(crate) fn destination(
        &mut self,
        id: &str,
        owner: Uuid,
        destination: String,
        name: String,
    ) -> Result<(), DownloadSessionError> {
        let entry = self.owned(id, owner)?;
        if entry.finished || !entry.record.destination.is_empty() {
            return Err(DownloadSessionError::DestinationAlreadySelected);
        }
        let revision = entry
            .revision
            .checked_add(1)
            .ok_or(DownloadSessionError::RevisionExhausted)?;
        entry.record.destination = destination;
        entry.display_name = name;
        entry.record.state = DownloadState::InProgress;
        entry.revision = revision;
        Ok(())
    }

    // Qt completion means transport finished. The final file becomes usable
    // only after the filesystem finalizer has published it successfully.
    pub(crate) fn finalized(&mut self, id: &str, owner: Uuid) -> Result<(), DownloadSessionError> {
        let entry = self.owned(id, owner)?;
        if entry.record.state != DownloadState::InProgress
            || entry.finished
            || entry.finalized
            || !entry.transport_finished
            || entry.record.destination.is_empty()
        {
            return Err(DownloadSessionError::FileNotFinalized);
        }
        entry.finalized = true;
        Ok(())
    }

    pub(crate) fn write_current(&self, id: &str, owner: Uuid) -> bool {
        self.entries.get(id).is_some_and(|entry| {
            entry.owner == owner
                && !entry.finished
                && !entry.transport_finished
                && !entry.record.destination.is_empty()
                && matches!(
                    entry.record.state,
                    DownloadState::InProgress | DownloadState::Paused
                )
        })
    }

    pub(crate) fn finalization_current(
        &self,
        id: &str,
        owner: Uuid,
        destination: &std::path::Path,
    ) -> bool {
        self.entries.get(id).is_some_and(|entry| {
            entry.owner == owner
                && !entry.finished
                && !entry.finalized
                && entry.transport_finished
                && entry.record.state == DownloadState::InProgress
                && !entry.record.destination.is_empty()
                && std::path::Path::new(&entry.record.destination) == destination
        })
    }

    pub(crate) fn metrics(
        &mut self,
        id: &str,
        owner: Uuid,
        total: i64,
        failure: Option<DownloadFailure>,
    ) -> Result<(), DownloadSessionError> {
        if total < -1 {
            return Err(DownloadSessionError::InvalidProgress);
        }
        let entry = self.owned(id, owner)?;
        entry.total_bytes = total;
        if failure.is_some() {
            entry.failure = failure;
        }
        Ok(())
    }

    pub(crate) fn update(
        &mut self,
        id: &str,
        owner: Uuid,
        state: DownloadState,
        bytes: i64,
        finished: bool,
        completed_at: Option<i64>,
    ) -> Result<(), DownloadSessionError> {
        if bytes < 0
            || (finished
                && !matches!(
                    state,
                    DownloadState::Completed
                        | DownloadState::Interrupted
                        | DownloadState::Cancelled
                ))
        {
            return Err(DownloadSessionError::InvalidProgress);
        }
        let entry = self.owned(id, owner)?;
        if entry.finished {
            return Ok(());
        }
        if finished && state == DownloadState::Completed && !entry.finalized {
            return Err(DownloadSessionError::FileNotFinalized);
        }
        let revision = entry
            .revision
            .checked_add(1)
            .ok_or(DownloadSessionError::RevisionExhausted)?;
        entry.record.bytes_received = entry.record.bytes_received.max(bytes);
        entry.transport_finished |= state == DownloadState::Completed;
        // Preserve an honest in-progress row during final file publication.
        entry.record.state = if state == DownloadState::Completed && !finished {
            DownloadState::InProgress
        } else {
            state
        };
        entry.finished = finished;
        entry.record.completed_at = (finished && state == DownloadState::Completed)
            .then_some(completed_at)
            .flatten();
        entry.revision = revision;
        Ok(())
    }

    pub(crate) fn rows(&self, saved: &[DownloadRecord]) -> Vec<DownloadRow> {
        // A stale durable snapshot can never contribute rows to a private session.
        let saved = if self.durable_history { saved } else { &[] };
        let mut rows: BTreeMap<String, DownloadRow> = saved
            .iter()
            .map(|record| {
                let name = std::path::Path::new(&record.destination)
                    .file_name()
                    .map_or_else(
                        || "Download".to_owned(),
                        |name| name.to_string_lossy().into_owned(),
                    );
                let mut record = record.clone();
                if matches!(
                    record.state,
                    DownloadState::Offered
                        | DownloadState::SelectingDestination
                        | DownloadState::InProgress
                        | DownloadState::Paused
                ) {
                    record.state = DownloadState::Interrupted;
                }
                (
                    record.id.clone(),
                    DownloadRow {
                        record: record.clone(),
                        display_name: name,
                        active: false,
                        finishing: false,
                        file_ready: record.state == DownloadState::Completed,
                        retry: self.retry_state(&record),
                        total_bytes: -1,
                        failure: None,
                    },
                )
            })
            .collect();
        for (id, entry) in &self.entries {
            rows.insert(
                id.clone(),
                DownloadRow {
                    record: entry.record.clone(),
                    display_name: entry.display_name.clone(),
                    retry: self.retry_state(&entry.record),
                    total_bytes: entry.total_bytes,
                    failure: entry.failure.clone(),
                    active: !entry.finished,
                    finishing: entry.transport_finished && !entry.finished,
                    file_ready: entry.finished
                        && entry.finalized
                        && entry.record.state == DownloadState::Completed,
                },
            );
        }
        let mut rows: Vec<_> = rows.into_values().collect();
        rows.sort_by(|left, right| {
            right
                .record
                .created_at
                .cmp(&left.record.created_at)
                .then_with(|| left.record.id.cmp(&right.record.id))
        });
        rows
    }

    pub(crate) fn has_unsaved(&self) -> bool {
        self.durable_history
            && self
                .entries
                .values()
                .any(|entry| entry.revision > entry.saved_revision)
    }

    pub(crate) fn unsaved(&self) -> Vec<(u64, DownloadRecord)> {
        self.entries
            .values()
            .filter(|entry| self.durable_history && entry.revision > entry.saved_revision)
            .map(|entry| (entry.revision, entry.record.clone()))
            .collect()
    }

    pub(crate) fn begin_write(&mut self) -> Vec<(u64, DownloadRecord)> {
        if self.pending_write || self.history_failed {
            return Vec::new();
        }
        let mut records = self.unsaved();
        let next = records.partition_point(|(_, record)| record.id <= self.write_cursor);
        records.rotate_left(next);
        records.truncate(32);
        if let Some((_, record)) = records.last() {
            self.write_cursor.clone_from(&record.id);
        }
        self.pending_write = !records.is_empty();
        records
    }

    pub(crate) fn finish_write(&mut self, records: &[(u64, DownloadRecord)], success: bool) {
        self.pending_write = false;
        if success {
            for (revision, record) in records {
                self.acknowledge(&record.id, *revision);
            }
        } else {
            self.history_failed = true;
        }
    }

    pub(crate) fn release_write(&mut self) {
        self.pending_write = false;
    }
    pub(crate) fn retry_history(&mut self) {
        self.history_failed = false;
    }
    pub(crate) fn history_failed(&self) -> bool {
        self.history_failed
    }

    pub(crate) fn acknowledge(&mut self, id: &str, revision: u64) {
        if let Some(entry) = self.entries.get_mut(id)
            && revision <= entry.revision
        {
            // An older successful write cannot acknowledge newer progress.
            entry.saved_revision = entry.saved_revision.max(revision);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offered(id: &str, created_at: i64) -> DownloadRecord {
        DownloadRecord {
            id: id.into(),
            source_url: "https://owned-download.test/file".into(),
            destination: String::new(),
            state: DownloadState::Offered,
            bytes_received: 0,
            created_at,
            completed_at: None,
        }
    }
    fn catalog() -> DownloadCatalog {
        let mut catalog = DownloadCatalog::durable();
        catalog
            .offer(
                offered("owned", 1),
                "Report <final>.pdf".into(),
                Uuid::from_u128(1),
            )
            .unwrap();
        catalog
    }
    #[test]
    fn continuously_updated_first_batch_does_not_starve_other_downloads() {
        let mut catalog = DownloadCatalog::durable();
        for i in 0..64 {
            catalog
                .offer(
                    offered(&format!("{i:02}"), 1),
                    "file".into(),
                    Uuid::from_u128(1),
                )
                .unwrap();
        }
        let first = catalog.begin_write();
        assert_eq!(first.len(), 32);
        assert_eq!(
            catalog.begin_write(),
            [] as [(u64, ferric_browser_storage::DownloadRecord); 0]
        );
        catalog.finish_write(&first, true);
        for (_, record) in &first {
            catalog
                .update(
                    &record.id,
                    Uuid::from_u128(1),
                    DownloadState::InProgress,
                    128,
                    false,
                    None,
                )
                .unwrap();
        }
        let second = catalog.begin_write();
        assert_eq!(second.len(), 32);
        assert!(
            second
                .iter()
                .all(|(_, record)| !first.iter().any(|(_, old)| old.id == record.id))
        );
        catalog.finish_write(&second, true);
        let newer_first = catalog.begin_write();
        assert_eq!(newer_first.len(), 32);
        assert!(
            newer_first
                .iter()
                .all(|(_, record)| record.bytes_received == 128)
        );
        catalog.finish_write(&newer_first, true);
        assert!(!catalog.has_unsaved());
    }

    #[test]
    fn a_second_destination_cannot_replace_the_accepted_save_location() {
        let mut catalog = catalog();
        catalog
            .destination(
                "owned",
                Uuid::from_u128(1),
                "/owned/first.pdf".into(),
                "first.pdf".into(),
            )
            .unwrap();
        assert_eq!(
            catalog.destination(
                "owned",
                Uuid::from_u128(1),
                "/owned/second.pdf".into(),
                "second.pdf".into()
            ),
            Err(DownloadSessionError::DestinationAlreadySelected)
        );
        let row = catalog.rows(&[]).remove(0);
        assert_eq!(row.record.destination, "/owned/first.pdf");
        assert_eq!(row.display_name, "first.pdf");
    }

    #[test]
    fn live_rows_shadow_old_history_without_hiding_other_saved_downloads() {
        let mut catalog = catalog();
        catalog
            .destination(
                "owned",
                Uuid::from_u128(1),
                "/owned/Report.pdf".into(),
                "Report.pdf".into(),
            )
            .unwrap();
        catalog
            .update(
                "owned",
                Uuid::from_u128(1),
                DownloadState::Paused,
                128,
                false,
                None,
            )
            .unwrap();
        let mut old = offered("owned", 1);
        old.state = DownloadState::InProgress;
        let rows = catalog.rows(&[old, offered("older", 0)]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].record.state, DownloadState::Paused);
        assert_eq!(rows[0].record.bytes_received, 128);
        assert_eq!(rows[0].display_name, "Report.pdf");
        assert!(rows[0].active);
        assert_eq!(rows[1].record.id, "older");
        assert_eq!(catalog.rows(&[]).len(), 1);
    }
    #[test]
    fn byte_delivery_requires_the_live_owner_and_stops_at_transport_completion() {
        let mut catalog = catalog();
        let owner = Uuid::from_u128(1);
        assert!(!catalog.write_current("owned", owner));
        catalog
            .destination("owned", owner, "/owned/file.bin".into(), "file.bin".into())
            .unwrap();
        catalog
            .update("owned", owner, DownloadState::InProgress, 0, false, None)
            .unwrap();
        assert!(catalog.write_current("owned", owner));
        assert!(!catalog.write_current("owned", Uuid::from_u128(2)));
        catalog
            .update("owned", owner, DownloadState::Paused, 128, false, None)
            .unwrap();
        assert!(catalog.write_current("owned", owner));
        catalog
            .update("owned", owner, DownloadState::Completed, 256, false, None)
            .unwrap();
        assert!(!catalog.write_current("owned", owner));
        assert!(catalog.rows(&[])[0].finishing);
        catalog
            .update("owned", owner, DownloadState::Cancelled, 256, true, Some(2))
            .unwrap();
        assert!(!catalog.write_current("owned", owner));
        assert!(!catalog.finalization_current(
            "owned",
            owner,
            std::path::Path::new("/owned/file.bin")
        ));
    }

    #[test]
    fn transport_completion_does_not_advertise_an_unpublished_file() {
        let mut catalog = catalog();
        catalog
            .destination(
                "owned",
                Uuid::from_u128(1),
                "/owned/Report.pdf".into(),
                "Report.pdf".into(),
            )
            .unwrap();
        catalog
            .update(
                "owned",
                Uuid::from_u128(1),
                DownloadState::Completed,
                256,
                false,
                None,
            )
            .unwrap();
        let row = catalog.rows(&[]).remove(0);
        assert!(row.finishing);
        assert!(!row.file_ready);
        assert_eq!(row.record.state, DownloadState::InProgress);
        assert_eq!(
            catalog.update(
                "owned",
                Uuid::from_u128(1),
                DownloadState::Completed,
                256,
                true,
                Some(2)
            ),
            Err(DownloadSessionError::FileNotFinalized)
        );
        catalog.finalized("owned", Uuid::from_u128(1)).unwrap();
        catalog
            .update(
                "owned",
                Uuid::from_u128(1),
                DownloadState::Completed,
                256,
                true,
                Some(2),
            )
            .unwrap();
        let row = catalog.rows(&[]).remove(0);
        assert!(row.file_ready);
        assert!(!row.finishing);
        assert_eq!(row.record.completed_at, Some(2));
        assert!(!row.active);
        catalog
            .update(
                "owned",
                Uuid::from_u128(1),
                DownloadState::InProgress,
                0,
                false,
                None,
            )
            .unwrap();
        assert_eq!(catalog.rows(&[])[0].record.state, DownloadState::Completed);
    }
    #[test]
    fn wrong_window_and_unknown_requests_cannot_mutate_or_steal_a_download() {
        let mut catalog = catalog();
        assert_eq!(
            catalog.update(
                "owned",
                Uuid::from_u128(2),
                DownloadState::Cancelled,
                0,
                true,
                None
            ),
            Err(DownloadSessionError::WrongOwner)
        );
        assert_eq!(
            catalog.destination(
                "missing",
                Uuid::from_u128(1),
                "/owned/file".into(),
                "file".into()
            ),
            Err(DownloadSessionError::UnknownDownload)
        );
        assert_eq!(
            catalog.offer(
                offered("owned", 2),
                "replacement".into(),
                Uuid::from_u128(2)
            ),
            Err(DownloadSessionError::AlreadyExists)
        );
        assert_eq!(catalog.rows(&[])[0].display_name, "Report <final>.pdf");
        assert_eq!(catalog.rows(&[])[0].record.state, DownloadState::Offered);
    }
    #[test]
    fn old_history_acknowledgements_do_not_discard_newer_progress() {
        let mut catalog = catalog();
        let (before, _) = catalog.unsaved().remove(0);
        catalog
            .update(
                "owned",
                Uuid::from_u128(1),
                DownloadState::InProgress,
                12,
                false,
                None,
            )
            .unwrap();
        catalog.acknowledge("owned", before);
        let (current, record) = catalog.unsaved().remove(0);
        assert!(current > before);
        assert_eq!(record.bytes_received, 12);
        catalog.acknowledge("owned", current);
        assert_eq!(
            catalog.unsaved(),
            [] as [(u64, ferric_browser_storage::DownloadRecord); 0]
        );
        catalog.acknowledge("owned", before);
        assert_eq!(
            catalog.unsaved(),
            [] as [(u64, ferric_browser_storage::DownloadRecord); 0]
        );
    }
    #[test]
    fn memory_only_downloads_never_produce_history_writes() {
        let mut catalog = DownloadCatalog::default();
        catalog
            .offer(
                offered("private", 1),
                "private.pdf".into(),
                Uuid::from_u128(1),
            )
            .unwrap();
        catalog
            .update(
                "private",
                Uuid::from_u128(1),
                DownloadState::Cancelled,
                0,
                true,
                None,
            )
            .unwrap();
        assert_eq!(
            catalog.unsaved(),
            [] as [(u64, ferric_browser_storage::DownloadRecord); 0]
        );
        assert_eq!(catalog.rows(&[])[0].display_name, "private.pdf");
        let rows = catalog.rows(&[offered("ordinary-history", 0)]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].record.id, "private");
    }
    #[test]
    fn unsaved_terminal_history_is_not_evicted_or_acknowledged_by_a_future_write() {
        let mut catalog = DownloadCatalog::durable();
        for id in 0..DownloadCatalog::CAPACITY {
            let id = id.to_string();
            catalog
                .offer(offered(&id, 1), "file".into(), Uuid::from_u128(1))
                .unwrap();
            catalog
                .update(
                    &id,
                    Uuid::from_u128(1),
                    DownloadState::Cancelled,
                    0,
                    true,
                    None,
                )
                .unwrap();
        }
        catalog.acknowledge("0", u64::MAX);
        assert_eq!(catalog.unsaved().len(), DownloadCatalog::CAPACITY);
        assert_eq!(
            catalog.offer(offered("new", 2), "file".into(), Uuid::from_u128(1)),
            Err(DownloadSessionError::Capacity)
        );
        catalog.acknowledge("0", 2);
        catalog
            .offer(offered("new", 2), "file".into(), Uuid::from_u128(1))
            .unwrap();
        assert_eq!(catalog.rows(&[]).len(), DownloadCatalog::CAPACITY);
        assert!(!catalog.rows(&[]).iter().any(|row| row.record.id == "0"));
    }
    #[test]
    fn capacity_never_evicts_an_active_request() {
        let mut catalog = DownloadCatalog::default();
        for id in 0..DownloadCatalog::CAPACITY {
            catalog
                .offer(
                    offered(&id.to_string(), i64::try_from(id).unwrap()),
                    "file".into(),
                    Uuid::from_u128(1),
                )
                .unwrap();
        }
        assert_eq!(
            catalog.offer(
                offered("overflow", 1_001),
                "file".into(),
                Uuid::from_u128(1)
            ),
            Err(DownloadSessionError::Capacity)
        );
        catalog
            .update(
                "0",
                Uuid::from_u128(1),
                DownloadState::Cancelled,
                0,
                true,
                None,
            )
            .unwrap();
        catalog
            .offer(offered("new", 1_002), "file".into(), Uuid::from_u128(1))
            .unwrap();
        let rows = catalog.rows(&[]);
        assert_eq!(rows.len(), DownloadCatalog::CAPACITY);
        assert!(rows.iter().any(|row| row.record.id == "1"));
        assert!(!rows.iter().any(|row| row.record.id == "0"));
    }
}
