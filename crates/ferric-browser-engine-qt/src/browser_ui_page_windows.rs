//! Application-wide presentation selection over actual registered Core owners.
//! No renderer-supplied window, profile or document claim enters this directory.

use super::{CxxQtType, ParsedCommand, Pin, PrivacyKind, QString, TabId, qobject};
use cxx_qt::{CxxQtThread, Threading};
use ferric_browser_application::NativeOperationOutcome;
use ferric_browser_application::ProfileInstanceId;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

#[derive(Clone)]
pub(super) struct PageHandoff(Arc<PageHandoffState>);

struct PageHandoffState {
    thread: CxxQtThread<qobject::BrowserUi>,
    anchor: Mutex<PageAnchor>,
    operation: Option<String>,
    reporter: Option<ferric_browser_application::NativeOperationReporter>,
    completion: Mutex<PageCompletionJoin>,
    focus_requested: AtomicBool,
    terminal_focus_started: Arc<AtomicBool>,
    admitted_focus_generation: u64,
    focus_generation: AtomicU64,
}

enum PageTerminal {
    Command {
        selected: bool,
        succeeded: bool,
        destination: Option<crate::page_window_focus::Destination>,
    },
    Outcome(NativeOperationOutcome),
    SettingsObserverRetired,
}

#[derive(Default)]
struct PageCompletionJoin {
    sealed: bool,
    pending: u16,
    terminal: Option<PageTerminal>,
    settings_outcome: Option<NativeOperationOutcome>,
}

impl PageCompletionJoin {
    fn begin_settings(&mut self) -> Result<(), String> {
        if self.sealed {
            return Err("The page command has already finished".into());
        }
        self.pending = self
            .pending
            .checked_add(1)
            .ok_or("Too many pending settings changes")?;
        Ok(())
    }

    fn seal(&mut self, terminal: PageTerminal) -> Option<PageTerminal> {
        if self.sealed {
            return None;
        }
        self.sealed = true;
        self.terminal = Some(terminal);
        self.take_ready()
    }

    fn resolve_settings(&mut self, outcome: NativeOperationOutcome) -> Option<PageTerminal> {
        debug_assert!(self.pending > 0);
        self.pending = self.pending.checked_sub(1)?;
        if outcome != NativeOperationOutcome::Applied {
            // An irreversible save must remain visible even if another step
            // failed or its observer disappeared. No raw diagnostic crosses
            // the original native reporter's privacy boundary.
            let saved = matches!(
                outcome,
                NativeOperationOutcome::SettingsSavedReloadRequired { .. }
            );
            let already_saved = matches!(
                self.settings_outcome,
                Some(NativeOperationOutcome::SettingsSavedReloadRequired { .. })
            );
            if self.settings_outcome.is_none() || (saved && !already_saved) {
                self.settings_outcome = Some(outcome);
            } else if let (
                Some(NativeOperationOutcome::SettingsSavedReloadRequired {
                    durability_uncertain,
                }),
                NativeOperationOutcome::SettingsSavedReloadRequired {
                    durability_uncertain: next,
                },
            ) = (&mut self.settings_outcome, outcome)
            {
                *durability_uncertain |= next;
            }
        }
        self.take_ready()
    }

    fn take_ready(&mut self) -> Option<PageTerminal> {
        if self.pending != 0 {
            return None;
        }
        let terminal = self.terminal.take()?;
        Some(match (self.settings_outcome, terminal) {
            (Some(saved @ NativeOperationOutcome::SettingsSavedReloadRequired { .. }), _) => {
                PageTerminal::Outcome(saved)
            }
            (_, terminal @ PageTerminal::Outcome(outcome))
                if outcome != NativeOperationOutcome::Applied =>
            {
                terminal
            }
            (Some(outcome), _) => PageTerminal::Outcome(outcome),
            (None, PageTerminal::SettingsObserverRetired) => {
                PageTerminal::Outcome(NativeOperationOutcome::OwnerLost)
            }
            (None, terminal) => terminal,
        })
    }
}

/// One obligation per mutation, moved through CSS and persistence. Dropping
/// an unresolved observer means an unknown outcome, never a rolled-back save.
pub(super) struct PageSettingsCompletion {
    handoff: PageHandoff,
    unresolved: bool,
}

impl AdmittedPageSettingsCompletion {
    pub(super) fn observe(
        completion: Option<PageSettingsCompletion>,
    ) -> (
        AdmittedPageSettingsCompletion,
        ferric_browser_application::ConfigurationWriteObserver,
    ) {
        let state = Arc::new(Mutex::new(AdmittedSettingsState {
            completion,
            persistence: None,
            retired: false,
        }));
        let delivery = Arc::clone(&state);
        let observer = Box::new(move |outcome| {
            let completion = {
                let Ok(mut state) = delivery.lock() else {
                    return;
                };
                state.persistence = Some(outcome);
                if state.retired {
                    state.completion.take()
                } else {
                    None
                }
            };
            if let Some(completion) = completion {
                finish_detached_settings(completion, outcome);
            }
        });
        (AdmittedPageSettingsCompletion { state }, observer)
    }
}

