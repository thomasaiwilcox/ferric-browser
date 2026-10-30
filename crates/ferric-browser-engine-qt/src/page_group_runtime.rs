//! One bounded compositor worker over actual, ready native/Core page owners.
use crate::hyprland::{GroupOutcome, HyprlandAdapter, QueuedSend};
use crate::page_window_native::{NativeWindowIdentity, NativeWindowPresentation};
use crate::page_window_undo::Scope;
use crate::{CxxQtType, Pin, TabId, qobject};
use cxx_qt::{CxxQtThread, Threading};
use ferric_browser_config::{HyprlandConfig, PageGroupingPolicy, TriState};
use ferric_browser_runtime::worker::{Poll, SingleFlightWorker};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

#[derive(Clone)]
struct Owner {
    scope: Scope,
    page: TabId,
    native: NativeWindowIdentity,
    config: HyprlandConfig,
    thread: CxxQtThread<qobject::BrowserUi>,
    seen_active: bool,
    ready: bool,
}

fn owners() -> &'static Mutex<BTreeMap<String, Owner>> {
    static OWNERS: OnceLock<Mutex<BTreeMap<String, Owner>>> = OnceLock::new();
    OWNERS.get_or_init(Default::default)
}

#[derive(Clone)]
struct Admission {
    scope: Scope,
    members: Vec<(String, TabId, NativeWindowIdentity, TriState)>,
    token: String,
    thread: CxxQtThread<qobject::BrowserUi>,
    config: HyprlandConfig,
}

fn admit(token: &str) -> Option<Admission> {
    let owners = owners().lock().ok()?;
    let source = owners.get(token)?;
    if owners
        .values()
        .any(|owner| owner.scope == source.scope && !owner.ready)
    {
        return None;
    }
    let members = owners
        .iter()
        .filter(|(_, owner)| owner.scope == source.scope)
        .map(|(token, owner)| {
            (
                token.clone(),
                owner.page,
                owner.native.clone(),
                owner.config.enabled.clone(),
            )
        })
        .collect::<Vec<_>>();
    (members.len() <= 64).then(|| Admission {
        scope: source.scope.clone(),
        token: token.into(),
        thread: source.thread.clone(),
        members,
        config: source.config.clone(),
    })
}

fn current_matches(expected: &Admission) -> bool {
    admit(&expected.token).is_some_and(|current| {
        current.scope == expected.scope
            && current.members == expected.members
            && current.config.enabled == expected.config.enabled
    })
}

fn run(admission: Admission) -> Result<GroupOutcome, String> {
    if admission.members.len() < 2 {
        return Ok(GroupOutcome::Deferred);
    }
    let adapter = HyprlandAdapter::from_config(&admission.config);
    let identities = admission
        .members
        .iter()
        .map(|(_, _, native, _)| native.clone())
        .collect::<Vec<_>>();
    let bindings = adapter
        .resolve_native_windows(&identities)
        .map_err(|error| error.to_string())?;
    let mapped = bindings.into_iter().flatten().collect::<Vec<_>>();
    if mapped.len() < 2 {
        return Ok(GroupOutcome::Deferred);
    }
    let write = adapter
        .prepare_native_group(&mapped)
        .map_err(|error| error.to_string())?;
    let deadline = write.deadline();
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let thread = admission.thread.clone();
    let queued = std::sync::Arc::new(QueuedSend::default());
    let gui_queued = queued.clone();
    // This GUI callback is the authorization/commit point. Native reads and
    // publication happen first; after the final scalar comparison, one
    // nonblocking send runs without any Qt callback or event processing.
    if thread
        .queue(move |ui| {
            publish(ui.as_ref().get_ref());
            let binding = ui.as_ref();
            let rust = binding.rust();
            let expected_page = admission
                .members
                .iter()
                .find(|(token, _, _, _)| token == &admission.token)
                .map(|(_, page, _, _)| *page);
            let source_current = rust.window_token.to_string() == admission.token
                && Scope::capture(rust).as_ref() == Some(&admission.scope)
                && expected_page.is_some_and(|page| {
                    rust.tab_ids.as_slice() == [page]
                        && rust
                            .state
                            .as_ref()
                            .and_then(|state| state.windows().get(&rust.window?))
                            .is_some_and(|window| window.tabs.as_slice() == [page])
                });
            let result = if source_current && current_matches(&admission) && gui_queued.try_commit()
            {
                write.dispatch()
            } else {
                Err(GroupOutcome::Deferred)
            };
            if matches!(result, Err(GroupOutcome::Deferred)) {
                gui_queued.not_sent();
            }
            let _ = sender.try_send(result);
        })
        .is_err()
    {
        return Ok(GroupOutcome::Deferred);
    }
    match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(Ok(reply)) => Ok(reply.finish()),
        Ok(Err(outcome)) => Ok(outcome),
        // Cancel a callback that never crossed its GUI commit point. Once it
        // started committing, a missing handoff cannot prove zero bytes.
        Err(_) if queued.cancel_or_prove_not_sent() => Ok(GroupOutcome::Deferred),
        Err(_) => Ok(GroupOutcome::Uncertain),
    }
}

