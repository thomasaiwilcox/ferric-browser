# M0-151 — Typed tab traversal counts

Completed the documented `tab-next` and `tab-prev` count contract.

- The command registry exposes an optional bounded integer count with examples.
- Core dispatch accepts `--count N` from 1 through 100 and activates the
  corresponding live tab with wraparound semantics.
- CLI and Qt typed IPC preserve the count as a native integer and reject
  out-of-range values.

Evidence:

- cargo fmt --all -- --check
- cargo test -p ferric-browser-core --locked --offline: 59 passed
- cargo test -p ferric-browser --locked --offline: 43 passed
- cargo test -p ferric-browser-engine-qt --locked --offline history_commands_use_typed_bounded_counts
