//! Frozen, sequential close plans over actual registered page owners. A request
//! or dialog approval is not completion: only Core/native release advances it.
use crate::browser_ui_page_windows::{
    Candidate, PageAnchor, PageContinuation, PageHandoff, PageWindowSnapshot, candidates,
    continuation_current,
};
use crate::{CxxQtType, ParsedCommand, Pin, QString, TabId, qobject};
use ferric_browser_application::NativeOperationOutcome;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

struct Plan {
    targets: Vec<Candidate>,
    closed: Vec<TabId>,
    survivor: Option<Candidate>,
    continuation: Option<PageContinuation>,
    handoff: Option<PageHandoff>,
    expires: Instant,
    focus_generation: u64,
}
type SharedPlan = Arc<Mutex<Plan>>;
fn plans() -> &'static Mutex<HashMap<String, SharedPlan>> {
    static PLANS: OnceLock<Mutex<HashMap<String, SharedPlan>>> = OnceLock::new();
    PLANS.get_or_init(Default::default)
}
fn lookup(nonce: &str) -> Option<SharedPlan> {
    plans().lock().ok()?.get(nonce).cloned()
}
pub(super) fn pending() -> bool {
    plans().lock().is_ok_and(|plans| !plans.is_empty())
}
pub(super) fn expire() {
    let expired = plans()
        .lock()
        .map(|plans| {
            plans
                .iter()
                .filter_map(|(nonce, plan)| {
                    plan.lock()
                        .ok()
                        .filter(|plan| {
                            Instant::now() >= plan.expires
                                || plan
                                    .targets
                                    .iter()
                                    .skip(plan.closed.len())
                                    .any(|target| target.owner.thread.is_destroyed())
                                || plan
                                    .continuation
                                    .as_ref()
                                    .is_some_and(PageContinuation::has_commands)
                                    && plan
                                        .survivor
                                        .as_ref()
                                        .is_some_and(|target| target.owner.thread.is_destroyed())
                        })
                        .map(|_| nonce.clone())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for nonce in expired {
        stop(&nonce, NativeOperationOutcome::Stale);
    }
}

pub(super) struct CloseIntent {
    nonce: String,
    target: Candidate,
    approved: bool,
    window_close: bool,
    focus_generation: u64,
    active: bool,
}
impl Drop for CloseIntent {
    fn drop(&mut self) {
        if self.active {
            stop(&self.nonce, NativeOperationOutcome::OwnerLost);
        }
    }
}
fn current(ui: &qobject::BrowserUi, target: &Candidate) -> bool {
    let rust = ui.rust();
    rust.window == Some(target.owner.window)
        && rust.window_token.to_string() == target.owner.token
        && PageWindowSnapshot::capture(ui).is_some_and(|snapshot| {
            snapshot.scope == target.owner.snapshot.scope
                && snapshot.group == target.owner.snapshot.group
                && snapshot.pages.iter().any(|page| {
                    page.id == target.page.id
                        && page.target == target.page.target
                        && page.pinned == target.page.pinned
                        && page.view_identity == target.page.view_identity
                })
        })
}
/// A per-Ui one-shot intent cannot authorize an unrelated tab/window request.
pub(super) fn allows_close(ui: &qobject::BrowserUi, index: Option<i32>) -> bool {
    let Some(intent) = &ui.rust().page_close_intent else {
        return true;
    };
    if !intent.approved || lookup(&intent.nonce).is_none() || !current(ui, &intent.target) {
        return false;
    }
    match index {
        Some(index) => {
            if intent.window_close || ui.rust().tab_ids.len() <= 1 {
                return false;
            }
            usize::try_from(index)
                .ok()
                .and_then(|index| ui.rust().tab_ids.get(index))
                == Some(&intent.target.page.id)
        }
        None => {
            intent.window_close
                && ui.rust().tab_ids.as_slice() == [intent.target.page.id]
                && ui
                    .rust()
                    .state
                    .as_ref()
                    .and_then(|state| state.windows().get(&intent.target.owner.window))
                    .is_some_and(|window| window.tabs.as_slice() == [intent.target.page.id])
        }
    }
}

fn stop(nonce: &str, outcome: NativeOperationOutcome) {
    let Some(plan) = plans()
        .lock()
        .ok()
        .and_then(|mut plans| plans.remove(nonce))
    else {
        return;
    };
    let Ok(mut plan) = plan.lock() else {
        return;
    };
    let closed = plan.closed.len() as u16;
    let requested = plan.targets.len() as u16;
    let outcome = if closed > 0 && closed < requested {
        NativeOperationOutcome::ClosedPartially { closed, requested }
    } else {
        outcome
    };
    if let Some(handoff) = plan.handoff.take() {
        handoff.finish_close(outcome);
    }
    // A UI notice is helpful even for local commands without an IPC receipt.
    let notice = if closed > 0 && closed < requested {
        format!("Closed {closed} of {requested} pages; remaining pages were kept")
    } else {
        "Page close stopped; remaining pages were kept".into()
    };
    let owner = plan
        .survivor
        .as_ref()
        .or_else(|| plan.targets.get(closed as usize))
        .cloned();
    drop(plan);
    if let Some(owner) = owner {
        let nonce = nonce.to_owned();
        let _ = owner.owner.thread.queue(move |mut ui| {
            if let Some(intent) = ui
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .page_close_intent
                .as_mut()
                && intent.nonce == nonce
            {
                intent.active = false;
            }
            ui.as_mut().set_status_text(QString::from(notice));
        });
    }
}

fn advance(nonce: &str) {
    let Some(shared) = lookup(nonce) else {
        return;
    };
    let (target, done, focus_generation) = {
        let Ok(plan) = shared.lock() else {
            return;
        };
        (
            plan.targets.get(plan.closed.len()).cloned(),
            plan.closed.len() == plan.targets.len(),
            plan.focus_generation,
        )
    };
    if done {
        complete(nonce);
        return;
    }
    let Some(target) = target else {
        stop(nonce, NativeOperationOutcome::Failed);
        return;
    };
    let nonce_owned = nonce.to_owned();
    let thread = target.owner.thread.clone();
    if thread
        .queue(move |mut ui| {
            if lookup(&nonce_owned).is_none() {
                return;
            }
            if !current(ui.as_ref().get_ref(), &target)
                || ui.as_ref().rust().page_close_intent.is_some()
            {
                stop(&nonce_owned, NativeOperationOutcome::Stale);
                return;
            }
            ui.as_mut().queue_pending_engine_action();
            let window_close = ui.as_ref().rust().tab_ids.len() == 1;
            ui.as_mut().rust_mut().as_mut().get_mut().page_close_intent =
                Some(Box::new(CloseIntent {
                    nonce: nonce_owned.clone(),
                    target: target.clone(),
                    approved: false,
                    window_close,
                    focus_generation,
                    active: true,
                }));
            ui.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .pending_engine_action = Some(format!(
                "window-page-close\t{nonce_owned}\t{}",
                target.page.id
            ));
            ui.as_mut().runtime_work_available();
        })
        .is_err()
    {
        stop(nonce, NativeOperationOutcome::OwnerLost);
    }
}

fn complete(nonce: &str) {
    let Some(shared) = plans()
        .lock()
        .ok()
        .and_then(|mut plans| plans.remove(nonce))
    else {
        return;
    };
    let Ok(mut plan) = shared.lock() else {
        return;
    };
    let Some(handoff) = plan.handoff.take() else {
        return;
    };
    let continuation = plan
        .continuation
        .take()
        .expect("admitted close continuation");
    if !continuation.has_commands() {
        handoff.finish_close(NativeOperationOutcome::Applied);
        return;
    }
    let Some(survivor) = plan.survivor.clone() else {
        handoff.finish_close(NativeOperationOutcome::Stale);
        return;
    };
    let closed = plan.closed.clone();
    drop(plan);
    let fallback = handoff.clone();
    if survivor
        .owner
        .thread
        .clone()
        .queue(move |mut ui| {
            let index = ui
                .as_ref()
                .rust()
                .tab_ids
                .iter()
                .position(|id| *id == survivor.page.id)
                .and_then(|index| i32::try_from(index).ok());
            if !current(ui.as_ref().get_ref(), &survivor)
                || !index.is_some_and(|index| ui.as_mut().select_tab(index))
            {
                handoff.finish_close(NativeOperationOutcome::Stale);
                return;
            }
            let Some(anchor) = PageAnchor::prepared(
                ui.as_ref().get_ref(),
                survivor.owner.snapshot.scope.clone(),
                survivor.owner.snapshot.group.clone(),
                survivor.page.id,
            ) else {
                handoff.finish_close(NativeOperationOutcome::Stale);
                return;
            };
            if !handoff.accept_closed_anchor(&closed, anchor)
                || !continuation_current(
                    ui.as_ref().get_ref(),
                    &survivor.owner.snapshot.scope,
                    &survivor.owner.snapshot.group,
                    survivor.page.target,
                )
            {
                handoff.finish_close(NativeOperationOutcome::Stale);
                return;
            }
            handoff.request_focus();
            let mut handoff = Some(handoff);
            let succeeded = match continuation {
                PageContinuation::Interactive { commands, count } => ui
                    .as_mut()
                    .execute_counted_parsed_commands_with_handoff(commands, count, &mut handoff),
                PageContinuation::Ipc { commands, .. } => ui
                    .as_mut()
                    .execute_routed_ipc_chain_with_handoff(commands, None, &mut handoff)
                    .is_ok(),
                PageContinuation::Recorded { commands, .. } => ui
                    .as_mut()
                    .execute_page_recorded_commands(commands, None, &mut handoff)
                    .is_ok(),
            };
            ui.as_mut().runtime_work_available();
            if let Some(handoff) = handoff {
                handoff.finish(true, succeeded, ui.as_ref().get_ref());
            }
        })
        .is_err()
    {
        fallback.finish_close(NativeOperationOutcome::OwnerLost);
    }
}

fn parse(command: &ParsedCommand, count: u32) -> Result<(Option<TabId>, usize), String> {
    let mut id = None;
    let mut explicit_count = None;
    let mut args = command.arguments.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--id" if id.is_none() => {
                id = Some(
                    TabId::from_display(args.next().ok_or("tab-close --id requires an ID")?)
                        .ok_or("Invalid page ID")?,
                );
            }
            "--count" if explicit_count.is_none() => {
                explicit_count = Some(
                    args.next()
                        .and_then(|arg| arg.parse::<usize>().ok())
                        .filter(|count| (1..=100).contains(count))
                        .ok_or("tab-close count must be 1 to 100")?,
                );
            }
            value if !value.starts_with('-') && id.is_none() => {
                id = Some(TabId::from_display(value).ok_or("Invalid page ID")?);
            }
            _ => return Err("Use tab-close [--id ID] [--count N]".into()),
        }
    }
    if id.is_some() && explicit_count.is_some() {
        return Err("tab-close cannot combine --id and --count".into());
    }
    Ok((id, explicit_count.unwrap_or(count.clamp(1, 100) as usize)))
}

impl qobject::BrowserUi {
    pub(super) fn execute_page_window_ipc_close(
        self: Pin<&mut Self>,
        command: &ParsedCommand,
        route: &crate::IpcRoute,
        mut commands: Vec<(ParsedCommand, crate::IpcRoute)>,
        operation: Option<&str>,
        handoff: &mut Option<PageHandoff>,
    ) -> Result<crate::Value, crate::PublicError> {
        if route.context.is_some() {
            return Err("context routing is not valid for page close".into());
        }
        self.as_ref().validate_ipc_route_public(route)?;
        if Some(self.as_ref().ipc_route_window(route)?) != self.as_ref().rust().window {
            return Err("Close pages in their issuing window".into());
        }
        for (_, route) in &mut commands {
            route.selector = crate::DispatchTarget::Active;
        }
        let mut tail_route = route.clone();
        tail_route.selector = crate::DispatchTarget::Active;
        self.execute_page_window_close(
            command,
            1,
            PageContinuation::Ipc {
                commands,
                route: tail_route,
            },
            operation,
            handoff,
        )
        .map_err(Into::into)
    }
    pub(super) fn execute_page_window_close(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        count: u32,
        continuation: PageContinuation,
        operation: Option<&str>,
        incoming: &mut Option<PageHandoff>,
    ) -> Result<crate::Value, String> {
        self.as_ref()
            .rust()
            .registry
            .validate(command, self.as_ref().rust().core_mode)
            .map_err(|e| e.to_string())?;
        let (id, count) = parse(command, count)?;
        let source = PageWindowSnapshot::capture_checked(self.as_ref().get_ref())?;
        let selected = source.selected.ok_or("Select a live page")?;
        self.as_mut().refresh_ipc_owner();
        let pages = candidates(
            crate::live_ipc_owners::page_window_owners(&source.scope),
            &source,
        );
        let selected_index = pages
            .iter()
            .position(|page| page.page.id == selected)
            .ok_or("Select a registered page")?;
        let mut targets = if let Some(id) = id {
            vec![
                pages
                    .iter()
                    .find(|page| page.page.id == id)
                    .cloned()
                    .ok_or("Page is outside this logical group")?,
            ]
        } else {
            pages
                .iter()
                .cycle()
                .skip(selected_index)
                .take(pages.len())
                .filter(|page| !page.page.pinned)
                .take(count)
                .cloned()
                .collect::<Vec<_>>()
        };
        if targets.is_empty() {
            return Err("No unpinned pages available to close".into());
        }
        let survivor = pages
            .iter()
            .cycle()
            .skip(selected_index)
            .take(pages.len())
            .find(|page| !targets.iter().any(|target| target.page.id == page.page.id))
            .cloned();
        if continuation.has_commands() && survivor.is_none() {
            return Err("Keep a page open for the commands following tab-close".into());
        }
        // Resolve the exact group before effects. Retire the issuing page last.
        targets.sort_by_key(|page| {
            (
                page.owner.token == self.as_ref().rust().window_token.to_string(),
                page.page.id == selected,
            )
        });
        let nonce = crate::Uuid::new_v4().to_string();
        let mut registry = plans().lock().map_err(|_| "Close plans are unavailable")?;
        if registry.len() >= 64 {
            return Err("Too many page close requests are pending".into());
        }
        if registry.values().any(|plan| {
            plan.lock().is_ok_and(|plan| {
                plan.targets
                    .iter()
                    .any(|other| targets.iter().any(|target| target.page.id == other.page.id))
            })
        }) {
            return Err("One of these pages already has a pending close".into());
        }
        let handoff = self
            .as_mut()
            .prepare_page_handoff(incoming.clone(), &source, operation)?;
        let requested = targets.len();
        registry.insert(
            nonce.clone(),
            Arc::new(Mutex::new(Plan {
                targets,
                closed: Vec::new(),
                survivor,
                continuation: Some(continuation),
                handoff: Some(handoff),
                expires: Instant::now() + Duration::from_secs(300),
                focus_generation: crate::page_window_focus::generation(),
            })),
        );
        drop(registry);
        incoming.take();
        advance(&nonce);
        self.as_mut().runtime_work_available();
        Ok(
            serde_json::json!({"status":"accepted", "action":"close", "count":requested, "pending":true}),
        )
    }
    pub(super) fn page_close_index(&self, nonce: &QString) -> i32 {
        let Some(intent) = &self.rust().page_close_intent else {
            return -1;
        };
        if intent.nonce != nonce.to_string()
            || lookup(&intent.nonce).is_none()
            || !current(self, &intent.target)
        {
            return -1;
        }
        self.rust()
            .tab_ids
            .iter()
            .position(|id| *id == intent.target.page.id)
            .and_then(|index| i32::try_from(index).ok())
            .unwrap_or(-1)
    }
    pub(super) fn page_close_focus_intent(&self, nonce: &QString) -> QString {
        if self.page_close_index(nonce) < 0 {
            return QString::default();
        }
        let intent = self
            .rust()
            .page_close_intent
            .as_ref()
            .expect("matched close intent");
        if !crate::page_window_focus::current(intent.focus_generation) {
            return QString::default();
        }
        QString::from(intent.focus_generation.to_string())
    }

    pub(super) fn authorize_page_close(mut self: Pin<&mut Self>, nonce: &QString) -> bool {
        if self.as_ref().get_ref().page_close_index(nonce) < 0
            || self
                .as_ref()
                .rust()
                .page_close_intent
                .as_ref()
                .is_none_or(|intent| {
                    intent.window_close != (self.as_ref().rust().tab_ids.len() == 1)
                })
        {
            return false;
        }
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_close_intent
            .as_mut()
            .expect("matched")
            .approved = true;
        true
    }
    pub(super) fn cancel_page_close(mut self: Pin<&mut Self>, nonce: &QString) {
        if self
            .as_ref()
            .rust()
            .page_close_intent
            .as_ref()
            .is_none_or(|intent| intent.nonce != nonce.to_string())
        {
            return;
        }
        let mut intent = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_close_intent
            .take()
            .expect("matched");
        intent.active = false;
        stop(&intent.nonce, NativeOperationOutcome::Rejected);
    }
    pub(super) fn page_close_released(mut self: Pin<&mut Self>, nonce: &QString) {
        let valid =
            self.as_ref()
                .rust()
                .page_close_intent
                .as_ref()
                .is_some_and(|intent| {
                    intent.nonce == nonce.to_string()
                        && intent.approved
                        && (self.as_ref().rust().state.as_ref().is_some_and(|state| {
                            !state.tabs().contains_key(&intent.target.page.id)
                        }) || (!self.as_ref().rust().view_alive
                            && self.as_ref().rust().page_window_closing))
                });
        if !valid {
            return;
        }
        let mut intent = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_close_intent
            .take()
            .expect("matched");
        intent.active = false;
        let Some(shared) = lookup(&intent.nonce) else {
            return;
        };
        {
            let Ok(mut plan) = shared.lock() else {
                return;
            };
            if plan
                .targets
                .get(plan.closed.len())
                .is_none_or(|target| target.page.id != intent.target.page.id)
            {
                return;
            }
            plan.closed.push(intent.target.page.id);
        }
        advance(&intent.nonce);
    }
    #[allow(clippy::unused_self)]
    pub(super) fn page_closes_pending(&self) -> bool {
        expire();
        pending()
            || crate::page_window_operations::completions_pending()
            || ferric_browser_ipc::event_writes_pending()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn close_count_is_bounded_and_explicit_ids_do_not_mix_with_counts() {
        let command = |args: &[&str]| ParsedCommand {
            name: "tab-close".into(),
            arguments: args.iter().map(|v| (*v).into()).collect(),
        };
        assert_eq!(parse(&command(&[]), 3).unwrap(), (None, 3));
        assert_eq!(
            parse(&command(&["--count", "100"]), 1).unwrap(),
            (None, 100)
        );
        for args in [
            vec!["--count", "0"],
            vec!["--count", "101"],
            vec!["--id", "tabid-1", "--count", "2"],
            vec!["--count", "2", "--count", "3"],
        ] {
            assert!(parse(&command(&args), 1).is_err());
        }
    }
}
