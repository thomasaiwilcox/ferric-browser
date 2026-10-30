//! Application-wide presentation selection over actual registered Core owners.
//! No renderer-supplied window, profile or document claim enters this directory.

use super::{CxxQtType, ParsedCommand, Pin, PrivacyKind, QString, TabId, qobject};
use cxx_qt::{CxxQtThread, Threading};
use ferric_browser_application::ProfileInstanceId;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub(super) struct PageHandoff(Arc<PageHandoffState>);

struct PageHandoffState {
    thread: CxxQtThread<qobject::BrowserUi>,
    scope: PageScope,
    group: String,
    page: TabId,
    operation: Option<String>,
    finished: AtomicBool,
    focus_requested: AtomicBool,
}

impl Drop for PageHandoffState {
    fn drop(&mut self) {
        if self.finished.load(Ordering::Acquire) {
            return;
        }
        // Qt discards queued callbacks when their receiver is destroyed.
        // The last receipt still reports failure to the original issuer.
        let operation = self.operation.clone();
        let _ = self.thread.queue(move |mut ui| {
            ui.as_mut().page_window_command_failed(
                "Page command failed because a window closed. Try again.",
            );
            if let Some(operation) = operation {
                ui.as_mut().rust_mut().as_mut().get_mut().set_operation(
                    operation,
                    crate::OperationUpdate::failed(
                        crate::FailureKind::Engine,
                        "page window closed",
                    ),
                );
            }
        });
    }
}

impl PageHandoff {
    pub(super) fn finish(self, selected: bool, succeeded: bool, target_token: String) {
        let root = self.0;
        let thread = root.thread.clone();
        let _ = thread.queue(move |mut source_ui| {
            root.finished.store(true, Ordering::Release);
            let source_current = PageWindowSnapshot::capture(source_ui.as_ref().get_ref())
                .is_some_and(|snapshot| {
                    snapshot.scope == root.scope
                        && snapshot.group == root.group
                        && snapshot.pages.iter().any(|page| page.id == root.page)
                });
            if !selected || !source_current {
                source_ui.as_mut().page_window_command_failed(
                    "Page command failed because the window changed. Try again.",
                );
                if let Some(operation) = &root.operation {
                    source_ui
                        .as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .set_operation(
                            operation.clone(),
                            crate::OperationUpdate::failed(
                                crate::FailureKind::StaleTarget,
                                "page command target changed",
                            ),
                        );
                }
                return;
            }
            if !succeeded {
                source_ui.as_mut().page_window_command_failed(
                    "The command failed in the requested page. Try again.",
                );
                if let Some(operation) = &root.operation {
                    source_ui
                        .as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .set_operation(
                            operation.clone(),
                            crate::OperationUpdate::failed(
                                crate::FailureKind::Engine,
                                "page command failed",
                            ),
                        );
                }
                return;
            }
            if !root.focus_requested.load(Ordering::Acquire) {
                if let Some(operation) = &root.operation {
                    source_ui
                        .as_mut()
                        .rust_mut()
                        .as_mut()
                        .get_mut()
                        .set_operation(
                            operation.clone(),
                            crate::OperationUpdate::completed("page state updated"),
                        );
                }
                return;
            }
            source_ui.as_mut().queue_pending_engine_action();
            let action = format!(
                "window-focus\t{target_token}{}",
                root.operation
                    .as_deref()
                    .map_or_else(String::new, |id| format!("\t{id}"))
            );
            source_ui
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some(action);
            source_ui.as_mut().runtime_work_available();
        });
    }
}

pub(super) enum PageContinuation {
    Interactive {
        commands: Vec<ParsedCommand>,
        count: u32,
    },
    Ipc {
        commands: Vec<(ParsedCommand, super::IpcRoute)>,
        route: super::IpcRoute,
    },
}

