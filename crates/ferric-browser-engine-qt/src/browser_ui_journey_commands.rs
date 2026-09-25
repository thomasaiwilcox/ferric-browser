use super::{
    BTreeSet, CxxQtType, Effect, EngineEffect, Event, JourneyEdgeKind,
    JourneyGraphEdgePresentation, JourneyQuerySnapshot, JourneyReopenRecord, MarkWrite,
    ParsedCommand, Pin, QString, StorageRequest, TabId, Uuid, ValidatedUrl, Value, current_target,
    is_bounded_journey_query, is_safe_history_url, parse_history_clear_arguments, qobject,
    safe_ipc_url, unix_timestamp,
};

impl qobject::BrowserUi {
    pub(super) fn finish_journey_reopen(
        mut self: Pin<&mut Self>,
        node: JourneyReopenRecord,
        target_kind: &str,
        current_tab: Option<TabId>,
        profile_id: Option<String>,
    ) -> Result<Value, String> {
        if !is_safe_history_url(&node.url) {
            return Err("journey node is stale or no longer safe to reopen".into());
        }
        let url = ValidatedUrl::parse(node.url.clone())
            .map_err(|_| "journey node is stale or unavailable".to_owned())?;
        if target_kind == "window" {
            let binding = self.as_ref();
            let rust = binding.rust();
            let profile_name = rust.profile_name.clone();
            let private = rust.profile_persistence.lacks_durable_storage();
            if private {
                return Err(
                    "window-target journey reopening is unavailable for transient profiles".into(),
                );
            }
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action =
                Some(format!("new-window\tfalse\t{}\t{}", profile_name, node.url));
            return Ok(serde_json::json!({
                "status": "accepted",
                "node_id": node.id,
                "target": target_kind,
                "profile_id": profile_id
            }));
        }
        let target = if target_kind == "current" {
            current_target(self.as_ref().rust().state.as_ref(), current_tab)
                .ok_or_else(|| "current tab is unavailable".to_owned())?
        } else {
            let window = self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .and_then(|state| current_tab.and_then(|tab| state.tabs().get(&tab)))
                .map(|tab| tab.window)
                .ok_or_else(|| "current window is unavailable".to_owned())?;
            self.as_mut()
                .reduce_event(Event::OpenTab { window })
                .map_err(|error| error.to_string())?;
            let tab = self
                .as_ref()
                .rust()
                .state
                .as_ref()
                .and_then(|state| state.windows().get(&window))
                .and_then(|window| window.active_tab)
                .ok_or_else(|| "new journey tab was not created".to_owned())?;
            current_target(self.as_ref().rust().state.as_ref(), Some(tab))
                .ok_or_else(|| "new journey tab is unavailable".to_owned())?
        };
        let effects = self
            .as_mut()
            .reduce_event(Event::StartNavigation { target, url })
            .map_err(|error| error.to_string())?;
        if !effects
            .iter()
            .any(|effect| matches!(effect, Effect::Engine(EngineEffect::Navigate { .. })))
        {
            return Err("journey reopen did not produce a navigation target".into());
        }
        self.as_mut()
            .mark_journey_transition(&effects, JourneyEdgeKind::Reopen, "journey-reopen");
        self.as_mut().set_pending_engine_action(&effects);
        self.as_mut().sync_core_tabs();
        Ok(serde_json::json!({
            "status": "accepted",
            "node_id": node.id,
            "target": target_kind,
            "profile_id": profile_id
        }))
    }