impl PageSettingsCompletion {
    pub(super) fn note_durability_uncertainty(&self) {
        if let Some(reporter) = &self.handoff.0.reporter {
            let _ = reporter.settings_saved_with_durability_uncertainty();
        }
    }

    pub(super) fn finish(mut self, outcome: NativeOperationOutcome) {
        self.unresolved = false;
        self.handoff.resolve_settings(outcome);
    }
}

impl Drop for PageSettingsCompletion {
    fn drop(&mut self) {
        if self.unresolved {
            self.handoff
                .resolve_settings(NativeOperationOutcome::OwnerLost);
        }
    }
}

/// Observation of one admitted write. Retirement leaves the original ticket
/// with the existing writer's result; it cannot turn a pending save into loss.
pub(super) struct AdmittedPageSettingsCompletion {
    state: Arc<Mutex<AdmittedSettingsState>>,
}

struct AdmittedSettingsState {
    completion: Option<PageSettingsCompletion>,
    persistence: Option<ferric_browser_application::ConfigurationPersistenceOutcome>,
    retired: bool,
}

impl AdmittedPageSettingsCompletion {
    pub(super) fn has_handoff(&self) -> bool {
        self.state
            .lock()
            .map_or(true, |state| state.completion.is_some())
    }

    pub(super) fn attach_handoff(&self, handoff: &PageHandoff) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "The settings receipt is unavailable")?;
        if state.retired {
            return Err("The settings observer has retired".into());
        }
        if let Some(completion) = state.completion.as_ref() {
            if !completion.handoff.same_root(handoff) {
                return Err("The settings receipt belongs to another command".into());
            }
        } else {
            state.completion = Some(handoff.begin_settings()?);
        }
        Ok(())
    }

    pub(super) fn note_durability_uncertainty(&self) {
        if let Ok(state) = self.state.lock()
            && let Some(completion) = &state.completion
        {
            completion.note_durability_uncertainty();
        }
    }

    pub(super) fn finish(self, outcome: NativeOperationOutcome) {
        self.resolve(outcome, true);
    }

    pub(super) fn rejected(self) {
        self.resolve(NativeOperationOutcome::Failed, false);
    }

    fn resolve(self, outcome: NativeOperationOutcome, admitted: bool) {
        let completed = self.state.lock().ok().and_then(|mut state| {
            state
                .completion
                .take()
                .map(|ticket| (ticket, state.persistence))
        });
        if let Some((completion, persistence)) = completed {
            completion.finish(
                if admitted
                    && persistence
                        == Some(
                            ferric_browser_application::ConfigurationPersistenceOutcome::Unknown,
                        )
                {
                    NativeOperationOutcome::OwnerLost
                } else {
                    outcome
                },
            );
        }
    }
}

impl Drop for AdmittedPageSettingsCompletion {
    fn drop(&mut self) {
        let retired = self.state.lock().ok().map(|mut state| {
            state.retired = true;
            let handoff = state
                .completion
                .as_ref()
                .map(|ticket| ticket.handoff.clone());
            let ready = state
                .persistence
                .and_then(|outcome| state.completion.take().map(|ticket| (ticket, outcome)));
            (handoff, ready)
        });
        if let Some((handoff, ready)) = retired {
            // Sealing does not complete while this write's ticket is pending.
            if let Some(handoff) = handoff {
                handoff.retire_settings_observer();
            }
            if let Some((completion, outcome)) = ready {
                finish_detached_settings(completion, outcome);
            }
        }
    }
}

fn finish_detached_settings(
    completion: PageSettingsCompletion,
    outcome: ferric_browser_application::ConfigurationPersistenceOutcome,
) {
    completion.handoff.clone().retire_settings_observer();
    completion.finish(match outcome {
        ferric_browser_application::ConfigurationPersistenceOutcome::Saved {
            durability_uncertain,
        } => NativeOperationOutcome::SettingsSavedReloadRequired {
            durability_uncertain,
        },
        ferric_browser_application::ConfigurationPersistenceOutcome::Failed => {
            NativeOperationOutcome::Failed
        }
        ferric_browser_application::ConfigurationPersistenceOutcome::Unknown => {
            NativeOperationOutcome::OwnerLost
        }
    });
}

impl Drop for PageHandoffState {
    fn drop(&mut self) {
        if self.completion.get_mut().is_ok_and(|join| join.sealed) {
            return;
        }
        // Qt discards queued callbacks when their receiver is destroyed.
        // The last receipt still reports failure to the original issuer.
        let _ = self.thread.queue(move |mut ui| {
            ui.as_mut().page_window_command_failed(
                "Page command failed because a window closed. Try again.",
            );
        });
    }
}

