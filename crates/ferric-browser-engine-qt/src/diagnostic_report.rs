//! Owned diagnostic collection. Qt objects stay on the UI thread.
use ferric_browser_runtime::worker::{Poll, SingleFlightWorker};
use std::time::{Duration, Instant};

use ferric_browser_config::{Config, load_theme_palette, theme_contrast_report};
use ferric_browser_storage::StorageRoots;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    MAX_DIAGNOSTICS_EXPORT_BYTES, crash_diagnostics, diagnostics, diagnostics_presentation,
    diagnostics_snapshot, link_cleaning_policy, presentation_text,
};

const CACHE_LIFETIME: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReportKey {
    pub owner: Uuid,
    pub configuration: u64,
}

pub(super) struct ReportInput {
    pub key: ReportKey,
    pub live: Value,
    pub roots: Option<StorageRoots>,
    pub profile: Option<Uuid>,
    pub config: Value,
    pub private: bool,
}

impl ReportInput {
    pub fn pending_snapshot(&self) -> Value {
        let mut snapshot = diagnostics_snapshot::pending_snapshot();
        merge_live(&mut snapshot, &self.live);
        snapshot["crash"] = json!({"marker": diagnostics::fact(
            "not-tested", None, "Diagnostic collection is pending", "not-probed")});
        snapshot["theme"]["contrast"] = json!({"status":"unknown","checks":[],"failing":[],
            "reason":"Diagnostic collection is pending"});
        if self.roots.is_some() {
            snapshot["link_cleaning"] = diagnostics::fact(
                "not-tested",
                None,
                "Diagnostic collection is pending",
                "not-probed",
            );
        }
        snapshot["collection"] = json!({"status":"pending"});
        snapshot
    }

    fn collect(self) -> Result<CollectedReport, String> {
        let mut snapshot = diagnostics_snapshot::snapshot();
        merge_live(&mut snapshot, &self.live);
        snapshot["crash"] = self.roots.as_ref().map_or_else(
            || {
                json!({"schema":1,"marker": diagnostics::fact("unavailable", None,
                "the active profile has no durable storage root", "not-probed")})
            },
            crash_diagnostics,
        );
        snapshot["link_cleaning"] = link_cleaning_policy::diagnostic_snapshot(self.roots.as_ref());
        let config = serde_json::from_value::<Config>(self.config).unwrap_or_default();
        let contrast = load_theme_palette(&config.theme)
            .map(|palette| serde_json::to_value(theme_contrast_report(&palette)))
            .ok()
            .and_then(Result::ok)
            .unwrap_or_else(|| {
                json!({"status":"unknown","checks":[],"failing":[],
                "reason":"the configured theme palette could not be assessed"})
            });
        snapshot["theme"] = json!({"contrast":contrast,"user_theme_remains_importable":true,
            "security_surfaces":"opaque semantic surfaces retain readable text requirements"});
        if let (Some(roots), Some(profile)) = (self.roots.as_ref(), self.profile) {
            snapshot["storage"]["health"] = diagnostics::storage_health(
                roots
                    .data
                    .join("profiles")
                    .join(profile.to_string())
                    .join("browser.sqlite"),
            );
        }
        snapshot["collection"] = json!({"status":"complete",
            "collected_at":ferric_browser_storage::unix_timestamp()});
        CollectedReport::from_snapshot(snapshot, self.private)
    }
}

/// Only the known in-memory groups merge by child key. Facts within a group
/// replace their entire old fact, preserving unknown/unavailable semantics.
pub(super) fn merge_live(snapshot: &mut Value, live: &Value) {
    if let Some(fields) = live.as_object() {
        for (key, value) in fields {
            if matches!(key.as_str(), "build" | "runtime" | "capabilities") {
                if let (Some(target), Some(source)) =
                    (snapshot[key].as_object_mut(), value.as_object())
                {
                    target.extend(source.clone());
                }
            } else {
                snapshot[key] = value.clone();
            }
        }
    }
}

#[derive(Clone)]
pub(super) struct CollectedReport {
    pub snapshot: Value,
    pub payload: String,
    pub preview: String,
    pub summary: Vec<String>,
    collected: Instant,
}

