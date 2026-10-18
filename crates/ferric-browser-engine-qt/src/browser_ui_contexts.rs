use super::{
    BrowserApplication, ContextCommand, ContextMember, ContextSnapshot, ContextTabDescriptor,
    CxxQtType, Event, ExistenceState, ParsedCommand, Pin, PublicError, QString, Value, qobject,
    safe_ipc_url, sanitize_untrusted_title,
};
use ferric_browser_ipc::ErrorCode;
use ferric_browser_storage::ContextRegistryError;
use std::fmt::Write as _;

impl qobject::BrowserUi {
    #[allow(clippy::too_many_lines)]
    pub(super) fn create_context_from_parts(
        mut self: Pin<&mut Self>,
        name: &str,
        label: &str,
        profile: &str,
        workspace: Option<&str>,
    ) -> Result<Value, String> {
        let effect = self
            .as_mut()
            .apply_context_change(ContextCommand::Create {
                name: name.to_owned(),
                label: label.to_owned(),
                profile: profile.to_owned(),
                workspace: workspace.map(ToOwned::to_owned),
            })
            .map_err(|error| error.to_string())?;
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
    ) -> Result<ferric_browser_application::ContextEffect, ContextRegistryError> {
        let mut effect = {
            let mut rust = self.as_mut().rust_mut();
            rust.as_mut()
                .get_mut()
                .state
                .as_mut()
                .ok_or(ContextRegistryError::NotFound)?
                .change_context(command)?
        };
        let _ = self
            .as_mut()
            .observe_runtime_effects(std::mem::take(&mut effect.runtime_effects));
        let snapshot = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(BrowserApplication::contexts)
            .map(ContextSnapshot::from_records);
        self.as_mut().rust_mut().as_mut().get_mut().contexts = snapshot;
        self.as_mut().sync_context_choices();
        self.as_mut().sync_context_metadata();
        self.as_mut().publish_context_catalog();
        Ok(effect)
    }

