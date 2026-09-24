use super::{MAX_COMMAND_HISTORY_ROWS, ProfileStore, StoreError, params};

impl ProfileStore {
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