impl CollectedReport {
    fn from_snapshot(snapshot: Value, private: bool) -> Result<Self, String> {
        let payload = serde_json::to_string_pretty(&snapshot)
            .map_err(|_| "The report couldn't be formatted".to_owned())?;
        if payload.len() > MAX_DIAGNOSTICS_EXPORT_BYTES {
            return Err("The diagnostic report exceeds its export size limit".into());
        }
        Ok(Self {
            preview: presentation_text::report_preview(&payload, 48 * 1024),
            summary: diagnostics_presentation::summary(&snapshot, private),
            snapshot,
            payload,
            collected: Instant::now(),
        })
    }
}

struct Job {
    ticket: u64,
    input: ReportInput,
}

pub(super) struct ReportCompletion {
    pub ticket: u64,
    pub key: ReportKey,
    pub result: Result<CollectedReport, String>,
}

/// One in-flight read and one latest replacement. Closing or switching a
/// profile invalidates authorization without joining an I/O thread on the UI.
#[derive(Default)]
pub(super) struct DiagnosticCollector {
    worker: Option<SingleFlightWorker<Job, ReportCompletion>>,
    key: Option<ReportKey>,
    active: Option<u64>,
    latest: Option<u64>,
    queued: Option<Job>,
    cached: Option<(u64, CollectedReport)>,
    next_ticket: u64,
    failure: Option<String>,
}

impl DiagnosticCollector {
    pub fn select(&mut self, key: ReportKey) {
        if self.key != Some(key) {
            self.clear();
            self.key = Some(key);
        }
    }

    pub fn clear(&mut self) {
        self.key = None;
        self.latest = None;
        self.queued = None;
        self.cached = None;
        self.failure = None;
    }

    pub fn cached(&self) -> Option<&CollectedReport> {
        self.cached.as_ref().map(|(_, report)| report)
    }

    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    pub const fn is_collecting(&self) -> bool {
        self.latest.is_some()
    }

    pub fn request(&mut self, input: ReportInput, force: bool) -> Result<u64, String> {
        self.select(input.key);
        if !force {
            if let Some(ticket) = self.latest {
                return Ok(ticket);
            }
            if let Some((ticket, report)) = &self.cached
                && self.failure.is_none()
                && report.collected.elapsed() < CACHE_LIFETIME
            {
                return Ok(*ticket);
            }
        }
        let ticket = self
            .next_ticket
            .checked_add(1)
            .ok_or_else(|| "Diagnostic request limit reached".to_owned())?;
        self.next_ticket = ticket;
        self.failure = None;
        self.latest = Some(ticket);
        self.queued = Some(Job { ticket, input });
        if let Err(error) = self.start_queued() {
            self.latest = None;
            self.failure = Some(error.clone());
            return Err(error);
        }
        Ok(ticket)
    }

    fn start_queued(&mut self) -> Result<(), String> {
        if self
            .worker
            .as_ref()
            .is_some_and(SingleFlightWorker::is_pending)
            || self.queued.is_none()
        {
            return Ok(());
        }
        if self.worker.is_none() {
            self.worker = Some(
                SingleFlightWorker::spawn("ferric-diagnostics", |job: Job| ReportCompletion {
                    ticket: job.ticket,
                    key: job.input.key,
                    result: job.input.collect(),
                })
                .map_err(|error| error.to_string())?,
            );
        }
        let Some(job) = self.queued.take() else {
            return Ok(());
        };
        let ticket = job.ticket;
        let result = self
            .worker
            .as_mut()
            .ok_or_else(|| "Diagnostic worker is unavailable".to_owned())?
            .submit(job);
        if let Err(error) = result {
            if let Some(worker) = self.worker.take() {
                worker.detach();
            }
            return Err(format!("Diagnostic collection couldn't start: {error:?}"));
        }
        self.active = Some(ticket);
        Ok(())
    }

