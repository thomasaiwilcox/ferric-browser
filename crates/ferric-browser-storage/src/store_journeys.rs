use super::{
    HistoryRecord, JourneyEdgeRecord, JourneyNodeRecord, JourneyWrite, MAX_STORED_IDENTIFIER_BYTES,
    MAX_STORED_TITLE_BYTES, OptionalExtension, ProfileStore, Quickmark, StoreError,
    bounded_stored_text, is_safe_history_url, params, params_from_iter, prune_journey,
    valid_history_origin, valid_journey_transition, valid_stored_text,
};

impl ProfileStore {
    /// Applies one durable journey mutation.
    ///
    /// # Errors
    ///
    /// Returns an error when the journey fields are invalid or SQLite fails.
    pub fn apply_journey_write(&self, write: &JourneyWrite) -> Result<bool, StoreError> {
        match write {
            JourneyWrite::RecordNode {
                id,
                profile_id,
                tab_id,
                url,
                title,
                committed_at,
                transition,
                source,
                retention_before,
                parent_id,
            } => {
                self.record_journey_node_with_retention_and_parent(
                    id,
                    profile_id,
                    tab_id,
                    url,
                    title,
                    *committed_at,
                    transition,
                    source.as_deref(),
                    *retention_before,
                    parent_id.as_deref(),
                )?;
                Ok(true)
            }
            JourneyWrite::SetCurrentById { tab_id, node_id } => {
                self.set_current_journey_node_by_id(tab_id, node_id)
            }
            JourneyWrite::SetCurrentForUrl { tab_id, url } => {
                self.set_current_journey_node_for_url(tab_id, url)
            }
        }
    }

