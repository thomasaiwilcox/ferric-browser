//! Private one-shot focus authority over actual Core owners. Qt retains native
//! window activation policy; the mapping-time marker supplies the real host.
use crate::browser_ui_page_windows::PageWindowSnapshot;
use crate::page_window_undo::Scope;
use crate::{CxxQtType, QString, Target, Uuid, WindowId, qobject};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

static GENERATION: AtomicU64 = AtomicU64::new(1);

fn advance_after(counter: &AtomicU64, expected: u64) -> Option<u64> {
    if expected == 0 {
        return None;
    }
    let next = expected.checked_add(1).unwrap_or(0);
    counter
        .compare_exchange(expected, next, Ordering::AcqRel, Ordering::Acquire)
        .ok()?;
    (next != 0).then_some(next)
}

pub(crate) fn generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

pub(crate) fn reserve_after(expected: u64) -> u64 {
    advance_after(&GENERATION, expected).unwrap_or(0)
}

pub(crate) fn current(generation: u64) -> bool {
    generation != 0 && self::generation() == generation
}

pub(crate) fn supersede_page_focus() -> QString {
    let value = reserve_after(generation());
    QString::from(if value == 0 {
        String::new()
    } else {
        value.to_string()
    })
}
pub(crate) fn current_page_focus() -> QString {
    QString::from(generation().to_string())
}

pub(crate) fn page_focus_intent_current(intent: &QString) -> bool {
    let intent = intent.to_string();
    intent
        .parse::<u64>()
        .is_ok_and(|value| value.to_string() == intent && current(value))
}

#[derive(Clone, Eq, PartialEq)]
struct Owner {
    token: String,
    window: WindowId,
    scope: Scope,
    selected: Target,
}

impl Owner {
    fn capture(ui: &qobject::BrowserUi) -> Option<Self> {
        let snapshot = PageWindowSnapshot::capture(ui)?;
        let rust = ui.rust();
        let selected = snapshot
            .pages
            .iter()
            .find(|page| Some(page.id) == snapshot.selected)?
            .target;
        if rust.tab != Some(selected.tab) {
            return None;
        }
        Some(Self {
            token: rust.window_token.to_string(),
            window: rust.window?,
            scope: Scope::capture(rust)?,
            selected,
        })
    }
}

pub(crate) struct Lease {
    pub(crate) reporter: Option<ferric_browser_application::NativeOperationReporter>,
    nonce: Uuid,
    generation: u64,
    issuer: Owner,
    target: Owner,
    target_marker: crate::page_window_native::NativePageHost,
    issuer_marker: crate::page_window_native::NativePageHost,
    expires: Instant,
    used: AtomicBool,
}

pub(super) fn expire_reporter(rust: &crate::BrowserUiRust) {
    if let Some(lease) = &rust.page_window_focus_lease
        && Instant::now() > lease.expires
        && let Some(reporter) = &lease.reporter
    {
        let _ =
            reporter.finish(ferric_browser_application::NativeOperationOutcome::ActivationUnknown);
    }
}

/// Captured only from the actual terminal Core owner of a trusted command.
/// Its group can differ after Detach/Give; renderer claims cannot construct it.
pub(super) struct Destination(Owner);

impl Destination {
    pub(super) fn capture(ui: &qobject::BrowserUi) -> Option<Self> {
        Owner::capture(ui).map(Self)
    }
    pub(super) fn token(&self) -> &str {
        &self.0.token
    }
}

pub(super) fn prepare(
    ui: &qobject::BrowserUi,
    destination: &Destination,
    generation: u64,
) -> Option<Lease> {
    if !current(generation) {
        return None;
    }
    let issuer = Owner::capture(ui)?;
    let target = &destination.0;
    if !issuer.scope.same_profile(&target.scope) || issuer.scope.context != target.scope.context {
        return None;
    }
    let source = PageWindowSnapshot::capture(ui)?;
    let owner = crate::live_ipc_owners::page_window_owners(&source.scope)
        .into_iter()
        .find(|owner| {
            owner.token == target.token
                && owner.window == target.window
                && owner.snapshot.group == target.scope.group
                && owner.snapshot.selected == Some(target.selected.tab)
                && owner
                    .snapshot
                    .pages
                    .iter()
                    .any(|page| page.target == target.selected)
        })?;
    Some(Lease {
        reporter: None,
        nonce: Uuid::new_v4(),
        generation,
        target: target.clone(),
        target_marker: owner.native_marker?.current_snapshot()?,
        issuer_marker: crate::page_window_native::NativePageHost::capture(ui)?,
        issuer,
        expires: Instant::now() + Duration::from_secs(2),
        used: AtomicBool::new(false),
    })
}

impl Lease {
    pub(crate) fn nonce(&self) -> String {
        self.nonce.to_string()
    }
}

/// Native CXX only. No `QObject` calls or callbacks occur inside this final Core
/// comparison. Phase 1 consumes permission before the first show/raise/activate;
/// phase 2 permits revalidation and actual native receipt observation only.
pub(crate) fn validate_page_focus(
    issuer: &qobject::BrowserUi,
    target: &qobject::BrowserUi,
    nonce: &QString,
    marker: &QString,
    phase: u8,
) -> bool {
    let Some(lease) = issuer.rust().page_window_focus_lease.as_ref() else {
        return false;
    };
    if !lease.issuer_marker.current()
        || !(if phase == 2 && lease.used.load(Ordering::Acquire) {
            lease.target_marker.current_after_show()
        } else {
            lease.target_marker.current()
        })
        || !current(lease.generation)
        || nonce.to_string() != lease.nonce.to_string()
        || marker.to_string() != lease.target_marker.tag
        || Instant::now() > lease.expires
        || Owner::capture(issuer).as_ref() != Some(&lease.issuer)
        || Owner::capture(target).as_ref() != Some(&lease.target)
    {
        return false;
    }
    match phase {
        0 => !lease.used.load(Ordering::Acquire),
        1 => lease
            .used
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok(),
        2 => lease.used.load(Ordering::Acquire),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn focus_generation_rejects_old_tickets_and_never_wraps() {
        let counter = AtomicU64::new(7);
        assert_eq!(advance_after(&counter, 7), Some(8));
        assert_eq!(advance_after(&counter, 7), None);
        assert_eq!(counter.load(Ordering::Acquire), 8);
        let counter = AtomicU64::new(u64::MAX);
        assert_eq!(advance_after(&counter, u64::MAX), None);
        assert_eq!(counter.load(Ordering::Acquire), 0);
        assert_eq!(advance_after(&counter, 0), None);
        assert_eq!(counter.load(Ordering::Acquire), 0);
    }
}
