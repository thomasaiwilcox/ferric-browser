# M0-109: worker-backed committed-navigation history writes

Requirements: STATE-005, STATE-009, STORE-005

Committed navigation history now uses a bounded write path owned by the same
profile metadata worker used for library snapshots. `BrowserUi` queues at most
1,024 pending visits, submits batches of at most 128 records, and tracks one
in-flight batch. Each batch is committed atomically by `ProfileStore` on the
worker thread; the Qt poller removes entries only after the worker acknowledges
the commit. Queue-full and temporary worker backpressure retain the entries,
while worker failure falls back to the main store's atomic batch API.

The library snapshot is marked stale as soon as a visit is queued and remains
stale while pending or in-flight writes exist. An older read response therefore
cannot make history consumers appear current after a navigation. Normal UI
polling remains non-blocking. During durable shutdown, the final flush waits up
to two seconds for the bounded history queue to drain, retries worker failures
through the fallback path, and refuses to report a successful flush when
entries remain unresolved.

The same queue also records safe same-document URL changes, including fragment
and history-API changes observed after the full navigation has settled. A URL
change belonging to a pending full load is excluded so one navigation cannot be
recorded twice; same-document changes do not create duplicate journey nodes.

Evidence:

- `cargo fmt --all -- --check`
- `cargo test -p ferric-browser-storage -p ferric-browser-engine-qt --locked --offline`
  (39 storage tests, 83 Qt-engine tests)
- The storage worker test exercises the bounded atomic batch and its explicit
  shutdown wait/acknowledgement path.
- The Qt-engine test covers recording a settled same-document change while
  suppressing the matching callback during a pending full load.

Lower-volume bookmark, quickmark, permission, download, journey, retention,
and session writes still use synchronous adapters pending later worker slices.
