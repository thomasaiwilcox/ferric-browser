//! Switcher and completion query projections.

#[allow(clippy::wildcard_imports)]
use super::*;

impl qobject::BrowserUi {
    pub(super) fn ipc_switcher_query(&self, params: &Value) -> Result<Value, String> {
        let object = query_object(
            params,
            "switcher.query",
            &[
                "query",
                "scope",
                "limit",
                "offset",
                "include_private",
                "private_scope",
            ],
        )?;
        let query_input = query_optional_string(object, "switcher.query", "query")?.unwrap_or("");
        let query = tokenize_switcher_query(query_input);
        if query.iter().any(|term| term.len() > 128) {
            return Err("switcher.query terms are too long".into());
        }
        let scope = query_optional_string(object, "switcher.query", "scope")?.unwrap_or("all");
        let scopes = [
            "all",
            "tabs",
            "windows",
            "contexts",
            "commands",
            "history",
            "marks",
            "sessions",
            "downloads",
            "closed",
            "actions",
        ];
        if !scopes.contains(&scope) {
            return Err(format!("unknown switcher scope: {scope}"));
        }
        let limit = query_limit(object, "switcher.query", 50)?;
        let offset = query_offset(object, "switcher.query")?;
        let include_private = query_bool_param(object, "switcher.query", "include_private", false)?;
        if include_private && !self.ipc_private_queries_enabled() {
            // A switcher explicitly opened inside a transient profile is
            // allowed to search only that same in-memory profile. External
            // IPC queries still require the existing opt-in configuration.
            if object.get("private_scope").is_none() {
                return Err("private switcher queries are disabled by configuration".into());
            }
        }
        let Some(state) = self.rust().state.as_ref() else {
            return Err("core state unavailable".into());
        };
        let current_context = state
            .active_tab()
            .and_then(|tab| state.windows().get(&tab.window))
            .and_then(|window| window.context.as_deref());
        let private_scope = query_optional_string(object, "switcher.query", "private_scope")?;
        if let Some(scope) = private_scope {
            let active_scope = state
                .active_tab()
                .and_then(|tab| state.profiles().get(&tab.profile))
                .filter(|profile| profile.privacy.is_transient())
                .map(|profile| profile.label.as_str());
            if active_scope != Some(scope) {
                return Err("private switcher scope is stale or unavailable".into());
            }
        }
        let contexts = self.rust().contexts.as_ref();
        let mut candidates = Vec::<SwitcherCandidate>::new();
        let mut add = |kind: &str,
                       id: String,
                       label: String,
                       secondary: String,
                       profile: Option<String>,
                       context: Option<(String, String)>,
                       workspace: Option<String>,
                       private: bool,
                       recency: i64,
                       generation: Option<u64>,
                       fields: Vec<String>,
                       actions: &[&str]| {
            if let Some(scope) = private_scope
                && !matches!(kind, "command" | "action")
                && profile.as_deref() != Some(scope)
            {
                return;
            }
            if private
                && (!include_private
                    || private_scope.is_some_and(|scope| profile.as_deref() != Some(scope)))
            {
                return;
            }
            let Some(rank) = switcher_rank(&query, &id, &label, &fields) else {
                return;
            };
            let rank = rank.saturating_add(switcher_context_boost(
                current_context,
                context.as_ref().map(|(_, name)| name.as_str()),
            ));
            let value = serde_json::json!({
                "kind": kind,
                "id": id,
                "owner_token": self.rust().window_token.to_string(),
                "generation": generation,
                "label": label,
                "secondary": secondary.replace(['\n', '\r'], " "),
                "profile": profile,
                "context": context.as_ref().map(|(id, name)| serde_json::json!({"id": id, "name": name})),
                "workspace": workspace,
                "privacy": if private { "private" } else { "normal" },
                "recency": recency,
                "rank": rank,
                "match_fields": fields,
                "actions": actions
            });
            candidates.push(SwitcherCandidate {
                rank,
                kind: kind.to_owned(),
                recency,
                value,
            });
        };

        if scope == "all" || scope == "tabs" {
            for tab in state.tabs().values() {
                let Some(profile_state) = state.profiles().get(&tab.profile) else {
                    continue;
                };
                let private = profile_state.privacy.is_transient();
                let context = state
                    .windows()
                    .get(&tab.window)
                    .and_then(|window| window.context.as_deref())
                    .and_then(|name| {
                        contexts?
                            .contexts()
                            .iter()
                            .find(|context| context.name == name)
                    })
                    .map(|context| (context.id.to_string(), context.name.clone()));
                let workspace = context.as_ref().and_then(|(_, name)| {
                    contexts?
                        .contexts()
                        .iter()
                        .find(|context| context.name == *name)?
                        .workspace
                        .clone()
                });
                let url = tab
                    .url
                    .as_deref()
                    .map_or_else(|| "about:blank".into(), safe_ipc_url);
                add(
                    "tab",
                    tab.id.to_string(),
                    if tab.title.is_empty() {
                        url.clone()
                    } else {
                        tab.title.clone()
                    },
                    url.clone(),
                    Some(profile_state.label.clone()),
                    context,
                    workspace,
                    private,
                    i64::try_from(state.revision()).unwrap_or(i64::MAX),
                    Some(tab.generation),
                    vec![tab.title.clone(), url, profile_state.label.clone()],
                    &["focus", "open"],
                );
            }
        }
        if scope == "all" || scope == "windows" {
            for window in state.windows().values() {
                let Some(profile_state) = state.profiles().get(&window.profile) else {
                    continue;
                };
                let private = profile_state.privacy.is_transient();
                let context = window.context.as_deref().and_then(|name| {
                    contexts?
                        .contexts()
                        .iter()
                        .find(|context| context.name == name)
                });
                let context_pair =
                    context.map(|context| (context.id.to_string(), context.name.clone()));
                add(
                    "window",
                    window.id.to_string(),
                    format!("Window {}", window.id),
                    format!("{} tab(s)", window.tabs.len()),
                    Some(profile_state.label.clone()),
                    context_pair,
                    context.and_then(|context| context.workspace.clone()),
                    private,
                    i64::try_from(state.revision()).unwrap_or(i64::MAX),
                    None,
                    vec![profile_state.label.clone(), window.id.to_string()],
                    &["focus"],
                );
            }
        }
        if (scope == "all" || scope == "contexts") && contexts.is_some() {
            for context in contexts.into_iter().flat_map(ContextSnapshot::contexts) {
                let private = state
                    .profiles()
                    .values()
                    .find(|profile| profile.label == context.profile)
                    .is_some_and(|profile| profile.privacy.is_transient());
                add(
                    "context",
                    context.id.to_string(),
                    context.label.clone(),
                    context.name.clone(),
                    Some(context.profile.clone()),
                    Some((context.id.to_string(), context.name.clone())),
                    context.workspace.clone(),
                    private,
                    context.updated_at,
                    None,
                    vec![
                        context.name.clone(),
                        context.label.clone(),
                        context.profile.clone(),
                    ],
                    &["enter"],
                );
            }
        }
        if scope == "all" || scope == "commands" {
            for definition in self.rust().registry.definitions() {
                add(
                    "command",
                    definition.action.to_string(),
                    definition.name.clone(),
                    definition.description.clone(),
                    None,
                    None,
                    None,
                    false,
                    0,
                    None,
                    vec![definition.name.clone(), definition.description.clone()],
                    &["execute", "help"],
                );
            }
        }
        if scope == "all" || scope == "actions" {
            let actions = ActionRegistry::default_v1();
            for definition in actions.definitions() {
                if !definition.sources.contains(&ActionSource::Switcher)
                    || definition
                        .arguments
                        .iter()
                        .any(|argument| argument.required)
                {
                    continue;
                }
                add(
                    "action",
                    definition.id.clone(),
                    definition.label.clone(),
                    definition.description.clone(),
                    None,
                    None,
                    None,
                    false,
                    0,
                    None,
                    vec![
                        definition.id.clone(),
                        definition.subject.as_str().into(),
                        definition.verb.clone(),
                        definition.description.clone(),
                    ],
                    &["execute"],
                );
            }
            for value in configured_switcher_action_values(
                &self.rust().config,
                self.active_profile_is_transient(),
            )? {
                let Some(subject) = value.get("subject").and_then(Value::as_str) else {
                    continue;
                };
                let Some(id) = value.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let description = value
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                add(
                    "action",
                    id.to_owned(),
                    value
                        .get("label")
                        .and_then(Value::as_str)
                        .unwrap_or("Configured action")
                        .to_owned(),
                    description.clone(),
                    None,
                    None,
                    None,
                    false,
                    0,
                    None,
                    vec![
                        id.to_owned(),
                        subject.to_owned(),
                        "send".into(),
                        description,
                    ],
                    &["execute"],
                );
            }
        }
        if scope == "all" || scope == "closed" {
            for closed in &self.rust().closed_tabs {
                add(
                    "closed",
                    closed.id.to_string(),
                    if closed.title.is_empty() {
                        closed.url.clone()
                    } else {
                        closed.title.clone()
                    },
                    closed.url.clone(),
                    Some(closed.profile.clone()),
                    None,
                    None,
                    closed.private,
                    closed.closed_at,
                    None,
                    vec![
                        closed.title.clone(),
                        closed.url.clone(),
                        closed.profile.clone(),
                    ],
                    &["reopen"],
                );
            }
        }
        let indexed_library = self.rust().switcher_library_index.as_ref().filter(|index| {
            self.rust().switcher_library_index_revision == self.rust().storage_library_revision
                && index.profile_name == self.rust().profile_name
        });
        if let Some(index) = indexed_library {
            let profile = index.profile_name.clone();
            for candidate in &index.candidates {
                let actions: &[&str] = match candidate.kind.as_str() {
                    "history" => &["open"],
                    "bookmark" | "quickmark" => &["open", "delete"],
                    "download" => &["show", "open"],
                    _ => &[],
                };
                add(
                    &candidate.kind,
                    candidate.id.clone(),
                    candidate.label.clone(),
                    candidate.secondary.clone(),
                    Some(profile.clone()),
                    None,
                    None,
                    false,
                    candidate.recency,
                    None,
                    candidate.fields.clone(),
                    actions,
                );
            }
        } else if let Some(library) = self.rust().storage_library.as_ref() {
            let profile = self.rust().profile_name.clone();
            if scope == "all" || scope == "history" {
                for page in &library.history {
                    add(
                        "history",
                        page.id.to_string(),
                        if page.title.is_empty() {
                            page.url.clone()
                        } else {
                            page.title.clone()
                        },
                        page.url.clone(),
                        Some(profile.clone()),
                        None,
                        None,
                        false,
                        page.last_visit,
                        None,
                        vec![page.title.clone(), page.url.clone()],
                        &["open"],
                    );
                }
            }
            if scope == "all" || scope == "marks" {
                for mark in &library.bookmarks {
                    add(
                        "bookmark",
                        mark.id.clone(),
                        if mark.title.is_empty() {
                            mark.url.clone()
                        } else {
                            mark.title.clone()
                        },
                        mark.url.clone(),
                        Some(profile.clone()),
                        None,
                        None,
                        false,
                        mark.updated_at,
                        None,
                        vec![mark.title.clone(), mark.url.clone()],
                        &["open", "delete"],
                    );
                }
                for mark in &library.quickmarks {
                    add(
                        "quickmark",
                        mark.name.clone(),
                        mark.name.clone(),
                        mark.url.clone(),
                        Some(profile.clone()),
                        None,
                        None,
                        false,
                        0,
                        None,
                        vec![mark.name.clone(), mark.url.clone()],
                        &["open", "delete"],
                    );
                }
            }
            if scope == "all" || scope == "downloads" {
                for download in &library.downloads {
                    add(
                        "download",
                        download.id.clone(),
                        download.destination.clone(),
                        download.source_url.clone(),
                        Some(profile.clone()),
                        None,
                        None,
                        false,
                        download.created_at,
                        None,
                        vec![download.destination.clone(), download.source_url.clone()],
                        &["show", "open"],
                    );
                }
            }
        } else if include_private && self.active_profile_is_transient() {
            let profile = self.rust().profile_name.clone();
            if scope == "all" || scope == "history" {
                for page in &self.rust().private_history {
                    add(
                        "history",
                        page.id.to_string(),
                        if page.title.is_empty() {
                            page.url.clone()
                        } else {
                            page.title.clone()
                        },
                        page.url.clone(),
                        Some(profile.clone()),
                        None,
                        None,
                        true,
                        page.last_visit,
                        None,
                        vec![page.title.clone(), page.url.clone()],
                        &["open"],
                    );
                }
            }
        }
        if (scope == "all" || scope == "sessions")
            && let Some(profile_id) = self.rust().profile_id
        {
            let profile = self.rust().profile_name.clone();
            for name in &self.rust().session_names {
                add(
                    "session",
                    format!("{profile_id}:{name}"),
                    name.clone(),
                    "saved session".into(),
                    Some(profile.clone()),
                    None,
                    None,
                    false,
                    0,
                    None,
                    vec![name.clone()],
                    &["load-preview", "load"],
                );
            }
        }
        let (total, results) = select_switcher_page(candidates, offset, limit);
        Ok(serde_json::json!({
            "sequence": self.rust().ipc_sequence,
            "scope": scope,
            "query": query.join(" "),
            "offset": offset,
            "limit": limit,
            "has_more": offset.saturating_add(results.len()) < total,
            "results": results
        }))
    }
}
