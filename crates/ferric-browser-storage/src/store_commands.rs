use super::{MAX_COMMAND_HISTORY_ROWS, ProfileStore, StoreError, params};

impl ProfileStore {
    /// Returns the most recent eligible commands for command-line recall.
    pub fn command_history(&self, limit: usize) -> Result<Vec<String>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT command FROM command_history ORDER BY created_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
            row.get::<_, String>(0)
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Records eligible command text. Ineligible/sensitive commands are a
    /// successful no-op and never reach SQLite.
    ///
    /// # Errors
    ///
    /// Returns an error for an eligible empty command or a database failure.
    pub fn record_command(
        &self,
        command: &str,
        timestamp: i64,
        eligible: bool,
    ) -> Result<bool, StoreError> {
        if !eligible {
            return Ok(false);
        }
        if command.is_empty() {
            return Err(StoreError::InvalidInput(
                "command history text cannot be empty",
            ));
        }
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO command_history (command, created_at) VALUES (?1, ?2)",
            params![command, timestamp],
        )?;
        transaction.execute(
            "DELETE FROM command_history
             WHERE id NOT IN (
                 SELECT id FROM command_history ORDER BY created_at DESC, id DESC LIMIT ?1
             )",
            [MAX_COMMAND_HISTORY_ROWS],
        )?;
        transaction.commit()?;
        Ok(true)
    }
}