struct Attempt {
    scope: Scope,
    next: Instant,
    outcome: Result<GroupOutcome, String>,
}

struct Coordinator {
    worker: SingleFlightWorker<Admission, Result<GroupOutcome, String>>,
    pending: Option<Admission>,
    inhibited: bool,
    attempts: Vec<Attempt>,
}

impl Coordinator {
    fn spawn() -> Result<Self, String> {
        Ok(Self {
            worker: SingleFlightWorker::spawn("ferric-native-page-groups", run)
                .map_err(|e| e.to_string())?,
            pending: None,
            inhibited: false,
            attempts: Vec::new(),
        })
    }

    fn drain(&mut self) {
        let completed = match self.worker.poll() {
            Poll::Ready(result) => Some(result),
            Poll::Stopped => Some(Ok(GroupOutcome::Uncertain)),
            Poll::Pending => None,
        };
        if let Some(outcome) = completed
            && let Some(pending) = self.pending.take()
        {
            let uncertain = outcome == Ok(GroupOutcome::Uncertain);
            if uncertain {
                self.inhibited = true;
            }
            let outcome = if !uncertain && !current_matches(&pending) {
                Ok(GroupOutcome::Deferred)
            } else {
                outcome
            };
            let scope = pending.scope;
            let delay = if matches!(outcome, Err(_) | Ok(GroupOutcome::Rejected)) {
                Duration::from_secs(30)
            } else {
                Duration::from_secs(1)
            };
            if let Some(attempt) = self.attempts.iter_mut().find(|entry| entry.scope == scope) {
                attempt.next = Instant::now() + delay;
                attempt.outcome = outcome;
            }
        }
        // A later opt-in begins with fresh admission after every owner has
        // retired permission and the old worker result has been drained.
        if self.pending.is_none() && owners().lock().is_ok_and(|owners| owners.is_empty()) {
            self.inhibited = false;
        }
    }

    fn tick(&mut self, token: &str) -> Option<Result<GroupOutcome, String>> {
        self.drain();
        let live = owners()
            .lock()
            .ok()?
            .values()
            .map(|owner| owner.scope.clone())
            .collect::<Vec<_>>();
        self.attempts
            .retain(|attempt| live.contains(&attempt.scope));
        let admission = admit(token)?;
        if self.inhibited {
            return Some(Ok(GroupOutcome::Uncertain));
        }
        let existing = self
            .attempts
            .iter()
            .find(|attempt| attempt.scope == admission.scope);
        let result = existing.map(|attempt| attempt.outcome.clone());
        let due = existing.is_none_or(|attempt| {
            attempt.outcome != Ok(GroupOutcome::Uncertain) && Instant::now() >= attempt.next
        });
        if due && self.pending.is_none() && (existing.is_some() || self.attempts.len() < 128) {
            let scope = admission.scope.clone();
            let pending = admission.clone();
            if self.worker.submit(admission).is_ok() {
                if !self.attempts.iter().any(|attempt| attempt.scope == scope) {
                    self.attempts.push(Attempt {
                        scope: scope.clone(),
                        next: Instant::now(),
                        outcome: Ok(GroupOutcome::Deferred),
                    });
                }
                self.pending = Some(pending);
            }
        }
        result
    }
}

fn coordinator() -> &'static Mutex<Option<Coordinator>> {
    static COORDINATOR: OnceLock<Mutex<Option<Coordinator>>> = OnceLock::new();
    COORDINATOR.get_or_init(Default::default)
}

pub(crate) fn is_pending() -> bool {
    coordinator()
        .lock()
        .is_ok_and(|c| c.as_ref().is_some_and(|c| c.pending.is_some()))
}

pub(crate) fn is_enabled() -> bool {
    owners().lock().is_ok_and(|owners| !owners.is_empty())
}

