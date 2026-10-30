//! Page state changes across registered native owners. The mutation targets a
//! stable page; document continuations return to the issuing page without
//! changing focus unless the chain itself selected a different owner.

use super::{CxxQtType, ParsedCommand, Pin, TabId, qobject};
use crate::browser_ui_page_windows::{
    PageContinuation, PageHandoff, PageWindowSnapshot, candidates, continuation_current,
};
use cxx_qt::Threading;

pub(super) fn handles(command: &ParsedCommand) -> bool {
    matches!(command.name.as_str(), "tab-pin" | "tab-mute")
}

impl qobject::BrowserUi {
    pub(super) fn execute_page_window_ipc_mutation(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        route: &super::IpcRoute,
        commands: Vec<(ParsedCommand, super::IpcRoute)>,
        operation_id: Option<&str>,
        handoff: &mut Option<PageHandoff>,
    ) -> Result<super::Value, super::PublicError> {
        if route.context.is_some() {
            return Err("context routing is not valid for page state changes".into());
        }
        self.as_ref().validate_ipc_route_public(route)?;
        if Some(self.as_ref().ipc_route_window(route)?) != self.as_ref().rust().window {
            return Err("Choose a page in the requested window.".into());
        }
        let result = self.as_mut().execute_page_window_mutation(
            command,
            PageContinuation::Ipc {
                commands,
                route: route.clone(),
            },
            operation_id,
            handoff,
        )?;
        self.as_mut()
            .note_repeatable_invocation(command, Some(route), 1);
        Ok(result)
    }

    pub(super) fn execute_page_window_mutation(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        continuation: PageContinuation,
        operation_id: Option<&str>,
        handoff: &mut Option<PageHandoff>,
    ) -> Result<super::Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|error| error.to_string())?;
        let source = PageWindowSnapshot::capture_checked(self.as_ref().get_ref())?;
        let current = self.as_ref().rust().tab.ok_or("No active page")?;
        let source_target = source
            .pages
            .iter()
            .find(|page| page.id == current)
            .ok_or("The issuing page is no longer presented")?
            .target;
        let mut normalized = command.clone();
        super::normalize_active_tab_command(&mut normalized, Some(current.to_string()))?;
        let id = match normalized.arguments.as_slice() {
            [id] => id,
            [id, state] if matches!(state.as_str(), "on" | "off" | "toggle") => id,
            _ => return Err("Use a stable page ID and optional on, off or toggle state".into()),
        };
        let id = TabId::from_display(id).ok_or("The page ID is invalid")?;
        self.as_mut().refresh_ipc_owner();
        let target = candidates(
            super::live_ipc_owners::page_window_owners(&source.scope),
            &source,
        )
        .into_iter()
        .find(|candidate| candidate.page.id == id)
        .ok_or("This page is outside the current group")?;
        let generation = target.page.target.generation;
        let target_token = target.owner.token;
        let target_group = target.owner.snapshot.group;
        let result_window = target.owner.window;
        let source_token = self.as_ref().rust().window_token.to_string();
        let source_thread = self.as_ref().get_ref().qt_thread();
        let receipt = self
            .as_mut()
            .prepare_page_handoff(handoff.clone(), &source, operation_id)?;
        target
            .owner
            .thread
            .queue(move |mut ui| {
                let owners = super::live_ipc_owners::page_window_owners(&source.scope);
                let source_current = owners.iter().any(|owner| {
                    owner.token == source_token
                        && owner.snapshot.group == source.group
                        && owner.snapshot.pages.iter().any(|page| page.id == current)
                });
                let eligible = source_current
                    && candidates(owners, &source).iter().any(|candidate| {
                        candidate.owner.token == target_token && candidate.page.id == id
                    });
                let valid = eligible
                    && PageWindowSnapshot::capture(ui.as_ref().get_ref()).is_some_and(|snapshot| {
                        snapshot.scope == source.scope
                            && snapshot.group == target_group
                            && snapshot
                                .pages
                                .iter()
                                .any(|page| page.id == id && page.target.generation == generation)
                    });
                if !valid {
                    receipt.finish(false, false, target_token);
                    return;
                }
                if ui.as_mut().execute_tab_action(&normalized, None).is_err() {
                    receipt.finish(true, false, target_token);
                    return;
                }
                ui.as_mut().runtime_work_available();
                let _ = source_thread.queue(move |mut issuer| {
                    let source_current = PageWindowSnapshot::capture(issuer.as_ref().get_ref())
                        .is_some_and(|snapshot| {
                            snapshot.scope == source.scope
                                && snapshot.group == source.group
                                && snapshot.pages.iter().any(|page| {
                                    page.id == current
                                        && page.target.generation == source_target.generation
                                })
                        });
                    let document_current = !continuation.has_commands()
                        || continuation_current(
                            issuer.as_ref().get_ref(),
                            &source.scope,
                            &source.group,
                            source_target,
                        );
                    if !source_current || !document_current {
                        receipt.finish(false, false, source_token);
                        return;
                    }
                    let mut handoff = Some(receipt);
                    let succeeded = match continuation {
                        PageContinuation::Interactive { commands, count } => issuer
                            .as_mut()
                            .execute_counted_parsed_commands_with_handoff(
                                commands,
                                count,
                                &mut handoff,
                            ),
                        PageContinuation::Ipc { commands, .. } => issuer
                            .as_mut()
                            .execute_routed_ipc_chain_with_handoff(commands, None, &mut handoff)
                            .is_ok(),
                    };
                    issuer.as_mut().runtime_work_available();
                    if let Some(receipt) = handoff {
                        receipt.finish(true, succeeded, source_token);
                    }
                });
            })
            .map_err(|_| "The requested page window is no longer available".to_owned())?;
        handoff.take();
        Ok(
            serde_json::json!({"status":"accepted", "action":command.name.trim_start_matches("tab-"),
            "tab_id":id.to_string(), "window_id":result_window.to_string(), "pending":true}),
        )
    }
}
