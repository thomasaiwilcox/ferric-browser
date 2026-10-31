//! Private consumer for moving an existing view between actual Core owners.
//! No serialized transfer can retain a document, history cursor or receipt.
use crate::browser_ui_page_windows::{
    PageAnchor, PageContinuation, PageHandoff, PageScope, PageWindowOwner, PageWindowSnapshot,
};
use crate::{
    CxxQtType, Event, Pin, QString, RuntimeEffect, RuntimeInput, TabTransfer, Target, Uuid,
    WindowId, qobject,
};
use cxx_qt::Threading;
use std::time::{Duration, Instant};

pub(super) struct LivePageMove {
    nonce: Uuid,
    source: PageWindowSnapshot,
    token: String,
    window: WindowId,
    target: Target,
    prompt: String,
    group: String,
    destination: Option<PageWindowOwner>,
    continuation: Option<PageContinuation>,
    handoff: Option<PageHandoff>,
    expires: Instant,
    effects: Option<Vec<RuntimeEffect>>,
}

pub(super) struct PreparedLivePage {
    nonce: Uuid,
    placeholder: Target,
    window: WindowId,
    scope: PageScope,
    group: String,
    anchor: PageAnchor,
    effects: Option<Vec<RuntimeEffect>>,
    completion: Option<LivePageCompletion>,
}

struct LivePageCompletion {
    handoff: PageHandoff,
    continuation: PageContinuation,
    target: Target,
    scope: PageScope,
    group: String,
}

/// Immutable, native-owned rollback authority. Cancellation can retire the
/// admission record without erasing proof of an untouched source document.
pub(crate) struct PageRollbackProof(Option<RollbackSource>);
struct RollbackSource {
    scope: PageScope,
    group: String,
    token: String,
    window: WindowId,
    target: Target,
}
pub(crate) fn capture_page_rollback(
    source: &qobject::BrowserUi,
    nonce: &QString,
) -> Box<PageRollbackProof> {
    Box::new(PageRollbackProof(
        source
            .rust()
            .live_page_move
            .as_ref()
            .filter(|pending| {
                pending.nonce.to_string() == nonce.to_string() && source_current(source, pending)
            })
            .map(|pending| RollbackSource {
                scope: pending.source.scope.clone(),
                group: pending.source.group.clone(),
                token: pending.token.clone(),
                window: pending.window,
                target: pending.target,
            }),
    ))
}
impl PageRollbackProof {
    pub(crate) fn source_current(&self, ui: &qobject::BrowserUi) -> bool {
        let Some(proof) = &self.0 else {
            return false;
        };
        let rust = ui.rust();
        rust.view_alive
            && !rust.profile_bootstrap_pending
            && !rust.page_window_closing
            && !rust.ipc_shutdown_gate
            && rust.window == Some(proof.window)
            && rust.window_token.to_string() == proof.token
            && rust.tab_ids.contains(&proof.target.tab)
            && rust.state.as_ref().is_some_and(|state| {
                state.shutdown() == ferric_browser_core::ShutdownState::Running
                    && state.capture_target(proof.target.tab) == Some(proof.target)
                    && state
                        .windows()
                        .get(&proof.window)
                        .is_some_and(|window| window.tabs.contains(&proof.target.tab))
            })
            && crate::page_window_undo::Scope::capture(rust)
                .is_some_and(|scope| proof.scope.matches_undo(&scope) && scope.group == proof.group)
    }
}

fn same_page_set(current: &[crate::TabId], captured: &[crate::TabId]) -> bool {
    // Both lists originate in Core, which guarantees unique page IDs. Permit
    // reordering without accepting an extra, removed or substituted page.
    current.len() == captured.len() && current.iter().all(|id| captured.contains(id))
}

