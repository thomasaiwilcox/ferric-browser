use super::{
    BrowserApplication, ContextCommand, ContextMember, ContextSnapshot, ContextTabDescriptor,
    CxxQtType, Event, ExistenceState, ParsedCommand, Pin, QString, Value, is_safe_history_url,
    qobject, safe_ipc_url, sanitize_untrusted_title,
};

impl qobject::BrowserUi {
    #[allow(clippy::too_many_lines)]
    pub(super) fn create_context_from_parts(
        mut self: Pin<&mut Self>,
        name: &str,
        label: &str,
        profile: &str,
        workspace: Option<&str>,
    ) -> Result<Value, String> {
        let effect = self.as_mut().apply_context_change(ContextCommand::Create {
            name: name.to_owned(),
            label: label.to_owned(),
            profile: profile.to_owned(),
            workspace: workspace.map(ToOwned::to_owned),
        })?;
        let context = effect.context;
        Ok(serde_json::json!({
            "status": "created",
            "context": {
                "id": context.id.to_string(),
                "name": context.name,
                "label": context.label,
                "profile": context.profile,
                "workspace": context.workspace
            }
        }))
    }

    pub(super) fn apply_context_change(
        mut self: Pin<&mut Self>,
        command: ContextCommand,
    ) -> Result<ferric_browser_application::ContextEffect, String> {
        let effect = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .state
                .as_mut()
                .ok_or_else(|| "application state is unavailable".to_owned())?
                .change_context(command)
                .map_err(|error| error.to_string())?
        };
        let snapshot = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(BrowserApplication::contexts)
            .map(ContextSnapshot::from_records);
        self.as_mut().rust_mut().as_mut().get_mut().contexts = snapshot;
        Ok(effect)
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn execute_context_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if command.name == "context-route" {
            return self.execute_context_route_command(command);
        }
        let (name, label, profile, workspace, confirmed) =
            self.as_ref().context_command_arguments(command)?;
        if command.name == "context-list" {
            return self.as_ref().ipc_contexts_query(&serde_json::json!({}));
        }
        if self.as_ref().active_profile_is_transient() {
            return Err("ephemeral and private profiles cannot use durable contexts".into());
        }
        if self.as_ref().rust().storage_roots.is_none() {
            return Err("transient profiles cannot create or delete durable contexts".into());
        }
        if command.name == "context-delete" && !confirmed {
            return Err("context deletion requires explicit confirmation".into());
        }
        if matches!(command.name.as_str(), "context-enter" | "context-save") {
            let context_name = if command.name == "context-save" && name.is_empty() {
                let binding = self.as_ref();
                let state = binding
                    .rust()
                    .state
                    .as_ref()
                    .ok_or_else(|| "core state unavailable".to_owned())?;
                let window = self
                    .as_ref()
                    .rust()
                    .window
                    .and_then(|window| state.windows().get(&window))
                    .ok_or_else(|| "current window unavailable".to_owned())?;
                window
                    .context
                    .clone()
                    .ok_or_else(|| "current window is not in a context".to_owned())?
            } else {
                name.clone()
            };
            let context = self
                .as_ref()
                .rust()
                .contexts
                .as_ref()
                .ok_or_else(|| "context registry unavailable".to_owned())?
                .contexts()
                .iter()
                .find(|context| context.name == context_name)
                .cloned()
                .ok_or_else(|| format!("context not found: {context_name}"))?;
            let window_id = self
                .as_ref()
                .rust()
                .window
                .ok_or_else(|| "current window unavailable".to_owned())?;
            if command.name == "context-enter" {
                let current_profile = self.as_ref().rust().profile_name.clone();
                let current_window_context = self
                    .as_ref()
                    .rust()
                    .state
                    .as_ref()
                    .and_then(|state| state.windows().get(&window_id))
                    .and_then(|window| window.context.clone());

                // A same-runtime member is the strongest signal: entering a
                // context must focus it even when the context's configured
                // target is `window`.
                let live_member_tab = context.members.iter().rev().find_map(|member| {
                    if member.window_id != window_id.to_string() {
                        return None;
                    }
                    let selected = member.selected_tab.as_deref()?;
                    self.as_ref()
                        .rust()
                        .state
                        .as_ref()?
                        .windows()
                        .get(&window_id)?
                        .tabs
                        .iter()
                        .copied()
                        .find(|tab| tab.to_string() == selected)
                });
                let live_member_window = self.as_ref().rust().state.as_ref().and_then(|state| {
                    context.members.iter().rev().find_map(|member| {
                        let member_window = state
                            .windows()
                            .keys()
                            .find(|candidate| candidate.to_string() == member.window_id)
                            .copied()?;
                        (member_window != window_id
                            && state.windows().get(&member_window).is_some_and(|window| {
                                state
                                    .profiles()
                                    .get(&window.profile)
                                    .is_some_and(|profile| profile.label == context.profile)
                                    && window.tabs.iter().any(|tab| {
                                        state.tabs().get(tab).is_some_and(|tab| {
                                            tab.existence == ExistenceState::Live
                                        })
                                    })
                            }))
                        .then_some(member_window)
                    })
                });
                let reuse_or_window_needs_new = context.default_target == "reuse-or-window"
                    && current_window_context
                        .as_deref()
                        .is_some_and(|current| current != context.name);
                let needs_new_window = !self.as_ref().rust().context_entry_force_reuse
                    && live_member_tab.is_none()
                    && live_member_window.is_none()
                    && (context.profile != current_profile
                        || context.default_target == "window"
                        || reuse_or_window_needs_new);
                if needs_new_window {
                    let restore_url = context
                        .members
                        .iter()
                        .rev()
                        .find_map(|member| {
                            let selected = member
                                .selected_index
                                .filter(|index| *index < member.tab_descriptors.len())
                                .unwrap_or(0);
                            member
                                .tab_descriptors
                                .get(selected)
                                .or_else(|| member.tab_descriptors.first())
                                .and_then(|descriptor| descriptor.safe_restore_url.clone())
                        })
                        .unwrap_or_else(|| "about:blank".into());
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .pending_engine_action = Some(format!(
                        "context-window\tfalse\t{}\t{}\t{}",
                        context.profile, context.name, restore_url
                    ));
                    self.as_mut().set_status_text(QString::from(format!(
                        "Context {} requires a separate {} window",
                        context.name, context.profile
                    )));
                    return Ok(serde_json::json!({
                        "status": "accepted",
                        "target_window": "pending",
                        "context": {"id": context.id.to_string(), "name": context.name},
                        "profile": context.profile
                    }));
                }
                if let Some(member_window) = live_member_window {
                    self.as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .pending_engine_action = Some(format!("window-focus\t{member_window}"));
                    self.as_mut().set_status_text(QString::from(format!(
                        "Context {} is already open in another window",
                        context.name
                    )));
                    return Ok(serde_json::json!({
                        "status": "accepted",
                        "focused_member": true,
                        "target_window": member_window.to_string(),
                        "context": {"id": context.id.to_string(), "name": context.name}
                    }));
                }
                if context.profile != current_profile {
                    return Err("context belongs to a different profile".into());
                }
                self.as_mut()
                    .reduce_event(Event::SetWindowContext {
                        window: window_id,
                        context: Some(context.name.clone()),
                    })
                    .map_err(|error| error.clone())?;

                // A member captured during the same runtime can still be
                // focused exactly. After restart its opaque runtime IDs are
                // stale, so fall back to the bounded safe descriptors.
                if let Some(tab) = live_member_tab {
                    self.as_mut()
                        .reduce_event(Event::FocusWindow { window: window_id })
                        .map_err(|error| error.to_string())?;
                    self.as_mut()
                        .reduce_event(Event::ActivateTab {
                            window: window_id,
                            tab,
                        })
                        .map_err(|error| error.to_string())?;
                    self.as_mut().sync_tab_order_from_core();
                    let result = serde_json::json!({
                        "status": "entered",
                        "focused_member": true,
                        "restored_tab_descriptors": 0,
                        "context": {"id": context.id.to_string(), "name": context.name}
                    });
                    return Ok(result);
                }

                let saved_member = context
                    .members
                    .iter()
                    .rev()
                    .find(|member| !member.tab_descriptors.is_empty())
                    .cloned();
                let restored_tab_descriptors = if let Some(member) = saved_member {
                    self.as_mut().restore_context_membership(&member)?
                } else {
                    0
                };
                self.as_mut().sync_core_tabs();
                let result = serde_json::json!({
                    "status": "entered",
                    "focused_member": false,
                    "restored_tab_descriptors": restored_tab_descriptors,
                    "context": {"id": context.id.to_string(), "name": context.name}
                });
                return Ok(result);
            }
            let binding = self.as_ref();
            let state = binding
                .rust()
                .state
                .as_ref()
                .ok_or_else(|| "core state unavailable".to_owned())?;
            let window = state
                .windows()
                .get(&window_id)
                .ok_or_else(|| "current window unavailable".to_owned())?;
            let tabs = window
                .tabs
                .iter()
                .filter(|tab| {
                    state
                        .tabs()
                        .get(tab)
                        .is_some_and(|tab| tab.existence == ExistenceState::Live)
                })
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            let tab_descriptors = window
                .tabs
                .iter()
                .filter_map(|tab_id| {
                    let tab = state.tabs().get(tab_id)?;
                    if tab.existence != ExistenceState::Live {
                        return None;
                    }
                    let url = tab.url.as_deref().filter(|url| is_safe_history_url(url))?;
                    Some(ContextTabDescriptor {
                        tab_id: tab.id.to_string(),
                        safe_restore_url: Some(safe_ipc_url(url)),
                        title: sanitize_untrusted_title(&tab.title),
                        pinned: tab.pinned,
                        muted: tab.muted,
                        zoom: tab.zoom,
                    })
                })
                .collect::<Vec<_>>();
            let selected_index = window.active_tab.and_then(|active| {
                tab_descriptors
                    .iter()
                    .position(|descriptor| descriptor.tab_id == active.to_string())
            });
            let skipped_tabs = tabs.len().saturating_sub(tab_descriptors.len());
            let saved_tab_descriptors = tab_descriptors.len();
            let members = vec![ContextMember {
                window_id: window.id.to_string(),
                tabs,
                selected_tab: window.active_tab.map(|tab| tab.to_string()),
                tab_descriptors,
                selected_index,
            }];
            self.as_mut()
                .apply_context_change(ContextCommand::SaveMembership {
                    name: context_name.clone(),
                    members,
                })?;
            let result = serde_json::json!({
                "status": "saved",
                "context": {"id": context.id.to_string(), "name": context.name},
                "saved_tab_descriptors": saved_tab_descriptors,
                "skipped_unsafe_tabs": skipped_tabs,
                "workspace": self.as_ref().rust().contexts.as_ref()
                    .and_then(|contexts| contexts.contexts().iter()
                        .find(|context| context.name == context_name))
                    .and_then(|context| context.workspace.clone())
            });
            return Ok(result);
        }
        if command.name == "context-create" {
            let queued = self.as_mut().request_profile_list_with_mode(false);
            if !queued {
                return Err("profile list is unavailable or busy".into());
            }
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_context_create = Some((name.clone(), label, profile, workspace));
            self.as_mut()
                .set_status_text(QString::from("Context profile lookup requested"));
            return Ok(serde_json::json!({
                "status": "accepted",
                "pending": "profile-lookup",
                "context": name,
                "profile": self.as_ref().rust().pending_context_create.as_ref().map(|value| value.2.clone())
            }));
        }
        let result = match command.name.as_str() {
            "context-create" => unreachable!("context-create is handled by the profile worker"),
            "context-delete" => self
                .as_mut()
                .apply_context_change(ContextCommand::Remove { name: name.clone() })
                .map(|effect| effect.context)
                .map(|context| {
                    serde_json::json!({
                        "status": "deleted",
                        "context": {"id": context.id.to_string(), "name": context.name}
                    })
                }),
            _ => unreachable!("context-list returned above"),
        };
        result
    }
}
