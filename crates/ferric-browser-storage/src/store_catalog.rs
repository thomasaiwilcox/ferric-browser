use super::{BookmarkRecord, ProfileStore, Quickmark, StoreError};

impl ProfileStore {
    /// Lists profile-local bookmarks newest first.
    ///
    /// # Errors
    ///
    /// Returns an error if SQLite cannot execute the bounded query.
    pub fn bookmarks(&self, limit: usize) -> Result<Vec<BookmarkRecord>, StoreError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT id, url, title, updated_at FROM bookmarks
             ORDER BY updated_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], |row| {
            Ok(BookmarkRecord {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Lists profile-local quickmarks newest first.
    ///
    /// # Errors
    ///
    /// Returns an error if SQLite cannot execute the bounded query.
    pub fn quickmarks(&self, limit: usize) -> Result<Vec<Quickmark>, StoreError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT name, url FROM quickmarks ORDER BY updated_at DESC, name ASC LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], |row| {
            Ok(Quickmark {
                name: row.get(0)?,
                url: row.get(1)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }
}
