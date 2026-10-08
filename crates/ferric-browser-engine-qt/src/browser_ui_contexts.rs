use super::context_capture_handoff::{
    BindingExpectation, ContextMembershipCandidate, record_bound_capture,
    record_confirmed_membership, snapshot_captures, snapshot_memberships, stage_capture,
    stage_membership, take_bound_capture_receipts, take_capture, take_confirmed_memberships,
    take_membership,
};
use super::{
    BrowserApplication, ContextCommand, ContextMember, ContextSnapshot, ContextTabDescriptor,
    CxxQtType, Event, ExistenceState, Instant, ParsedCommand, Pin, QString, QStringList, Uuid,
    Value, is_safe_history_url, qobject, safe_ipc_url, sanitize_untrusted_title,
};
use ferric_browser_application::versioning::PreparedContextCandidateCommit;
use ferric_browser_core::WindowId;
use ferric_browser_storage::ContextRecord;

pub(super) struct PendingContextCapture {
    id: Uuid,
    expires_at: Instant,
    source_window: WindowId,
    owner_token: String,
    expected: Vec<BindingExpectation>,
    plan: PreparedContextCandidateCommit,
}

pub(super) struct PendingContextSave {
    id: Uuid,
    expires_at: Instant,
    source_window: WindowId,
    owner_token: String,
    source_revision: u64,
    source_context: ContextRecord,
    expected: Vec<BindingExpectation>,
    members: Vec<ContextMember>,
    saved_descriptors: usize,
    skipped_unsafe_tabs: usize,
}

