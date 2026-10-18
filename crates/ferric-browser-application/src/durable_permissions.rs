//! Exact-key durable preferences shared by owners of one validated profile.
use super::{ProfileSession, SessionScope};
use crate::{BrowserApplication, StorageTicket};
use ferric_browser_config::PermissionDecision;
use ferric_browser_core::PrivacyKind;
use ferric_browser_storage::{
    PermissionRule, StorageCommand, StorageCompletion, StorageWorkerError,
    is_rememberable_permission, normalize_permission_origin,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

type Key = (String, String);
const MAX_CACHED_PERMISSION_ENTRIES: usize = 4096;
const PERMISSION_READ_RETRY_DELAY: Duration = Duration::from_secs(2);

/// Availability of saved choices, independent of whether a request is allowed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermissionPreferencesStatus {
    Ready,
    Loading,
    Unchecked,
    Unavailable,
    NotStored,
}

/// One consistent projection from the same authority used by live requests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedPermissionPreferences {
    pub rules: Vec<PermissionRule>,
    pub complete: bool,
    pub status: PermissionPreferencesStatus,
}
#[derive(Clone)]
struct Barrier {
    epoch: u64,
    // None is an uncertain completion; retain a previous deny while refreshing.
    decision: Option<PermissionDecision>,
}
#[derive(Default)]
pub(crate) struct PermissionCache {
    epoch: u64,
    pending_epochs: BTreeMap<u64, usize>,
    exhausted: bool,
    ready: bool,
    rules: BTreeMap<Key, PermissionRule>,
    barriers: BTreeMap<Key, Barrier>,
    confirmed: BTreeSet<Key>,
    ready_origins: BTreeSet<String>,
    loading_origins: BTreeMap<String, usize>,
    failed_reads: BTreeMap<String, Instant>,
    snapshot_unavailable: bool,
}

/// Policy outcome and whether the adapter should request a metadata snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionResolution {
    pub decision: PermissionDecision,
    pub refresh_needed: bool,
    /// The saved preference is still unknown; retain the native request while
    /// the adapter performs the bounded metadata read.
    pub pending: bool,
}
impl PermissionResolution {
    const fn known(decision: PermissionDecision) -> Self {
        Self {
            decision,
            refresh_needed: false,
            pending: false,
        }
    }
}

impl PermissionCache {
    fn capture(&self) -> Option<u64> {
        (!self.exhausted && self.pending_epochs.is_empty()).then_some(self.epoch)
    }
    fn lookup(&self, key: &Key, now: i64) -> (Option<PermissionDecision>, bool) {
        let saved = self
            .rules
            .get(key)
            .filter(|rule| rule.expires_at.is_none_or(|expires| expires > now))
            .map(|rule| match rule.decision.as_str() {
                "allow" => PermissionDecision::Allow,
                "deny" => PermissionDecision::Deny,
                _ => PermissionDecision::Ask,
            });
        if let Some(barrier) = self.barriers.get(key) {
            return (
                barrier
                    .decision
                    .clone()
                    .or_else(|| saved.filter(|decision| *decision == PermissionDecision::Deny)),
                false,
            );
        }
        if !self.exhausted
            && (self.ready || self.ready_origins.contains(&key.0) || self.confirmed.contains(key))
        {
            (saved, true)
        } else {
            (
                saved.filter(|decision| *decision == PermissionDecision::Deny),
                false,
            )
        }
    }

    fn read_retry_delayed(&self, origin: &str) -> bool {
        self.failed_reads
            .get(origin)
            .is_some_and(|failed| failed.elapsed() < PERMISSION_READ_RETRY_DELAY)
    }

    fn read_failed(&mut self, origin: Option<&str>) {
        if let Some(origin) = origin {
            self.failed_reads.insert(origin.to_owned(), Instant::now());
        } else {
            self.snapshot_unavailable = true;
        }
        self.bound();
    }

