//! One bounded compositor worker over actual, ready native/Core page owners.
use crate::hyprland::{
    GroupOutcome, GroupReply, HyprlandAdapter, ManagedCapability, ManagedRequest, ManagedRole,
    QueuedSend,
};
use crate::page_window_native::{NativeWindowIdentity, NativeWindowPresentation};
use crate::page_window_undo::Scope;
use crate::{CxxQtType, Pin, TabId, Uuid, qobject};
use cxx_qt::{CxxQtThread, Threading};
use ferric_browser_config::{HyprlandConfig, PageGroupingPolicy, TriState};
use ferric_browser_runtime::worker::{Poll, SingleFlightWorker};
use std::{
    collections::BTreeMap,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
struct Owner {
    scope: Scope,
    scope_key: Uuid,
    page: TabId,
    pinned: bool,
    native: NativeWindowIdentity,
    config: HyprlandConfig,
    thread: CxxQtThread<qobject::BrowserUi>,
    seen_active: bool,
    ready: bool,
}

static RETAIN_ROLES: AtomicBool = AtomicBool::new(false);

fn automatic(config: &HyprlandConfig) -> bool {
    config.enabled != TriState::Off && config.page_grouping == PageGroupingPolicy::Auto
}

fn process_key() -> Uuid {
    static KEY: OnceLock<Uuid> = OnceLock::new();
    *KEY.get_or_init(Uuid::new_v4)
}

fn owners() -> &'static Mutex<BTreeMap<String, Owner>> {
    static OWNERS: OnceLock<Mutex<BTreeMap<String, Owner>>> = OnceLock::new();
    OWNERS.get_or_init(Default::default)
}

#[derive(Clone, Eq, PartialEq)]
struct Member {
    token: String,
    page: TabId,
    pinned: bool,
    order: usize,
    native: NativeWindowIdentity,
    scope: Scope,
    scope_key: Uuid,
    enabled: TriState,
    auto: bool,
    ready: bool,
}

#[derive(Clone)]
struct Admission {
    scope: Scope,
    scope_key: Uuid,
    members: Vec<Member>,
    token: String,
    thread: CxxQtThread<qobject::BrowserUi>,
    config: HyprlandConfig,
}

#[derive(Clone)]
struct Job {
    admission: Admission,
    managed: ManagedRequest,
}