#[derive(Clone)]
pub(super) struct PageAnchor {
    thread: CxxQtThread<qobject::BrowserUi>,
    scope: PageScope,
    group: String,
    page: TabId,
    token: String,
    window: super::WindowId,
}

impl PageAnchor {
    pub(super) fn prepared(
        ui: &qobject::BrowserUi,
        scope: PageScope,
        group: String,
        page: TabId,
    ) -> Option<Self> {
        Some(Self {
            thread: ui.qt_thread(),
            scope,
            group,
            page,
            token: ui.rust().window_token.to_string(),
            window: ui.rust().window?,
        })
    }

    fn current(&self, ui: &qobject::BrowserUi) -> bool {
        ui.rust().window_token.to_string() == self.token
            && ui.rust().window == Some(self.window)
            && PageWindowSnapshot::capture(ui).is_some_and(|snapshot| {
                snapshot.scope == self.scope
                    && snapshot.group == self.group
                    && snapshot.pages.iter().any(|page| page.id == self.page)
            })
    }
}

impl PageHandoff {
    pub(super) fn same_root(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub(super) fn begin_settings(&self) -> Result<PageSettingsCompletion, String> {
        self.0
            .completion
            .lock()
            .map_err(|_| "The page command completion is unavailable")?
            .begin_settings()?;
        Ok(PageSettingsCompletion {
            handoff: self.clone(),
            unresolved: true,
        })
    }

    fn resolve_settings(&self, outcome: NativeOperationOutcome) {
        let terminal = self
            .0
            .completion
            .lock()
            .ok()
            .and_then(|mut join| join.resolve_settings(outcome));
        if let Some(terminal) = terminal {
            self.clone().dispatch_terminal(terminal);
        }
    }

    fn request_terminal(self, terminal: PageTerminal) {
        let ready = self
            .0
            .completion
            .lock()
            .ok()
            .and_then(|mut join| join.seal(terminal));
        if let Some(ready) = ready {
            self.dispatch_terminal(ready);
        }
    }

    pub(super) fn authorize_regroup(&self, source: &PageWindowSnapshot, group: &str) -> bool {
        let Ok(mut anchor) = self.0.anchor.lock() else {
            return false;
        };
        if !source.pages.iter().any(|page| page.id == anchor.page) {
            return true;
        }
        if anchor.scope != source.scope || anchor.group != source.group {
            return false;
        }
        anchor.group = group.into();
        true
    }

    // Only the accepted-close broker calls this after actual release. The
    // issuing receipt stays immutable; a pre-admitted survivor carries its tail.
    pub(super) fn accept_closed_anchor(&self, closed: &[TabId], survivor: PageAnchor) -> bool {
        let Ok(mut anchor) = self.0.anchor.lock() else {
            return false;
        };
        if !closed.contains(&anchor.page) {
            return true;
        }
        if anchor.scope != survivor.scope || anchor.group != survivor.group {
            return false;
        }
        *anchor = survivor;
        true
    }

    pub(super) fn can_create_from(&self, source: &PageWindowSnapshot) -> bool {
        let Ok(anchor) = self.0.anchor.lock() else {
            return false;
        };
        anchor.scope == source.scope
            && anchor.group == source.group
            && (source.pages.iter().any(|page| page.id == anchor.page)
                || super::live_ipc_owners::page_window_owners(&anchor.scope)
                    .iter()
                    .any(|owner| {
                        owner.token == anchor.token
                            && owner.window == anchor.window
                            && owner.snapshot.group == anchor.group
                            && owner
                                .snapshot
                                .pages
                                .iter()
                                .any(|page| page.id == anchor.page)
                    }))
    }

    // Only a privately dispatched new-page effect may mint this transition.
    // It changes the live anchor while retaining the immutable original receipt.
    pub(super) fn accept_created_anchor(
        &self,
        source: &PageWindowSnapshot,
        destination: PageAnchor,
    ) -> bool {
        let Ok(mut anchor) = self.0.anchor.lock() else {
            return false;
        };
        if anchor.scope != source.scope
            || anchor.group != source.group
            || destination.scope != source.scope
            || destination.group != source.group
            || source.reserved.contains(&destination.page)
        {
            return false;
        }
        let current = source.pages.iter().any(|page| page.id == anchor.page)
            || super::live_ipc_owners::page_window_owners(&anchor.scope)
                .iter()
                .any(|owner| {
                    owner.token == anchor.token
                        && owner.window == anchor.window
                        && owner.snapshot.group == anchor.group
                        && owner
                            .snapshot
                            .pages
                            .iter()
                            .any(|page| page.id == anchor.page)
                });
        if !current {
            return false;
        }
        *anchor = destination;
        true
    }

    // Prepared-window Core commit moves this newly created identity without
    // callbacks, before source or child can publish their ownership changes.
    pub(super) fn move_created_anchor(
        &self,
        source: &qobject::BrowserUi,
        destination: PageAnchor,
    ) -> bool {
        let Ok(mut anchor) = self.0.anchor.lock() else {
            return false;
        };
        if anchor.page != destination.page
            || anchor.scope != destination.scope
            || anchor.group != destination.group
            || anchor.token != source.rust().window_token.to_string()
            || Some(anchor.window) != source.rust().window
        {
            return false;
        }
        *anchor = destination;
        true
    }

    pub(super) fn finish_close(self, outcome: ferric_browser_application::NativeOperationOutcome) {
        self.request_terminal(PageTerminal::Outcome(outcome));
    }

    pub(super) fn owner_lost(self) {
        self.request_terminal(PageTerminal::Outcome(NativeOperationOutcome::OwnerLost));
    }

    fn retire_settings_observer(self) {
        self.request_terminal(PageTerminal::SettingsObserverRetired);
    }

    pub(super) fn terminal_focus_guard(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.0.terminal_focus_started)
    }