fn source_current(ui: &qobject::BrowserUi, pending: &LivePageMove) -> bool {
    let rust = ui.rust();
    let Some(state) = rust.state.as_ref() else {
        return false;
    };
    let scope = crate::page_window_undo::Scope::capture(rust);
    pending.effects.is_none()
        && Instant::now() <= pending.expires
        && rust.view_alive
        && !rust.profile_bootstrap_pending
        && !rust.page_window_closing
        && !rust.ipc_shutdown_gate
        && rust.window_token.to_string() == pending.token
        && rust.window == Some(pending.window)
        && rust.tab == Some(pending.target.tab)
        && rust.tab_ids.len() > 1
        && same_page_set(&rust.tab_ids, &pending.source.reserved)
        && state.shutdown() == ferric_browser_core::ShutdownState::Running
        && state.capture_target(pending.target.tab) == Some(pending.target)
        && state.windows().get(&pending.window).is_some_and(|window| {
            window.active_tab == Some(pending.target.tab)
                && same_page_set(&window.tabs, &pending.source.reserved)
        })
        && scope.is_some_and(|scope| {
            pending.source.scope.matches_undo(&scope) && scope.group == pending.source.group
        })
        && pending.destination.as_ref().is_none_or(|expected| {
            crate::live_ipc_owners::page_window_owners(&pending.source.scope)
                .iter()
                .any(|owner| {
                    owner.token == expected.token
                        && owner.window == expected.window
                        && owner.snapshot.group == expected.snapshot.group
                        && owner.snapshot.pages.iter().any(|page| {
                            expected
                                .snapshot
                                .pages
                                .iter()
                                .any(|saved| saved.target == page.target)
                        })
                })
        })
}

fn destination_current(ui: &qobject::BrowserUi, pending: &LivePageMove, ready: bool) -> bool {
    let rust = ui.rust();
    let Some(prepared) = rust.prepared_live_page.as_ref() else {
        return false;
    };
    let Some(state) = rust.state.as_ref() else {
        return false;
    };
    prepared.nonce == pending.nonce
        && prepared.effects.is_none()
        && prepared.scope == pending.source.scope
        && prepared.group == pending.group
        && rust.page_window_preparing
        && rust.view_alive
        && (!ready || !rust.profile_bootstrap_pending)
        && !rust.ipc_shutdown_gate
        && !rust.page_window_closing
        && rust.ipc_owner.is_none()
        && rust.interaction.prompt_view_identities.is_empty()
        && state.shutdown() == ferric_browser_core::ShutdownState::Running
        && rust.window == Some(prepared.window)
        && state.capture_target(prepared.placeholder.tab) == Some(prepared.placeholder)
        && crate::page_window_undo::Scope::capture(rust).is_some_and(|scope| {
            prepared.scope.matches_undo(&scope) && scope.group == prepared.group
        })
}

impl qobject::BrowserUi {
    pub(super) fn begin_live_page_regroup(
        mut self: Pin<&mut Self>,
        source: PageWindowSnapshot,
        target: Target,
        group: String,
        destination: Option<PageWindowOwner>,
        continuation: PageContinuation,
        handoff: PageHandoff,
    ) -> Result<crate::Value, String> {
        if self.as_ref().rust().live_page_move.is_some()
            || self
                .as_mut()
                .tab_has_active_operations(&QString::from(target.tab.to_string()))
        {
            return Err("Finish the page's pending work before moving it".into());
        }
        let prompt = self
            .as_ref()
            .rust()
            .interaction
            .prompt_view_identities
            .get(&target.tab)
            .cloned()
            .ok_or("The selected page has no current view")?;
        let window = self
            .as_ref()
            .rust()
            .window
            .ok_or("The window is unavailable")?;
        let token = self.as_ref().rust().window_token.to_string();
        let nonce = Uuid::new_v4();
        handoff.request_focus();
        self.as_mut().queue_pending_engine_action();
        let result = serde_json::json!({"status":"accepted", "pending":true,
            "preserves_live_state":true, "tab_id":target.tab.to_string(), "group":group});
        let mut rust = self.as_mut().rust_mut();
        let this = rust.as_mut().get_mut();
        this.live_page_move = Some(Box::new(LivePageMove {
            nonce,
            source,
            token,
            window,
            target,
            prompt,
            group,
            destination,
            continuation: Some(continuation),
            handoff: Some(handoff),
            expires: Instant::now() + Duration::from_secs(15),
            effects: None,
        }));
        this.pending_engine_action = Some(format!("window-live-transfer\t{nonce}\t{}", target.tab));
        Ok(result)
    }