fn current_context_member(
    state: &BrowserApplication,
    window_id: WindowId,
    context_name: &str,
) -> Result<(ContextMember, usize), String> {
    let window = state
        .windows()
        .get(&window_id)
        .filter(|window| window.context.as_deref() == Some(context_name))
        .ok_or_else(|| "context window changed".to_owned())?;
    let tabs = window
        .tabs
        .iter()
        .filter(|tab| {
            state.tabs().get(tab).is_some_and(|tab| {
                tab.existence == ExistenceState::Live
                    && tab.window == window_id
                    && tab.profile == window.profile
            })
        })
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if tabs.is_empty() || tabs.len() != window.tabs.len() {
        return Err("context window tabs changed".to_owned());
    }
    let tab_descriptors = window
        .tabs
        .iter()
        .filter_map(|tab_id| {
            let tab = state.tabs().get(tab_id)?;
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
    let skipped = tabs.len().saturating_sub(tab_descriptors.len());
    let selected_index = window.active_tab.and_then(|active| {
        tab_descriptors
            .iter()
            .position(|descriptor| descriptor.tab_id == active.to_string())
    });
    Ok((
        ContextMember {
            window_id: window.id.to_string(),
            tabs,
            selected_tab: window.active_tab.map(|tab| tab.to_string()),
            tab_descriptors,
            selected_index,
        },
        skipped,
    ))
}

impl qobject::BrowserUi {
    #[allow(clippy::too_many_lines)]
    pub(super) fn prepare_staged_context_save(
        mut self: Pin<&mut Self>,
        context_name: &QString,
        handles: &QStringList,
    ) -> QString {
        let result = (|| {
            let names = handles.iter().map(ToString::to_string).collect::<Vec<_>>();
            let staged = snapshot_memberships(&names)
                .ok_or_else(|| "context membership handles are stale or ambiguous".to_owned())?;
            let binding = self.as_ref();
            let rust = binding.rust();
            if rust
                .pending_context_save
                .as_ref()
                .is_some_and(|pending| pending.expires_at > Instant::now())
            {
                return Err("another context save is pending".to_owned());
            }
            let window = rust.window.ok_or("source window is unavailable")?;
            let state = rust
                .state
                .as_ref()
                .ok_or("application state is unavailable")?;
            let name = context_name.to_string();
            let (profile_uuid, _) = state
                .context_membership_owner(window, &name)
                .map_err(|error| error.to_string())?;
            let profile = state.profile_snapshot().ok_or("profile is unavailable")?;
            let mut matching = state
                .contexts()
                .ok_or("context registry is unavailable")?
                .iter()
                .filter(|context| context.name == name && context.profile == profile.name);
            let context = matching.next().ok_or("context is unavailable")?;
            if matching.next().is_some() {
                return Err("context identity is ambiguous".to_owned());
            }
            let mut seen = std::collections::BTreeSet::new();
            let mut members = Vec::with_capacity(staged.len());
            let mut saved_descriptors = 0usize;
            let mut skipped_unsafe_tabs = 0usize;
            for entry in &staged {
                let candidate = &entry.value;
                if !seen.insert(entry.window)
                    || candidate.profile_uuid != profile_uuid
                    || candidate.context_name != name
                    || candidate.member.window_id != entry.window.to_string()
                    || candidate.member.tabs.is_empty()
                {
                    return Err("context member identity or profile changed".to_owned());
                }
                saved_descriptors = saved_descriptors
                    .checked_add(candidate.member.tab_descriptors.len())
                    .ok_or("context descriptor count is invalid")?;
                skipped_unsafe_tabs = skipped_unsafe_tabs
                    .checked_add(candidate.skipped_unsafe_tabs)
                    .ok_or("context skipped-tab count is invalid")?;
                members.push(candidate.member.clone());
            }
            if !seen.contains(&window)
                || staged.iter().any(|entry| {
                    entry.window == window && entry.owner_token != rust.window_token.to_string()
                })
            {
                return Err("source context owner is missing or stale".to_owned());
            }
            Ok(PendingContextSave {
                id: Uuid::new_v4(),
                expires_at: Instant::now() + std::time::Duration::from_secs(30),
                source_window: window,
                owner_token: rust.window_token.to_string(),
                source_revision: state.revision(),
                source_context: context.clone(),
                expected: staged
                    .into_iter()
                    .map(|entry| BindingExpectation {
                        handle: entry.handle,
                        window: entry.window,
                        owner_token: entry.owner_token,
                    })
                    .collect(),
                members,
                saved_descriptors,
                skipped_unsafe_tabs,
            })
        })();
        match result {
            Ok(pending) => {
                let id = pending.id.to_string();
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_context_save = Some(pending);
                QString::from(id)
            }
            Err(error) => {
                self.as_mut().set_status_text(QString::from(error));
                QString::default()
            }
        }
    }

    pub(super) fn finish_staged_context_save(mut self: Pin<&mut Self>, plan_id: &QString) -> bool {
        let requested = plan_id.to_string();
        let ready = {
            let binding = self.as_ref();
            let rust = binding.rust();
            rust.pending_context_save.as_ref().is_some_and(|pending| {
                pending.id.to_string() == requested
                    && pending.expires_at > Instant::now()
                    && rust.window == Some(pending.source_window)
                    && rust.window_token.to_string() == pending.owner_token
                    && rust.state.as_ref().is_some_and(|state| {
                        state.revision() == pending.source_revision
                            && state
                                .context_membership_owner(
                                    pending.source_window,
                                    &pending.source_context.name,
                                )
                                .is_ok()
                            && state.contexts().is_some_and(|contexts| {
                                contexts
                                    .iter()
                                    .find(|context| context.id == pending.source_context.id)
                                    == Some(&pending.source_context)
                            })
                    })
                    && take_confirmed_memberships(&pending.expected)
            })
        };
        if !ready {
            self.as_mut()
                .set_status_text(QString::from("Context save owners are stale or incomplete"));
            return false;
        }
        let pending = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_context_save
            .take()
            .expect("verified pending context save");
        let name = pending.source_context.name;
        match self
            .as_mut()
            .apply_context_change(ContextCommand::SaveMembership {
                name,
                members: pending.members,
            }) {
            Ok(_) => {
                self.as_mut().set_status_text(QString::from(format!(
                    "Context membership saved ({} descriptors, {} unsafe tabs skipped)",
                    pending.saved_descriptors, pending.skipped_unsafe_tabs
                )));
                true
            }
            Err(error) => {
                self.as_mut().set_status_text(QString::from(error));
                false
            }
        }
    }

    pub(super) fn cancel_staged_context_save(mut self: Pin<&mut Self>, plan_id: &QString) -> bool {
        let requested = plan_id.to_string();
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if this
            .pending_context_save
            .as_ref()
            .is_none_or(|pending| pending.id.to_string() != requested)
        {
            return false;
        }
        this.pending_context_save.take().is_some()
    }

    pub(super) fn stage_context_window_membership(
        mut self: Pin<&mut Self>,
        context_name: &QString,
    ) -> QString {
        let result = (|| {
            let binding = self.as_ref();
            let rust = binding.rust();
            let window = rust.window.ok_or("context window is unavailable")?;
            let state = rust
                .state
                .as_ref()
                .ok_or("application state is unavailable")?;
            let name = context_name.to_string();
            let (profile_uuid, native) = state
                .context_membership_owner(window, &name)
                .map_err(|error| error.to_string())?;
            let (member, skipped_unsafe_tabs) = current_context_member(state, window, &name)?;
            let candidate = ContextMembershipCandidate {
                profile_uuid,
                native,
                application_revision: state.revision(),
                context_name: name,
                member,
                skipped_unsafe_tabs,
            };
            stage_membership(window, &rust.window_token.to_string(), candidate)
                .ok_or_else(|| "context membership handoff is full".to_owned())
        })();
        match result {
            Ok(handle) => QString::from(handle),
            Err(error) => {
                self.as_mut().set_status_text(QString::from(error));
                QString::default()
            }
        }
    }

    pub(super) fn confirm_staged_context_window_membership(
        mut self: Pin<&mut Self>,
        handle: &QString,
    ) -> bool {
        let (window, token) = {
            let binding = self.as_ref();
            let rust = binding.rust();
            let Some(window) = rust.window else {
                return false;
            };
            (window, rust.window_token.to_string())
        };
        let Some((candidate, claim)) = take_membership(&handle.to_string(), window, &token) else {
            self.as_mut()
                .set_status_text(QString::from("Context member owner is stale"));
            return false;
        };
        let current = {
            let binding = self.as_ref();
            let Some(state) = binding.rust().state.as_ref() else {
                return false;
            };
            state.revision() == candidate.application_revision
                && state
                    .context_membership_owner(window, &candidate.context_name)
                    .is_ok_and(|owner| owner == (candidate.profile_uuid, candidate.native))
                && current_context_member(state, window, &candidate.context_name).is_ok_and(
                    |(member, skipped)| {
                        member == candidate.member && skipped == candidate.skipped_unsafe_tabs
                    },
                )
        };
        if !current {
            self.as_mut()
                .set_status_text(QString::from("Context member changed during save"));
            return false;
        }
        record_confirmed_membership(claim)
    }

    pub(super) fn take_context_commit_message(mut self: Pin<&mut Self>) -> QString {
        let mut rust = self.as_mut().rust_mut();
        QString::from(
            rust.as_mut()
                .get_mut()
                .pending_context_commit_message
                .take()
                .unwrap_or_default(),
        )
    }

    pub(super) fn active_context_capture_members(
        self: Pin<&mut Self>,
        context_name: &QString,
    ) -> QStringList {
        let binding = self.as_ref();
        let rust = binding.rust();
        rust.window
            .zip(rust.state.as_ref())
            .and_then(|(window, state)| {
                state
                    .active_context_capture_members(window, &context_name.to_string())
                    .ok()
            })
            .map(|members| {
                members
                    .into_iter()
                    .map(|window| QString::from(window.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn prepare_staged_context_commit(
        mut self: Pin<&mut Self>,
        handles: &QStringList,
        message: &QString,
    ) -> QString {
        let result = (|| {
            let handles = handles.iter().map(ToString::to_string).collect::<Vec<_>>();
            let staged = snapshot_captures(&handles)
                .ok_or_else(|| "context capture handles are stale or ambiguous".to_owned())?;
            let candidates = staged
                .iter()
                .map(|entry| entry.value.clone())
                .collect::<Vec<_>>();
            let binding = self.as_ref();
            let rust = binding.rust();
            if rust
                .pending_context_capture
                .as_ref()
                .is_some_and(|pending| pending.expires_at > Instant::now())
            {
                return Err("another context capture is pending".to_owned());
            }
            let window = rust
                .window
                .ok_or_else(|| "source window is unavailable".to_owned())?;
            let plan = rust
                .state
                .as_ref()
                .ok_or_else(|| "application state is unavailable".to_owned())?
                .prepare_context_candidate_commit(window, message.to_string(), &candidates)
                .map_err(|error| error.to_string())?;
            if plan.complete_member_count() != Some(staged.len()) {
                return Err("context members are missing or changed".to_owned());
            }
            let pending = PendingContextCapture {
                id: Uuid::new_v4(),
                expires_at: Instant::now() + std::time::Duration::from_secs(30),
                source_window: window,
                owner_token: rust.window_token.to_string(),
                expected: staged
                    .into_iter()
                    .map(|entry| BindingExpectation {
                        handle: entry.handle,
                        window: entry.window,
                        owner_token: entry.owner_token,
                    })
                    .collect(),
                plan,
            };
            Ok(pending)
        })();
        match result {
            Ok(pending) => {
                let id = pending.id.to_string();
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .pending_context_capture = Some(pending);
                QString::from(id)
            }
            Err(error) => {
                self.as_mut().set_status_text(QString::from(error));
                QString::default()
            }
        }
    }

    pub(super) fn finish_staged_context_commit(
        mut self: Pin<&mut Self>,
        plan_id: &QString,
    ) -> QStringList {
        let requested = plan_id.to_string();
        let ready = {
            let binding = self.as_ref();
            let rust = binding.rust();
            rust.pending_context_capture
                .as_ref()
                .is_some_and(|pending| {
                    pending.id.to_string() == requested
                        && pending.expires_at > Instant::now()
                        && rust.window == Some(pending.source_window)
                        && rust.window_token.to_string() == pending.owner_token
                        && take_bound_capture_receipts(&pending.expected)
                })
        };
        if !ready {
            return QStringList::default();
        }
        let result = {
            let mut rust = self.as_mut().rust_mut();
            let this = rust.as_mut().get_mut();
            let Some(pending) = this.pending_context_capture.take() else {
                return QStringList::default();
            };
            let Some(application) = this.state.as_mut() else {
                return QStringList::default();
            };
            application.begin_prepared_context_candidate_commit(pending.plan)
        };
        match result {
            Ok((operation, commit)) => {
                self.as_mut().set_status_text(QString::from(
                    "Versioned context commit pending durable publication",
                ));
                [operation.to_string(), commit.to_string()]
                    .into_iter()
                    .map(QString::from)
                    .collect()
            }
            Err(error) => {
                self.as_mut()
                    .set_status_text(QString::from(error.to_string()));
                QStringList::default()
            }
        }
    }

    pub(super) fn cancel_staged_context_commit(
        mut self: Pin<&mut Self>,
        plan_id: &QString,
    ) -> bool {
        let requested = plan_id.to_string();
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        if this
            .pending_context_capture
            .as_ref()
            .is_none_or(|pending| pending.id.to_string() != requested)
        {
            return false;
        }
        this.pending_context_capture.take().is_some()
    }
    pub(super) fn active_context_capture_ref(
        self: Pin<&mut Self>,
        context_name: &QString,
    ) -> QString {
        let binding = self.as_ref();
        let rust = binding.rust();
        rust.window
            .zip(rust.state.as_ref())
            .and_then(|(window, state)| {
                state
                    .active_context_capture_ref(window, &context_name.to_string())
                    .ok()
            })
            .map_or_else(QString::default, QString::from)
    }

    pub(super) fn stage_context_window_capture(
        mut self: Pin<&mut Self>,
        context_name: &QString,
        context_ref: &QString,
        ordinal: i32,
    ) -> QString {
        let result = (|| {
            let ordinal = u16::try_from(ordinal)
                .ok()
                .filter(|ordinal| *ordinal < 256)
                .ok_or_else(|| "context member ordinal is invalid".to_owned())?;
            let binding = self.as_ref();
            let rust = binding.rust();
            let window = rust
                .window
                .ok_or_else(|| "current window owner is unavailable".to_owned())?;
            let candidate = rust
                .state
                .as_ref()
                .ok_or_else(|| "application state is unavailable".to_owned())?
                .prepare_context_window_candidate(
                    window,
                    &context_name.to_string(),
                    &context_ref.to_string(),
                    ordinal,
                )
                .map_err(|error| error.to_string())?;
            stage_capture(window, &rust.window_token.to_string(), candidate)
                .ok_or_else(|| "context capture handoff is full".to_owned())
        })();
        match result {
            Ok(handle) => QString::from(handle),
            Err(error) => {
                self.as_mut().set_status_text(QString::from(error));
                QString::default()
            }
        }
    }

    pub(super) fn bind_staged_context_window_capture(
        mut self: Pin<&mut Self>,
        handle: &QString,
    ) -> bool {
        let (window, token) = {
            let binding = self.as_ref();
            let rust = binding.rust();
            let Some(window) = rust.window else {
                return false;
            };
            (window, rust.window_token.to_string())
        };
        let Some((candidate, claim)) = take_capture(&handle.to_string(), window, &token) else {
            self.as_mut()
                .set_status_text(QString::from("Context capture owner is stale"));
            return false;
        };
        let (previous_revision, effects) = {
            let mut rust = self.as_mut().rust_mut();
            let Some(application) = rust.as_mut().get_mut().state.as_mut() else {
                return false;
            };
            let previous_revision = application.revision();
            (
                previous_revision,
                application.bind_context_window_candidate(candidate),
            )
        };
        let Ok(effects) = effects else {
            self.as_mut()
                .set_status_text(QString::from("Context capture owner changed"));
            return false;
        };
        let qt_effects = self
            .as_mut()
            .accept_runtime_effects(previous_revision, effects);
        self.as_mut().set_pending_engine_action(&qt_effects);
        record_bound_capture(claim)
    }
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
                .ok_or_else(|| "context does not exist".to_owned())?;
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
                    let source_profile = state.windows().get(&window_id)?.profile;
                    context.members.iter().rev().find_map(|member| {
                        let member_window = state
                            .windows()
                            .keys()
                            .find(|candidate| candidate.to_string() == member.window_id)
                            .copied()?;
                        (current_profile == context.profile
                            && member_window != window_id
                            && state.windows().get(&member_window).is_some_and(|window| {
                                window.profile == source_profile
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
                    // The structured command result below contains the
                    // requested destination. Keep the shared status/IPC
                    // error projection free of user-defined labels.
                    self.as_mut().set_status_text(QString::from(
                        "Context requires a separate profile window",
                    ));
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
                    self.as_mut().set_status_text(QString::from(
                        "Context is already open in another window",
                    ));
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
            let current_profile = self.as_ref().rust().profile_name.clone();
            if context.profile != current_profile {
                return Err("context belongs to a different profile".into());
            }
            self.as_mut()
                .reduce_event(Event::SetWindowContext {
                    window: window_id,
                    context: Some(context.name.clone()),
                })
                .map_err(|error| error.clone())?;
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some("context-save-request".into());
            self.as_mut()
                .set_status_text(QString::from("Context membership capture requested"));
            return Ok(serde_json::json!({
                "status": "accepted",
                "pending": "context-membership-capture",
                "context": {"id": context.id.to_string(), "name": context.name}
            }));
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