    /// Clears profile-local history, optionally retaining pages newer than a
    /// timestamp. `origin` is an exact security-origin filter supplied by the
    /// caller after validation; it never accepts arbitrary SQL fragments.
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction fails.
    pub fn clear_history(
        &self,
        since: Option<i64>,
        origin: Option<&str>,
    ) -> Result<u64, StoreError> {
        if since.is_some_and(|timestamp| timestamp < 0) {
            return Err(StoreError::InvalidInput(
                "history clear timestamp must not be negative",
            ));
        }
        if origin.is_some_and(|origin| !valid_history_origin(origin)) {
            return Err(StoreError::InvalidInput(
                "history clear origin must be an exact HTTP(S) origin",
            ));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let mut deleted: u64 = 0;
        let mut statement = transaction.prepare(
            "SELECT id, normalized_url, last_visit FROM pages
             WHERE (?1 IS NULL OR last_visit < ?1)
               AND (?2 IS NULL OR normalized_url = ?2
                    OR normalized_url LIKE ?2 || '/%'
                    OR normalized_url LIKE ?2 || '?%'
                    OR normalized_url LIKE ?2 || '#%')",
        )?;
        let rows = statement.query_map(params![since, origin], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let page_ids = rows.collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        for (page_id, _) in page_ids {
            transaction.execute("DELETE FROM pages WHERE id = ?1", [page_id])?;
            deleted = deleted.saturating_add(1);
        }
        transaction.execute(
            "DELETE FROM journey_nodes
             WHERE (?1 IS NULL OR committed_at < ?1)
               AND (?2 IS NULL OR url = ?2
                    OR url LIKE ?2 || '/%'
                    OR url LIKE ?2 || '?%'
                    OR url LIKE ?2 || '#%')",
            params![since, origin],
        )?;
        transaction.commit()?;
        Ok(deleted)
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the SQLite query fails.
    pub fn quickmark(&self, name: &str) -> Result<Option<Quickmark>, StoreError> {
        self.connection
            .query_row(
                "SELECT name, url FROM quickmarks WHERE name = ?1",
                [name],
                |row| {
                    Ok(Quickmark {
                        name: row.get(0)?,
                        url: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// Lists profile-local history pages newest first, without exposing visit
    /// rows or any URL that failed the safe-history policy at write time.
    ///
    /// # Errors
    ///
    /// Returns an error if SQLite cannot execute the bounded query.
    pub fn history(&self, limit: usize) -> Result<Vec<HistoryRecord>, StoreError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT id, normalized_url, safe_title, visit_count, last_visit
             FROM pages ORDER BY last_visit DESC, id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], |row| {
            Ok(HistoryRecord {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                visit_count: row.get(3)?,
                last_visit: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Persists one safe core journey node and links it to the tab's previous
    /// current node. The profile database is already profile-local, but the
    /// profile and tab identities are retained to make exports and audits
    /// explicit. Retention is bounded to 50,000 nodes and 100,000 edges.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe or unbounded fields, an unknown transition,
    /// or a SQLite failure.
    pub fn record_journey_node(
        &self,
        id: &str,
        profile_id: &str,
        tab_id: &str,
        url: &str,
        title: &str,
        committed_at: i64,
        transition: &str,
        source: Option<&str>,
    ) -> Result<(), StoreError> {
        self.record_journey_node_with_retention(
            id,
            profile_id,
            tab_id,
            url,
            title,
            committed_at,
            transition,
            source,
            None,
        )
    }

    /// Persists one journey node while applying an optional profile retention
    /// cutoff in the same transaction as insertion and hard-cap pruning.
    pub fn record_journey_node_with_retention(
        &self,
        id: &str,
        profile_id: &str,
        tab_id: &str,
        url: &str,
        title: &str,
        committed_at: i64,
        transition: &str,
        source: Option<&str>,
        retention_before: Option<i64>,
    ) -> Result<(), StoreError> {
        self.record_journey_node_with_retention_and_parent(
            id,
            profile_id,
            tab_id,
            url,
            title,
            committed_at,
            transition,
            source,
            retention_before,
            None,
        )
    }

    /// Persists one journey node and optionally links it to an exact
    /// cross-tab parent node, such as a popup opener.
    pub fn record_journey_node_with_retention_and_parent(
        &self,
        id: &str,
        profile_id: &str,
        tab_id: &str,
        url: &str,
        title: &str,
        committed_at: i64,
        transition: &str,
        source: Option<&str>,
        retention_before: Option<i64>,
        parent_id: Option<&str>,
    ) -> Result<(), StoreError> {
        if !valid_stored_text(id, MAX_STORED_IDENTIFIER_BYTES)
            || !valid_stored_text(profile_id, MAX_STORED_IDENTIFIER_BYTES)
            || !valid_stored_text(tab_id, MAX_STORED_IDENTIFIER_BYTES)
            || !is_safe_history_url(url)
            || !bounded_stored_text(title, MAX_STORED_TITLE_BYTES)
            || !valid_journey_transition(transition)
            || source.is_some_and(|value| !bounded_stored_text(value, 128))
            || parent_id.is_some_and(|value| !valid_stored_text(value, MAX_STORED_IDENTIFIER_BYTES))
        {
            return Err(StoreError::InvalidInput(
                "journey records must contain safe bounded fields",
            ));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let previous: Option<String> = transaction
            .query_row(
                "SELECT node_id FROM journey_current WHERE tab_id = ?1",
                [tab_id],
                |row| row.get(0),
            )
            .optional()?;
        transaction.execute(
            "INSERT INTO journey_nodes
                (id, profile_id, tab_id, url, title, committed_at, transition, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                profile_id,
                tab_id,
                url,
                title,
                committed_at,
                transition,
                source
            ],
        )?;
        let relation = if previous.is_some() {
            previous
        } else if let Some(parent_id) = parent_id {
            transaction
                .query_row(
                    "SELECT id FROM journey_nodes WHERE id = ?1",
                    [parent_id],
                    |row| row.get(0),
                )
                .optional()?
        } else {
            None
        };
        if let Some(previous) = relation {
            transaction.execute(
                "INSERT INTO journey_edges (source_id, target_id, transition, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![previous, id, transition, committed_at],
            )?;
        }
        transaction.execute(
            "INSERT INTO journey_current (tab_id, node_id) VALUES (?1, ?2)
             ON CONFLICT(tab_id) DO UPDATE SET node_id = excluded.node_id",
            params![tab_id, id],
        )?;
        prune_journey(&transaction, retention_before)?;
        transaction.commit()?;
        Ok(())
    }

    /// Lists durable journey nodes newest first.
    pub fn journey_nodes(&self, limit: usize) -> Result<Vec<JourneyNodeRecord>, StoreError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT id, profile_id, tab_id, url, title, committed_at, transition, source
             FROM journey_nodes ORDER BY committed_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], |row| {
            Ok(JourneyNodeRecord {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                tab_id: row.get(2)?,
                url: row.get(3)?,
                title: row.get(4)?,
                committed_at: row.get(5)?,
                transition: row.get(6)?,
                source: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Searches the safe journey fields without loading the whole graph.
    ///
    /// The query is literal (SQL wildcard characters in the user's text do
    /// not become wildcards) and remains bounded for native search surfaces.
    pub fn journey_nodes_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<JourneyNodeRecord>, StoreError> {
        if query.is_empty()
            || query.len() > MAX_STORED_IDENTIFIER_BYTES
            || query.chars().any(char::is_control)
        {
            return Err(StoreError::InvalidInput(
                "journey search must be 1..256 bytes without control characters",
            ));
        }
        let escaped = query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped}%");
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT id, profile_id, tab_id, url, title, committed_at, transition, source
             FROM journey_nodes
             WHERE title LIKE ?1 ESCAPE '\\' COLLATE NOCASE
                OR url LIKE ?1 ESCAPE '\\' COLLATE NOCASE
                OR transition LIKE ?1 ESCAPE '\\' COLLATE NOCASE
                OR COALESCE(source, '') LIKE ?1 ESCAPE '\\' COLLATE NOCASE
             ORDER BY committed_at DESC, id DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![pattern, limit], |row| {
            Ok(JourneyNodeRecord {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                tab_id: row.get(2)?,
                url: row.get(3)?,
                title: row.get(4)?,
                committed_at: row.get(5)?,
                transition: row.get(6)?,
                source: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Lists the bounded one-hop neighborhood of a retained node.
    pub fn journey_neighbors(
        &self,
        node_id: &str,
        limit: usize,
    ) -> Result<Vec<JourneyNodeRecord>, StoreError> {
        if !valid_stored_text(node_id, MAX_STORED_IDENTIFIER_BYTES) {
            return Err(StoreError::InvalidInput("journey node ID is invalid"));
        }
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT DISTINCT n.id, n.profile_id, n.tab_id, n.url, n.title,
                    n.committed_at, n.transition, n.source
             FROM journey_nodes n
             JOIN journey_edges e ON (e.source_id = ?1 AND e.target_id = n.id)
                                  OR (e.target_id = ?1 AND e.source_id = n.id)
             ORDER BY n.committed_at DESC, n.id DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![node_id, limit], |row| {
            Ok(JourneyNodeRecord {
                id: row.get(0)?,
                profile_id: row.get(1)?,
                tab_id: row.get(2)?,
                url: row.get(3)?,
                title: row.get(4)?,
                committed_at: row.get(5)?,
                transition: row.get(6)?,
                source: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Lists edges whose two endpoints are in a bounded native-view set.
    pub fn journey_edges_for_nodes(
        &self,
        node_ids: &[String],
        limit: usize,
    ) -> Result<Vec<JourneyEdgeRecord>, StoreError> {
        if node_ids.is_empty() {
            return Ok(Vec::new());
        }
        if node_ids.len() > 1_000
            || node_ids
                .iter()
                .any(|id| !valid_stored_text(id, MAX_STORED_IDENTIFIER_BYTES))
        {
            return Err(StoreError::InvalidInput(
                "journey node set is invalid or too large",
            ));
        }
        let placeholders = (1..=node_ids.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let second_placeholders = (node_ids.len() + 1..=node_ids.len() * 2)
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let bounded_limit = limit.min(1_000);
        let sql = format!(
            "SELECT source_id, target_id, transition, created_at
             FROM journey_edges
             WHERE source_id IN ({placeholders}) AND target_id IN ({second_placeholders})
             ORDER BY created_at ASC, rowid ASC LIMIT {bounded_limit}"
        );
        let values = node_ids
            .iter()
            .map(String::as_str)
            .chain(node_ids.iter().map(String::as_str))
            .collect::<Vec<_>>();
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values), |row| {
            Ok(JourneyEdgeRecord {
                source_id: row.get(0)?,
                target_id: row.get(1)?,
                transition: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Reads one durable journey node by UUID without constructing a fallback
    /// from caller-supplied URL-shaped input.
    pub fn journey_node(&self, id: &str) -> Result<Option<JourneyNodeRecord>, StoreError> {
        self.connection
            .query_row(
                "SELECT id, profile_id, tab_id, url, title, committed_at, transition, source
                 FROM journey_nodes WHERE id = ?1",
                [id],
                |row| {
                    Ok(JourneyNodeRecord {
                        id: row.get(0)?,
                        profile_id: row.get(1)?,
                        tab_id: row.get(2)?,
                        url: row.get(3)?,
                        title: row.get(4)?,
                        committed_at: row.get(5)?,
                        transition: row.get(6)?,
                        source: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// Lists durable journey edges oldest first, bounded for native views.
    pub fn journey_edges(&self, limit: usize) -> Result<Vec<JourneyEdgeRecord>, StoreError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = self.connection.prepare(
            "SELECT source_id, target_id, transition, created_at
             FROM journey_edges ORDER BY created_at ASC, rowid ASC LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], |row| {
            Ok(JourneyEdgeRecord {
                source_id: row.get(0)?,
                target_id: row.get(1)?,
                transition: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// Returns the durable current node for one live tab key, if one exists.
    pub fn current_journey_node(&self, tab_id: &str) -> Result<Option<String>, StoreError> {
        self.connection
            .query_row(
                "SELECT node_id FROM journey_current WHERE tab_id = ?1",
                [tab_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// Moves a durable tab pointer to the newest retained node with the
    /// committed URL. Traversal never creates a new journey node.
    pub fn set_current_journey_node_for_url(
        &self,
        tab_id: &str,
        url: &str,
    ) -> Result<bool, StoreError> {
        if !valid_stored_text(tab_id, MAX_STORED_IDENTIFIER_BYTES) || !is_safe_history_url(url) {
            return Err(StoreError::InvalidInput(
                "journey traversal requires safe bounded fields",
            ));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let node_id: Option<String> = transaction
            .query_row(
                "SELECT id FROM journey_nodes
                 WHERE tab_id = ?1 AND url = ?2
                 ORDER BY committed_at DESC, id DESC LIMIT 1",
                params![tab_id, url],
                |row| row.get(0),
            )
            .optional()?;
        let Some(node_id) = node_id else {
            transaction.commit()?;
            return Ok(false);
        };
        transaction.execute(
            "INSERT INTO journey_current (tab_id, node_id) VALUES (?1, ?2)
             ON CONFLICT(tab_id) DO UPDATE SET node_id = excluded.node_id",
            params![tab_id, node_id],
        )?;
        transaction.commit()?;
        Ok(true)
    }

    /// Moves a durable tab pointer to an exact retained node identifier.
    pub fn set_current_journey_node_by_id(
        &self,
        tab_id: &str,
        node_id: &str,
    ) -> Result<bool, StoreError> {
        if !valid_stored_text(tab_id, MAX_STORED_IDENTIFIER_BYTES)
            || !valid_stored_text(node_id, MAX_STORED_IDENTIFIER_BYTES)
        {
            return Err(StoreError::InvalidInput(
                "journey traversal requires safe bounded identifiers",
            ));
        }
        let transaction = self.connection.unchecked_transaction()?;
        let exists: Option<i64> = transaction
            .query_row(
                "SELECT 1 FROM journey_nodes WHERE id = ?1 AND tab_id = ?2",
                params![node_id, tab_id],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            transaction.commit()?;
            return Ok(false);
        }
        transaction.execute(
            "INSERT INTO journey_current (tab_id, node_id) VALUES (?1, ?2)
             ON CONFLICT(tab_id) DO UPDATE SET node_id = excluded.node_id",
            params![tab_id, node_id],
        )?;
        transaction.commit()?;
        Ok(true)
    }

    /// Clears all durable journey state for this profile.
    pub fn clear_journey(&self) -> Result<u64, StoreError> {
        let transaction = self.connection.unchecked_transaction()?;
        let deleted = transaction.execute("DELETE FROM journey_nodes", [])? as u64;
        transaction.commit()?;
        Ok(deleted)
    }
}
