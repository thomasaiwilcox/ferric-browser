# M0-112: history clear worker

Confirmed `history-clear [--since UNIX_SECONDS] [--origin EXACT_HTTP_ORIGIN]
--confirm` mutations now execute on the profile SQLite worker. The command
returns a queued result without waiting on SQLite in the Qt callback; the
poller reports the committed deleted-entry count, preserving explicit
confirmation and transactional storage semantics. A late completion is
drained during storage polling or shutdown, and worker failures are surfaced
without falling back to a GUI-thread database write.
The origin filter is canonicalized at the Qt command boundary and matched at
the exact origin/path delimiter in SQLite.

Evidence:

- `cargo test -p browser-storage worker_clears_history_before_acknowledging --locked --offline`
- `cargo check -p browser-engine-qt --locked --offline`
- `cargo xtask check --locked`
- `cargo build -p rustbrowser --locked --offline`
- `QT_QPA_PLATFORM=wayland timeout 8s target/debug/rustbrowser --temp-basedir`
  (expected timeout, no lingering process)
