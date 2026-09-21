# M0-104: bounded profile metadata worker

Requirements: STATE-005, STORE-005, SWITCH-006

The durable profile library used by the switcher now has a dedicated
`ProfileStoreWorker`. The worker owns its SQLite connection on a named
background thread and accepts one bounded library request at a time. Its
request and response channels each have capacity one; overlapping requests
return a typed busy result instead of growing a queue. History, bookmarks,
quickmarks, bounded download metadata, and the capped durable permission-rule
set are copied into a revisioned
`BrowserUi` snapshot before the Qt switcher composes results.

The QML poller checks each registered browser window continuously for normal
profiles; completed responses increment the snapshot revision and refresh only
visible consumers without blocking the GUI thread. Durable navigation and
library/download/permission mutations mark the snapshot stale, so the next
consumer refresh queues one replacement read. Private and ephemeral profiles
never start this worker because they have no durable metadata store. Worker
ownership is released before the profile store and profile lock are torn down.

Evidence:

- `cargo test -p browser-storage -p browser-engine-qt --locked --offline`
  (64 storage tests, 167 Qt-engine tests)
- `cargo fmt --all`
- Storage worker unit tests verify off-thread reads, session-name enumeration,
  session restore-plan validation (including crash-recovery checkpoint
  discovery), atomic session snapshot saves, confirmation-gated named-session
  deletion, and stale current-checkpoint cleanup,
  single-flight backpressure, and bounded result delivery.

The current slice covers the bounded snapshot and its switcher, download,
read-only library-command, permission, site-status, and session-catalog
consumers. Durable journey-node lookup for `journey-reopen` and bounded
current/search/expand journey queries are also worker requests; returned nodes
are revalidated against the captured profile before navigation. Session-name
enumeration, crash-recovery checkpoint discovery and
parsing, atomic named/current-session writes, confirmation-gated named-session
deletion, and stale current-checkpoint cleanup are now bounded worker requests;
the Qt-side cache is invalidated by successful named-session save/delete
operations. Retention cleanup is performed as part of bounded history and
journey transactions; migrations remain an open startup/recovery
qualification area. High-volume committed-navigation history writes are covered by
`m0-109-history-write-worker.md`. Full multi-window and 100,000-row latency
qualification also remain open.

Download controls now resolve their durable record from the worker-owned
library snapshot. History, permission, and download metadata queues no longer
fall back to Qt-thread SQLite writes when the bounded worker is unavailable or
backpressured; in-flight work is retained for retry and queue saturation is
reported explicitly. Bookmark/quickmark mutations use the same bounded
queued/in-flight retry boundary, including when the worker disconnects.

If a normal-profile worker disconnects, the Qt adapter recreates it from the
validated profile database path and retries retained requests without opening
SQLite on the GUI thread.

Durable journey export preview data is also loaded by the profile worker with
bounded node and edge limits; the Qt thread only serializes the returned
snapshot and performs the explicit local file write.

The final SQLite WAL checkpoint used by orderly shutdown is also queued and
acknowledged on the metadata worker; the Qt thread no longer calls the store's
checkpoint API directly. Shutdown now drains bookmark/quickmark mutations
before requesting that final checkpoint, so a queued mark write cannot be
overtaken by the flush request; a failed mark acknowledgement now fails the
durable shutdown stage instead of being silently treated as flushed.
