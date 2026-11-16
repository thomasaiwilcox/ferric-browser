//! Independent bounded read turns. No selector slot, writer, reservation or
//! configuration publication permission is acquired by scheduling a query.
use crate::browser_ui_page_windows::{PageContinuation, PageHandoff};
use crate::{CxxQtType, ParsedCommand, Pin, PublicError, Value, qobject};
use cxx_qt::Threading;
use ferric_browser_application::{ConfigurationRequestCapture, OperationUpdate};
use ferric_browser_ipc::ErrorCode;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

const MAX_PENDING_READS: usize = 64;

#[derive(Default)]
pub(super) struct ReadQueueState {
    pending: Arc<AtomicUsize>,
    claims: BTreeMap<String, Arc<()>>,
}
impl ReadQueueState {
    pub(super) fn revoke_operation(&mut self, id: &str) {
        self.claims.remove(id);
    }
    fn acquire(&self) -> Result<ReadTicket, PublicError> {
        self.pending
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                (count < MAX_PENDING_READS).then_some(count + 1)
            })
            .map_err(|_| {
                PublicError::new(
                    ErrorCode::Busy,
                    "Configuration reads are still pending. Try again.",
                    "read queue is full",
                )
            })?;
        Ok(ReadTicket(Arc::clone(&self.pending)))
    }
}
struct ReadTicket(Arc<AtomicUsize>);
impl Drop for ReadTicket {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

struct ReadTailScope {
    window: Option<crate::WindowId>,
    tab: Option<crate::TabId>,
    target: Option<crate::Target>,
}
impl ReadTailScope {
    fn capture(rust: &crate::BrowserUiRust) -> Self {
        Self {
            window: rust.window,
            tab: rust.tab,
            target: crate::runtime_guard::current_target(rust.state.as_ref(), rust.tab),
        }
    }
    fn validate(&self, rust: &crate::BrowserUiRust) -> Result<(), PublicError> {
        if rust.window == self.window
            && rust.tab == self.tab
            && self.target.is_none_or(|target| {
                crate::runtime_guard::captured_target_is_current(rust.state.as_ref(), target)
            })
        {
            Ok(())
        } else {
            Err(PublicError::new(
                ErrorCode::StaleTarget,
                "The command target changed. Try again.",
                "configuration read continuation target retired",
            ))
        }
    }
}

pub(super) struct ConfigurationReadTurn {
    owner: ConfigurationRequestCapture,
    generation: u64,
}
impl ConfigurationReadTurn {
    // Call sites are the IPC timer, an admitted preparation turn, and the Qt
    // queued read callback. This capability is never stored in routes/macros.
    pub(super) fn capture(ui: &qobject::BrowserUi) -> Option<Self> {
        Some(Self {
            owner: ui.rust().state.as_ref()?.capture_configuration_request()?,
            generation: ui.rust().configuration_io.selector_origin_generation,
        })
    }
    pub(super) fn validate_continuation(&self, ui: &qobject::BrowserUi) -> Result<(), PublicError> {
        self.validate(ui)?;
        if self.generation == ui.rust().configuration_io.selector_origin_generation {
            Ok(())
        } else {
            Err(PublicError::new(
                ErrorCode::StaleTarget,
                "The settings request changed. Try again.",
                "read continuation superseded by configuration request",
            ))
        }
    }

    fn lifetime_current(&self, ui: &qobject::BrowserUi) -> bool {
        let rust = ui.rust();
        rust.view_alive
            && !rust.profile_bootstrap_pending
            && !rust.ipc_shutdown_gate
            && rust
                .state
                .as_ref()
                .is_some_and(|state| state.configuration_request_owner_is_current(&self.owner))
    }