impl PageContinuation {
    pub(super) fn has_commands(&self) -> bool {
        match self {
            Self::Interactive { commands, .. } => !commands.is_empty(),
            Self::Ipc { commands, .. } => !commands.is_empty(),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct PageScope {
    profile: ProfileInstanceId,
    privacy: PrivacyKind,
    context: Option<String>,
}

impl PageScope {
    pub(super) fn matches_undo_profile(&self, scope: &crate::page_window_undo::Scope) -> bool {
        self.profile == scope.profile && self.privacy == scope.privacy
    }
    pub(super) fn matches_undo(&self, scope: &crate::page_window_undo::Scope) -> bool {
        self.matches_undo_profile(scope) && self.context == scope.context
    }
}

#[derive(Clone)]
pub(super) struct PresentedPage {
    pub(super) id: TabId,
    pub(super) pinned: bool,
    pub(super) target: super::Target,
    pub(super) order: usize,
}

#[derive(Clone)]
pub(super) struct PageWindowSnapshot {
    pub(super) scope: PageScope,
    pub(super) group: String,
    pub(super) pages: Vec<PresentedPage>,
    pub(super) reserved: Vec<TabId>,
    pub(super) previous: Option<TabId>,
}

impl PageWindowSnapshot {
    pub(super) fn capture(ui: &qobject::BrowserUi) -> Option<Self> {
        Self::capture_checked(ui).ok()
    }

    pub(super) fn capture_checked(ui: &qobject::BrowserUi) -> Result<Self, String> {
        let rust = ui.rust();
        let state = rust
            .state
            .as_ref()
            .ok_or("The browser state is unavailable")?;
        if !rust.page_window_ordinary
            || !rust.view_alive
            || rust.page_window_closing
            || rust.page_window_preparing
            || rust.profile_bootstrap_pending
            || rust.ipc_shutdown_gate
            || state.shutdown() != ferric_browser_core::ShutdownState::Running
            || state.configuration().tabs.presentation
                != ferric_browser_config::TabsPresentation::Windows
        {
            return Err("Page commands require a ready ordinary window".into());
        }
        let window = rust
            .window
            .and_then(|id| state.windows().get(&id))
            .ok_or("The browser window is unavailable")?;
        let profile = state
            .profiles()
            .get(&window.profile)
            .ok_or("The browser profile is unavailable")?;
        let pages = window
            .tabs
            .iter()
            .filter_map(|id| {
                let page = state.tabs().get(id)?;
                (page.existence == ferric_browser_core::ExistenceState::Live
                    && rust.interaction.prompt_view_identities.contains_key(id))
                .then_some(PresentedPage {
                    id: *id,
                    pinned: page.pinned,
                    target: state.capture_target(*id)?,
                    order: usize::MAX,
                })
            })
            .collect::<Vec<_>>();
        if pages.is_empty() {
            return Err("This window has no registered page view".into());
        }
        Ok(Self {
            scope: PageScope {
                profile: state
                    .profile_instance_id()
                    .ok_or("The live profile session is unavailable")?,
                privacy: profile.privacy,
                context: window.context.clone(),
            },
            group: if rust.page_window_group.is_empty() {
                rust.window_token.to_string()
            } else {
                rust.page_window_group.clone()
            },
            pages,
            reserved: rust
                .tab_ids
                .iter()
                .copied()
                .filter(|id| {
                    !rust.popup_tab_ids.contains(id)
                        && state.tabs().get(id).is_some_and(|page| {
                            page.existence == ferric_browser_core::ExistenceState::Live
                        })
                })
                .collect(),
            previous: window.previous_tab,
        })
    }
}

#[derive(Clone)]
pub(super) struct PageWindowOwner {
    pub(super) snapshot: PageWindowSnapshot,
    pub(super) token: String,
    pub(super) window: super::WindowId,
    pub(super) order: u64,
    pub(super) focused: u64,
    pub(super) thread: CxxQtThread<qobject::BrowserUi>,
}

#[derive(Clone)]
pub(super) struct Candidate {
    pub(super) owner: PageWindowOwner,
    pub(super) page: PresentedPage,
    index: usize,
}

pub(super) fn candidates(
    owners: Vec<PageWindowOwner>,
    source: &PageWindowSnapshot,
) -> Vec<Candidate> {
    let mut pages = owners
        .into_iter()
        .filter(|owner| owner.snapshot.scope == source.scope)
        .flat_map(|owner| {
            owner
                .snapshot
                .pages
                .clone()
                .into_iter()
                .enumerate()
                .map(move |(index, page)| Candidate {
                    owner: owner.clone(),
                    page,
                    index,
                })
        })
        .collect::<Vec<_>>();
    let grouped = pages
        .iter()
        .filter(|page| page.owner.snapshot.group == source.group)
        .count();
    if grouped > 1 {
        pages.retain(|page| page.owner.snapshot.group == source.group);
    }
    pages.sort_by_key(|page| {
        (
            !page.page.pinned,
            page.page.order,
            page.owner.order,
            page.index,
        )
    });
    pages
}

pub(super) fn continuation_current(
    ui: &qobject::BrowserUi,
    scope: &PageScope,
    group: &str,
    target: super::Target,
) -> bool {
    let rust = ui.rust();
    if rust.tab != Some(target.tab)
        || usize::try_from(rust.active_tab_index)
            .ok()
            .and_then(|index| rust.tab_ids.get(index))
            != Some(&target.tab)
        || rust
            .state
            .as_ref()
            .and_then(|state| state.windows().get(&rust.window?))
            .is_none_or(|window| window.active_tab != Some(target.tab))
    {
        return false;
    }
    PageWindowSnapshot::capture(ui).is_some_and(|snapshot| {
        snapshot.scope == *scope
            && snapshot.group == group
            && snapshot.pages.iter().any(|page| page.target == target)
    })
}

pub(super) fn handles(command: &ParsedCommand) -> bool {
    matches!(
        command.name.as_str(),
        "tab-next" | "tab-prev" | "tab-select" | "tab-focus"
    )
}

pub(super) fn is_command_workflow(command: &ParsedCommand) -> bool {
    matches!(
        command.name.as_str(),
        "repeat" | "macro-play" | "action" | "command-execute"
    )
}

pub(super) fn validate_chain_workflows(
    commands: &[ParsedCommand],
    handed_off: bool,
) -> Result<(), String> {
    if (handed_off || commands.len() > 1) && commands.iter().any(is_command_workflow) {
        return Err("Repeat, macro playback and command wrappers cannot run in a window-selection chain yet. Run them separately.".into());
    }
    Ok(())
}

fn select_index(
    command: &ParsedCommand,
    count: u32,
    ids: &[TabId],
    current: usize,
    previous: Option<TabId>,
) -> Result<usize, String> {
    if ids.is_empty() {
        return Err("No page windows are ready".into());
    }
    match command.name.as_str() {
        "tab-next" | "tab-prev" => {
            let count = match command.arguments.as_slice() {
                [] => count,
                [flag, value] if flag == "--count" => value
                    .parse::<u32>()
                    .map_err(|_| "Tab count must be from 1 to 100")?,
                _ => return Err("Use tab-next or tab-prev with an optional --count N".into()),
            };
            if !(1..=100).contains(&count) {
                return Err("Tab count must be from 1 to 100".into());
            }
            let offset = count as usize % ids.len();
            Ok(if command.name == "tab-next" {
                (current + offset) % ids.len()
            } else {
                (current + ids.len() - offset) % ids.len()
            })
        }
        "tab-select" | "tab-focus" => {
            let [value] = command.arguments.as_slice() else {
                return Err("Select a displayed page index or stable page ID".into());
            };
            if command.name == "tab-select" {
                if value == "last" {
                    return Ok(ids.len() - 1);
                }
                if value == "previous" {
                    return previous
                        .and_then(|id| ids.iter().position(|candidate| *candidate == id))
                        .ok_or_else(|| "No previous page window is available".into());
                }
                if let Ok(index) = value.parse::<usize>() {
                    return index
                        .checked_sub(1)
                        .filter(|index| *index < ids.len())
                        .ok_or_else(|| "The page index is outside this group".into());
                }
            }
            let id = TabId::from_display(value).ok_or("The page ID is invalid")?;
            ids.iter()
                .position(|candidate| *candidate == id)
                .ok_or_else(|| "This page is outside the current group".into())
        }
        _ => Err("Unsupported page-window selection".into()),
    }
}

impl qobject::BrowserUi {
    pub(super) fn prepare_page_handoff(
        mut self: Pin<&mut Self>,
        incoming: Option<PageHandoff>,
        source: &PageWindowSnapshot,
        operation_id: Option<&str>,
    ) -> Result<PageHandoff, String> {
        if let Some(incoming) = incoming {
            return Ok(incoming);
        }
        let page = self.as_ref().rust().tab.ok_or("No active page")?;
        let operation = operation_id
            .map(|id| {
                self.as_mut()
                    .rust_mut()
                    .as_mut()
                    .get_mut()
                    .start_native_handoff(Some(id))
            })
            .transpose()?;
        Ok(PageHandoff(Arc::new(PageHandoffState {
            thread: self.as_ref().get_ref().qt_thread(),
            scope: source.scope.clone(),
            group: source.group.clone(),
            page,
            operation,
            finished: AtomicBool::new(false),
            focus_requested: AtomicBool::new(false),
        })))
    }

    pub(super) fn in_page_window_mode(&self) -> bool {
        self.rust().state.as_ref().is_some_and(|state| {
            state.configuration().tabs.presentation
                == ferric_browser_config::TabsPresentation::Windows
        })
    }

    pub(super) fn execute_page_window_ipc_selection(
        self: Pin<&mut Self>,
        command: &ParsedCommand,
        route: &super::IpcRoute,
        commands: Vec<(ParsedCommand, super::IpcRoute)>,
        operation_id: Option<&str>,
    ) -> Result<super::Value, super::PublicError> {
        self.execute_page_window_ipc_selection_with_handoff(
            command,
            route,
            commands,
            operation_id,
            &mut None,
        )
    }

    fn execute_page_window_ipc_selection_with_handoff(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        route: &super::IpcRoute,
        commands: Vec<(ParsedCommand, super::IpcRoute)>,
        operation_id: Option<&str>,
        handoff: &mut Option<PageHandoff>,
    ) -> Result<super::Value, super::PublicError> {
        if route.context.is_some() {
            return Err("context routing is not valid for page selection".into());
        }
        self.as_ref().validate_ipc_route_public(route)?;
        // Resolve the external selector at the issuing owner before changing
        // presentation. Continuations are rebound to the selected native owner.
        if Some(self.as_ref().ipc_route_window(route)?) != self.as_ref().rust().window {
            return Err("Choose a page in the requested window.".into());
        }
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let mut continuation_route = route.clone();
        continuation_route.selector = super::DispatchTarget::Active;
        let commands = commands
            .into_iter()
            .map(|(command, mut route)| {
                route.selector = super::DispatchTarget::Active;
                (command, route)
            })
            .collect();
        let result = self.as_mut().execute_page_window_selection_with_handoff(
            command,
            1,
            PageContinuation::Ipc {
                commands,
                route: continuation_route,
            },
            operation_id,
            handoff.clone(),
        )?;
        handoff.take();
        self.as_mut()
            .note_repeatable_invocation(command, Some(route), 1);
        Ok(result)
    }

    pub(super) fn execute_page_window_selection_with_handoff(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        count: u32,
        continuation: PageContinuation,
        operation_id: Option<&str>,
        handoff: Option<PageHandoff>,
    ) -> Result<super::Value, String> {
        let source = PageWindowSnapshot::capture_checked(self.as_ref().get_ref())?;
        let current = self.as_ref().rust().tab.ok_or("No active page")?;
        self.as_mut().refresh_ipc_owner();
        let pages = candidates(
            super::live_ipc_owners::page_window_owners(&source.scope),
            &source,
        );
        let ids = pages.iter().map(|page| page.page.id).collect::<Vec<_>>();
        let index = ids
            .iter()
            .position(|id| *id == current)
            .ok_or("This page is no longer presented")?;
        let previous = source
            .previous
            .filter(|id| *id != current && ids.contains(id))
            .or_else(|| {
                pages
                    .iter()
                    .filter(|page| page.page.id != current && page.owner.focused > 0)
                    .max_by_key(|page| page.owner.focused)
                    .map(|page| page.page.id)
            });
        let target = pages[select_index(command, count, &ids, index, previous)?].clone();
        let target_id = target.page.id;
        let target_document = target.page.target;
        let document_continuation = continuation.has_commands();
        let target_token = target.owner.token.clone();
        let result_window = target.owner.window;
        let source_token = self.as_ref().rust().window_token.to_string();
        let same_owner = source_token == target_token;
        let handoff = self
            .as_mut()
            .prepare_page_handoff(handoff, &source, operation_id)?;
        handoff.0.focus_requested.store(true, Ordering::Release);
        let scope = source.scope;
        let group = source.group;
        let repeat_command = command.clone();
        target
            .owner
            .thread
            .queue(move |mut ui| {
                let mut handoff = Some(handoff);
                let source_current = super::live_ipc_owners::page_window_owners(&scope)
                    .iter()
                    .any(|owner| {
                        owner.token == source_token
                            && owner.snapshot.group == group
                            && owner.snapshot.pages.iter().any(|page| page.id == current)
                    });
                let valid = source_current
                    && PageWindowSnapshot::capture(ui.as_ref().get_ref()).is_some_and(|snapshot| {
                        snapshot.scope == scope
                            && snapshot.group == target.owner.snapshot.group
                            && snapshot.pages.iter().any(|page| page.id == target_id)
                    });
                let valid = valid
                    && (!document_continuation
                        || ui.as_ref().rust().state.as_ref().is_some_and(|state| {
                            state.capture_target(target_id) == Some(target_document)
                        }));
                let target_index = ui
                    .as_ref()
                    .rust()
                    .tab_ids
                    .iter()
                    .position(|id| *id == target_id)
                    .and_then(|index| i32::try_from(index).ok());
                let mut selected =
                    valid && target_index.is_some_and(|index| ui.as_mut().select_tab(index));
                let mut succeeded = selected;
                if selected {
                    ui.as_mut().set_core_mode(super::Mode::Normal);
                    match continuation {
                        PageContinuation::Interactive { commands, count } => {
                            if !same_owner {
                                ui.as_mut().note_repeatable_invocation(
                                    &repeat_command,
                                    None,
                                    count,
                                );
                            }
                            if !commands.is_empty() {
                                // Selection, mode and repeat recording emit Qt
                                // signals. Recheck after all synchronous reentry,
                                // immediately before capturing document work.
                                selected = continuation_current(
                                    ui.as_ref().get_ref(),
                                    &scope,
                                    &target.owner.snapshot.group,
                                    target_document,
                                );
                                succeeded = selected
                                    && ui.as_mut().execute_counted_parsed_commands_with_handoff(
                                        commands,
                                        count,
                                        &mut handoff,
                                    );
                            }
                        }
                        PageContinuation::Ipc { commands, route } => {
                            if !same_owner {
                                ui.as_mut().note_repeatable_invocation(
                                    &repeat_command,
                                    Some(&route),
                                    1,
                                );
                            }
                            selected = commands.is_empty()
                                || continuation_current(
                                    ui.as_ref().get_ref(),
                                    &scope,
                                    &target.owner.snapshot.group,
                                    target_document,
                                );
                            if !selected {
                                succeeded = false;
                            } else if let Err(error) = ui
                                .as_mut()
                                .execute_routed_ipc_chain_with_handoff(commands, None, &mut handoff)
                            {
                                ui.as_mut()
                                    .set_status_text(QString::from(error.user_message()));
                                succeeded = false;
                            }
                        }
                    }
                    ui.as_mut().runtime_work_available();
                }
                // A later selection now owns this same receipt. Only its
                // terminal owner can focus or finish the original operation.
                let Some(handoff) = handoff else {
                    return;
                };
                handoff.finish(selected, succeeded, target_token);
            })
            .map_err(|_| "The requested page window is no longer available".to_owned())?;
        Ok(
            serde_json::json!({"status":"accepted", "action":"focus", "tab_id":target_id.to_string(), "window_id":result_window.to_string(), "pending":true}),
        )
    }

    fn page_window_command_failed(mut self: Pin<&mut Self>, message: &str) {
        self.as_mut().set_status_text(QString::from(message));
        self.as_mut().queue_pending_engine_action();
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .pending_engine_action = Some(format!("command-error\t{message}"));
        self.as_mut().runtime_work_available();
    }

    pub(super) fn execute_page_window_ipc_chain(
        self: Pin<&mut Self>,
        commands: Vec<ParsedCommand>,
        route: &super::IpcRoute,
        operation_id: Option<&str>,
    ) -> Result<super::Value, super::PublicError> {
        self.execute_routed_ipc_chain(
            commands
                .into_iter()
                .map(|command| (command, route.clone()))
                .collect(),
            operation_id,
        )
    }

    pub(super) fn execute_routed_ipc_chain(
        self: Pin<&mut Self>,
        commands: Vec<(ParsedCommand, super::IpcRoute)>,
        operation_id: Option<&str>,
    ) -> Result<super::Value, super::PublicError> {
        self.execute_routed_ipc_chain_with_handoff(commands, operation_id, &mut None)
    }

    pub(super) fn execute_routed_ipc_chain_with_handoff(
        mut self: Pin<&mut Self>,
        commands: Vec<(ParsedCommand, super::IpcRoute)>,
        operation_id: Option<&str>,
        handoff: &mut Option<PageHandoff>,
    ) -> Result<super::Value, super::PublicError> {
        // Expand and check the entire external chain before its first effect.
        // Each step keeps its own open/profile/context policy; only the active
        // native owner changes after a page-selection continuation.
        let mut expanded = Vec::new();
        for (command, route) in commands {
            let commands = self
                .as_ref()
                .rust()
                .registry
                .expand_command(command)
                .map_err(|error| {
                    super::PublicError::new(
                        ferric_browser_ipc::ErrorCode::InvalidArgument,
                        "The command couldn't be expanded. Check its definition.",
                        error.to_string(),
                    )
                })?;
            super::browser_ui_hint_pointer::reject_external_hint_interaction_chain(&commands)
                .map_err(|error| {
                    super::PublicError::new(
                        ferric_browser_ipc::ErrorCode::Denied,
                        "Use hints from the browser window.",
                        error,
                    )
                })?;
            expanded.extend(commands.into_iter().map(|command| (command, route.clone())));
            if expanded.len() > 64 {
                return Err(super::PublicError::new(
                    ferric_browser_ipc::ErrorCode::InvalidArgument,
                    "A command chain can contain at most 64 commands.",
                    "expanded command chain exceeds 64 commands",
                ));
            }
        }
        let mut result = serde_json::json!({"status":"accepted"});
        if self.as_ref().in_page_window_mode() {
            validate_chain_workflows(
                &expanded
                    .iter()
                    .map(|(command, _)| command.clone())
                    .collect::<Vec<_>>(),
                handoff.is_some(),
            )
            .map_err(|error| {
                super::PublicError::new(
                    ferric_browser_ipc::ErrorCode::Unsupported,
                    error.clone(),
                    error,
                )
            })?;
        }
        let mut operations = Vec::new();
        for (index, (command, route)) in expanded.iter().enumerate() {
            if self.as_ref().in_page_window_mode()
                && super::browser_ui_page_mutations::handles(command)
            {
                return self.execute_page_window_ipc_mutation(
                    command,
                    route,
                    expanded[index + 1..].to_vec(),
                    operation_id,
                    handoff,
                );
            }
            if self.as_ref().in_page_window_mode() && handles(command) {
                return self.execute_page_window_ipc_selection_with_handoff(
                    command,
                    route,
                    expanded[index + 1..].to_vec(),
                    operation_id,
                    handoff,
                );
            }
            result =
                self.as_mut()
                    .execute_ipc_command_with_operation(command.clone(), route, None)?;
            if let Some(id) = result.get("operation_id").and_then(super::Value::as_str) {
                operations.push(id.to_owned());
            }
        }
        result["operation_ids"] = serde_json::json!(operations);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(number: u64) -> TabId {
        TabId::from_display(&format!("tabid-{number}")).unwrap()
    }
    fn command(name: &str, arguments: &[&str]) -> ParsedCommand {
        ParsedCommand {
            name: name.into(),
            arguments: arguments.iter().map(|v| (*v).into()).collect(),
        }
    }
    #[test]
    fn selections_wrap_with_counts_and_validate_group_ids() {
        let ids = [id(1), id(2), id(3)];
        assert_eq!(
            select_index(&command("tab-next", &["--count", "4"]), 1, &ids, 0, None),
            Ok(1)
        );
        assert_eq!(
            select_index(&command("tab-prev", &[]), 2, &ids, 0, None),
            Ok(1)
        );
        assert_eq!(
            select_index(&command("tab-select", &["last"]), 1, &ids, 0, None),
            Ok(2)
        );
        assert_eq!(
            select_index(
                &command("tab-select", &["previous"]),
                1,
                &ids,
                0,
                Some(id(2))
            ),
            Ok(1)
        );
        assert!(select_index(&command("tab-focus", &["tabid-4"]), 1, &ids, 0, None).is_err());
        assert!(select_index(&command("tab-select", &["0"]), 1, &ids, 0, None).is_err());
        assert!(select_index(&command("tab-next", &["--count", "101"]), 1, &ids, 0, None).is_err());
    }
}
