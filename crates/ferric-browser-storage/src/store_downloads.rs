use super::{
    DownloadRecord, DownloadState, DownloadUpdate, MAX_STORED_IDENTIFIER_BYTES, OptionalExtension,
    ProfileStore, StoreError, download_from_row, params, valid_stored_text,
};

impl ProfileStore {
    /// Creates or replaces the durable record for a normal-profile download.
    /// The engine request and file contents remain owned by Qt; this row is
    /// only the Rust metadata index.
    ///
    /// # Errors
    ///
    /// Returns an error for empty identifiers/URLs or a database failure.
    pub fn create_download(
        &self,
        id: &str,
        source_url: &str,
        destination: &str,
        state: DownloadState,
        created_at: i64,
    ) -> Result<(), StoreError> {
        if !valid_stored_text(id, MAX_STORED_IDENTIFIER_BYTES)
            || !valid_stored_text(source_url, 16 * 1024)
        {
            return Err(StoreError::InvalidInput(
                "download ID and source URL must be bounded nonempty text",
            ));
        }
        self.connection.execute(
            "INSERT INTO downloads (id, source_url, destination, state, bytes_received, created_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, NULL)
             ON CONFLICT(id) DO UPDATE SET source_url = excluded.source_url,
             destination = excluded.destination, state = excluded.state,
             bytes_received = 0, created_at = excluded.created_at, completed_at = NULL",
            params![id, source_url, destination, state.as_str(), created_at],
        )?;
        Ok(())
    }

    /// Updates a download's lifecycle and byte progress.
    ///
    /// # Errors
    ///
    /// Returns an error if the row is missing or SQLite fails.
    pub fn update_download(
        &self,
        id: &str,
        state: DownloadState,
        bytes_received: i64,
        completed_at: Option<i64>,
    ) -> Result<(), StoreError> {
        if bytes_received < 0 {
            return Err(StoreError::InvalidInput(
                "download byte count cannot be negative",
            ));
        }
        let changed = self.connection.execute(
            "UPDATE downloads SET state = ?2, bytes_received = ?3, completed_at = ?4 WHERE id = ?1",
            params![id, state.as_str(), bytes_received, completed_at],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidInput("download ID was not found"));
        }
        Ok(())
    }

    /// Applies a bounded group of download lifecycle updates atomically.
    ///
    /// # Errors
    ///
    /// Returns an error if the batch is empty, an update is invalid, a
    /// download is missing, or SQLite fails. A failed update rolls back the
    /// entire batch.
    pub fn update_download_batch(&self, updates: &[DownloadUpdate]) -> Result<(), StoreError> {
        if updates.is_empty() {
            return Err(StoreError::InvalidInput(
                "download update batch cannot be empty",
            ));
        }
        for update in updates {
            if !valid_stored_text(&update.id, MAX_STORED_IDENTIFIER_BYTES)
                || update.bytes_received < 0
            {
                return Err(StoreError::InvalidInput(
                    "download update ID or byte count is invalid",
                ));
            }
        }
        let transaction = self.connection.unchecked_transaction()?;
        for update in updates {
            let changed = transaction.execute(
                "UPDATE downloads SET state = ?2, bytes_received = ?3, completed_at = ?4 WHERE id = ?1",
                params![
                    update.id,
                    update.state.as_str(),
                    update.bytes_received,
                    update.completed_at
                ],
            )?;
            if changed == 0 {
                return Err(StoreError::InvalidInput("download ID was not found"));
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// Records the final user-selected destination before the engine starts
    /// writing bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the row is missing or SQLite fails.
    pub fn set_download_destination(&self, id: &str, destination: &str) -> Result<(), StoreError> {
        if destination.is_empty() {
            return Err(StoreError::InvalidInput(
                "download destination cannot be empty",
            ));
        }
        let changed = self.connection.execute(
            "UPDATE downloads SET destination = ?2 WHERE id = ?1",
            params![id, destination],
        )?;
        if changed == 0 {
            return Err(StoreError::InvalidInput("download ID was not found"));
        }
        Ok(())
    }

    /// Reads one durable download record.
    ///
    /// # Errors
    ///
    /// Returns an error if SQLite fails or a stored state is unknown.
    pub fn download(&self, id: &str) -> Result<Option<DownloadRecord>, StoreError> {
        self.connection
            .query_row(
                "SELECT id, source_url, destination, state, bytes_received, created_at, completed_at FROM downloads WHERE id = ?1",
                [id],
                download_from_row,
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// Lists durable downloads newest first.
    ///
    /// # Errors
    ///
    /// Returns an error if SQLite fails or a stored state is unknown.
    pub fn downloads(&self) -> Result<Vec<DownloadRecord>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, source_url, destination, state, bytes_received, created_at, completed_at
             FROM downloads ORDER BY created_at DESC, id DESC",
        )?;
        let rows = statement.query_map([], download_from_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }
}