    pub(super) fn validate(&self, ui: &qobject::BrowserUi) -> Result<(), PublicError> {
        let rust = ui.rust();
        if rust.view_alive
            && !rust.profile_bootstrap_pending
            && !rust.ipc_shutdown_gate
            && rust
                .state
                .as_ref()
                .is_some_and(|state| state.configuration_request_is_current(&self.owner))
        {
            Ok(())
        } else {
            Err(PublicError::new(
                ErrorCode::StaleTarget,
                "The settings owner changed. Try again.",
                "configuration read owner/revision retired",
            ))
        }
    }
}

struct ReadOperation {
    id: String,
    sequence: u64,
    claim: Arc<()>,
}
impl ReadOperation {
    fn owns(&self, rust: &crate::BrowserUiRust) -> bool {
        rust.configuration_reads
            .claims
            .get(&self.id)
            .is_some_and(|claim| Arc::ptr_eq(claim, &self.claim))
            && self.same_request(rust)
    }
    fn same_request(&self, rust: &crate::BrowserUiRust) -> bool {
        rust.operation(&self.id)
            .is_some_and(|record| record.sequence == self.sequence && !record.state.is_terminal())
    }
}

fn validate_execution(
    turn: &ConfigurationReadTurn,
    operation: Option<&ReadOperation>,
    ui: &qobject::BrowserUi,
) -> Result<(), PublicError> {
    turn.validate(ui)?;
    if operation.is_none_or(|operation| operation.owns(ui.rust())) {
        Ok(())
    } else {
        Err(PublicError::new(
            ErrorCode::Cancelled,
            "Configuration inspection was cancelled.",
            "read operation is no longer live",
        ))
    }
}

impl qobject::BrowserUi {
    pub(super) fn queue_configuration_read(
        mut self: Pin<&mut Self>,
        command: ParsedCommand,
        tail: PageContinuation,
        operation: Option<&str>,
        handoff: &mut Option<PageHandoff>,
    ) -> Result<Value, PublicError> {
        let owner = self
            .as_ref()
            .rust()
            .state
            .as_ref()
            .and_then(ferric_browser_application::BrowserApplication::capture_configuration_request)
            .ok_or_else(|| {
                PublicError::new(
                    ErrorCode::Busy,
                    "Settings are still loading. Try again.",
                    "read owner unavailable",
                )
            })?;
        let generation = self
            .as_ref()
            .rust()
            .configuration_io
            .selector_origin_generation;
        let ticket = self.as_ref().rust().configuration_reads.acquire()?;
        let tail_scope = tail
            .has_commands()
            .then(|| ReadTailScope::capture(self.as_ref().rust()));
        // Preserve the accepted prefix before a queued tail can produce a new
        // engine action. This also covers single-command/action callers.
        self.as_mut().queue_pending_engine_action();
        let operation = if handoff.is_none() {
            if let Some(id) = operation {
                let mut rust = self.as_mut().rust_mut();
                let this = rust.as_mut().get_mut();
                let record = this
                    .operation(id)
                    .ok_or_else(|| PublicError::engine("read operation is unavailable"))?;
                let sequence = record.sequence;
                if record.state.is_terminal() {
                    return Err(PublicError::engine("read operation is already terminal"));
                }
                let claim = Arc::new(());
                this.set_operation(
                    id.to_owned(),
                    OperationUpdate::running("configuration inspection"),
                );
                this.configuration_reads
                    .claims
                    .insert(id.to_owned(), Arc::clone(&claim));
                Some(ReadOperation {
                    id: id.into(),
                    sequence,
                    claim,
                })
            } else {
                None
            }
        } else {
            None
        };
        let response_operation = operation.as_ref().map(|operation| operation.id.clone());
        let mut handoff = handoff.take();
        self.as_ref()
            .qt_thread()
            .queue(move |mut ui| {
                let _ticket = ticket;
                let turn = ConfigurationReadTurn { owner, generation };
                let current = validate_execution(&turn, operation.as_ref(), ui.as_ref().get_ref());
                let result = current
                    .and_then(|()| {
                        ui.as_mut()
                            .execute_configuration_read_command(&command, &turn)
                    })
                    .and_then(|_| {
                        validate_execution(&turn, operation.as_ref(), ui.as_ref().get_ref())?;
                        if tail_scope.is_some() {
                            turn.validate_continuation(ui.as_ref().get_ref())?;
                        }
                        if let Some(scope) = &tail_scope {
                            scope.validate(ui.as_ref().rust())?;
                        }
                        let id = operation.as_ref().map(|operation| operation.id.as_str());
                        match tail {
                            PageContinuation::Interactive { commands, count } => {
                                if commands.is_empty()
                                    || ui.as_mut().execute_counted_parsed_commands_with_read(
                                        commands,
                                        count,
                                        &mut handoff,
                                        None,
                                        Some(&turn),
                                    )
                                {
                                    Ok(())
                                } else {
                                    {
                                        let message = ui.as_ref().rust().status_text.to_string();
                                        Err(PublicError::new(
                                            ErrorCode::InvalidArgument,
                                            message.clone(),
                                            message,
                                        ))
                                    }
                                }
                            }
                            PageContinuation::Ipc { commands, .. } => {
                                if commands.is_empty() {
                                    Ok(())
                                } else {
                                    ui.as_mut()
                                        .execute_routed_ipc_chain_with_read(
                                            commands,
                                            id,
                                            &mut handoff,
                                            None,
                                            Some(&turn),
                                        )
                                        .map(|_| ())
                                }
                            }
                            PageContinuation::Recorded { commands, .. } => {
                                if commands.is_empty() {
                                    Ok(())
                                } else {
                                    ui.as_mut()
                                        .execute_page_recorded_commands_with_read(
                                            commands,
                                            id,
                                            &mut handoff,
                                            None,
                                            Some(&turn),
                                        )
                                        .map(|_| ())
                                }
                            }
                        }
                    });
                let still_owner = turn.lifetime_current(ui.as_ref().get_ref());
                if let Some(operation) = &operation {
                    let should_finish = operation.owns(ui.as_ref().rust());
                    if still_owner && should_finish {
                        let update = match &result {
                            Ok(()) => {
                                OperationUpdate::completed("configuration inspection applied")
                            }
                            Err(error) => OperationUpdate::failed(
                                crate::browser_ui_operations::failure_kind(error.code()),
                                error.user_message(),
                            ),
                        };
                        ui.as_mut()
                            .rust_mut()
                            .as_mut()
                            .get_mut()
                            .set_operation(operation.id.clone(), update);
                    }
                }
                if let Some(operation) = &operation {
                    let mut rust = ui.as_mut().rust_mut();
                    let this = rust.as_mut().get_mut();
                    if this
                        .configuration_reads
                        .claims
                        .get(&operation.id)
                        .is_some_and(|claim| Arc::ptr_eq(claim, &operation.claim))
                    {
                        this.configuration_reads.claims.remove(&operation.id);
                    }
                }
                if still_owner && let Err(error) = &result {
                    ui.as_mut()
                        .set_status_text(crate::QString::from(error.user_message()));
                }
                if let Some(handoff) = handoff {
                    handoff.finish(still_owner, result.is_ok(), ui.as_ref().get_ref());
                }
            })
            .map_err(|_| {
                PublicError::engine(
                    "The settings window closed before inspection could be scheduled",
                )
            })?;
        Ok(
            serde_json::json!({"status":"accepted","pending":true,"operation_id":response_operation}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_capacity_is_released_by_callback_ticket_drop() {
        let queue = ReadQueueState::default();
        let mut tickets = (0..MAX_PENDING_READS)
            .map(|_| queue.acquire().unwrap())
            .collect::<Vec<_>>();
        assert!(queue.acquire().is_err());
        tickets.pop();
        assert!(queue.acquire().is_ok());
        drop(tickets);
        assert_eq!(queue.pending.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn exact_operation_updates_transfer_completion_but_unrelated_updates_do_not() {
        let mut rust = crate::BrowserUiRust::default();
        rust.admit_operation("read".into()).unwrap();
        rust.set_operation(
            "read".into(),
            OperationUpdate::running("configuration inspection"),
        );
        let claim = Arc::new(());
        rust.configuration_reads
            .claims
            .insert("read".into(), Arc::clone(&claim));
        let operation = ReadOperation {
            id: "read".into(),
            sequence: rust.operation("read").unwrap().sequence,
            claim,
        };
        assert!(operation.owns(&rust));
        rust.admit_operation("unrelated".into()).unwrap();
        rust.set_operation(
            "unrelated".into(),
            OperationUpdate::running("settings validation or persistence"),
        );
        assert!(operation.owns(&rust));
        rust.set_operation(
            "read".into(),
            OperationUpdate::running("native handoff pending"),
        );
        assert!(!operation.owns(&rust));
        assert!(operation.same_request(&rust));
        rust.set_operation("read".into(), OperationUpdate::cancelled());
        assert!(!operation.same_request(&rust));
    }
}
