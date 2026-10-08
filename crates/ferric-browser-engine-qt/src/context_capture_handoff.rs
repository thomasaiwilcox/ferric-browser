//! Opaque, bounded same-process handoff of owner-issued context captures.
//!
//! QML may transport a random handle, but never the structural window,
//! runtime binding plan, or profile authority. The originating `BrowserUi`
//! alone may consume a handle for its own exact window and owner token.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use ferric_browser_application::versioning::ContextWindowCaptureCandidate;
use ferric_browser_core::{NativeProfileOwner, WindowId};
use ferric_browser_storage::ContextMember;
use uuid::Uuid;

const MAX_STAGED_CAPTURES: usize = 256;
const CAPTURE_LIFETIME: Duration = Duration::from_secs(30);

struct Entry<T> {
    window: WindowId,
    owner_token: String,
    value: T,
    expires_at: Instant,
}

pub(super) struct StagedCaptureCopy<T> {
    pub handle: Uuid,
    pub window: WindowId,
    pub owner_token: String,
    pub value: T,
}

pub(super) struct CaptureClaim {
    handle: Uuid,
    window: WindowId,
    owner_token: String,
    expires_at: Instant,
}

#[derive(Clone)]
pub(super) struct BindingExpectation {
    pub handle: Uuid,
    pub window: WindowId,
    pub owner_token: String,
}

#[derive(Clone)]
pub(super) struct ContextMembershipCandidate {
    pub profile_uuid: Uuid,
    pub native: NativeProfileOwner,
    pub application_revision: u64,
    pub context_name: String,
    pub member: ContextMember,
    pub skipped_unsafe_tabs: usize,
}

struct CaptureHandoff<T> {
    entries: BTreeMap<Uuid, Entry<T>>,
    bound: BTreeMap<Uuid, CaptureClaim>,
}

impl<T> Default for CaptureHandoff<T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            bound: BTreeMap::new(),
        }
    }
}

impl<T> CaptureHandoff<T> {
    fn prune(&mut self) {
        let now = Instant::now();
        self.entries.retain(|_, entry| entry.expires_at > now);
        self.bound.retain(|_, receipt| receipt.expires_at > now);
    }

    fn stage(&mut self, window: WindowId, owner_token: &str, value: T) -> Option<Uuid> {
        if owner_token.is_empty() || owner_token.len() > 128 {
            return None;
        }
        self.prune();
        if self.entries.len() + self.bound.len() >= MAX_STAGED_CAPTURES {
            return None;
        }
        let mut handle = Uuid::new_v4();
        while self.entries.contains_key(&handle) {
            handle = Uuid::new_v4();
        }
        self.entries.insert(
            handle,
            Entry {
                window,
                owner_token: owner_token.to_owned(),
                value,
                expires_at: Instant::now() + CAPTURE_LIFETIME,
            },
        );
        Some(handle)
    }

    fn take_owned(
        &mut self,
        handle: Uuid,
        window: WindowId,
        owner_token: &str,
    ) -> Option<(T, CaptureClaim)> {
        self.prune();
        let entry = self.entries.get(&handle)?;
        if entry.window != window || entry.owner_token != owner_token {
            return None;
        }
        self.entries.remove(&handle).map(|entry| {
            (
                entry.value,
                CaptureClaim {
                    handle,
                    window,
                    owner_token: entry.owner_token,
                    expires_at: entry.expires_at,
                },
            )
        })
    }

    fn record_bound(&mut self, claim: CaptureClaim) -> bool {
        self.prune();
        if claim.expires_at <= Instant::now()
            || self.entries.contains_key(&claim.handle)
            || self.bound.contains_key(&claim.handle)
            || self.entries.len() + self.bound.len() >= MAX_STAGED_CAPTURES
        {
            return false;
        }
        self.bound.insert(claim.handle, claim);
        true
    }

