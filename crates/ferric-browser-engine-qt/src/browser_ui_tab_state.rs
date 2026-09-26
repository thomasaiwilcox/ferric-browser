use super::{
    CompletionCandidate, CompletionCategory, CxxQtType, DEFAULT_COMPLETION_LIMIT, Effect,
    EngineEffect, Event, ExistenceState, JourneyEdgeKind, JourneyNodeId, LoadingState,
    MAX_JOURNEY_REDIRECT_HOPS, Mode, Pin, QString, SearchCase, TabId, TabTransfer, Target,
    ValidatedUrl, Value, complete, current_target, engine_action_name, qobject,
    recordable_same_document_change, safe_ipc_url, sanitize_untrusted_title,
    scalar_cursor_to_utf16, setting_metadata_all, strip_url_fragment, utf16_cursor_to_scalar,
};

impl qobject::BrowserUi {
    pub(super) fn set_core_mode(mut self: Pin<&mut Self>, mode: Mode) {
        let leaving_grid = self.as_ref().rust().core_mode == Mode::Grid && mode != Mode::Grid;
        if leaving_grid {
            self.as_mut().clear_spatial_session_state();
        }
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.core_mode = mode;
        if let Some(bindings) = this.bindings.as_mut() {
            bindings.set_mode(mode);
        }
    }