    fn finished_loading(&mut self, origin: Option<&str>) {
        if let Some(origin) = origin
            && let Some(count) = self.loading_origins.get_mut(origin)
        {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.loading_origins.remove(origin);
            }
        }
    }

    fn finished_mutation(&mut self, epoch: u64) {
        if let Some(count) = self.pending_epochs.get_mut(&epoch) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.pending_epochs.remove(&epoch);
            }
        }
    }

    fn bound(&mut self) {
        if self.rules.len()
            + self.barriers.len()
            + self.ready_origins.len()
            + self.failed_reads.len()
            > MAX_CACHED_PERMISSION_ENTRIES
        {
            // Eviction must also forget proof of absence. A later request must
            // read this site's policy again before any configured Allow applies.
            self.distrust_snapshot();
            self.rules.clear();
            self.failed_reads.clear();
            // Live decisions are bounded by admitted storage work, not by the
            // metadata cache. Retain them until completion or cancellation.
            self.barriers
                .retain(|_, barrier| self.pending_epochs.contains_key(&barrier.epoch));
        }
    }

    fn distrust_snapshot(&mut self) {
        self.ready = false;
        self.ready_origins.clear();
        self.confirmed.clear();
    }

    fn accept_snapshot(
        &mut self,
        epoch: Option<u64>,
        origin: Option<&str>,
        values: &[PermissionRule],
        complete: bool,
    ) {
        if epoch.is_none() || self.capture() != epoch {
            return;
        }
        let mut rules = BTreeMap::new();
        for rule in values {
            let Ok(normalized) = normalize_permission_origin(&rule.origin) else {
                self.distrust_snapshot();
                return;
            };
            if origin.is_some_and(|scope| scope != normalized)
                || !is_rememberable_permission(&rule.permission)
                || !matches!(rule.decision.as_str(), "allow" | "deny" | "ask")
                || rules
                    .insert((normalized, rule.permission.clone()), rule.clone())
                    .is_some()
            {
                self.distrust_snapshot();
                return;
            }
        }
        if complete {
            if let Some(origin) = origin {
                self.failed_reads.remove(origin);
                self.rules.retain(|(saved, _), _| saved != origin);
                self.confirmed.retain(|(saved, _)| saved != origin);
                self.barriers.retain(|(saved, _), _| saved != origin);
                self.ready_origins.insert(origin.to_owned());
                self.confirmed.extend(rules.keys().cloned());
                self.rules.extend(rules);
            } else {
                self.confirmed = rules.keys().cloned().collect();
                self.rules = rules;
                self.ready = true;
                self.failed_reads.clear();
                self.snapshot_unavailable = false;
                self.ready_origins.clear();
                self.barriers.clear();
            }
        } else {
            self.ready = false;
            if let Some(origin) = origin {
                self.ready_origins.remove(origin);
            }
            for key in rules.keys() {
                self.barriers.remove(key);
                self.confirmed.insert(key.clone());
            }
            self.rules.extend(rules);
        }
        self.bound();
    }
}