    pub(super) fn execute_journey_reopen(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let (node_id, target_kind) = match command.arguments.as_slice() {
            [node] => (node.as_str(), "current"),
            [node, flag, target] if flag == "--target" => (node.as_str(), target.as_str()),
            _ => {
                return Err(
                    "journey-reopen requires NODE_UUID [--target current|tab|window]".into(),
                );
            }
        };
        if !matches!(target_kind, "current" | "tab" | "window") {
            return Err("journey-reopen target must be current, tab, or window".into());
        }
        let (current_tab, profile_id, durable) = {
            let binding = self.as_ref();
            let rust = binding.rust();
            (
                rust.tab,
                rust.profile_id,
                rust.profile_persistence.is_durable(),
            )
        };
        if durable {
            let profile_id =
                profile_id.ok_or_else(|| "durable profile identity is unavailable".to_owned())?;
            self.as_mut()
                .submit_storage(StorageRequest::JourneyNode {
                    id: node_id.to_owned(),
                })
                .map_err(|error| error.to_string())?;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_journey_reopen = Some((
                node_id.to_owned(),
                target_kind.to_owned(),
                current_tab,
                profile_id.to_string(),
            ));
            return Ok(serde_json::json!({
                "status": "queued",
                "node_id": node_id,
                "target": target_kind,
                "profile_id": profile_id,
            }));
        }
        let node = {
            let binding = self.as_ref();
            let rust = binding.rust();
            let profile = rust
                .tab
                .and_then(|tab| rust.state.as_ref()?.tabs().get(&tab))
                .map(|tab| tab.profile);
            let node = rust
                .state
                .as_ref()
                .and_then(|state| {
                    state.journey().nodes().find(|node| {
                        Some(node.profile) == profile && node.id.to_string() == node_id
                    })
                })
                .ok_or_else(|| "journey node is stale or unavailable".to_owned())?;
            JourneyReopenRecord {
                id: node.id.to_string(),
                url: node.url.clone(),
            }
        };
        self.finish_journey_reopen(node, target_kind, current_tab, None)
    }

    pub(super) fn parse_journey_query(
        command: &ParsedCommand,
    ) -> Result<(bool, Option<String>, Option<String>), String> {
        let mut current_only = false;
        let mut search = None;
        let mut expand = None;
        let mut index = 0;
        while index < command.arguments.len() {
            match command.arguments[index].as_str() {
                "--current" if !current_only => current_only = true,
                "--search" | "--expand" => {
                    let option = command.arguments[index].as_str();
                    let value = command
                        .arguments
                        .get(index + 1)
                        .ok_or_else(|| format!("journey {option} requires a value"))?;
                    if !is_bounded_journey_query(value) {
                        return Err(
                            "journey search/node value must be 1..256 bytes without control characters"
                                .into(),
                        );
                    }
                    let slot = if option == "--search" {
                        &mut search
                    } else {
                        &mut expand
                    };
                    if slot.replace(value.clone()).is_some() {
                        return Err(format!("journey {option} may be supplied only once"));
                    }
                    index += 1;
                }
                _ => {
                    return Err(
                        "journey accepts --current, --search TEXT, and --expand NODE_UUID".into(),
                    );
                }
            }
            index += 1;
        }
        if expand.is_some() && (current_only || search.is_some()) {
            return Err("journey --expand cannot be combined with --current or --search".into());
        }
        Ok((current_only, search, expand))
    }

    pub(super) fn apply_journey_query_snapshot(
        mut self: Pin<&mut Self>,
        snapshot: JourneyQuerySnapshot,
    ) {
        let graph_edges = snapshot
            .edges
            .iter()
            .take(1_000)
            .map(|edge| JourneyGraphEdgePresentation {
                source: edge.source_id.clone(),
                target: edge.target_id.clone(),
                transition: edge.transition.clone(),
            })
            .collect::<Vec<_>>();
        let values = snapshot
            .nodes
            .into_iter()
            .map(|node| {
                format!(
                    "{}\t{}\t{}\t{}\t{}",
                    node.id,
                    node.title.replace(['\t', '\n', '\r'], " "),
                    safe_ipc_url(&node.url),
                    node.transition,
                    node.source
                        .unwrap_or_default()
                        .replace(['\t', '\n', '\r'], " ")
                )
            })
            .collect::<Vec<_>>();
        self.as_mut().set_library_kind(QString::from("journey"));
        self.as_mut()
            .set_library_values(QString::from(values.join("\n")));
        self.as_mut().set_library_graph_edges(&graph_edges);
    }