    pub(super) fn focus_generation(&self) -> u64 {
        self.0.focus_generation.load(Ordering::Acquire)
    }

    pub(super) fn request_focus(&self) {
        if !self.0.focus_requested.swap(true, Ordering::AcqRel) {
            let generation =
                crate::page_window_focus::reserve_after(self.0.admitted_focus_generation);
            self.0.focus_generation.store(generation, Ordering::Release);
        }
    }

    // Called only around the native consumer's final callback-free Core commit.
    // The original reporter is immutable; only its live page anchor can move.
    pub(super) fn transfer_anchor(
        &self,
        source: &PageWindowSnapshot,
        token: &str,
        window: super::WindowId,
        destination: PageAnchor,
        commit: impl FnOnce() -> bool,
    ) -> bool {
        let Ok(mut anchor) = self.0.anchor.lock() else {
            return false;
        };
        let moving_anchor = anchor.page == destination.page;
        if moving_anchor
            && (anchor.scope != source.scope
                || anchor.group != source.group
                || anchor.token != token
                || anchor.window != window)
        {
            return false;
        }
        if !commit() {
            return false;
        }
        if moving_anchor {
            *anchor = destination;
        }
        true
    }

    pub(super) fn finish(self, selected: bool, succeeded: bool, target: &qobject::BrowserUi) {
        let destination = crate::page_window_focus::Destination::capture(target);
        self.request_terminal(PageTerminal::Command {
            selected,
            succeeded,
            destination,
        });
    }

