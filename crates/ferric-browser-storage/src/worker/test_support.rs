//! Legacy convenience polling used only by storage integration tests.
use super::*;

#[cfg(test)]
impl ProfileStoreWorker {
    pub fn request_library(&mut self, limit: usize) -> Result<(), StorageWorkerError> {
        if !(1..=MAX_LIBRARY_LIMIT).contains(&limit) {
            return Err(StorageWorkerError::InvalidLimit);
        }
        self.submit(StorageOperation::Library, StorageCommand::Library { limit })
    }

    pub fn poll_library(&mut self) -> Option<Result<ProfileLibrarySnapshot, String>> {
        match self.poll_operation(StorageOperation::Library)? {
            StorageCompletion::Library(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn request_session_list(
        &mut self,
        root: PathBuf,
        profile_id: Uuid,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::SessionList,
            StorageCommand::SessionList { root, profile_id },
        )
    }

    pub fn poll_session_list(&mut self) -> Option<Result<Vec<String>, String>> {
        match self.poll_operation(StorageOperation::SessionList)? {
            StorageCompletion::SessionList(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn session_list_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::SessionList)
    }

    pub fn request_session_restore(&mut self, path: PathBuf) -> Result<(), StorageWorkerError> {
        self.request_session_restore_paths(vec![path])
    }

    pub fn request_session_restore_paths(
        &mut self,
        paths: Vec<PathBuf>,
    ) -> Result<(), StorageWorkerError> {
        if paths.is_empty() {
            return Err(StorageWorkerError::QueueFull);
        }
        self.submit(
            StorageOperation::SessionRestore,
            StorageCommand::SessionRestore { paths },
        )
    }

    pub fn request_session_recovery(
        &mut self,
        root: PathBuf,
        profile_id: Uuid,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::SessionRestore,
            StorageCommand::SessionRecovery { root, profile_id },
        )
    }

    pub fn poll_session_restore(&mut self) -> Option<Result<SessionRestoreSnapshot, String>> {
        match self.poll_operation(StorageOperation::SessionRestore)? {
            StorageCompletion::SessionRestore(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn session_restore_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::SessionRestore)
    }

    pub fn request_session_save(
        &mut self,
        path: PathBuf,
        snapshot: SessionSnapshot,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::SessionSave,
            StorageCommand::SessionSave { path, snapshot },
        )
    }

    pub fn poll_session_save(&mut self) -> Option<Result<(), String>> {
        match self.poll_operation(StorageOperation::SessionSave)? {
            StorageCompletion::SessionSave(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn session_save_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::SessionSave)
    }

    pub fn request_session_delete(
        &mut self,
        root: PathBuf,
        profile_id: Uuid,
        name: String,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::SessionDelete,
            StorageCommand::SessionDelete {
                root,
                profile_id,
                name,
            },
        )
    }

    pub fn poll_session_delete(&mut self) -> Option<Result<(), String>> {
        match self.poll_operation(StorageOperation::SessionDelete)? {
            StorageCompletion::SessionDelete(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn request_session_checkpoint_clear(
        &mut self,
        root: PathBuf,
        profile_id: Uuid,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::SessionCheckpointClear,
            StorageCommand::SessionCheckpointClear { root, profile_id },
        )
    }

    pub fn poll_session_checkpoint_clear(&mut self) -> Option<Result<(), String>> {
        match self.poll_operation(StorageOperation::SessionCheckpointClear)? {
            StorageCompletion::SessionCheckpointClear(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn request_history_batch(
        &mut self,
        visits: Vec<VisitInput>,
    ) -> Result<(), StorageWorkerError> {
        if visits.is_empty() || visits.len() > MAX_HISTORY_BATCH {
            return Err(StorageWorkerError::InvalidBatch);
        }
        self.submit(
            StorageOperation::HistoryBatch,
            StorageCommand::HistoryBatch { visits },
        )
    }

    pub fn poll_history_batch(&mut self) -> Option<Result<usize, String>> {
        match self.poll_operation(StorageOperation::HistoryBatch)? {
            StorageCompletion::HistoryBatch(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn request_permission_batch(
        &mut self,
        rules: Vec<PermissionRule>,
    ) -> Result<(), StorageWorkerError> {
        if rules.is_empty() || rules.len() > MAX_PERMISSION_BATCH {
            return Err(StorageWorkerError::InvalidPermissionBatch);
        }
        self.submit(
            StorageOperation::PermissionBatch,
            StorageCommand::PermissionBatch { rules },
        )
    }

    pub fn poll_permission_batch(&mut self) -> Option<Result<usize, String>> {
        match self.poll_operation(StorageOperation::PermissionBatch)? {
            StorageCompletion::PermissionBatch(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn request_permission_reset(
        &mut self,
        origin: String,
        permissions: Vec<String>,
    ) -> Result<(), StorageWorkerError> {
        if origin.is_empty() || permissions.is_empty() || permissions.len() > 2 {
            return Err(StorageWorkerError::InvalidPermissionBatch);
        }
        self.submit(
            StorageOperation::PermissionReset,
            StorageCommand::PermissionReset {
                origin,
                permissions,
            },
        )
    }

    pub fn poll_permission_reset(&mut self) -> Option<Result<bool, String>> {
        match self.poll_operation(StorageOperation::PermissionReset)? {
            StorageCompletion::PermissionReset(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn request_download_batch(
        &mut self,
        updates: Vec<DownloadUpdate>,
    ) -> Result<(), StorageWorkerError> {
        if updates.is_empty() || updates.len() > MAX_DOWNLOAD_BATCH {
            return Err(StorageWorkerError::InvalidDownloadBatch);
        }
        self.submit(
            StorageOperation::DownloadBatch,
            StorageCommand::DownloadBatch { updates },
        )
    }

    pub fn poll_download_batch(&mut self) -> Option<Result<usize, String>> {
        match self.poll_operation(StorageOperation::DownloadBatch)? {
            StorageCompletion::DownloadBatch(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn request_download_destination(
        &mut self,
        id: String,
        destination: String,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::DownloadDestination,
            StorageCommand::DownloadDestination { id, destination },
        )
    }

    pub fn poll_download_destination(&mut self) -> Option<Result<(), String>> {
        match self.poll_operation(StorageOperation::DownloadDestination)? {
            StorageCompletion::DownloadDestination(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_download_destination(&mut self, timeout: Duration) -> Option<Result<(), String>> {
        match self.wait_operation(StorageOperation::DownloadDestination, timeout)? {
            StorageCompletion::DownloadDestination(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn download_destination_pending(&self) -> bool {
        self.pending
            .contains(&StorageOperation::DownloadDestination)
    }

    pub fn request_download_create(
        &mut self,
        id: String,
        source_url: String,
        destination: String,
        state: crate::DownloadState,
        created_at: i64,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::DownloadCreate,
            StorageCommand::DownloadCreate {
                id,
                source_url,
                destination,
                state,
                created_at,
            },
        )
    }

    pub fn poll_download_create(&mut self) -> Option<Result<(), String>> {
        match self.poll_operation(StorageOperation::DownloadCreate)? {
            StorageCompletion::DownloadCreate(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_download_create(&mut self, timeout: Duration) -> Option<Result<(), String>> {
        match self.wait_operation(StorageOperation::DownloadCreate, timeout)? {
            StorageCompletion::DownloadCreate(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn download_create_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::DownloadCreate)
    }

    pub fn request_journey_write(&mut self, write: JourneyWrite) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::JourneyWrite,
            StorageCommand::JourneyWrite { write },
        )
    }

    pub fn poll_journey_write(&mut self) -> Option<Result<bool, String>> {
        match self.poll_operation(StorageOperation::JourneyWrite)? {
            StorageCompletion::JourneyWrite(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_journey_write(&mut self, timeout: Duration) -> Option<Result<bool, String>> {
        match self.wait_operation(StorageOperation::JourneyWrite, timeout)? {
            StorageCompletion::JourneyWrite(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn journey_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::JourneyWrite)
    }

    pub fn request_journey_node(&mut self, id: String) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::JourneyNode,
            StorageCommand::JourneyNode { id },
        )
    }

    pub fn poll_journey_node(&mut self) -> Option<Result<Option<JourneyNodeRecord>, String>> {
        match self.poll_operation(StorageOperation::JourneyNode)? {
            StorageCompletion::JourneyNode(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_journey_node(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<Option<JourneyNodeRecord>, String>> {
        match self.wait_operation(StorageOperation::JourneyNode, timeout)? {
            StorageCompletion::JourneyNode(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn journey_node_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::JourneyNode)
    }

    pub fn request_journey_query(
        &mut self,
        current_tab_id: Option<String>,
        search: Option<String>,
        expand: Option<String>,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::JourneyQuery,
            StorageCommand::JourneyQuery {
                current_tab_id,
                search,
                expand,
            },
        )
    }

    pub fn poll_journey_query(&mut self) -> Option<Result<JourneyQuerySnapshot, String>> {
        match self.poll_operation(StorageOperation::JourneyQuery)? {
            StorageCompletion::JourneyQuery(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_journey_query(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<JourneyQuerySnapshot, String>> {
        match self.wait_operation(StorageOperation::JourneyQuery, timeout)? {
            StorageCompletion::JourneyQuery(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn journey_query_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::JourneyQuery)
    }

    pub fn request_journey_export(&mut self) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::JourneyExport,
            StorageCommand::JourneyExport,
        )
    }

    pub fn poll_journey_export(&mut self) -> Option<Result<JourneyQuerySnapshot, String>> {
        match self.poll_operation(StorageOperation::JourneyExport)? {
            StorageCompletion::JourneyExport(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_journey_export(
        &mut self,
        timeout: Duration,
    ) -> Option<Result<JourneyQuerySnapshot, String>> {
        match self.wait_operation(StorageOperation::JourneyExport, timeout)? {
            StorageCompletion::JourneyExport(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn journey_export_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::JourneyExport)
    }

    pub fn request_mark_write(&mut self, write: MarkWrite) -> Result<(), StorageWorkerError> {
        self.request_mark_writes(vec![write])
    }

    pub fn request_mark_writes(
        &mut self,
        writes: Vec<MarkWrite>,
    ) -> Result<(), StorageWorkerError> {
        if writes.is_empty() || writes.len() > MAX_MARK_BATCH {
            return Err(StorageWorkerError::InvalidMarkBatch);
        }
        self.submit(
            StorageOperation::MarkWrite,
            StorageCommand::MarkBatch { writes },
        )
    }

    pub fn poll_mark_write(&mut self) -> Option<Result<usize, String>> {
        match self.poll_operation(StorageOperation::MarkWrite)? {
            StorageCompletion::MarkWrite(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_mark_write(&mut self, timeout: Duration) -> Option<Result<usize, String>> {
        match self.wait_operation(StorageOperation::MarkWrite, timeout)? {
            StorageCompletion::MarkWrite(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn mark_write_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::MarkWrite)
    }

    pub fn request_history_clear(
        &mut self,
        since: Option<i64>,
        origin: Option<String>,
    ) -> Result<(), StorageWorkerError> {
        self.submit(
            StorageOperation::HistoryClear,
            StorageCommand::HistoryClear { since, origin },
        )
    }

    pub fn poll_history_clear(&mut self) -> Option<Result<u64, String>> {
        match self.poll_operation(StorageOperation::HistoryClear)? {
            StorageCompletion::HistoryClear(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_history_clear(&mut self, timeout: Duration) -> Option<Result<u64, String>> {
        match self.wait_operation(StorageOperation::HistoryClear, timeout)? {
            StorageCompletion::HistoryClear(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn history_clear_pending(&self) -> bool {
        self.pending.contains(&StorageOperation::HistoryClear)
    }
}

#[cfg(test)]
impl ProfileStoreWorker {
    pub fn wait_download_batch(&mut self, timeout: Duration) -> Option<Result<usize, String>> {
        match self.wait_operation(StorageOperation::DownloadBatch, timeout)? {
            StorageCompletion::DownloadBatch(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_permission_batch(&mut self, timeout: Duration) -> Option<Result<usize, String>> {
        match self.wait_operation(StorageOperation::PermissionBatch, timeout)? {
            StorageCompletion::PermissionBatch(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }

    pub fn wait_history_batch(&mut self, timeout: Duration) -> Option<Result<usize, String>> {
        match self.wait_operation(StorageOperation::HistoryBatch, timeout)? {
            StorageCompletion::HistoryBatch(result) => Some(stringify_error(result)),
            _ => unreachable!("operation-keyed completion changed variant"),
        }
    }
}