fn admit(token: &str) -> Option<Admission> {
    let owners = owners().lock().ok()?;
    let source = owners.get(token)?;
    if !source.ready || owners.len() > 64 {
        return None;
    }
    let members = owners
        .iter()
        .map(|(token, owner)| {
            Some(Member {
                token: token.clone(),
                page: owner.page,
                pinned: owner.pinned,
                order: crate::live_ipc_owners::page_window_rank(owner.page)?,
                native: owner.native.clone(),
                scope: owner.scope.clone(),
                scope_key: owner.scope_key,
                enabled: owner.config.enabled.clone(),
                auto: automatic(&owner.config),
                ready: owner.ready,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Admission {
        scope: source.scope.clone(),
        scope_key: source.scope_key,
        token: token.into(),
        thread: source.thread.clone(),
        members,
        config: source.config.clone(),
    })
}

fn current_matches(expected: &Admission) -> bool {
    admit(&expected.token).is_some_and(|current| {
        current.scope == expected.scope
            && current.scope_key == expected.scope_key
            && current.members == expected.members
            && current.config == expected.config
    })
}

fn run(job: Job) -> Result<GroupReply, String> {
    let Job {
        admission,
        mut managed,
    } = job;
    if admission.members.len() < 2 && managed.capabilities.is_empty() {
        return Ok(GroupOutcome::Deferred.into());
    }
    // Auto-off retains only the exact retirement authority from a previously
    // created group. Each role's independent Auto flag still fences all adds.
    let mut config = admission.config.clone();
    if !managed.capabilities.is_empty() {
        config.enabled = TriState::On;
    }
    let adapter = HyprlandAdapter::from_config(&config);
    let identities = admission
        .members
        .iter()
        .map(|member| member.native.clone())
        .collect::<Vec<_>>();
    let bindings = adapter
        .resolve_native_windows(&identities)
        .map_err(|error| error.to_string())?;
    let mut ordered = admission.members.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|member| (!member.pinned, member.order, &member.token));
    let roles = bindings
        .into_iter()
        .zip(&admission.members)
        .filter_map(|(binding, member)| {
            binding.map(|binding| ManagedRole {
                binding,
                scope: member.scope_key,
                auto: member.auto,
                ready: member.ready,
                order: ordered
                    .iter()
                    .position(|candidate| candidate.token == member.token)
                    .expect("member belongs to admission")
                    + 1,
            })
        })
        .collect::<Vec<_>>();
    let Some(first) = roles.first() else {
        return Ok(GroupOutcome::Deferred.into());
    };
    managed.restrict_to_instance(&first.binding.instance);
    let write = adapter
        .prepare_native_group(&roles, managed)
        .map_err(|error| error.to_string())?;
    let deadline = write.deadline();
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let thread = admission.thread.clone();
    let queued = std::sync::Arc::new(QueuedSend::default());
    let gui_queued = queued.clone();
    // All roles (including removal-only, independent and private owners) are
    // compared immediately before one nonblocking send. No callbacks follow
    // the final scalar comparison; native identity repeats inside the Lua op.
    if thread
        .queue(move |ui| {
            publish(ui.as_ref().get_ref());
            let binding = ui.as_ref();
            let rust = binding.rust();
            let expected_page = admission
                .members
                .iter()
                .find(|member| member.token == admission.token)
                .map(|member| member.page);
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
        return Ok(GroupOutcome::Deferred.into());
    }
    match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(Ok(reply)) => Ok(reply.finish()),
        Ok(Err(outcome)) => Ok(outcome.into()),
        Err(_) if queued.cancel_or_prove_not_sent() => Ok(GroupOutcome::Deferred.into()),
        Err(_) => Ok(GroupOutcome::Uncertain.into()),
    }
}

struct Lease {
    capability: ManagedCapability,
    witnesses: Vec<NativeWindowIdentity>,
}

struct Attempt {
    scope: Scope,
    next: Instant,
    outcome: Result<GroupOutcome, String>,
}

struct Coordinator {
    worker: SingleFlightWorker<Job, Result<GroupReply, String>>,
    pending: Option<Job>,
    epoch: Uuid,
    leases: Vec<Lease>,
    inhibited: bool,
    attempts: Vec<Attempt>,
}

impl Coordinator {
    fn spawn() -> Result<Self, String> {
        Ok(Self {
            worker: SingleFlightWorker::spawn("ferric-native-page-groups", run)
                .map_err(|e| e.to_string())?,
            pending: None,
            epoch: Uuid::new_v4(),
            leases: Vec::new(),
            inhibited: false,
            attempts: Vec::new(),
        })
    }

    fn drain(&mut self) {
        let completed = match self.worker.poll() {
            Poll::Ready(result) => Some(result),
            Poll::Stopped => Some(Ok(GroupOutcome::Uncertain.into())),
            Poll::Pending => None,
        };
        if let Some(result) = completed
            && let Some(pending) = self.pending.take()
        {
            let mut outcome = result.map(|reply| {
                // A definite success retains retirement-only authority even
                // when Core scope changed while the reply was in flight. Its
                // birth scope is immutable; it never becomes Available stale.
                if let Some(receipt) = reply.receipt {
                    self.leases
                        .retain(|lease| lease.capability.key != receipt.capability.key);
                    if !receipt.members.is_empty() {
                        self.leases.push(Lease {
                            capability: receipt.capability,
                            witnesses: receipt.members,
                        });
                    }
                }
                reply.outcome
            });
            let uncertain = outcome == Ok(GroupOutcome::Uncertain);
            if uncertain {
                self.inhibited = true;
            }
            if !uncertain && !current_matches(&pending.admission) {
                outcome = Ok(GroupOutcome::Deferred);
            }
            let delay = if matches!(outcome, Err(_) | Ok(GroupOutcome::Rejected)) {
                Duration::from_secs(30)
            } else {
                Duration::from_secs(1)
            };
            if let Some(attempt) = self
                .attempts
                .iter_mut()
                .find(|entry| entry.scope == pending.admission.scope)
            {
                attempt.next = Instant::now() + delay;
                attempt.outcome = outcome;
            }
        }
        if let Ok(mut owners) = owners().lock() {
            self.leases.retain(|lease| {
                owners
                    .values()
                    .any(|owner| lease.witnesses.contains(&owner.native))
            });
            RETAIN_ROLES.store(
                self.pending.is_some() || !self.leases.is_empty(),
                Ordering::Release,
            );
            if self.pending.is_none()
                && self.leases.is_empty()
                && !owners.values().any(|owner| automatic(&owner.config))
            {
                owners.clear();
                self.inhibited = false;
                self.epoch = Uuid::new_v4();
            }
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
        if !automatic(&admission.config) && self.leases.is_empty() {
            return None;
        }
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
            let job = Job {
                managed: ManagedRequest::new(
                    process_key(),
                    self.epoch,
                    admission.scope_key,
                    self.leases
                        .iter()
                        .map(|lease| lease.capability.clone())
                        .collect(),
                ),
                admission,
            };
            let pending = job.clone();
            if self.worker.submit(job).is_ok() {
                if !self.attempts.iter().any(|attempt| attempt.scope == scope) {
                    self.attempts.push(Attempt {
                        scope,
                        next: Instant::now(),
                        outcome: Ok(GroupOutcome::Deferred),
                    });
                }
                self.pending = Some(pending);
                RETAIN_ROLES.store(true, Ordering::Release);
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
    RETAIN_ROLES.load(Ordering::Acquire)
        || owners()
            .lock()
            .is_ok_and(|owners| owners.values().any(|owner| automatic(&owner.config)))
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
        if !automatic(&config) && !RETAIN_ROLES.load(Ordering::Acquire) {
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
            scope_key: Uuid::nil(),
            page: page.id,
            pinned: page.pinned,
            native,
            config,
            thread: ui.qt_thread(),
            seen_active,
            ready: !presentation.foreground || seen_active,
        })
    })();
    let Some(mut owner) = captured else {
        release(&token);
        return;
    };
    if let Ok(mut owners) = owners().lock()
        && (owners.len() < 128 || owners.contains_key(&token))
    {
        owner.scope_key = owners
            .values()
            .find(|previous| previous.scope == owner.scope)
            .map_or_else(Uuid::new_v4, |previous| previous.scope_key);
        owners.insert(token, owner);
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
    if !eligible || !requested {
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
