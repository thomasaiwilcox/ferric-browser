# M0-149 — Bounded history traversal counts

Implemented the V1 back/forward count contract.

- back and forward accept an optional --count from 1 through 100.
- Core dispatch validates the option and emits one typed history event with
  the requested signed offset.
- CLI and IPC preserve the count as a native integer rather than reparsing
  command text.
- Qt action generation carries the count to QML, which performs bounded
  native WebEngine steps and stops at the actual history boundary.
- Primary and secondary navigation buttons report a boundary without issuing
  an invalid engine operation.

Evidence:

- cargo fmt --all -- --check
- cargo test -p ferric-browser-core --locked --offline: 59 passed
- cargo test -p ferric-browser --locked --offline: 43 passed
- cargo test -p ferric-browser-engine-qt --locked --offline: 114 passed
- cargo xtask check --locked
- Wayland startup smoke with --temp-basedir