enum WorkKind {
    Read {
        epoch: Option<u64>,
        origin: Option<String>,
    },
    Mutation {
        epoch: u64,
        previous: Vec<(Key, Option<Barrier>)>,
    },
}
/// Captures the original session even if its application changes profile.
/// An admitted request must be finished or dropped; both release accounting.
pub(crate) struct PermissionWork {
    session: ProfileSession,
    kind: Option<WorkKind>,
    admitted: bool,
}
impl PermissionWork {
    pub(crate) fn prepare(
        session: Option<ProfileSession>,
        command: &StorageCommand,
    ) -> Option<Self> {
        let session = session?;
        if !matches!(&session.0.scope, SessionScope::Durable { .. }) {
            return None;
        }
        let kind = {
            let mut cache = session.0.durable.lock().ok()?;
            match command {
                StorageCommand::Library { .. } => WorkKind::Read {
                    epoch: cache.capture(),
                    origin: None,
                },
                StorageCommand::PermissionLookup { origin } => {
                    let origin = normalize_permission_origin(origin).ok()?;
                    let loading = cache.loading_origins.entry(origin.clone()).or_default();
                    *loading = loading.checked_add(1)?;
                    WorkKind::Read {
                        epoch: cache.capture(),
                        origin: Some(origin),
                    }
                }
                StorageCommand::PermissionBatch { rules } => {
                    let keys = rules
                        .iter()
                        .filter_map(|rule| {
                            let origin = normalize_permission_origin(&rule.origin).ok()?;
                            is_rememberable_permission(&rule.permission).then(|| {
                                (
                                    (origin, rule.permission.clone()),
                                    if rule.decision == "deny" {
                                        PermissionDecision::Deny
                                    } else {
                                        PermissionDecision::Ask
                                    },
                                )
                            })
                        })
                        .collect();
                    Self::begin(&mut cache, keys)
                }
                StorageCommand::PermissionReset {
                    origin,
                    permissions,
                } => {
                    let origin = normalize_permission_origin(origin).ok()?;
                    let keys = permissions
                        .iter()
                        .filter(|permission| is_rememberable_permission(permission))
                        .map(|permission| {
                            (
                                (origin.clone(), permission.clone()),
                                PermissionDecision::Ask,
                            )
                        })
                        .collect();
                    Self::begin(&mut cache, keys)
                }
                _ => return None,
            }
        };
        Some(Self {
            session,
            kind: Some(kind),
            admitted: false,
        })
    }
    fn begin(cache: &mut PermissionCache, keys: BTreeMap<Key, PermissionDecision>) -> WorkKind {
        if let Some(next) = cache.epoch.checked_add(1) {
            cache.epoch = next;
        } else {
            cache.exhausted = true;
        }
        let epoch = cache.epoch;
        *cache.pending_epochs.entry(epoch).or_default() += 1;
        let previous = keys
            .into_iter()
            .map(|(key, decision)| {
                let previous = cache.barriers.insert(
                    key.clone(),
                    Barrier {
                        epoch,
                        decision: Some(decision),
                    },
                );
                (key, previous)
            })
            .collect();
        cache.bound();
        WorkKind::Mutation { epoch, previous }
    }
    pub(crate) fn admitted(mut self) -> Self {
        self.admitted = true;
        self
    }
    pub(crate) fn finish(mut self, completion: &StorageCompletion) {
        let Some(kind) = self.kind.take() else {
            return;
        };
        let Ok(mut cache) = self.session.0.durable.lock() else {
            return;
        };
        match kind {
            WorkKind::Read { epoch, origin } => {
                cache.finished_loading(origin.as_deref());
                match (completion, origin.as_deref()) {
                    (StorageCompletion::Library(Ok(snapshot)), None) => cache.accept_snapshot(
                        epoch,
                        None,
                        &snapshot.permissions,
                        snapshot.permissions_complete,
                    ),
                    (StorageCompletion::PermissionLookup(Ok(snapshot)), Some(origin)) => cache
                        .accept_snapshot(epoch, Some(origin), &snapshot.rules, snapshot.complete),
                    _ => cache.read_failed(origin.as_deref()),
                }
            }
            WorkKind::Mutation { epoch, previous } => {
                cache.finished_mutation(epoch);
                if !matches!(
                    completion,
                    StorageCompletion::PermissionBatch(Ok(_))
                        | StorageCompletion::PermissionReset(Ok(_))
                ) {
                    Self::uncertain(&mut cache, epoch, previous);
                }
                cache.bound();
            }
        }
    }
    fn uncertain(cache: &mut PermissionCache, epoch: u64, previous: Vec<(Key, Option<Barrier>)>) {
        for (key, _) in previous {
            if let Some(barrier) = cache.barriers.get_mut(&key)
                && barrier.epoch == epoch
            {
                barrier.decision = None;
            }
        }
    }
}
impl Drop for PermissionWork {
    fn drop(&mut self) {
        let Some(kind) = self.kind.take() else {
            return;
        };
        let Ok(mut cache) = self.session.0.durable.lock() else {
            return;
        };
        let (epoch, previous) = match kind {
            WorkKind::Read { origin, .. } => {
                cache.finished_loading(origin.as_deref());
                if self.admitted {
                    cache.read_failed(origin.as_deref());
                }
                return;
            }
            WorkKind::Mutation { epoch, previous } => (epoch, previous),
        };
        cache.finished_mutation(epoch);
        if self.admitted {
            Self::uncertain(&mut cache, epoch, previous);
        } else {
            for (key, previous) in previous {
                if cache
                    .barriers
                    .get(&key)
                    .is_some_and(|barrier| barrier.epoch == epoch)
                {
                    if let Some(previous) = previous {
                        cache.barriers.insert(key, previous);
                    } else {
                        cache.barriers.remove(&key);
                    }
                }
            }
        }
        cache.bound();
    }
}