    pub(super) fn initialize_live_page_owner(
        self: Pin<&mut Self>,
        mut destination: Pin<&mut Self>,
        nonce: &QString,
    ) -> bool {
        let owner = self.as_ref();
        let Some(pending) = owner.rust().live_page_move.as_ref() else {
            return false;
        };
        if nonce.to_string() != pending.nonce.to_string()
            || !source_current(self.as_ref().get_ref(), pending)
            || destination.as_ref().rust().prepared_live_page.is_some()
        {
            return false;
        }
        let (nonce, scope, group, tab) = (
            pending.nonce,
            pending.source.scope.clone(),
            pending.group.clone(),
            pending.target.tab,
        );
        destination
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_window_preparing = true;
        let Ok(placeholder) = self
            .as_ref()
            .get_ref()
            .inherit_owner_profile(destination.as_mut(), None)
        else {
            return false;
        };
        destination
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_window_group = group.clone();
        let Some(window) = destination.as_ref().rust().window else {
            return false;
        };
        let Some(anchor) = PageAnchor::prepared(
            destination.as_ref().get_ref(),
            scope.clone(),
            group.clone(),
            tab,
        ) else {
            return false;
        };
        destination
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .prepared_live_page = Some(Box::new(PreparedLivePage {
            nonce,
            placeholder,
            window,
            scope,
            group,
            anchor,
            effects: None,
            completion: None,
        }));
        self.as_ref()
            .rust()
            .live_page_move
            .as_ref()
            .is_some_and(|pending| {
                source_current(self.as_ref().get_ref(), pending)
                    && destination_current(destination.as_ref().get_ref(), pending, false)
            })
    }

    // Native-only initial proof of the actual view's stable identity and its
    // original prompt registration. QML may not substitute another live view.
    pub(super) fn validate_live_page_owner(
        &self,
        destination: &Self,
        nonce: &QString,
        tab: &QString,
        prompt: &QString,
        index: i32,
    ) -> bool {
        self.rust().live_page_move.as_ref().is_some_and(|pending| {
            pending.nonce.to_string() == nonce.to_string()
                && pending.target.tab.to_string() == tab.to_string()
                && pending.prompt == prompt.to_string()
                && usize::try_from(index)
                    .ok()
                    .and_then(|index| self.rust().tab_ids.get(index))
                    == Some(&pending.target.tab)
                && self
                    .rust()
                    .interaction
                    .prompt_view_identities
                    .get(&pending.target.tab)
                    == Some(&pending.prompt)
                && source_current(self, pending)
                && destination_current(destination, pending, true)
        })
    }