    pub(super) fn tab_for_index(&self, index: i32) -> Option<TabId> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.rust().tab_ids.get(index).copied())
    }

    pub(super) fn tab_index_for_id(self: Pin<&mut Self>, id: &QString) -> i32 {
        let id = id.to_string();
        if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
            return -1;
        }
        self.as_ref()
            .rust()
            .tab_ids
            .iter()
            .position(|candidate| candidate.to_string() == id)
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1)
    }

    pub(super) fn tab_id_for_index(self: Pin<&mut Self>, index: i32) -> QString {
        self.as_ref()
            .tab_for_index(index)
            .map_or_else(QString::default, |tab| QString::from(tab.to_string()))
    }

    pub(super) fn tab_transfer_payload(self: Pin<&mut Self>, id: &QString) -> QString {
        let id = id.to_string();
        let tab = self
            .as_ref()
            .rust()
            .tab_ids
            .iter()
            .find(|candidate| candidate.to_string() == id)
            .copied();
        let Some(tab) = tab else {
            return QString::default();
        };
        let binding = self.as_ref();
        let Some(tab_state) = binding
            .rust()
            .state
            .as_ref()
            .and_then(|state| state.tabs().get(&tab))
        else {
            return QString::default();
        };
        if tab_state.existence != ExistenceState::Live {
            return QString::default();
        }
        let loading = match tab_state.loading {
            LoadingState::Idle => 0,
            LoadingState::Provisional => 1,
            LoadingState::Committed => 2,
            LoadingState::Complete => 3,
            LoadingState::Failed => 4,
            LoadingState::Cancelled => 5,
        };
        let zoom_hundredths = (tab_state.zoom * 100.0).round() as u32;
        let payload = serde_json::json!({
            "tab_id": tab.to_string(),
            "profile_name": self.as_ref().rust().profile_name,
            "url": tab_state.url.as_deref().map(safe_ipc_url),
            "title": tab_state.title,
            "loading": loading,
            "pinned": tab_state.pinned,
            "muted": tab_state.muted,
            "zoom_hundredths": zoom_hundredths.clamp(25, 500),
        });
        QString::from(payload.to_string())
    }

    pub(super) fn adopt_tab_transfer(mut self: Pin<&mut Self>, payload: &QString) -> bool {
        let parsed = match serde_json::from_str::<Value>(&payload.to_string()) {
            Ok(Value::Object(object)) => object,
            _ => {
                self.set_status_text(QString::from("Tab transfer payload is invalid"));
                return false;
            }
        };
        if parsed.get("profile_name").and_then(Value::as_str)
            != Some(self.as_ref().rust().profile_name.as_str())
        {
            self.set_status_text(QString::from(
                "Tab transfer rejected: profile boundary mismatch",
            ));
            return false;
        }
        let Some(window) = self.as_ref().rust().window else {
            self.set_status_text(QString::from("Tab transfer has no destination window"));
            return false;
        };
        let url = match parsed.get("url") {
            None | Some(Value::Null) => None,
            Some(Value::String(url)) => match ValidatedUrl::parse(url) {
                Ok(url) => Some(url.to_string()),
                Err(error) => {
                    self.as_mut().set_status_text(QString::from(format!(
                        "Tab transfer URL rejected: {error}"
                    )));
                    return false;
                }
            },
            _ => {
                self.set_status_text(QString::from("Tab transfer URL is invalid"));
                return false;
            }
        };
        let loading = match parsed.get("loading").and_then(Value::as_u64) {
            Some(0) => LoadingState::Idle,
            Some(1) => LoadingState::Provisional,
            Some(2) => LoadingState::Committed,
            Some(3) => LoadingState::Complete,
            Some(4) => LoadingState::Failed,
            Some(5) => LoadingState::Cancelled,
            _ => {
                self.set_status_text(QString::from("Tab transfer loading state is invalid"));
                return false;
            }
        };
        let Some(zoom_hundredths) = parsed
            .get("zoom_hundredths")
            .and_then(Value::as_u64)
            .and_then(|zoom| u32::try_from(zoom).ok())
            .filter(|zoom| (25..=500).contains(zoom))
        else {
            self.set_status_text(QString::from("Tab transfer zoom is invalid"));
            return false;
        };
        let title = parsed
            .get("title")
            .and_then(Value::as_str)
            .map(sanitize_untrusted_title)
            .unwrap_or_default();
        let pinned = parsed
            .get("pinned")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let muted = parsed
            .get("muted")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if self
            .as_mut()
            .reduce_event(Event::TransferTabIn {
                window,
                transfer: TabTransfer {
                    url,
                    title,
                    loading,
                    pinned,
                    muted,
                    zoom_hundredths,
                },
            })
            .is_err()
        {
            self.set_status_text(QString::from("Tab transfer adoption was rejected"));
            return false;
        }
        self.as_mut().sync_tab_order_from_core();
        self.set_status_text(QString::from("Live tab adopted in same-profile window"));
        true
    }

    pub(super) fn complete_tab_transfer(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let id = id.to_string();
        let Some(tab) = self
            .as_ref()
            .rust()
            .tab_ids
            .iter()
            .find(|candidate| candidate.to_string() == id)
            .copied()
        else {
            self.set_status_text(QString::from("Tab transfer source is stale"));
            return false;
        };
        if self
            .as_mut()
            .reduce_event(Event::TransferTabOut { tab })
            .is_err()
        {
            self.set_status_text(QString::from("Tab transfer source could not be released"));
            return false;
        }
        let needs_fallback = self
            .as_ref()
            .rust()
            .window
            .and_then(|window| {
                self.as_ref()
                    .rust()
                    .state
                    .as_ref()
                    .and_then(|state| state.windows().get(&window))
                    .map(|window| window.tabs.is_empty())
            })
            .unwrap_or(false);
        if needs_fallback
            && let Some(window) = self.as_ref().rust().window
            && self
                .as_mut()
                .reduce_event(Event::OpenTab { window })
                .is_err()
        {
            self.set_status_text(QString::from("Source window fallback tab failed"));
            return false;
        }
        self.as_mut().sync_tab_order_from_core();
        self.set_status_text(QString::from("Live tab moved to same-profile window"));
        true
    }

    pub(super) fn rollback_tab_transfer(mut self: Pin<&mut Self>, id: &QString) -> bool {
        let id = id.to_string();
        let Some(tab) = self
            .as_ref()
            .rust()
            .tab_ids
            .iter()
            .find(|candidate| candidate.to_string() == id)
            .copied()
        else {
            self.set_status_text(QString::from("Tab transfer rollback target is stale"));
            return false;
        };
        if self
            .as_mut()
            .reduce_event(Event::TransferTabOut { tab })
            .is_err()
        {
            self.set_status_text(QString::from("Tab transfer rollback was rejected"));
            return false;
        }
        let needs_fallback = self.as_ref().rust().window.is_some_and(|window| {
            self.as_ref()
                .rust()
                .state
                .as_ref()
                .and_then(|state| state.windows().get(&window))
                .is_some_and(|window| window.tabs.is_empty())
        });
        if needs_fallback
            && let Some(window) = self.as_ref().rust().window
            && self
                .as_mut()
                .reduce_event(Event::OpenTab { window })
                .is_err()
        {
            self.set_status_text(QString::from("Tab transfer rollback fallback failed"));
            return false;
        }
        self.as_mut().sync_tab_order_from_core();
        self.set_status_text(QString::from("Tab transfer adoption rolled back"));
        true
    }

    pub(super) fn clear_completion(mut self: Pin<&mut Self>) {
        self.as_mut().set_completion_text(QString::default());
        self.as_mut().set_completion_values(QString::default());
        self.as_mut().set_completion_start(0);
        self.as_mut().set_completion_end(0);
        self.as_mut().set_completion_selected(-1);
        self.set_completion_visible(false);
    }

    pub(super) fn completion_catalog(&self) -> Vec<CompletionCandidate> {
        let rust = self.rust();
        let mut catalog = setting_metadata_all()
            .iter()
            .map(|metadata| CompletionCandidate {
                insert_text: metadata.key.to_owned(),
                label: metadata.key.to_owned(),
                detail: format!(
                    "{} · {} · {} · default {}",
                    metadata.value_type,
                    metadata.apply_time,
                    metadata.supported_scopes.join("/"),
                    metadata.default_value
                ),
                category: CompletionCategory::Setting,
                recency: 0,
                frequency: 0,
            })
            .collect::<Vec<_>>();
        let Some(state) = rust.state.as_ref() else {
            return catalog;
        };
        let Some(window) = rust.window else {
            return catalog;
        };
        catalog.extend(
            state
                .tabs()
                .values()
                .filter(|tab| tab.window == window)
                .filter_map(|tab| {
                    tab.url.as_ref().map(|url| {
                        let safe_url = safe_ipc_url(url);
                        let profile = state
                            .profiles()
                            .get(&tab.profile)
                            .map_or(rust.profile_name.as_str(), |profile| profile.label.as_str());
                        CompletionCandidate {
                            insert_text: safe_url.clone(),
                            label: if tab.title.is_empty() {
                                safe_url.clone()
                            } else {
                                tab.title.clone()
                            },
                            detail: format!("{safe_url} · {profile}"),
                            category: CompletionCategory::Tab,
                            recency: 0,
                            frequency: 0,
                        }
                    })
                })
                .collect::<Vec<_>>(),
        );
        if let Some(library) = rust.storage_library.as_ref() {
            catalog.extend(library.history.iter().map(|page| {
                let safe_url = safe_ipc_url(&page.url);
                CompletionCandidate {
                    insert_text: safe_url.clone(),
                    label: if page.title.is_empty() {
                        safe_url.clone()
                    } else {
                        page.title.clone()
                    },
                    detail: format!(
                        "{safe_url} · {} · {} visits",
                        rust.profile_name, page.visit_count
                    ),
                    category: CompletionCategory::History,
                    recency: u64::try_from(page.last_visit).unwrap_or(0),
                    frequency: u32::try_from(page.visit_count).unwrap_or(u32::MAX),
                }
            }));
            catalog.extend(library.bookmarks.iter().map(|bookmark| {
                let safe_url = safe_ipc_url(&bookmark.url);
                CompletionCandidate {
                    insert_text: safe_url.clone(),
                    label: if bookmark.title.is_empty() {
                        safe_url.clone()
                    } else {
                        bookmark.title.clone()
                    },
                    detail: format!("{safe_url} · {} · bookmark", rust.profile_name),
                    category: CompletionCategory::Bookmark,
                    recency: u64::try_from(bookmark.updated_at).unwrap_or(0),
                    frequency: 0,
                }
            }));
            catalog.extend(library.quickmarks.iter().map(|mark| {
                let safe_url = safe_ipc_url(&mark.url);
                CompletionCandidate {
                    insert_text: safe_url.clone(),
                    label: mark.name.clone(),
                    detail: format!("{safe_url} · {} · quickmark", rust.profile_name),
                    category: CompletionCategory::Bookmark,
                    recency: 0,
                    frequency: 0,
                }
            }));
        } else if self.active_profile_is_transient()
            && rust
                .config
                .get("privacy")
                .and_then(|privacy| privacy.get("private_history_suggestions"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            catalog.extend(rust.private_history.iter().map(|page| {
                let safe_url = safe_ipc_url(&page.url);
                CompletionCandidate {
                    insert_text: safe_url.clone(),
                    label: if page.title.is_empty() {
                        safe_url.clone()
                    } else {
                        page.title.clone()
                    },
                    detail: format!(
                        "{safe_url} · {} · private history · {} visits",
                        rust.profile_name, page.visit_count
                    ),
                    category: CompletionCategory::History,
                    recency: u64::try_from(page.last_visit).unwrap_or(0),
                    frequency: u32::try_from(page.visit_count).unwrap_or(u32::MAX),
                }
            }));
        }
        catalog
    }

    pub(super) fn update_completion(
        mut self: Pin<&mut Self>,
        input: &QString,
        cursor: i32,
    ) -> bool {
        if self.as_ref().rust().core_mode != Mode::Command {
            self.as_mut().clear_completion();
            return false;
        }
        if self.as_ref().rust().storage_library.is_none()
            || self.as_ref().rust().storage_library_dirty.get()
        {
            self.as_mut().request_storage_library();
        }
        let input = input.to_string();
        let cursor = utf16_cursor_to_scalar(&input, usize::try_from(cursor).unwrap_or(0));
        let registry = self.as_ref().rust().registry.clone();
        let result = complete(
            &input,
            cursor,
            &registry,
            &self.as_ref().get_ref().completion_catalog(),
            DEFAULT_COMPLETION_LIMIT,
        );
        let values = result
            .candidates
            .iter()
            .map(|candidate| candidate.insert_text.replace(['\n', '\r'], " "))
            .collect::<Vec<_>>();
        let display = result
            .candidates
            .iter()
            .map(|candidate| {
                format!(
                    "{}\t{}\t{}",
                    candidate.category.as_str(),
                    sanitize_untrusted_title(&candidate.label).replace('\t', " "),
                    crate::presentation_text::sanitize_display_text(&candidate.detail, false)
                        .replace('\t', " ")
                )
            })
            .collect::<Vec<_>>();
        self.as_mut()
            .set_completion_values(QString::from(values.join("\n")));
        self.as_mut()
            .set_completion_text(QString::from(display.join("\n")));
        self.as_mut().set_completion_start(
            i32::try_from(scalar_cursor_to_utf16(&input, result.replacement_start))
                .unwrap_or(i32::MAX),
        );
        self.as_mut().set_completion_end(
            i32::try_from(scalar_cursor_to_utf16(&input, result.replacement_end))
                .unwrap_or(i32::MAX),
        );
        self.as_mut().set_completion_selected(-1);
        self.as_mut()
            .set_completion_visible(!result.candidates.is_empty());
        !result.candidates.is_empty()
    }

    pub(super) fn completion_move(self: Pin<&mut Self>, delta: i32) {
        let values = self.as_ref().rust().completion_values.to_string();
        let count = values.lines().count();
        if count == 0 {
            return;
        }
        let current = self.as_ref().rust().completion_selected;
        let count = i64::try_from(count).unwrap_or(i64::MAX);
        let next = if current < 0 {
            if delta < 0 { count - 1 } else { 0 }
        } else {
            (i64::from(current) + i64::from(delta)).rem_euclid(count)
        };
        self.set_completion_selected(i32::try_from(next).unwrap_or(0));
    }

    pub(super) fn completion_select(self: Pin<&mut Self>, index: i32) {
        let count = self
            .as_ref()
            .rust()
            .completion_values
            .to_string()
            .lines()
            .count();
        if let Ok(index) = usize::try_from(index)
            && index < count
        {
            self.set_completion_selected(i32::try_from(index).unwrap_or(0));
        }
    }

    pub(super) fn set_pending_engine_action(mut self: Pin<&mut Self>, effects: &[Effect]) {
        let traversal_target = effects.iter().find_map(|effect| match effect {
            Effect::Engine(EngineEffect::TraverseHistory { target, .. }) => Some(*target),
            _ => None,
        });
        let action = effects.iter().find_map(|effect| match effect {
            Effect::Engine(EngineEffect::Navigate { target, url }) => {
                let index = self
                    .as_ref()
                    .rust()
                    .tab_ids
                    .iter()
                    .position(|tab| *tab == target.tab)?;
                Some(format!("navigate\t{index}\t{}", url.as_str()))
            }
            Effect::Engine(EngineEffect::Reload { bypass_cache, .. }) => {
                Some(format!("reload\t{bypass_cache}"))
            }
            Effect::Engine(EngineEffect::TraverseHistory { offset, .. }) => {
                let direction = if *offset < 0 { "back" } else { "forward" };
                Some(format!("{direction}\t{}", offset.unsigned_abs()))
            }
            Effect::Engine(EngineEffect::FindText { backward, case, .. }) => Some(format!(
                "find\t{backward}\t{}",
                match case {
                    SearchCase::Smart => "smart",
                    SearchCase::Sensitive => "sensitive",
                    SearchCase::Insensitive => "insensitive",
                }
            )),
            _ => engine_action_name(effect).map(ToOwned::to_owned),
        });
        if let Some(action) = action {
            let mut rust = self.as_mut().rust_mut();
            let rust = rust.as_mut().get_mut();
            rust.pending_engine_action = Some(action);
            rust.pending_journey_traversal = traversal_target;
        }
    }

    pub(super) fn take_journey_traversal(mut self: Pin<&mut Self>, target: Target) -> bool {
        let queued_target = self
            .as_ref()
            .rust()
            .pending_journey_traversals
            .front()
            .copied();
        if let Some(queued_target) = queued_target {
            if queued_target != target {
                return false;
            }
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_journey_traversals
                .pop_front();
            return true;
        }
        if self
            .as_ref()
            .rust()
            .pending_journey_traversal
            .is_some_and(|pending| pending == target)
        {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_journey_traversal = None;
            true
        } else {
            false
        }
    }

    pub(super) fn note_navigation_started(mut self: Pin<&mut Self>, target: Target, url: &str) {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let previous = this
            .pending_navigation_urls
            .insert(target.tab, url.to_owned());
        if previous
            .as_deref()
            .is_some_and(|previous| strip_url_fragment(previous) != strip_url_fragment(url))
        {
            this.pending_redirect_tabs.insert(target.tab);
            let hops = this.pending_redirect_hops.entry(target.tab).or_default();
            *hops = hops.saturating_add(1).min(MAX_JOURNEY_REDIRECT_HOPS);
        }
    }

    pub(super) fn note_navigation_url_change(mut self: Pin<&mut Self>, target: Target, url: &str) {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let Some(started_url) = this.pending_navigation_urls.get(&target.tab) else {
            return;
        };
        if strip_url_fragment(started_url) != strip_url_fragment(url) {
            this.pending_redirect_tabs.insert(target.tab);
            let hops = this.pending_redirect_hops.entry(target.tab).or_default();
            *hops = hops.saturating_add(1).min(MAX_JOURNEY_REDIRECT_HOPS);
            this.pending_navigation_urls
                .insert(target.tab, url.to_owned());
        }
    }

    pub(super) fn should_record_same_document_visit(&self, target: Target, url: &str) -> bool {
        let rust = self.rust();
        let pending = rust.pending_navigation_urls.contains_key(&target.tab);
        let previous = rust
            .state
            .as_ref()
            .and_then(|state| state.tabs().get(&target.tab))
            .and_then(|tab| tab.url.as_deref());
        recordable_same_document_change(pending, previous, url)
    }

    pub(super) fn take_journey_redirect(mut self: Pin<&mut Self>, target: Target) -> Option<u8> {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.pending_navigation_urls.remove(&target.tab);
        let observed = this.pending_redirect_tabs.remove(&target.tab);
        let hops = this.pending_redirect_hops.remove(&target.tab);
        observed.then(|| hops.unwrap_or(1))
    }

    pub(super) fn take_journey_transition(
        mut self: Pin<&mut Self>,
        target: Target,
    ) -> Option<(JourneyEdgeKind, String)> {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        let position = this
            .pending_journey_transitions
            .iter()
            .position(|(pending, _, _)| *pending == target)?;
        let (_, transition, source) = this.pending_journey_transitions.remove(position);
        Some((transition, source))
    }

    pub(super) fn mark_journey_transition(
        mut self: Pin<&mut Self>,
        effects: &[Effect],
        transition: JourneyEdgeKind,
        source: &str,
    ) {
        let Some(target) = effects.iter().find_map(|effect| match effect {
            Effect::Engine(EngineEffect::Navigate { target, .. }) => Some(*target),
            _ => None,
        }) else {
            return;
        };
        self.as_mut()
            .mark_journey_transition_for_target(target, transition, source);
    }

    pub(super) fn mark_journey_transition_for_target(
        mut self: Pin<&mut Self>,
        target: Target,
        transition: JourneyEdgeKind,
        source: &str,
    ) {
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if let Some(existing) = this
            .pending_journey_transitions
            .iter_mut()
            .find(|(pending, _, _)| *pending == target)
        {
            *existing = (target, transition, source.to_owned());
        } else {
            this.pending_journey_transitions
                .push((target, transition, source.to_owned()));
            if this.pending_journey_transitions.len() > 256 {
                this.pending_journey_transitions.remove(0);
            }
        }
    }

    pub(super) fn mark_journey_parent(
        mut self: Pin<&mut Self>,
        effects: &[Effect],
        parent: JourneyNodeId,
    ) {
        let Some(target) = effects.iter().find_map(|effect| match effect {
            Effect::Engine(EngineEffect::Navigate { target, .. }) => Some(*target),
            _ => None,
        }) else {
            return;
        };
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_journey_parent = Some((target, parent));
    }

    pub(super) fn take_journey_parent(
        mut self: Pin<&mut Self>,
        target: Target,
    ) -> Option<JourneyNodeId> {
        if !self
            .as_ref()
            .rust()
            .pending_journey_parent
            .as_ref()
            .is_some_and(|(pending, _)| *pending == target)
        {
            return None;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_journey_parent
            .take()
            .map(|(_, parent)| parent)
    }

    pub(super) fn enter_search(mut self: Pin<&mut Self>, backward: bool) {
        let Some(window) = self.as_ref().rust().window else {
            self.set_status_text(QString::from("No active window"));
            return;
        };
        if self
            .as_mut()
            .reduce_event(Event::PushMode {
                window,
                mode: Mode::Search,
            })
            .is_err()
        {
            self.set_status_text(QString::from("Search mode rejected"));
            return;
        }
        self.as_mut().set_core_mode(Mode::Search);
        self.as_mut().set_mode(QString::from("search"));
        self.as_mut().set_search_backward(backward);
        self.as_mut().set_search_text(QString::default());
        self.set_status_text(QString::from(if backward {
            "Search backward"
        } else {
            "Search forward"
        }));
    }

    pub(super) fn search_changed(mut self: Pin<&mut Self>, input: &QString) {
        if self.as_ref().rust().core_mode != Mode::Search {
            return;
        }
        let query = input.to_string();
        self.as_mut().set_search_text(input.clone());
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        );
        let Some(target) = target else {
            self.set_status_text(QString::from("Search target is no longer live"));
            return;
        };
        let backward = self.as_ref().rust().search_backward;
        match self.as_mut().reduce_event(Event::StartSearch {
            target,
            query,
            backward,
            case: SearchCase::Smart,
        }) {
            Ok(effects) => {
                self.as_mut().set_pending_engine_action(&effects);
                self.set_status_text(QString::from("Searching"));
            }
            Err(error) => self.set_status_text(QString::from(error)),
        }
    }

    pub(super) fn search_next(mut self: Pin<&mut Self>, backward: bool) {
        let target = current_target(
            self.as_ref().rust().state.as_ref(),
            self.as_ref().rust().tab,
        );
        let Some(target) = target else {
            self.set_status_text(QString::from("Search target is no longer live"));
            return;
        };
        match self
            .as_mut()
            .reduce_event(Event::SearchNext { target, backward })
        {
            Ok(effects) => {
                self.as_mut().set_search_backward(backward);
                self.as_mut().set_pending_engine_action(&effects);
                self.set_status_text(QString::from("Search match requested"));
            }
            Err(error) => self.set_status_text(QString::from(error)),
        }
    }

    pub(super) fn accept_search(mut self: Pin<&mut Self>) {
        let Some(window) = self.as_ref().rust().window else {
            return;
        };
        if self
            .as_mut()
            .reduce_event(Event::PopMode { window })
            .is_err()
        {
            return;
        }
        self.as_mut().set_core_mode(Mode::Normal);
        self.as_mut().set_mode(QString::from("normal"));
        self.set_status_text(QString::from("Search retained"));
    }

    pub(super) fn set_active_tab_properties(mut self: Pin<&mut Self>, index: i32, tab: TabId) {
        if self.as_ref().rust().core_mode == Mode::Grid {
            self.as_mut()
                .spatial_invalidated(&QString::from("target-changed"));
        }
        let (url, title, load_state) = {
            let rust = self.as_ref().get_ref().rust();
            let Some(state) = rust.state.as_ref() else {
                return;
            };
            let Some(tab_state) = state.tabs().get(&tab) else {
                return;
            };
            (
                tab_state
                    .url
                    .clone()
                    .unwrap_or_else(|| "about:blank".into()),
                tab_state.title.clone(),
                format!("{:?}", tab_state.loading).to_ascii_lowercase(),
            )
        };
        self.as_mut().set_active_tab_index(index);
        self.as_mut().update_current_url(QString::from(&url));
        self.as_mut().set_initial_url(QString::from(&url));
        self.as_mut().set_page_title(QString::from(&title));
        self.as_mut().set_load_state(QString::from(&load_state));
    }
}