impl BrowserApplication {
    /// The exact site's saved preferences and whether absence is established.
    /// Uses the same shared cache and mutation barriers as live decisions.
    #[must_use]
    pub fn saved_permission_preferences(&self, origin: &str) -> SavedPermissionPreferences {
        let mut result = SavedPermissionPreferences {
            rules: vec![],
            complete: false,
            status: PermissionPreferencesStatus::Unavailable,
        };
        let Ok(origin) = normalize_permission_origin(origin) else {
            return result;
        };
        let Ok(session) = self.permission_session() else {
            return result;
        };
        if !matches!(session.0.scope, SessionScope::Durable { .. }) {
            // A private/temporary session has no durable choices. A normal
            // profile with failed storage has unknown choices, not an empty set.
            result.complete = self
                .profile_snapshot()
                .is_some_and(|profile| profile.privacy.is_transient());
            if result.complete {
                result.status = PermissionPreferencesStatus::NotStored;
            }
            return result;
        }
        let Ok(cache) = session.0.durable.lock() else {
            return result;
        };
        result.complete = !cache.exhausted
            && (cache.ready || cache.ready_origins.contains(&origin))
            && !cache.barriers.keys().any(|(saved, _)| saved == &origin);
        result.status = if result.complete {
            PermissionPreferencesStatus::Ready
        } else if !self.storage_is_open()
            || cache.snapshot_unavailable
            || cache.failed_reads.contains_key(&origin)
        {
            PermissionPreferencesStatus::Unavailable
        } else if cache.loading_origins.contains_key(&origin)
            || cache.barriers.iter().any(|((saved, _), barrier)| {
                saved == &origin && cache.pending_epochs.contains_key(&barrier.epoch)
            })
        {
            PermissionPreferencesStatus::Loading
        } else {
            PermissionPreferencesStatus::Unchecked
        };
        let now = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        )
        .unwrap_or(i64::MAX);
        result.rules = cache
            .rules
            .values()
            .filter(|rule| {
                rule.origin == origin && rule.expires_at.is_none_or(|expires| expires > now)
            })
            .cloned()
            .collect();
        result
    }
    /// Queues one exact-origin policy read at the actual profile owner.
    /// The shared session suppresses duplicate in-flight reads across windows.
    /// # Errors
    /// Rejects invalid origins, unavailable owners and storage backpressure.
    pub fn refresh_permission_preferences(
        &mut self,
        origin: &str,
    ) -> Result<Option<StorageTicket>, StorageWorkerError> {
        let origin = normalize_permission_origin(origin)
            .map_err(|_| StorageWorkerError::InvalidPermissionOrigin)?;
        let session = self
            .permission_session()
            .map_err(|_| StorageWorkerError::Stopped)?
            .clone();
        if !matches!(session.0.scope, SessionScope::Durable { .. }) {
            return Ok(None);
        }
        let cache = session
            .0
            .durable
            .lock()
            .map_err(|_| StorageWorkerError::Stopped)?;
        let loading =
            cache.loading_origins.contains_key(&origin) || cache.read_retry_delayed(&origin);
        drop(cache);
        if loading {
            return Ok(None);
        }
        self.submit_storage(StorageCommand::PermissionLookup { origin })
            .map(Some)
    }
    /// Resolves configured, durable and session preferences at their live owner.
    /// Unknown durable data requires consent before using an Allow default.
    #[must_use]
    pub fn resolve_permission(&self, origin: &str, permission: &str) -> PermissionResolution {
        if let Ok([Some(first), Some(second)]) = super::capabilities(permission) {
            let first = self.resolve_permission(origin, first);
            let second = self.resolve_permission(origin, second);
            let decision = if first.decision == PermissionDecision::Deny
                || second.decision == PermissionDecision::Deny
            {
                PermissionDecision::Deny
            } else if first.decision == PermissionDecision::Allow
                && second.decision == PermissionDecision::Allow
            {
                PermissionDecision::Allow
            } else {
                PermissionDecision::Ask
            };
            return PermissionResolution {
                pending: decision != PermissionDecision::Deny && (first.pending || second.pending),
                refresh_needed: first.refresh_needed || second.refresh_needed,
                decision,
            };
        }
        if permission == "screen-capture" {
            return PermissionResolution::known(PermissionDecision::Ask);
        }
        if !is_rememberable_permission(permission) {
            return PermissionResolution::known(PermissionDecision::Deny);
        }
        let Ok(session) = self.permission_session() else {
            return PermissionResolution::known(PermissionDecision::Deny);
        };
        let config = self.configuration.effective();
        let Ok(origin) = normalize_permission_origin(origin) else {
            // An origin that cannot be matched to saved preferences must never
            // bypass a denial through a configured Allow default.
            return PermissionResolution::known(match default_decision(config, permission) {
                PermissionDecision::Allow => PermissionDecision::Ask,
                decision => decision,
            });
        };
        let Some(profile) = self.profile_snapshot() else {
            return PermissionResolution::known(PermissionDecision::Deny);
        };
        let mut ready = true;
        let mut durable = None;
        let mut loading = false;
        if profile.privacy == PrivacyKind::Normal {
            if let Some(rule) = config.permission_rules.iter().find(|rule| {
                rule.profile == profile.name
                    && rule.permission == permission
                    && normalize_permission_origin(&rule.origin).ok().as_deref()
                        == Some(origin.as_str())
            }) {
                return PermissionResolution::known(rule.decision.clone());
            }
            ready = false;
            if profile.durable
                && let Ok(cache) = session.0.durable.lock()
            {
                let now = i64::try_from(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                )
                .unwrap_or(i64::MAX);
                loading = cache.loading_origins.contains_key(&origin);
                (durable, ready) = cache.lookup(&(origin.clone(), permission.to_owned()), now);
            }
        }
        let refresh_needed = !ready && !loading && self.storage_is_open();
        if let Some(decision) = durable {
            return PermissionResolution {
                decision,
                refresh_needed,
                pending: false,
            };
        }
        if let Some(decision) = self.session_permission(&origin, permission) {
            return PermissionResolution {
                decision,
                refresh_needed,
                pending: false,
            };
        }
        let decision = if ready {
            default_decision(config, permission)
        } else {
            PermissionDecision::Ask
        };
        PermissionResolution {
            decision,
            refresh_needed,
            pending: !ready && self.storage_is_open(),
        }
    }
}

fn default_decision(
    config: &ferric_browser_config::Config,
    permission: &str,
) -> PermissionDecision {
    let defaults = &config.permissions;
    match permission {
        "camera" => &defaults.camera,
        "microphone" => &defaults.microphone,
        "notifications" => &defaults.notifications,
        "geolocation" => &defaults.geolocation,
        "clipboard" => &defaults.clipboard,
        "local-fonts" => &defaults.local_fonts,
        _ => &PermissionDecision::Deny,
    }
    .clone()
}

#[cfg(test)]
#[path = "durable_permissions_tests.rs"]
mod tests;