    // The native association changes immediately before this entry point.
    // There are no Qt calls, event publication or worker polls in this method.
    pub(super) fn commit_live_page_owner(
        mut self: Pin<&mut Self>,
        mut destination: Pin<&mut Self>,
        nonce: &QString,
    ) -> bool {
        let owner = self.as_ref();
        let Some(pending) = owner.rust().live_page_move.as_ref() else {
            return false;
        };
        if nonce.to_string() != pending.nonce.to_string()
            || !source_current(self.as_ref().get_ref(), pending)
            || !destination_current(destination.as_ref().get_ref(), pending, true)
        {
            return false;
        }
        let Some(source_state) = owner.rust().state.as_ref() else {
            return false;
        };
        let Some(page) = source_state.tabs().get(&pending.target.tab) else {
            return false;
        };
        let Some(transfer) =
            TabTransfer::capture_presented_with_history(page, source_state.journey())
        else {
            return false;
        };
        let (tab, handoff, snapshot, token, window) = (
            pending.target.tab,
            pending
                .handoff
                .as_ref()
                .expect("uncommitted receipt")
                .clone(),
            pending.source.clone(),
            pending.token.clone(),
            pending.window,
        );
        let child_owner = destination.as_ref();
        let prepared = child_owner
            .rust()
            .prepared_live_page
            .as_ref()
            .expect("validated");
        let (child_window, anchor) = (prepared.window, prepared.anchor.clone());
        handoff.transfer_anchor(&snapshot, &token, window, anchor, || {
            let imported = destination
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .state
                .as_mut()
                .expect("validated")
                .dispatch_runtime(RuntimeInput::Lifecycle(Event::TransferTabIn {
                    window: child_window,
                    transfer,
                }));
            let Ok(imported) = imported else {
                return false;
            };
            let released = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .state
                .as_mut()
                .expect("validated")
                .dispatch_runtime(RuntimeInput::Lifecycle(Event::TransferTabOut { tab }));
            let Ok(released) = released else {
                return false;
            };
            let mut source = self.as_mut().rust_mut();
            let source = source.as_mut().get_mut();
            let mut child = destination.as_mut().rust_mut();
            let child = child.as_mut().get_mut();
            if let Some(history) = source.navigation.content_histories.remove(&tab) {
                child.navigation.content_histories.insert(tab, history);
            }
            if let Some(position) = source.navigation.scroll_positions.remove(&tab) {
                child.navigation.scroll_positions.insert(tab, position);
            }
            if let Some(identity) = source.navigation.page_document_identities.get(tab) {
                child
                    .navigation
                    .page_document_identities
                    .establish(tab, identity);
            }
            source.navigation.page_document_identities.invalidate(tab);
            if let Some(kind) = source.navigation.engine_navigation_kinds.remove(&tab) {
                child.navigation.engine_navigation_kinds.insert(tab, kind);
            }
            for node in child.state.as_ref().expect("validated").journey().nodes() {
                if let Some(id) = source.navigation.durable_journey_id(node.id) {
                    child
                        .navigation
                        .journey_durable_ids
                        .borrow_mut()
                        .insert(node.id, id);
                }
            }
            let mut transitions = Vec::new();
            source
                .navigation
                .pending_journey_transitions
                .retain(|transition| {
                    if transition.0.tab == tab {
                        transitions.push(transition.clone());
                        false
                    } else {
                        true
                    }
                });
            child
                .navigation
                .pending_journey_transitions
                .extend(transitions);
            if source
                .navigation
                .pending_journey_parent
                .is_some_and(|(target, _)| target.tab == tab)
            {
                child.navigation.pending_journey_parent =
                    source.navigation.pending_journey_parent.take();
            }
            // Update identity lookup before native completion can notify a
            // lifecycle consumer. QObject/model notifications follow later.
            source.tab_ids.retain(|candidate| *candidate != tab);
            source.tab = source
                .state
                .as_ref()
                .and_then(|state| state.windows().get(&window))
                .and_then(|window| window.active_tab);
            source.active_tab_index = source
                .tab
                .and_then(|active| source.tab_ids.iter().position(|id| *id == active))
                .and_then(|index| i32::try_from(index).ok())
                .unwrap_or(0);
            child.tab_ids = vec![tab];
            child.tab = Some(tab);
            child.active_tab_index = 0;
            source.restored_page_windows.remove(&tab);
            source.live_page_move.as_mut().expect("validated").effects = Some(released);
            let pending = source.live_page_move.as_mut().expect("validated");
            let completion = LivePageCompletion {
                handoff: pending.handoff.take().expect("single transfer"),
                continuation: pending.continuation.take().expect("single transfer"),
                target: pending.target,
                scope: pending.source.scope.clone(),
                group: pending.group.clone(),
            };
            let prepared = child.prepared_live_page.as_mut().expect("validated");
            prepared.effects = Some(imported);
            prepared.completion = Some(completion);
            true
        })
    }

    // Native completion has already removed exclusive staging. This publishes
    // no notification and makes ordinary registration possible for the child.
    pub(super) fn ready_live_page_owner(mut self: Pin<&mut Self>, nonce: &QString) -> bool {
        let valid = self
            .as_ref()
            .rust()
            .prepared_live_page
            .as_ref()
            .is_some_and(|prepared| {
                let rust = self.as_ref();
                let rust = rust.rust();
                prepared.nonce.to_string() == nonce.to_string()
                    && prepared.effects.is_some()
                    && prepared.completion.as_ref().is_some_and(|completion| {
                        rust.state.as_ref().is_some_and(|state| {
                            state
                                .tabs()
                                .get(&completion.target.tab)
                                .is_some_and(|page| {
                                    page.window == prepared.window
                                        && page.existence
                                            == ferric_browser_core::ExistenceState::Live
                                })
                        }) && crate::page_window_undo::Scope::capture(rust)
                            .is_some_and(|scope| prepared.scope.matches_undo_profile(&scope))
                    })
            });
        if valid {
            self.as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .page_window_preparing = false;
        }
        valid
    }

