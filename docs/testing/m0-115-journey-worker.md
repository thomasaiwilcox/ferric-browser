# M0-115: journey mutation worker

Durable journey node creation and traversal-current updates now run through
the profile SQLite worker. Node insertion keeps its existing transactional
parent-edge, current-pointer, retention, and bounded-field semantics. Qt
callbacks enqueue writes into a bounded single-flight queue without waiting on
SQLite; the poller consumes each worker acknowledgement and preserves the
core-to-durable node mapping needed by later popup, background-link, redirect,
and branch commits. Worker failures are surfaced without a GUI-thread
fallback, while private and ephemeral profiles remain memory-only.

Evidence:

- `cargo test -p browser-storage worker_commits_journey_node_and_current_pointer_before_acknowledging --locked --offline`
- `cargo check -p browser-engine-qt --locked --offline`
- `cargo xtask check --locked`
- `cargo build -p rustbrowser --locked --offline`
- `QT_QPA_PLATFORM=wayland timeout 8s target/debug/rustbrowser --temp-basedir`
  (expected timeout, no lingering process)
