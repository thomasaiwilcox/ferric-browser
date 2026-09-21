# M0-105: snapshot-backed switcher consumers

Requirements: STATE-005, SWITCH-004, SWITCH-006

Switcher activation no longer performs a second synchronous SQLite lookup after
the user selects a history, bookmark, or quickmark result. It resolves the
selected identifier against the same revisioned library snapshot that produced
the result. If a mutation has made that snapshot stale, activation reports a
bounded retry message while the worker refreshes it; if the initial read has
not completed, activation does not guess from a different record.

Command-line completion also consumes the profile-library snapshot rather than
querying SQLite on every keystroke. Entering command mode queues the existing
bounded worker request, and the QML poll refreshes completion when the response
arrives. Tab completions remain immediately available while durable metadata is
pending. The native downloads manager, its open/reveal action, and the external
`downloads.query` IPC method follow the same pattern. An external query returns
`E_BUSY` while the snapshot is loading instead of returning an incomplete
durable result. Private and ephemeral profiles continue to omit the durable
snapshot entirely.

Evidence:

- `cargo test -p ferric-browser-storage -p ferric-browser-engine-qt --locked --offline`
- `cargo xtask check --locked`
- `cargo build -p ferric-browser --locked`
- Native Wayland startup smoke reached the event loop for eight seconds with
  no application-owned QML warnings.

Permission queries, session enumeration, direct library-command writes, and
worker-backed writes remain subsequent `STATE-005` migration slices. Read-only
history, bookmark, and quickmark commands are covered in
`docs/testing/m0-107-library-command-snapshots.md`.
