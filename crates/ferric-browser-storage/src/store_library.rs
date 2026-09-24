use super::{
    MAX_STORED_IDENTIFIER_BYTES, MAX_STORED_TITLE_BYTES, MarkWrite, ProfileStore, StoreError,
    bounded_stored_text, params, valid_stored_text,
};

impl ProfileStore {
    /// Stores a bookmark with a caller-owned stable ID.
    ///
    /// # Errors
    ///
    /// Returns an error for empty IDs/URLs or a database failure.
    pub fn add_bookmark(
        &self,
        id: &str,
        url: &str,
        title: &str,
        timestamp: i64,
    ) -> Result<(), StoreError> {
        if !valid_stored_text(id, MAX_STORED_IDENTIFIER_BYTES)
            || !bounded_stored_text(title, MAX_STORED_TITLE_BYTES)
            || !valid_stored_text(url, 16 * 1024)
        {
            return Err(StoreError::InvalidInput(
                "bookmark ID and URL must be bounded nonempty text; title must be bounded text",
            ));
        }
        self.connection.execute(
            "INSERT INTO bookmarks (id, url, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(id) DO UPDATE SET url = excluded.url, title = excluded.title, updated_at = excluded.updated_at",
            params![id, url, title, timestamp],
        )?;
        Ok(())
    }

    /// Deletes one bookmark by its stable caller-owned ID.
    ///
    /// # Errors
    ///
    /// Returns an error if SQLite fails.
    pub fn delete_bookmark(&self, id: &str) -> Result<bool, StoreError> {
        Ok(self
            .connection
            .execute("DELETE FROM bookmarks WHERE id = ?1", [id])?
            != 0)
    }

    /// Inserts or updates a case-sensitive quickmark name.
    ///
    /// # Errors
    ///
    /// Returns an error for empty names/URLs or a database failure.
    pub fn set_quickmark(&self, name: &str, url: &str, timestamp: i64) -> Result<(), StoreError> {
        if !valid_stored_text(name, MAX_STORED_IDENTIFIER_BYTES)
            || !valid_stored_text(url, 16 * 1024)
        {
            return Err(StoreError::InvalidInput(
                "quickmark name and URL must be bounded nonempty text",
            ));
        }
        self.connection.execute(
            "INSERT INTO quickmarks (name, url, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(name) DO UPDATE SET url = excluded.url, updated_at = excluded.updated_at",
            params![name, url, timestamp],
        )?;
        Ok(())
    }

    /// Deletes one quickmark by its exact name.
    ///
    /// # Errors
    ///
    /// Returns an error if SQLite fails.
    pub fn delete_quickmark(&self, name: &str) -> Result<bool, StoreError> {
        Ok(self
            .connection
            .execute("DELETE FROM quickmarks WHERE name = ?1", [name])?
            != 0)
    }

    /// Applies one bounded bookmark/quickmark mutation atomically.
    ///
    /// Delete mutations fail when their target is absent, preserving the
    /// command layer's stale-entry semantics.
    pub fn apply_mark_write(&self, write: &MarkWrite) -> Result<(), StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        Self::apply_mark_write_in_transaction(&transaction, write)?;
        transaction.commit()?;
        Ok(())
    }

    /// Applies a bounded group of bookmark/quickmark mutations atomically.
    ///
    /// Delete mutations fail when their target is absent, preserving the
    /// command layer's stale-entry semantics. If any mutation fails, none of
    /// the preceding mutations are committed.
    pub fn apply_mark_write_batch(&self, writes: &[MarkWrite]) -> Result<(), StoreError> {
        if writes.is_empty() {
            return Err(StoreError::InvalidInput("mark write batch cannot be empty"));
        }
        let transaction = self.connection.unchecked_transaction()?;
        for write in writes {
            Self::apply_mark_write_in_transaction(&transaction, write)?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn apply_mark_write_in_transaction(
        transaction: &rusqlite::Transaction<'_>,
        write: &MarkWrite,
    ) -> Result<(), StoreError> {
        match write {
            MarkWrite::AddBookmark {
                id,
                url,
                title,
                timestamp,
            } => {
                if !valid_stored_text(id, MAX_STORED_IDENTIFIER_BYTES)
                    || !bounded_stored_text(title, MAX_STORED_TITLE_BYTES)
                    || !valid_stored_text(url, 16 * 1024)
                {
                    return Err(StoreError::InvalidInput(
                        "bookmark ID and URL must be bounded nonempty text; title must be bounded text",
                    ));
                }
                transaction.execute(
                    "INSERT INTO bookmarks (id, url, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)
                     ON CONFLICT(id) DO UPDATE SET url = excluded.url, title = excluded.title, updated_at = excluded.updated_at",
                    params![id, url, title, timestamp],
                )?;
            }
            MarkWrite::DeleteBookmark { id } => {
                if !valid_stored_text(id, MAX_STORED_IDENTIFIER_BYTES) {
                    return Err(StoreError::InvalidInput("bookmark ID is invalid"));
                }
                if transaction.execute("DELETE FROM bookmarks WHERE id = ?1", [id])? == 0 {
                    return Err(StoreError::InvalidInput("bookmark was not found"));
                }
            }
            MarkWrite::EditBookmark {
                id,
                title,
                timestamp,
            } => {
                if !valid_stored_text(id, MAX_STORED_IDENTIFIER_BYTES)
                    || !bounded_stored_text(title, MAX_STORED_TITLE_BYTES)
                {
                    return Err(StoreError::InvalidInput(
                        "bookmark ID and title must be bounded valid text",
                    ));
                }
                if transaction.execute(
                    "UPDATE bookmarks SET title = ?2, updated_at = ?3 WHERE id = ?1",
                    params![id, title, timestamp],
                )? == 0
                {
                    return Err(StoreError::InvalidInput("bookmark was not found"));
                }
            }
            MarkWrite::SetQuickmark {
                name,
                url,
                timestamp,
            } => {
                if !valid_stored_text(name, MAX_STORED_IDENTIFIER_BYTES)
                    || !valid_stored_text(url, 16 * 1024)
                {
                    return Err(StoreError::InvalidInput(
                        "quickmark name and URL must be bounded nonempty text",
                    ));
                }
                transaction.execute(
                    "INSERT INTO quickmarks (name, url, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)
                     ON CONFLICT(name) DO UPDATE SET url = excluded.url, updated_at = excluded.updated_at",
                    params![name, url, timestamp],
                )?;
            }
            MarkWrite::DeleteQuickmark { name } => {
                if !valid_stored_text(name, MAX_STORED_IDENTIFIER_BYTES) {
                    return Err(StoreError::InvalidInput("quickmark name is invalid"));
                }
                if transaction.execute("DELETE FROM quickmarks WHERE name = ?1", [name])? == 0 {
                    return Err(StoreError::InvalidInput("quickmark was not found"));
                }
            }
            MarkWrite::EditQuickmark {
                name,
                url,
                timestamp,
            } => {
                if !valid_stored_text(name, MAX_STORED_IDENTIFIER_BYTES)
                    || !valid_stored_text(url, 16 * 1024)
                {
                    return Err(StoreError::InvalidInput(
                        "quickmark name and URL must be bounded nonempty text",
                    ));
                }
                if transaction.execute(
                    "UPDATE quickmarks SET url = ?2, updated_at = ?3 WHERE name = ?1",
                    params![name, url, timestamp],
                )? == 0
                {
                    return Err(StoreError::InvalidInput("quickmark was not found"));
                }
            }
        }
        Ok(())
    }
}