    // The committed destination owns continuation and reporter independently
    // of the old Ui. Retiring that old source cannot discard this live page.
    pub(super) fn publish_live_page_owner(
        mut self: Pin<&mut Self>,
        nonce: &QString,
        continue_commands: bool,
    ) -> bool {
        if self
            .as_ref()
            .rust()
            .prepared_live_page
            .as_ref()
            .is_none_or(|prepared| {
                prepared.nonce.to_string() != nonce.to_string()
                    || prepared.effects.is_none()
                    || prepared.completion.is_none()
            })
        {
            return false;
        }
        let mut prepared = *self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .prepared_live_page
            .take()
            .expect("validated");
        let _ = self.as_ref().get_ref().qt_thread().queue(move |mut ui| {
            ui.as_mut()
                .observe_runtime_effects(prepared.effects.take().expect("committed"));
            ui.as_mut().sync_tab_order_from_core();
            let completion = prepared.completion.take().expect("single use");
            if !continue_commands {
                completion
                    .handoff
                    .finish(true, false, ui.as_ref().get_ref());
                return;
            }
            if !crate::browser_ui_page_windows::continuation_current(
                ui.as_ref().get_ref(),
                &completion.scope,
                &completion.group,
                completion.target,
            ) {
                completion
                    .handoff
                    .finish(false, false, ui.as_ref().get_ref());
                return;
            }
            let mut handoff = Some(completion.handoff);
            let succeeded = match completion.continuation {
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
        });
        true
    }

    pub(super) fn publish_live_page_source(mut self: Pin<&mut Self>, nonce: &QString) {
        if self
            .as_ref()
            .rust()
            .live_page_move
            .as_ref()
            .is_none_or(|pending| {
                pending.nonce.to_string() != nonce.to_string() || pending.effects.is_none()
            })
        {
            return;
        }
        let mut pending = *self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .live_page_move
            .take()
            .expect("committed");
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_window_preparing = false;
        let _ = self.as_ref().get_ref().qt_thread().queue(move |mut ui| {
            ui.as_mut()
                .observe_runtime_effects(pending.effects.take().expect("committed"));
            ui.as_mut().sync_tab_order_from_core();
            ui.as_mut().runtime_work_available();
        });
    }

    pub(super) fn fail_live_page_owner(mut self: Pin<&mut Self>, nonce: &QString) {
        if self
            .as_ref()
            .rust()
            .prepared_live_page
            .as_ref()
            .is_some_and(|prepared| prepared.nonce.to_string() == nonce.to_string())
        {
            let prepared = self
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .prepared_live_page
                .take()
                .expect("matched");
            if let Some(completion) = prepared.completion {
                completion.handoff.owner_lost();
            }
        }
    }

    pub(super) fn cancel_live_page_move(mut self: Pin<&mut Self>, nonce: &QString) {
        if self
            .as_ref()
            .rust()
            .live_page_move
            .as_ref()
            .is_none_or(|pending| pending.nonce.to_string() != nonce.to_string())
        {
            return;
        }
        if self
            .as_ref()
            .rust()
            .live_page_move
            .as_ref()
            .is_some_and(|pending| pending.effects.is_some())
        {
            self.as_mut().publish_live_page_source(nonce);
            return;
        }
        let pending = self
            .as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .live_page_move
            .take()
            .expect("matched");
        self.as_mut()
            .rust_mut()
            .as_mut()
            .get_mut()
            .page_window_preparing = false;
        if let Some(handoff) = pending.handoff {
            handoff.finish(false, false, self.as_ref().get_ref());
        }
        self.as_mut().runtime_work_available();
    }
}