    fn take_bound_receipts(&mut self, expected: &[BindingExpectation]) -> bool {
        self.prune();
        if expected.is_empty() || expected.len() > MAX_STAGED_CAPTURES {
            return false;
        }
        let mut seen = BTreeSet::new();
        if expected.iter().any(|candidate| {
            !seen.insert(candidate.handle)
                || self.bound.get(&candidate.handle).is_none_or(|receipt| {
                    receipt.window != candidate.window
                        || receipt.owner_token != candidate.owner_token
                })
        }) {
            return false;
        }
        for candidate in expected {
            self.bound.remove(&candidate.handle);
        }
        true
    }
}

impl<T: Clone> CaptureHandoff<T> {
    fn snapshot(&mut self, handles: &[Uuid]) -> Option<Vec<StagedCaptureCopy<T>>> {
        if handles.is_empty() || handles.len() > MAX_STAGED_CAPTURES {
            return None;
        }
        self.prune();
        let mut seen = BTreeSet::new();
        let mut copies = Vec::with_capacity(handles.len());
        for handle in handles {
            if !seen.insert(*handle) {
                return None;
            }
            let entry = self.entries.get(handle)?;
            copies.push(StagedCaptureCopy {
                handle: *handle,
                window: entry.window,
                owner_token: entry.owner_token.clone(),
                value: entry.value.clone(),
            });
        }
        Some(copies)
    }
}

static CAPTURES: OnceLock<Mutex<CaptureHandoff<ContextWindowCaptureCandidate>>> = OnceLock::new();
static MEMBERSHIPS: OnceLock<Mutex<CaptureHandoff<ContextMembershipCandidate>>> = OnceLock::new();

fn captures() -> &'static Mutex<CaptureHandoff<ContextWindowCaptureCandidate>> {
    CAPTURES.get_or_init(|| Mutex::new(CaptureHandoff::default()))
}

fn memberships() -> &'static Mutex<CaptureHandoff<ContextMembershipCandidate>> {
    MEMBERSHIPS.get_or_init(|| Mutex::new(CaptureHandoff::default()))
}

pub(super) fn stage_membership(
    window: WindowId,
    owner_token: &str,
    candidate: ContextMembershipCandidate,
) -> Option<String> {
    memberships()
        .lock()
        .ok()?
        .stage(window, owner_token, candidate)
        .map(|handle| handle.to_string())
}

pub(super) fn snapshot_memberships(
    handles: &[String],
) -> Option<Vec<StagedCaptureCopy<ContextMembershipCandidate>>> {
    let parsed = handles
        .iter()
        .map(|text| {
            Uuid::parse_str(text)
                .ok()
                .filter(|id| id.to_string() == *text)
        })
        .collect::<Option<Vec<_>>>()?;
    memberships().lock().ok()?.snapshot(&parsed)
}

pub(super) fn take_membership(
    handle: &str,
    window: WindowId,
    owner_token: &str,
) -> Option<(ContextMembershipCandidate, CaptureClaim)> {
    let handle = Uuid::parse_str(handle).ok()?;
    memberships()
        .lock()
        .ok()?
        .take_owned(handle, window, owner_token)
}

pub(super) fn record_confirmed_membership(claim: CaptureClaim) -> bool {
    memberships()
        .lock()
        .is_ok_and(|mut memberships| memberships.record_bound(claim))
}

pub(super) fn take_confirmed_memberships(expected: &[BindingExpectation]) -> bool {
    memberships()
        .lock()
        .is_ok_and(|mut memberships| memberships.take_bound_receipts(expected))
}

pub(super) fn stage_capture(
    window: WindowId,
    owner_token: &str,
    candidate: ContextWindowCaptureCandidate,
) -> Option<String> {
    captures()
        .lock()
        .ok()?
        .stage(window, owner_token, candidate)
        .map(|handle| handle.to_string())
}

pub(super) fn take_capture(
    handle: &str,
    window: WindowId,
    owner_token: &str,
) -> Option<(ContextWindowCaptureCandidate, CaptureClaim)> {
    let handle = Uuid::parse_str(handle).ok()?;
    captures()
        .lock()
        .ok()?
        .take_owned(handle, window, owner_token)
}

pub(super) fn record_bound_capture(claim: CaptureClaim) -> bool {
    captures()
        .lock()
        .is_ok_and(|mut captures| captures.record_bound(claim))
}

