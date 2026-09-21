# M0-106: worker-backed external downloads query

Requirements: STATE-005, STORE-005, IPC-003

The external `downloads.query` IPC method now consumes the durable download
records from the profile-library snapshot owned by the bounded storage worker.
The first request for a durable profile queues a single-flight read and returns
`E_BUSY` until the QML poller has delivered the snapshot. Once ready, the
response includes the sanitized download fields, a `ready` marker, and the
snapshot revision. A worker failure is reported as `E_ENGINE`; it is never
represented as an empty successful durable result.

Private and ephemeral profiles continue to return an explicitly marked empty
result without creating a durable worker or database record.

The storage poller now runs for every normal profile window, so an IPC query
made without a visible library surface still allows the pending worker response
to be consumed. Its UI refresh work remains conditional on the affected
surface being visible.

Evidence:

- `cargo fmt --all -- --check`
- `cargo test -p browser-engine-qt --locked --offline` (83 tests)
- Existing workspace `cargo xtask check --locked` evidence remains valid for
  the preceding snapshot-consumer slice.

Direct library-command reads, permission queries, session enumeration, and
worker-backed writes remain subsequent `STATE-005` migration slices.
