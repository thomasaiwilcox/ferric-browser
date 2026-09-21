# M0-108: snapshot-backed permission consumers

Requirements: STATE-005, STORE-005, IPC-003

The bounded profile-library snapshot now includes the storage layer's capped
durable permission rules. `permissions.query`, the `permissions` command, and
native permission-decision lookup consume that snapshot instead of performing
synchronous Qt-thread SQLite reads. Origin filtering still uses the same exact
normalized-origin rules, and the existing precedence remains configuration,
durable site rule, session grant, then capability default.

Durable permission remember and reset operations now queue bounded atomic
requests on the same SQLite-owning worker. Combined camera/microphone resets
are committed in one SQLite transaction. The worker acknowledges completion,
and the browser invalidates
the snapshot, and drain during the bounded shutdown flush. If the worker is
unavailable, the browser retains the rules for retry and reports the worker
failure without a Qt-thread SQLite fallback. A permission decision does not consult the stale
snapshot during that refresh; it falls through to the safe session/config
default until the worker installs the new revision. IPC permission queries
return `E_BUSY` while the snapshot is loading and `E_ENGINE` if the worker is
unavailable.

Evidence:

- `cargo fmt --all -- --check`
- `cargo test -p browser-storage -p browser-engine-qt --locked --offline`
  (64 storage tests, 167 Qt tests)

The later worker-backed write and journey/session query families remain.