pub(super) fn take_bound_capture_receipts(expected: &[BindingExpectation]) -> bool {
    captures()
        .lock()
        .is_ok_and(|mut captures| captures.take_bound_receipts(expected))
}

pub(super) fn snapshot_captures(
    handles: &[String],
) -> Option<Vec<StagedCaptureCopy<ContextWindowCaptureCandidate>>> {
    let parsed = handles
        .iter()
        .map(|text| {
            Uuid::parse_str(text)
                .ok()
                .filter(|id| id.to_string() == *text)
        })
        .collect::<Option<Vec<_>>>()?;
    captures().lock().ok()?.snapshot(&parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_exact_owner_can_consume_one_opaque_capture() {
        let window = WindowId::from_display("windowid-901").expect("window ID");
        let other = WindowId::from_display("windowid-902").expect("other window ID");
        let mut handoff = CaptureHandoff::default();
        let handle = handoff.stage(window, "owner-1", 17u8).expect("stage");
        assert!(handoff.take_owned(handle, other, "owner-1").is_none());
        assert!(handoff.take_owned(handle, window, "stale-owner").is_none());
        let (value, claim) = handoff
            .take_owned(handle, window, "owner-1")
            .expect("exact owner consumes once");
        assert_eq!(value, 17);
        assert!(handoff.take_owned(handle, window, "owner-1").is_none());
        assert!(handoff.record_bound(claim));
        let expected = [BindingExpectation {
            handle,
            window,
            owner_token: "owner-1".into(),
        }];
        assert!(!handoff.take_bound_receipts(&[BindingExpectation {
            handle,
            window,
            owner_token: "stale-owner".into(),
        }]));
        assert!(handoff.take_bound_receipts(&expected));
        assert!(!handoff.take_bound_receipts(&expected));
    }

    #[test]
    fn expired_and_full_handoffs_refuse_without_overwriting() {
        let window = WindowId::from_display("windowid-903").expect("window ID");
        let mut handoff = CaptureHandoff::default();
        assert!(handoff.stage(window, "", 1usize).is_none());
        let handle = handoff.stage(window, "owner", 2usize).expect("stage");
        handoff
            .entries
            .get_mut(&handle)
            .expect("staged entry")
            .expires_at = Instant::now();
        assert!(handoff.take_owned(handle, window, "owner").is_none());
        for index in 0..MAX_STAGED_CAPTURES {
            assert!(handoff.stage(window, "owner", index).is_some());
        }
        assert!(handoff.stage(window, "owner", 999).is_none());
        assert_eq!(handoff.entries.len(), MAX_STAGED_CAPTURES);
    }

    #[test]
    fn snapshot_is_bounded_unique_and_does_not_consume_owner_claims() {
        let window = WindowId::from_display("windowid-904").expect("window ID");
        let mut handoff = CaptureHandoff::default();
        let first = handoff.stage(window, "owner", 11u8).expect("first capture");
        let second = handoff
            .stage(window, "owner", 12u8)
            .expect("second capture");
        assert!(handoff.snapshot(&[first, first]).is_none());
        assert!(handoff.snapshot(&[first, Uuid::new_v4()]).is_none());
        let copies = handoff.snapshot(&[first, second]).expect("exact handles");
        assert_eq!(copies.len(), 2);
        assert_eq!(copies[0].value, 11);
        assert_eq!(copies[1].value, 12);
        assert_eq!(copies[0].owner_token, "owner");
        let (first_value, first_claim) = handoff
            .take_owned(first, window, "owner")
            .expect("first exact owner claim");
        assert_eq!(first_value, 11);
        assert!(handoff.record_bound(first_claim));
        let expected = [first, second].map(|handle| BindingExpectation {
            handle,
            window,
            owner_token: "owner".into(),
        });
        assert!(
            !handoff.take_bound_receipts(&expected),
            "one peer receipt never authorizes the whole aggregate"
        );
        let (second_value, second_claim) = handoff
            .take_owned(second, window, "owner")
            .expect("second exact owner claim");
        assert_eq!(second_value, 12);
        assert!(handoff.record_bound(second_claim));
        assert!(handoff.take_bound_receipts(&expected));
    }
}