    pub fn poll(&mut self) -> Option<ReportCompletion> {
        let outcome = self.worker.as_mut().map(SingleFlightWorker::poll)?;
        let completion = match outcome {
            Poll::Pending => return None,
            Poll::Ready(completion) => completion,
            Poll::Stopped => {
                if let Some(worker) = self.worker.take() {
                    worker.detach();
                }
                let ticket = self.active.take()?;
                ReportCompletion {
                    ticket,
                    key: self.key?,
                    result: Err("Diagnostic collector stopped".into()),
                }
            }
        };
        self.active = None;
        let current = self.latest == Some(completion.ticket) && self.key == Some(completion.key);
        if current {
            self.latest = None;
            match &completion.result {
                Ok(report) => {
                    self.cached = Some((completion.ticket, report.clone()));
                    self.failure = None;
                }
                Err(error) => self.failure = Some(error.clone()),
            }
        }
        if let Err(error) = self.start_queued()
            && let (Some(ticket), Some(key)) = (self.latest.take(), self.key)
        {
            self.failure = Some(error.clone());
            return Some(ReportCompletion {
                ticket,
                key,
                result: Err(error),
            });
        }
        current.then_some(completion)
    }
}

impl Drop for DiagnosticCollector {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            worker.detach();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, thread};

    fn key(owner: u128, configuration: u64) -> ReportKey {
        ReportKey {
            owner: Uuid::from_u128(owner),
            configuration,
        }
    }
    fn input(key: ReportKey, marker: &str) -> ReportInput {
        ReportInput {
            key,
            live: json!({"marker":marker}),
            roots: None,
            profile: None,
            config: json!({}),
            private: false,
        }
    }
    fn fake(job: Job) -> ReportCompletion {
        ReportCompletion {
            ticket: job.ticket,
            key: job.input.key,
            result: CollectedReport::from_snapshot(job.input.live, false),
        }
    }
    fn collector(
        handle: impl FnMut(Job) -> ReportCompletion + Send + 'static,
    ) -> DiagnosticCollector {
        let mut collector = DiagnosticCollector::default();
        collector.worker =
            Some(SingleFlightWorker::spawn("diagnostics-test", handle).expect("worker"));
        collector
    }
    fn wait(collector: &mut DiagnosticCollector) -> ReportCompletion {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(completion) = collector.poll() {
                return completion;
            }
            assert!(Instant::now() < deadline, "collector didn't complete");
            thread::yield_now();
        }
    }

    #[test]
    fn blocked_collection_is_off_thread_and_only_latest_replacement_is_published() {
        let (started, starts) = mpsc::channel();
        let (release, releases) = mpsc::channel();
        let mut collector = collector(move |job| {
            started
                .send((job.ticket, thread::current().id()))
                .expect("start observed");
            let _ = releases.recv();
            fake(job)
        });
        let first = collector
            .request(input(key(1, 1), "first"), true)
            .expect("first");
        let (started, worker) = starts
            .recv_timeout(Duration::from_secs(1))
            .expect("started");
        assert_eq!(started, first);
        assert_ne!(worker, thread::current().id());
        let replaced = collector
            .request(input(key(1, 1), "replaced"), true)
            .expect("replace");
        let latest = collector
            .request(input(key(1, 1), "latest"), true)
            .expect("latest");
        assert!(latest > replaced);
        assert!(collector.poll().is_none());
        assert!(collector.cached().is_none());
        release.send(()).expect("release first");
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            assert!(collector.poll().is_none(), "obsolete result was published");
            if let Ok((ticket, _)) = starts.try_recv() {
                assert_eq!(ticket, latest);
                break;
            }
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        release.send(()).expect("release latest");
        let result = wait(&mut collector);
        assert_eq!(result.ticket, latest);
        assert_eq!(result.result.expect("report").snapshot["marker"], "latest");
        assert!(!collector.is_collecting());
    }

    #[test]
    fn owner_and_configuration_changes_never_reuse_a_cached_report() {
        let mut collector = collector(fake);
        collector
            .request(input(key(1, 1), "private"), true)
            .expect("first");
        wait(&mut collector).result.expect("report");
        for next in [key(2, 1), key(2, 2)] {
            collector.select(next);
            assert!(collector.cached().is_none());
            let ticket = collector
                .request(input(next, "current"), false)
                .expect("request");
            let completion = wait(&mut collector);
            assert_eq!(completion.key, next);
            assert_eq!(completion.ticket, ticket);
        }
    }

    #[test]
    fn stale_inflight_owner_is_discarded_before_current_owner_runs() {
        let (release, releases) = mpsc::channel();
        let mut collector = collector(move |job| {
            let _ = releases.recv();
            fake(job)
        });
        collector
            .request(input(key(1, 1), "old owner"), true)
            .expect("old");
        let latest = collector
            .request(input(key(2, 1), "new owner"), true)
            .expect("new");
        release.send(()).expect("old completes");
        let deadline = Instant::now() + Duration::from_secs(2);
        while collector.active != Some(latest) {
            assert!(collector.poll().is_none());
            assert!(collector.cached().is_none());
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        release.send(()).expect("new completes");
        assert_eq!(
            wait(&mut collector).result.expect("report").snapshot["marker"],
            "new owner"
        );
    }

    #[test]
    fn ipc_reads_share_pending_work_and_refresh_expired_cache_without_losing_it() {
        let mut collector = collector(fake);
        let first = collector
            .request(input(key(1, 1), "first"), false)
            .expect("first");
        assert_eq!(
            collector
                .request(input(key(1, 1), "duplicate"), false)
                .expect("join"),
            first
        );
        wait(&mut collector).result.expect("report");
        assert_eq!(
            collector
                .request(input(key(1, 1), "cached"), false)
                .expect("cache"),
            first
        );
        assert!(!collector.is_collecting());
        collector.cached.as_mut().expect("cached").1.collected = Instant::now()
            .checked_sub(CACHE_LIFETIME)
            .expect("cache lifetime fits monotonic clock");
        let next = collector
            .request(input(key(1, 1), "refreshed"), false)
            .expect("refresh");
        assert!(next > first);
        assert!(collector.cached().is_some());
        assert_eq!(
            wait(&mut collector).result.expect("report").snapshot["marker"],
            "refreshed"
        );
    }

    #[test]
    fn worker_failure_is_reported_once_and_explicit_retry_recovers() {
        let mut collector = collector(|job| ReportCompletion {
            ticket: job.ticket,
            key: job.input.key,
            result: Err("fixture read failure".into()),
        });
        let ticket = collector
            .request(input(key(1, 1), "failure"), true)
            .expect("start");
        let result = wait(&mut collector);
        assert_eq!(result.ticket, ticket);
        assert!(result.result.is_err());
        assert_eq!(collector.failure(), Some("fixture read failure"));
        assert!(!collector.is_collecting());
        assert!(collector.poll().is_none());
        if let Some(worker) = collector.worker.take() {
            worker.detach();
        }
        collector.worker =
            Some(SingleFlightWorker::spawn("diagnostics-retry", fake).expect("retry worker"));
        collector
            .request(input(key(1, 1), "retry"), true)
            .expect("retry");
        assert_eq!(
            wait(&mut collector).result.expect("report").snapshot["marker"],
            "retry"
        );
        assert!(collector.failure().is_none());
    }

    #[test]
    fn dropping_a_collector_does_not_wait_for_its_blocked_read() {
        let (release, releases) = mpsc::channel();
        let (started, starts) = mpsc::channel();
        let mut collector = collector(move |job| {
            let _ = started.send(());
            let _ = releases.recv();
            fake(job)
        });
        collector
            .request(input(key(1, 1), "blocked"), true)
            .expect("request");
        starts
            .recv_timeout(Duration::from_secs(1))
            .expect("started");
        let (dropped, drops) = mpsc::channel();
        let dropping = thread::spawn(move || {
            drop(collector);
            let _ = dropped.send(());
        });
        let finished = drops.recv_timeout(Duration::from_secs(1)).is_ok();
        let _ = release.send(());
        dropping.join().expect("drop thread");
        assert!(finished, "drop blocked on worker I/O");
    }

    #[test]
    fn report_limits_and_atomic_fact_replacement_preserve_unknown_states() {
        assert!(
            CollectedReport::from_snapshot(
                json!({"oversize":"x".repeat(MAX_DIAGNOSTICS_EXPORT_BYTES)}),
                false
            )
            .is_err()
        );
        let mut snapshot =
            json!({"capabilities":{"network_blocking":{"status":"available","old":true}}});
        merge_live(
            &mut snapshot,
            &json!({"capabilities":{"network_blocking":{"status":"unavailable"}}}),
        );
        assert_eq!(
            snapshot["capabilities"]["network_blocking"],
            json!({"status":"unavailable"})
        );
        let pending = input(key(1, 1), "pending").pending_snapshot();
        assert_eq!(pending["runtime"]["portal"]["status"], "not-tested");
        assert_eq!(pending["storage"]["health"]["status"], "not-tested");
        assert_eq!(pending["collection"]["status"], "pending");
    }
}