    pub(super) fn execute_context_delete_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, PublicError> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| {
                PublicError::new(
                    ErrorCode::InvalidArgument,
                    "This context command can't run in the current mode.",
                    error.to_string(),
                )
            })?;
        let (name, _, _, _, confirmed) = self
            .as_ref()
            .context_command_arguments(command)
            .map_err(|error| PublicError::new(ErrorCode::InvalidArgument, error.clone(), error))?;
        if self.as_ref().rust().profile_bootstrap_pending {
            return Err(PublicError::new(
                ErrorCode::Busy,
                "The profile is still opening. Try deleting the context again shortly.",
                "context deletion requested during profile bootstrap",
            ));
        }
        if self.as_ref().active_profile_is_transient() {
            return Err(PublicError::new(
                ErrorCode::Denied,
                "Saved contexts can't be deleted from a private or temporary window. Open a regular profile first.",
                "context deletion requires a durable normal profile",
            ));
        }
        if self.as_ref().rust().storage_roots.is_none() {
            return Err(PublicError::new(
                ErrorCode::Storage,
                "Saved contexts are unavailable in this window. Wait for the profile to open and try again.",
                "context deletion has no durable storage roots",
            ));
        }
        if !confirmed {
            return Err(PublicError::new(
                ErrorCode::ConfirmationRequired,
                "Review the context name, then use :context-delete NAME --confirm to delete it.",
                "context deletion was not confirmed",
            ));
        }
        let effect = self.as_mut().apply_context_change(ContextCommand::Remove { name })
            .map_err(|error| {
                let (code, message) = match &error {
                    ContextRegistryError::DefinedInConfiguration => (ErrorCode::Config, error.to_string()),
                    ContextRegistryError::NotFound => (ErrorCode::NotFound,
                        "The context is no longer available. Run :config-reload to refresh saved contexts.".into()),
                    _ => (ErrorCode::Storage,
                        "Context deletion couldn't be confirmed. Run :config-reload before trying again; details are available in diagnostics.".into()),
                };
                PublicError::new(code, message, error.to_string())
            })?;
        let context = effect.context;
        self.as_mut()
            .set_status_text(QString::from(format!("Context deleted: {}", context.label)));
        Ok(serde_json::json!({
            "status": "deleted",
            "context": {"id": context.id.to_string(), "name": context.name}
        }))
    }

    pub(super) fn receive_context_catalog(
        mut self: Pin<&mut Self>,
        catalog: std::sync::Arc<ferric_browser_application::ContextCatalog>,
        revision: u64,
    ) {
        let effects = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            if this.profile_bootstrap_pending {
                None
            } else {
                this.state
                    .as_mut()
                    .and_then(|state| state.refresh_context_catalog(&catalog))
            }
        };
        let Some(effects) = effects else {
            return;
        };
        let _ = self.as_mut().observe_runtime_effects(effects);
        let snapshot = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(BrowserApplication::contexts)
            .map(ContextSnapshot::from_records);
        {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            this.context_catalog_revision = this.context_catalog_revision.max(revision);
            this.contexts = snapshot;
        }
        self.as_mut().remember_context_catalog(revision, catalog);
        self.as_mut().sync_context_choices();
        self.sync_context_metadata();
    }

    /// Publish the committed registry, including command-created contexts.
    /// Routing definitions remain independent inputs in `contexts_json`.
    pub(super) fn sync_context_choices(mut self: Pin<&mut Self>) {
        let records = self.as_ref().rust().contexts.clone().unwrap_or_default();
        self.as_mut().set_context_choice_labels(
            records
                .contexts()
                .iter()
                .map(|context| QString::from(&context.label))
                .collect(),
        );
        self.as_mut().set_context_choice_profiles(
            records
                .contexts()
                .iter()
                .map(|context| QString::from(&context.profile))
                .collect(),
        );
        // Names are the presentation readiness signal; publish them last.
        self.set_context_choice_names(
            records
                .contexts()
                .iter()
                .map(|context| QString::from(&context.name))
                .collect(),
        );
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn execute_context_command(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
    ) -> Result<Value, String> {
        if command.name == "context-delete" {
            return self
                .execute_context_delete_command(command)
                .map_err(|error| error.to_string());
        }
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        if command.name == "context-route" {
            return self.execute_context_route_command(command);
        }
        let (name, label, profile, workspace, _) =
            self.as_ref().context_command_arguments(command)?;
        if command.name == "context-list" {
            let result = self.as_ref().ipc_contexts_query(&serde_json::json!({}))?;
            let records = self.as_ref().rust().contexts.clone().ok_or_else(|| {
                if self.as_ref().active_profile_is_transient() {
                    "Saved contexts are unavailable in private or temporary windows.".to_owned()
                } else {
                    "Contexts are unavailable in this window. Wait for the profile to open and try again.".to_owned()
                }
            })?;
            let contexts = records.contexts();
            let status = if contexts.is_empty() {
                "No contexts yet. Create one with :context-create NAME --profile PROFILE.".into()
            } else {
                let mut names = contexts
                    .iter()
                    .take(3)
                    .map(|context| {
                        if context.label == context.name {
                            context.name.clone()
                        } else {
                            format!("{} ({})", context.label, context.name)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                if contexts.len() > 3 {
                    let _ = write!(names, "; {} more", contexts.len() - 3);
                }
                self.as_mut().execute_switcher_command(&ParsedCommand {
                    name: "switcher".into(),
                    arguments: vec!["--scope".into(), "contexts".into()],
                })?;
                format!("Contexts: {names}. Choose a context to switch.")
            };
            self.as_mut().set_status_text(QString::from(status));
            return Ok(result);
        }
        if self.as_ref().active_profile_is_transient() {
            return Err("ephemeral and private profiles cannot use durable contexts".into());
        }
        if self.as_ref().rust().storage_roots.is_none() {
            return Err("transient profiles cannot create or delete durable contexts".into());
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
                    self.as_mut().set_status_text(QString::from(format!(
                        "Entered context: {}",
                        context.label
                    )));
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
                self.as_mut()
                    .set_status_text(QString::from(format!("Entered context: {}", context.label)));
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
                    let url = tab
                        .url
                        .as_deref()
                        .filter(|url| super::url_safety::is_safe_tab_descriptor(url))?;
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
                })
                .map_err(|error| error.to_string())?;
            let status = if skipped_tabs == 0 {
                format!("Context saved: {}", context.label)
            } else {
                format!(
                    "Context saved: {}; {skipped_tabs} tab(s) without a safe restore address were left out",
                    context.label
                )
            };
            self.as_mut().set_status_text(QString::from(status));
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
                .set_status_text(QString::from("Checking the context profile…"));
            return Ok(serde_json::json!({
                "status": "accepted",
                "pending": "profile-lookup",
                "context": name,
                "profile": self.as_ref().rust().pending_context_create.as_ref().map(|value| value.2.clone())
            }));
        }
        unreachable!("validated context command returned above")
    }
}
