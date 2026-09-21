# M0-111: bookmark and quickmark write worker

Bookmark and quickmark mutations now use the profile SQLite worker rather than
opening the database from the Qt command path. Each command submits one
validated worker-owned mutation and waits for its bounded acknowledgement
before reporting `added`, `edited`, or `deleted`, preserving durable-success
semantics.
Delete operations still fail when the requested entry is stale or absent, and
the worker has a SQLite transaction boundary for the mutation.

If the worker has stopped, the existing bounded fallback reports storage
failure or applies the mutation through the already-open normal-profile store;
private profiles remain ineligible for durable marks. Snapshot consumers are
marked dirty only after a committed acknowledgement.

Edit mutations use the same stale-target-safe transaction as add/delete, so a
bookmark title or quickmark destination cannot be acknowledged unless the
target row was updated.

Evidence:

- `cargo test -p browser-storage mark --locked --offline` (5 focused tests)
- `cargo check -p browser-engine-qt --locked --offline`
- `cargo xtask check --locked`
- `cargo build -p rustbrowser --locked --offline`
- `QT_QPA_PLATFORM=wayland timeout 8s target/debug/rustbrowser --temp-basedir`
  (expected timeout, no lingering process)