    fn dispatch_terminal(self, terminal: PageTerminal) {
        let PageTerminal::Command {
            selected,
            succeeded,
            destination,
        } = terminal
        else {
            if let PageTerminal::Outcome(outcome) = terminal
                && let Some(reporter) = &self.0.reporter
            {
                let _ = reporter.finish(outcome);
            }
            return;
        };
        let root = self.0;
        let Ok(anchor) = root.anchor.lock().map(|anchor| anchor.clone()) else {
            return;
        };
        let thread = anchor.thread.clone();
        let _ = thread.queue(move |mut source_ui| {
            let source_current = anchor.current(source_ui.as_ref().get_ref());
            if !selected || !source_current {
                source_ui.as_mut().page_window_command_failed(
                    "Page command failed because the window changed. Try again.",
                );
                if let Some(reporter) = &root.reporter {
                    let _ =
                        reporter.finish(ferric_browser_application::NativeOperationOutcome::Stale);
                }
                return;
            }
            if !succeeded {
                source_ui.as_mut().page_window_command_failed(
                    "The command failed in the requested page. Try again.",
                );
                if let Some(reporter) = &root.reporter {
                    let _ =
                        reporter.finish(ferric_browser_application::NativeOperationOutcome::Failed);
                }
                return;
            }
            if !root.focus_requested.load(Ordering::Acquire) {
                if let Some(reporter) = &root.reporter {
                    let _ = reporter
                        .finish(ferric_browser_application::NativeOperationOutcome::Applied);
                }
                return;
            }
            let generation = root.focus_generation.load(Ordering::Acquire);
            if !crate::page_window_focus::current(generation) {
                if let Some(reporter) = &root.reporter {
                    let _ = reporter.finish(
                        ferric_browser_application::NativeOperationOutcome::ActivationUnknown,
                    );
                }
                return;
            }
            let Some(destination) = destination else {
                if let Some(reporter) = &root.reporter {
                    let _ =
                        reporter.finish(ferric_browser_application::NativeOperationOutcome::Stale);
                }
                return;
            };
            let target_token = destination.token();
            let Some(mut lease) = crate::page_window_focus::prepare(
                source_ui.as_ref().get_ref(),
                &destination,
                generation,
            ) else {
                source_ui
                    .as_mut()
                    .page_window_command_failed("Page focus target changed. Try again.");
                if let Some(reporter) = &root.reporter {
                    let _ =
                        reporter.finish(ferric_browser_application::NativeOperationOutcome::Stale);
                }
                return;
            };
            root.terminal_focus_started.store(true, Ordering::Release);
            lease.reporter = root.reporter.clone();
            let nonce = lease.nonce();
            source_ui.as_mut().queue_pending_engine_action();
            let action = format!(
                "window-focus\t{target_token}\t{}\t{nonce}\t{generation}",
                root.operation.as_deref().unwrap_or_default()
            );
            source_ui
                .as_mut()
                .rust_mut()
                .as_mut()
                .get_mut()
                .page_window_focus_lease = Some(lease);
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
    Recorded {
        commands: Vec<super::RepeatableCommand>,
        invocation: super::RepeatableCommand,
    },
}

impl PageContinuation {
    pub(super) fn has_commands(&self) -> bool {
        match self {
            Self::Interactive { commands, .. } => !commands.is_empty(),
            Self::Ipc { commands, .. } => !commands.is_empty(),
            Self::Recorded { commands, .. } => !commands.is_empty(),
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
    pub(super) fn matches_owner(&self, ui: &qobject::BrowserUi) -> bool {
        let rust = ui.rust();
        rust.state.as_ref().is_some_and(|state| {
            state.profile_instance_id() == Some(self.profile)
                && rust
                    .window
                    .and_then(|id| state.windows().get(&id))
                    .is_some_and(|window| {
                        window.context == self.context
                            && state
                                .profiles()
                                .get(&window.profile)
                                .is_some_and(|profile| profile.privacy == self.privacy)
                    })
        })
    }

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
    pub(super) view_identity: String,
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
    pub(super) selected: Option<TabId>,
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
                    view_identity: rust.interaction.prompt_view_identities.get(id)?.clone(),
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
            selected: window.active_tab,
        })
    }
}

#[derive(Clone)]
pub(super) struct PageWindowOwner {
    pub(super) native_marker: Option<crate::page_window_native::NativePageHost>,
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
        .filter(|owner| {
            owner.snapshot.scope == source.scope && owner.snapshot.group == source.group
        })
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
        preparation: Option<
            &super::browser_ui_configuration_preparation::ConfigurationPreparationTurn,
        >,
    ) -> Result<PageHandoff, String> {
        if let Some(turn) = preparation {
            turn.validate_handoff(self.as_ref().get_ref(), operation_id, incoming.as_ref())?;
        }
        if let Some(incoming) = incoming {
            if incoming
                .0
                .completion
                .lock()
                .map_or(true, |join| join.sealed)
            {
                return Err("The page command has already finished".into());
            }
            if let Some(turn) = preparation {
                self.as_mut()
                    .attach_configuration_handoff(&incoming, turn)?;
            }
            return Ok(incoming);
        }
        let page = self.as_ref().rust().tab.ok_or("No active page")?;
        let reporter = operation_id
            .map(|id| {
                let mut rust = self.as_mut().rust_mut();
                crate::page_window_operations::begin(rust.as_mut().get_mut(), id)
            })
            .transpose()?;
        let operation = reporter.as_ref().map(|reporter| reporter.id().to_owned());
        let handoff = PageHandoff(Arc::new(PageHandoffState {
            thread: self.as_ref().get_ref().qt_thread(),
            anchor: Mutex::new(PageAnchor {
                thread: self.as_ref().get_ref().qt_thread(),
                scope: source.scope.clone(),
                group: source.group.clone(),
                page,
                token: self.as_ref().rust().window_token.to_string(),
                window: self.as_ref().rust().window.ok_or("No issuing window")?,
            }),
            operation,
            reporter,
            completion: Mutex::new(PageCompletionJoin::default()),
            focus_requested: AtomicBool::new(false),
            terminal_focus_started: Arc::new(AtomicBool::new(false)),
            admitted_focus_generation: crate::page_window_focus::generation(),
            focus_generation: AtomicU64::new(0),
        }));
        if let Some(turn) = preparation {
            self.as_mut().attach_configuration_handoff(&handoff, turn)?;
        }
        Ok(handoff)
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
        preparation: Option<
            &super::browser_ui_configuration_preparation::ConfigurationPreparationTurn,
        >,
    ) -> Result<super::Value, super::PublicError> {
        self.execute_page_window_ipc_selection_with_handoff(
            command,
            route,
            commands,
            operation_id,
            &mut None,
            preparation,
        )
    }

    fn execute_page_window_ipc_selection_with_handoff(
        mut self: Pin<&mut Self>,
        command: &ParsedCommand,
        route: &super::IpcRoute,
        commands: Vec<(ParsedCommand, super::IpcRoute)>,
        operation_id: Option<&str>,
        handoff: &mut Option<PageHandoff>,
        preparation: Option<
            &super::browser_ui_configuration_preparation::ConfigurationPreparationTurn,
        >,
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
            preparation,
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
        preparation: Option<
            &super::browser_ui_configuration_preparation::ConfigurationPreparationTurn,
        >,
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
        let handoff =
            self.as_mut()
                .prepare_page_handoff(handoff, &source, operation_id, preparation)?;
        handoff.request_focus();
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
                            if !same_owner
                                && crate::page_window_undo::Scope::capture(ui.as_ref().rust())
                                    .is_none()
                            {
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
                            if !same_owner
                                && crate::page_window_undo::Scope::capture(ui.as_ref().rust())
                                    .is_none()
                            {
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
                        PageContinuation::Recorded {
                            commands,
                            invocation,
                        } => {
                            if !same_owner
                                && crate::page_window_undo::Scope::capture(ui.as_ref().rust())
                                    .is_none()
                            {
                                ui.as_mut().note_repeatable_invocation(
                                    &repeat_command,
                                    invocation.route.as_ref(),
                                    invocation.count,
                                );
                            }
                            selected = commands.is_empty()
                                || continuation_current(
                                    ui.as_ref().get_ref(),
                                    &scope,
                                    &target.owner.snapshot.group,
                                    target_document,
                                );
                            succeeded = selected
                                && ui
                                    .as_mut()
                                    .execute_page_recorded_commands(commands, None, &mut handoff)
                                    .is_ok();
                        }
                    }
                    ui.as_mut().runtime_work_available();
                }
                // A later selection now owns this same receipt. Only its
                // terminal owner can focus or finish the original operation.
                let Some(handoff) = handoff else {
                    return;
                };
                handoff.finish(selected, succeeded, ui.as_ref().get_ref());
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

    pub(super) fn execute_routed_ipc_chain_with_handoff(
        self: Pin<&mut Self>,
        commands: Vec<(ParsedCommand, super::IpcRoute)>,
        operation_id: Option<&str>,
        handoff: &mut Option<PageHandoff>,
    ) -> Result<super::Value, super::PublicError> {
        self.execute_routed_ipc_chain_in_turn(commands, operation_id, handoff, None)
    }

    pub(super) fn execute_routed_ipc_chain_in_turn(
        self: Pin<&mut Self>,
        commands: Vec<(ParsedCommand, super::IpcRoute)>,
        operation_id: Option<&str>,
        handoff: &mut Option<PageHandoff>,
        preparation: Option<
            &super::browser_ui_configuration_preparation::ConfigurationPreparationTurn,
        >,
    ) -> Result<super::Value, super::PublicError> {
        let read = preparation
            .filter(|turn| turn.is_current(self.as_ref().get_ref()))
            .and_then(|_| {
                super::browser_ui_configuration_read::ConfigurationReadTurn::capture(
                    self.as_ref().get_ref(),
                )
            });
        self.execute_routed_ipc_chain_with_read(
            commands,
            operation_id,
            handoff,
            preparation,
            read.as_ref(),
        )
    }

    pub(super) fn execute_routed_ipc_chain_with_read(
        mut self: Pin<&mut Self>,
        commands: Vec<(ParsedCommand, super::IpcRoute)>,
        operation_id: Option<&str>,
        handoff: &mut Option<PageHandoff>,
        preparation: Option<
            &super::browser_ui_configuration_preparation::ConfigurationPreparationTurn,
        >,
        read: Option<&super::browser_ui_configuration_read::ConfigurationReadTurn>,
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
        if self.as_ref().in_page_window_mode()
            && expanded
                .iter()
                .any(|(command, _)| is_command_workflow(command))
        {
            return self.execute_page_workflow_with_read(
                expanded
                    .into_iter()
                    .map(|(command, route)| crate::RepeatableCommand {
                        command,
                        count: 1,
                        route: Some(route),
                    })
                    .collect(),
                operation_id,
                handoff,
                preparation,
                read,
            );
        }
        if !self.as_ref().in_page_window_mode()
            && super::browser_ui_page_workflows::needs_read_expansion(
                expanded.iter().map(|(command, _)| command),
            )
            && let Some(commands) = super::browser_ui_page_workflows::expanded_read_workflow(
                self.as_ref().rust(),
                expanded
                    .iter()
                    .cloned()
                    .map(|(command, route)| crate::RepeatableCommand {
                        command,
                        count: 1,
                        route: Some(route),
                    })
                    .collect(),
            )
        {
            return self.execute_page_recorded_commands_with_read(
                commands,
                operation_id,
                handoff,
                preparation,
                read,
            );
        }
        let mut operations = Vec::new();
        for (index, (command, route)) in expanded.iter().enumerate() {
            if index > 0 {
                self.as_mut().queue_pending_engine_action();
            }
            if index > 0
                && preparation.is_none()
                && let Some(read) = read
            {
                read.validate_continuation(self.as_ref().get_ref())?;
            }
            if operation_id.is_some_and(|id| {
                self.as_ref()
                    .rust()
                    .operation(id)
                    .is_none_or(|record| record.state.is_terminal())
            }) {
                return Err(crate::PublicError::new(
                    ferric_browser_ipc::ErrorCode::Cancelled,
                    "The command was cancelled.",
                    "command continuation operation is no longer live",
                ));
            }

            if command.name == "get" || super::browser_ui_site_policy::uses_read_turn(command) {
                if preparation.is_some_and(|turn| !turn.is_current(self.as_ref().get_ref())) {
                    return Err("The command owner changed during settings preparation".into());
                }
                if operation_id.is_some_and(|id| {
                    self.as_ref()
                        .rust()
                        .operation(id)
                        .is_none_or(|record| record.state.is_terminal())
                }) {
                    return Err(crate::PublicError::new(
                        ferric_browser_ipc::ErrorCode::Cancelled,
                        "The command was cancelled.",
                        "read operation is no longer live",
                    ));
                }

                let rebound = preparation
                    .filter(|turn| turn.is_current(self.as_ref().get_ref()))
                    .and_then(|_| {
                        super::browser_ui_configuration_read::ConfigurationReadTurn::capture(
                            self.as_ref().get_ref(),
                        )
                    });
                let read = rebound.as_ref().or(read);
                self.as_ref().validate_configuration_get_route(route)?;
                let Some(read) = read else {
                    return self.queue_configuration_read(
                        command.clone(),
                        PageContinuation::Ipc {
                            commands: expanded[index + 1..].to_vec(),
                            route: route.clone(),
                        },
                        operation_id,
                        handoff,
                    );
                };
                result = self
                    .as_mut()
                    .execute_configuration_read_command(command, read)?;
                continue;
            }
            if preparation.is_some_and(|turn| !turn.is_current(self.as_ref().get_ref())) {
                return Err("The command owner changed during settings preparation".into());
            }
            if preparation.is_none()
                && !(super::browser_ui_site_policy::is_draft_management(command) && read.is_some())
                && super::browser_ui_configuration_preparation::needs_preparation(
                    command,
                    &self.as_ref().rust().registry,
                )
            {
                self.as_ref().validate_ipc_route_public(route)?;
                return self.queue_configuration_preparation(
                    super::browser_ui_configuration_preparation::DeferredConfigurationAction::Commands(PageContinuation::Ipc {
                        commands: expanded[index..].to_vec(), route: route.clone(),
                    }), operation_id, handoff,
                ).map_err(Into::into);
            }
            if self.as_ref().in_page_window_mode()
                && super::browser_ui_page_creation::handles(command, Some(route))
            {
                return self
                    .execute_page_window_ipc_creation(
                        command,
                        route,
                        expanded[index + 1..].to_vec(),
                        operation_id,
                        handoff,
                        preparation,
                    )
                    .map_err(|error| error.after_accepted_commands(index));
            }
            if self.as_ref().in_page_window_mode() && command.name == "tab-close" {
                return self
                    .execute_page_window_ipc_close(
                        command,
                        route,
                        expanded[index + 1..].to_vec(),
                        operation_id,
                        handoff,
                        preparation,
                    )
                    .map_err(|error| error.after_accepted_commands(index));
            }
            if self.as_ref().in_page_window_mode()
                && super::browser_ui_page_regroup::handles(command)
            {
                return self
                    .execute_page_window_ipc_regroup(
                        command,
                        route,
                        expanded[index + 1..].to_vec(),
                        operation_id,
                        handoff,
                        preparation,
                    )
                    .map_err(|error| error.after_accepted_commands(index));
            }
            if self.as_ref().in_page_window_mode()
                && super::browser_ui_page_mutations::handles(command)
            {
                return self
                    .execute_page_window_ipc_mutation(
                        command,
                        route,
                        expanded[index + 1..].to_vec(),
                        operation_id,
                        handoff,
                        preparation,
                    )
                    .map_err(|error| error.after_accepted_commands(index));
            }
            if self.as_ref().in_page_window_mode() && handles(command) {
                return self
                    .execute_page_window_ipc_selection_with_handoff(
                        command,
                        route,
                        expanded[index + 1..].to_vec(),
                        operation_id,
                        handoff,
                        preparation,
                    )
                    .map_err(|error| error.after_accepted_commands(index));
            }
            result = self
                .as_mut()
                .execute_ipc_command_with_read(command.clone(), route, None, preparation, read)
                .map_err(|error| error.after_accepted_commands(index))?;
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
    fn terminal_outcome(terminal: Option<PageTerminal>) -> Option<NativeOperationOutcome> {
        terminal.map(|terminal| match terminal {
            PageTerminal::Outcome(outcome) => outcome,
            PageTerminal::Command { .. } => panic!("unexpected focus completion"),
            PageTerminal::SettingsObserverRetired => panic!("unresolved settings retirement"),
        })
    }

    #[test]
    fn retired_settings_wait_for_the_actual_receipt_in_either_order() {
        for outcome in [
            NativeOperationOutcome::SettingsSavedReloadRequired {
                durability_uncertain: false,
            },
            NativeOperationOutcome::SettingsSavedReloadRequired {
                durability_uncertain: true,
            },
            NativeOperationOutcome::Failed,
            NativeOperationOutcome::OwnerLost,
        ] {
            for receipt_first in [false, true] {
                let mut join = PageCompletionJoin::default();
                join.begin_settings().unwrap();
                let result = if receipt_first {
                    assert!(join.resolve_settings(outcome).is_none());
                    join.seal(PageTerminal::SettingsObserverRetired)
                } else {
                    assert!(join.seal(PageTerminal::SettingsObserverRetired).is_none());
                    assert!(join.begin_settings().is_err());
                    join.resolve_settings(outcome)
                };
                assert_eq!(terminal_outcome(result), Some(outcome));
                assert!(join.seal(PageTerminal::SettingsObserverRetired).is_none());
            }
        }
    }

    #[test]
    fn settings_join_waits_for_all_receipts_and_seals_new_work() {
        let mut join = PageCompletionJoin::default();
        join.begin_settings().unwrap();
        join.begin_settings().unwrap();
        assert!(
            join.seal(PageTerminal::Outcome(NativeOperationOutcome::Applied))
                .is_none()
        );
        assert!(join.begin_settings().is_err());
        assert!(
            join.resolve_settings(NativeOperationOutcome::Applied)
                .is_none()
        );
        assert_eq!(
            terminal_outcome(join.resolve_settings(NativeOperationOutcome::Applied)),
            Some(NativeOperationOutcome::Applied)
        );
        assert!(
            join.seal(PageTerminal::Outcome(NativeOperationOutcome::Failed))
                .is_none()
        );
    }

    #[test]
    fn settings_join_remembers_receipt_failure_before_tail_finishes() {
        let mut join = PageCompletionJoin::default();
        join.begin_settings().unwrap();
        assert!(
            join.resolve_settings(NativeOperationOutcome::Failed)
                .is_none()
        );
        assert_eq!(
            terminal_outcome(join.seal(PageTerminal::Command {
                selected: true,
                succeeded: true,
                destination: None,
            })),
            Some(NativeOperationOutcome::Failed)
        );
    }

    #[test]
    fn settings_join_keeps_committed_save_when_tail_or_observer_fails() {
        for tail in [
            NativeOperationOutcome::Failed,
            NativeOperationOutcome::OwnerLost,
        ] {
            for saved_first in [false, true] {
                let mut join = PageCompletionJoin::default();
                join.begin_settings().unwrap();
                join.begin_settings().unwrap();
                let saved = NativeOperationOutcome::SettingsSavedReloadRequired {
                    durability_uncertain: true,
                };
                let (first, last) = if saved_first {
                    (saved, NativeOperationOutcome::OwnerLost)
                } else {
                    (NativeOperationOutcome::OwnerLost, saved)
                };
                assert!(join.resolve_settings(first).is_none());
                assert!(join.seal(PageTerminal::Outcome(tail)).is_none());
                assert_eq!(terminal_outcome(join.resolve_settings(last)), Some(saved));
            }
        }
    }

    #[test]
    fn settings_join_preserves_partial_close_receipt_and_durability_warning() {
        let partial = NativeOperationOutcome::ClosedPartially {
            closed: 1,
            requested: 2,
        };
        let mut join = PageCompletionJoin::default();
        join.begin_settings().unwrap();
        assert!(join.seal(PageTerminal::Outcome(partial)).is_none());
        assert_eq!(
            terminal_outcome(join.resolve_settings(NativeOperationOutcome::Failed)),
            Some(partial)
        );

        let mut join = PageCompletionJoin::default();
        join.begin_settings().unwrap();
        join.begin_settings().unwrap();
        join.resolve_settings(NativeOperationOutcome::SettingsSavedReloadRequired {
            durability_uncertain: false,
        });
        join.resolve_settings(NativeOperationOutcome::SettingsSavedReloadRequired {
            durability_uncertain: true,
        });
        assert_eq!(
            terminal_outcome(join.seal(PageTerminal::Outcome(NativeOperationOutcome::Applied))),
            Some(NativeOperationOutcome::SettingsSavedReloadRequired {
                durability_uncertain: true
            })
        );
    }

    #[test]
    fn settings_join_bounds_pending_receipts_without_wraparound() {
        let mut join = PageCompletionJoin {
            pending: u16::MAX,
            ..Default::default()
        };
        assert!(join.begin_settings().is_err());
        assert_eq!(join.pending, u16::MAX);
        assert!(!join.sealed);
    }

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
