//! Normal session snapshots survive routing teardown during application Quit.
//! This lease retains metadata only and never grants native/routing authority.
use crate::browser_ui_page_sessions::PageSessionOwner;
use crate::{TabId, Uuid};
use ferric_browser_application::ProfileInstanceId;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
pub(super) struct ShutdownOwner {
    pub(super) token: String,
    pub(super) order: u64,
    pub(super) focused: u64,
    pub(super) snapshot: PageSessionOwner,
    pub(super) ranks: HashMap<TabId, usize>,
}
struct ShutdownSessions {
    nonce: Uuid,
    owners: HashMap<(ProfileInstanceId, String), ShutdownOwner>,
}
fn store() -> &'static Mutex<Option<ShutdownSessions>> {
    static STORE: OnceLock<Mutex<Option<ShutdownSessions>>> = OnceLock::new();
    STORE.get_or_init(Default::default)
}
pub(super) struct ShutdownSessionLease(Uuid);
impl Drop for ShutdownSessionLease {
    fn drop(&mut self) {
        if let Ok(mut store) = store().lock()
            && store.as_ref().is_some_and(|active| active.nonce == self.0)
        {
            *store = None;
        }
    }
}
impl ShutdownSessions {
    fn observe(&mut self, owner: ShutdownOwner) {
        // Empty/retired owners must not erase the snapshot approved for Quit.
        if owner.snapshot.groups.is_empty() {
            return;
        }
        let key = (owner.snapshot.profile, owner.token.clone());
        if let Some(previous) = self.owners.get_mut(&key) {
            // Dialog activation while quitting must not change restore focus.
            // Final actual owner metadata may still update before native release.
            previous.snapshot = owner.snapshot;
            previous.ranks.extend(
                owner
                    .ranks
                    .into_iter()
                    .filter(|(_, rank)| *rank != usize::MAX),
            );
        } else {
            self.owners.insert(key, owner);
        }
    }
}
pub(super) fn begin(owners: Vec<ShutdownOwner>) -> Option<ShutdownSessionLease> {
    let mut store = store().lock().ok()?;
    if store.is_some() {
        return None;
    }
    let nonce = Uuid::new_v4();
    let mut active = ShutdownSessions {
        nonce,
        owners: HashMap::new(),
    };
    for owner in owners {
        active.observe(owner);
    }
    *store = Some(active);
    Some(ShutdownSessionLease(nonce))
}
pub(super) fn merge(
    profile: ProfileInstanceId,
    owners: Vec<ShutdownOwner>,
) -> (bool, Vec<ShutdownOwner>) {
    let Ok(mut store) = store().lock() else {
        return (false, owners);
    };
    let Some(active) = store.as_mut() else {
        return (false, owners);
    };
    for owner in owners {
        active.observe(owner);
    }
    (
        true,
        active
            .owners
            .values()
            .filter(|owner| owner.snapshot.profile == profile)
            .cloned()
            .collect(),
    )
}