    pub(super) fn execute_memory_journey_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        let (current_only, search, expand) = Self::parse_journey_query(command)?;
        let values = {
            let binding = self.as_ref();
            let rust = binding.rust();
            let state = rust
                .state
                .as_ref()
                .ok_or_else(|| "core state unavailable".to_owned())?;
            let current_tab = rust.tab;
            let current_profile = current_tab
                .and_then(|tab| state.tabs().get(&tab))
                .map(|tab| tab.profile);
            let current_node = current_tab.and_then(|tab| state.journey().current_node(tab));
            let search = search.as_deref().map(str::to_ascii_lowercase);
            let expanded_ids = expand.as_deref().map(|center| {
                let mut ids = BTreeSet::from([center.to_owned()]);
                for edge in state.journey().edges() {
                    if edge.source.to_string() == center {
                        ids.insert(edge.target.to_string());
                    }
                    if edge.target.to_string() == center {
                        ids.insert(edge.source.to_string());
                    }
                }
                ids
            });
            if let Some(center) = expand.as_deref()
                && !state
                    .journey()
                    .nodes()
                    .any(|node| node.id.to_string() == center)
            {
                return Err("journey node is stale or unavailable".into());
            }
            state
                .journey()
                .nodes()
                .filter(|node| {
                    Some(node.profile) == current_profile
                        && (!current_only || Some(node.id) == current_node)
                        && expanded_ids
                            .as_ref()
                            .is_none_or(|ids| ids.contains(&node.id.to_string()))
                        && search.as_ref().is_none_or(|query| {
                            node.title.to_ascii_lowercase().contains(query)
                                || node.url.to_ascii_lowercase().contains(query)
                                || node.transition.name().contains(query)
                                || node.source.as_deref().is_some_and(|source| {
                                    source.to_ascii_lowercase().contains(query)
                                })
                        })
                })
                .map(|node| {
                    format!(
                        "{}\t{}\t{}\t{}\t{}",
                        node.id,
                        node.title.replace(['\t', '\n', '\r'], " "),
                        safe_ipc_url(&node.url),
                        node.transition.name(),
                        node.source.clone().unwrap_or_default()
                    )
                })
                .take(1_000)
                .collect::<Vec<_>>()
        };
        let graph_edges = {
            let binding = self.as_ref();
            let rust = binding.rust();
            let Some(state) = rust.state.as_ref() else {
                return Err("core state unavailable".into());
            };
            let visible_ids = values
                .iter()
                .filter_map(|value| value.split('\t').next())
                .map(ToOwned::to_owned)
                .collect::<BTreeSet<_>>();
            state
                .journey()
                .edges()
                .iter()
                .filter_map(|edge| {
                    let source = edge.source.to_string();
                    let target = edge.target.to_string();
                    (visible_ids.contains(&source) && visible_ids.contains(&target)).then(|| {
                        JourneyGraphEdgePresentation {
                            source,
                            target,
                            transition: edge.kind.name().to_owned(),
                        }
                    })
                })
                .take(1_000)
                .collect::<Vec<_>>()
        };
        self.as_mut().set_library_kind(QString::from("journey"));
        self.as_mut()
            .set_library_values(QString::from(values.join("\n")));
        self.as_mut().set_library_graph_edges(&graph_edges);
        Ok(serde_json::json!({
            "status": "listed",
            "kind": "journey",
            "current_only": current_only,
            "search": search,
            "expanded": expand,
            "count": values.len(),
            "durability": "memory-only"
        }))
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn execute_library_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        if command.name == "journey-reopen" {
            return self.execute_journey_reopen(command);
        }
        if command.name == "journey"
            && self
                .as_ref()
                .rust()
                .profile_persistence
                .lacks_durable_storage()
        {
            return self.execute_memory_journey_command(command);
        }
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let reads_profile_library = matches!(
            command.name.as_str(),
            "history-open"
                | "bookmark-open"
                | "bookmark-edit"
                | "quickmark-open"
                | "quickmark-edit"
                | "bookmark-list"
                | "quickmark-list"
                | "history"
                | "journey"
        );
        if reads_profile_library && command.name != "journey" {
            let private_history_command = self.as_ref().active_profile_is_transient()
                && matches!(
                    command.name.as_str(),
                    "history" | "history-open" | "history-clear"
                );
            if self
                .as_ref()
                .rust()
                .profile_persistence
                .lacks_durable_storage()
                && !private_history_command
            {
                return Err("private profiles have no durable history or marks".into());
            }
            if !private_history_command
                && (self.as_ref().rust().storage_library.is_none()
                    || self.as_ref().rust().storage_library_dirty.get())
            {
                self.as_mut().request_storage_library();
                self.as_mut().set_command_retryable(true);
                return Err("profile library metadata is still loading; retry".into());
            }
        }
        if command.name == "journey" {
            let (current_only, search, expand) = Self::parse_journey_query(command)?;
            let current_tab_id = if current_only {
                Some(
                    self.as_ref()
                        .rust()
                        .tab
                        .ok_or_else(|| "current tab unavailable".to_owned())?
                        .to_string(),
                )
            } else {
                None
            };
            self.as_mut()
                .submit_storage(StorageRequest::JourneyQuery {
                    current_tab_id,
                    search,
                    expand,
                })
                .map_err(|error| error.to_string())?;
            self.as_mut().set_library_kind(QString::from("journey"));
            self.as_mut().set_library_values(QString::default());
            self.as_mut().clear_library_graph_edges();
            return Ok(serde_json::json!({"status": "queued", "kind": "journey"}));
        }
        let binding = self.as_ref();
        let state = binding
            .rust()
            .state
            .as_ref()
            .ok_or_else(|| "core state unavailable".to_owned())?;
        let current = self
            .as_ref()
            .rust()
            .tab
            .and_then(|tab| state.tabs().get(&tab))
            .ok_or_else(|| "current tab unavailable".to_owned())?;
        let current_url = current
            .url
            .as_deref()
            .map_or_else(|| "about:blank".to_owned(), safe_ipc_url);
        let current_title = current.title.clone();
        let command_name = command.name.as_str();
        match command_name {
            "bookmark-add" => {
                let title = match command.arguments.as_slice() {
                    [] => current_title.clone(),
                    [flag, title] if flag == "--title" && !title.is_empty() => title.clone(),
                    _ => return Err("bookmark-add accepts optional --title TEXT".into()),
                };
                if !is_safe_history_url(&current_url) {
                    return Err("current URL is not eligible for a durable bookmark".into());
                }
                let id = Uuid::new_v4().to_string();
                self.as_mut().apply_mark_write(MarkWrite::AddBookmark {
                    id: id.clone(),
                    url: current_url.clone(),
                    title: title.clone(),
                    timestamp: unix_timestamp(),
                })?;
                return Ok(serde_json::json!({
                    "status": "added",
                    "bookmark": {"id": id, "url": current_url, "title": title}
                }));
            }
            "bookmark-delete" => {
                let [id] = command.arguments.as_slice() else {
                    return Err("bookmark-delete requires a bookmark ID".into());
                };
                self.as_mut()
                    .apply_mark_write(MarkWrite::DeleteBookmark { id: id.clone() })?;
                return Ok(serde_json::json!({"status": "queued", "bookmark_id": id}));
            }
            "bookmark-edit" => {
                let [id, flag, title] = command.arguments.as_slice() else {
                    return Err("bookmark-edit requires ID --title TEXT".into());
                };
                if flag != "--title" {
                    return Err("bookmark-edit requires ID --title TEXT".into());
                }
                self.as_mut().apply_mark_write(MarkWrite::EditBookmark {
                    id: id.clone(),
                    title: title.clone(),
                    timestamp: unix_timestamp(),
                })?;
                return Ok(
                    serde_json::json!({"status": "queued", "bookmark_id": id, "title": title}),
                );
            }
            "quickmark-add" => {
                let (name, url) = match command.arguments.as_slice() {
                    [name] => (name.clone(), current_url.clone()),
                    [name, url] => (name.clone(), url.clone()),
                    _ => return Err("quickmark-add requires NAME and optional URL".into()),
                };
                if name.is_empty() || name.chars().any(char::is_control) {
                    return Err("quickmark name is invalid".into());
                }
                if !is_safe_history_url(&url) {
                    return Err("quickmark URL is not eligible for durable storage".into());
                }
                self.as_mut().apply_mark_write(MarkWrite::SetQuickmark {
                    name: name.clone(),
                    url: url.clone(),
                    timestamp: unix_timestamp(),
                })?;
                return Ok(serde_json::json!({
                    "status": "queued",
                    "quickmark": {"name": name, "url": url}
                }));
            }
            "quickmark-delete" => {
                let [name] = command.arguments.as_slice() else {
                    return Err("quickmark-delete requires a name".into());
                };
                self.as_mut()
                    .apply_mark_write(MarkWrite::DeleteQuickmark { name: name.clone() })?;
                return Ok(serde_json::json!({"status": "queued", "quickmark": name}));
            }
            "quickmark-edit" => {
                let [name, url] = command.arguments.as_slice() else {
                    return Err("quickmark-edit requires NAME URL".into());
                };
                if !is_safe_history_url(url) {
                    return Err("quickmark URL is not eligible for durable storage".into());
                }
                self.as_mut().apply_mark_write(MarkWrite::EditQuickmark {
                    name: name.clone(),
                    url: url.clone(),
                    timestamp: unix_timestamp(),
                })?;
                return Ok(serde_json::json!({"status": "queued", "quickmark": name, "url": url}));
            }
            _ => {}
        }
        if command_name == "history-clear" {
            let (since, origin, confirmed) = parse_history_clear_arguments(command)?;
            if !confirmed {
                return Err("history deletion requires explicit confirmation".into());
            }
            if self.as_ref().active_profile_is_transient() {
                let deleted = self
                    .as_mut()
                    .clear_private_history(since, origin.as_deref());
                self.as_mut().set_library_values(QString::default());
                return Ok(serde_json::json!({
                    "status": "cleared",
                    "deleted": deleted,
                    "durability": "memory-only",
                    "since": since,
                    "origin": origin
                }));
            }
            self.as_mut()
                .clear_history_via_worker(since, origin.clone())?;
            return Ok(serde_json::json!({
                "status": "queued",
                "since": since,
                "origin": origin
            }));
        }
        let mut listed_kind = None;
        let mut listed_values = Vec::new();
        let result = match command_name {
            "history-open" | "bookmark-open" | "quickmark-open" => {
                let [entry_id] = command.arguments.as_slice() else {
                    return Err(format!("{command_name} requires one entry ID"));
                };
                let stored_url = match command_name {
                    "history-open" => {
                        let history = if self
                            .as_ref()
                            .rust()
                            .profile_persistence
                            .lacks_durable_storage()
                        {
                            self.as_ref().private_history_records()
                        } else {
                            self.as_ref()
                                .rust()
                                .storage_library
                                .as_ref()
                                .ok_or_else(|| {
                                    "profile library metadata is unavailable".to_owned()
                                })?
                                .history
                                .clone()
                        };
                        history
                            .into_iter()
                            .find(|record| record.id.to_string() == *entry_id)
                            .map(|record| record.url)
                    }
                    "bookmark-open" => self
                        .as_ref()
                        .rust()
                        .storage_library
                        .as_ref()
                        .ok_or_else(|| "profile library metadata is unavailable".to_owned())?
                        .bookmarks
                        .iter()
                        .find(|record| record.id == *entry_id)
                        .map(|record| record.url.clone()),
                    "quickmark-open" => self
                        .as_ref()
                        .rust()
                        .storage_library
                        .as_ref()
                        .ok_or_else(|| "profile library metadata is unavailable".to_owned())?
                        .quickmarks
                        .iter()
                        .find(|record| record.name == *entry_id)
                        .map(|record| record.url.clone()),
                    _ => unreachable!("checked stored open command"),
                }
                .ok_or_else(|| "stored entry was not found".to_owned())?;
                if !is_safe_history_url(&stored_url) {
                    return Err("stored entry is not eligible for safe navigation".into());
                }
                let url = ValidatedUrl::parse(stored_url).map_err(|error| error.to_string())?;
                let target = current_target(
                    self.as_ref().rust().state.as_ref(),
                    self.as_ref().rust().tab,
                )
                .ok_or_else(|| "current tab unavailable".to_owned())?;
                let effects = self
                    .as_mut()
                    .reduce_event(Event::StartNavigation { target, url })
                    .map_err(|error| error.to_string())?;
                self.as_mut().set_pending_engine_action(&effects);
                self.as_mut().sync_core_tabs();
                serde_json::json!({"status": "accepted", "action": "open", "entry_id": entry_id})
            }
            "bookmark-add" => {
                unreachable!("bookmark-add is handled by the worker path above")
            }
            "bookmark-delete" => {
                unreachable!("bookmark-delete is handled by the worker path above")
            }
            "bookmark-list" => {
                if !command.arguments.is_empty() {
                    return Err("bookmark-list does not accept arguments".into());
                }
                listed_kind = Some("bookmarks");
                listed_values = self
                    .as_ref()
                    .rust()
                    .storage_library
                    .clone()
                    .ok_or_else(|| "profile library metadata is unavailable".to_owned())?
                    .bookmarks
                    .iter()
                    .map(|bookmark| {
                        format!(
                            "{}\t{}\t{}",
                            bookmark.id,
                            bookmark.title.replace(['\t', '\n', '\r'], " "),
                            safe_ipc_url(&bookmark.url)
                        )
                    })
                    .collect();
                serde_json::json!({"status": "listed", "kind": "bookmarks", "count": listed_values.len()})
            }
            "quickmark-add" => {
                unreachable!("quickmark-add is handled by the worker path above")
            }
            "quickmark-delete" => {
                unreachable!("quickmark-delete is handled by the worker path above")
            }
            "quickmark-list" => {
                if !command.arguments.is_empty() {
                    return Err("quickmark-list does not accept arguments".into());
                }
                listed_kind = Some("quickmarks");
                listed_values = self
                    .as_ref()
                    .rust()
                    .storage_library
                    .clone()
                    .ok_or_else(|| "profile library metadata is unavailable".to_owned())?
                    .quickmarks
                    .iter()
                    .map(|mark| format!("{}\t{}", mark.name, safe_ipc_url(&mark.url)))
                    .collect();
                serde_json::json!({"status": "listed", "kind": "quickmarks", "count": listed_values.len()})
            }
            "history" => {
                if !command.arguments.is_empty() {
                    return Err("history does not accept arguments".into());
                }
                listed_kind = Some("history");
                let history = if self
                    .as_ref()
                    .rust()
                    .profile_persistence
                    .lacks_durable_storage()
                {
                    self.as_ref().private_history_records()
                } else {
                    self.as_ref()
                        .rust()
                        .storage_library
                        .clone()
                        .ok_or_else(|| "profile library metadata is unavailable".to_owned())?
                        .history
                };
                listed_values = history
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
                    .collect();
                serde_json::json!({"status": "listed", "kind": "history", "count": listed_values.len()})
            }
            "history-clear" => {
                unreachable!("history-clear is handled by the worker path above")
            }
            _ => return Err("not a library command".into()),
        };
        if matches!(
            command_name,
            "bookmark-add"
                | "bookmark-delete"
                | "bookmark-edit"
                | "quickmark-add"
                | "quickmark-delete"
                | "quickmark-edit"
                | "history-clear"
        ) {
            self.as_ref().rust().storage_library_dirty.set(true);
        }
        if let Some(kind) = listed_kind {
            self.as_mut().set_library_kind(QString::from(kind));
            self.as_mut()
                .set_library_values(QString::from(listed_values.join("\n")));
            self.as_mut().clear_library_graph_edges();
        } else {
            self.as_mut().set_library_kind(QString::default());
            self.as_mut().set_library_values(QString::default());
            self.as_mut().clear_library_graph_edges();
        }
        Ok(result)
    }
}