pub(crate) fn release(token: &str) {
    let live = {
        let Ok(mut owners) = owners().lock() else {
            return;
        };
        if owners.remove(token).is_none() {
            return;
        }
        owners
            .values()
            .map(|owner| owner.scope.clone())
            .collect::<Vec<_>>()
    };
    if let Ok(mut coordinator) = coordinator().lock()
        && let Some(coordinator) = coordinator.as_mut()
    {
        coordinator
            .attempts
            .retain(|attempt| live.contains(&attempt.scope));
        if live.is_empty() && coordinator.pending.is_none() {
            coordinator.inhibited = false;
        }
    }
}

/// Called on the GUI thread after native/Core ownership is published. No I/O.
pub(crate) fn publish(ui: &qobject::BrowserUi) {
    let token = ui.rust().window_token.to_string();
    let captured = (|| {
        ui.rust().ipc_owner.as_ref()?;
        let config = ui
            .rust()
            .config
            .get("hyprland")
            .cloned()
            .and_then(|value| serde_json::from_value::<HyprlandConfig>(value).ok())?;
        if config.enabled == TriState::Off || config.page_grouping != PageGroupingPolicy::Auto {
            return None;
        }
        let presentation = NativeWindowPresentation::capture(ui)?;
        let native = presentation.identity;
        let snapshot = crate::browser_ui_page_windows::PageWindowSnapshot::capture(ui)?;
        let rust = ui.rust();
        let [page] = snapshot.pages.as_slice() else {
            return None;
        };
        if rust.tab_ids.as_slice() != [page.id]
            || rust
                .state
                .as_ref()?
                .windows()
                .get(&rust.window?)?
                .tabs
                .as_slice()
                != [page.id]
        {
            return None;
        }
        let seen_active = presentation.active
            || owners().lock().ok()?.get(&token).is_some_and(|previous| {
                previous.page == page.id && previous.native == native && previous.seen_active
            });
        Some(Owner {
            scope: Scope::capture(rust)?,
            page: page.id,
            native,
            config,
            thread: ui.qt_thread(),
            seen_active,
            ready: !presentation.foreground || seen_active,
        })
    })();
    let Some(owner) = captured else {
        release(&token);
        return;
    };
    if let Ok(mut owners) = owners().lock() {
        if owners.len() < 128 || owners.contains_key(&token) {
            owners.insert(token, owner);
        }
    }
}

pub(crate) fn poll(mut ui: Pin<&mut qobject::BrowserUi>) {
    publish(ui.as_ref().get_ref());
    let token = ui.as_ref().rust().window_token.to_string();
    let eligible = owners()
        .lock()
        .is_ok_and(|owners| owners.contains_key(&token));
    let result = {
        let Ok(mut coordinator) = coordinator().lock() else {
            return;
        };
        if coordinator.is_none() && eligible {
            *coordinator = Coordinator::spawn().ok();
        }
        if let Some(coordinator) = coordinator.as_mut() {
            coordinator.drain();
        }
        if eligible {
            coordinator
                .as_mut()
                .and_then(|coordinator| coordinator.tick(&token))
        } else {
            None
        }
    };
    if !eligible {
        let requested = ui
            .as_ref()
            .rust()
            .config
            .get("hyprland")
            .cloned()
            .and_then(|value| serde_json::from_value::<HyprlandConfig>(value).ok())
            .is_some_and(|config| {
                config.enabled != TriState::Off && config.page_grouping == PageGroupingPolicy::Auto
            });
        if requested {
            ui.as_mut()
                .set_feature_page_grouping_status(crate::QString::from("unavailable"));
            ui.as_mut()
                .set_feature_page_grouping_reason(crate::QString::from(
                    "this window has no eligible native page owner",
                ));
        }
        return;
    }
    let (status, reason) = match result {
        Some(Ok(GroupOutcome::Changed | GroupOutcome::Unchanged)) => (
            "available",
            "native identities and group membership verified",
        ),
        Some(Ok(GroupOutcome::Uncertain)) => (
            "unavailable",
            "compositor outcome is uncertain; further group writes are inhibited",
        ),
        Some(Ok(GroupOutcome::Rejected)) => (
            "unavailable",
            "existing native group ownership or policy could not be verified",
        ),
        Some(Err(_)) => (
            "unavailable",
            "the qualified compositor interface is unavailable",
        ),
        Some(Ok(GroupOutcome::Deferred)) | None => (
            "waiting",
            "waiting for mapped pages and a safe active window",
        ),
    };
    ui.as_mut()
        .set_feature_page_grouping_status(crate::QString::from(status));
    ui.set_feature_page_grouping_reason(crate::QString::from(reason));
}
