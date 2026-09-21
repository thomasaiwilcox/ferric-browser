# M0-154 — Configuration lifecycle commands

Implemented and verified the first explicit configuration lifecycle command
slice:

- `config-check` queues validation of the configured file and adjacent
  contexts/profiles files on the bounded configuration worker without applying
  changes, then reports the result through the GUI polling path.
- `config-reload` drives the debounced transactional reload path; file loading,
  include expansion, validation, and profile-layer parsing run on a bounded
  single-flight worker, while Qt commits the candidate atomically and
  preserves the last known good configuration when reload is rejected.
- `config-export <PATH>` and `config-write-defaults <PATH>` queue mode-restricted
  writes on a bounded single-flight worker, report completion or failure
  through the configuration poll, and use collision-safe create-new semantics.
- `theme-reload` refreshes the palette and provider status without page reloads
  or capture restarts.
- Core registry metadata and typed CLI/Qt IPC adapters cover all four commands.

Evidence:

- `cargo fmt --all`
- `cargo clippy -p browser-engine-qt --all-targets --locked --offline -- -D warnings`
- `cargo test -p browser-core --locked --offline` — 59 passed
- `cargo test -p rustbrowser --locked --offline cli_configuration_lifecycle_commands_use_typed_paths_and_no_arguments` — passed
- `cargo test -p browser-engine-qt --locked --offline ipc_configuration_lifecycle_commands_use_typed_paths_and_no_arguments` — passed
- `cargo test -p browser-engine-qt --locked --offline config_write_worker_creates_private_non_overwriting_file` — passed
- `cargo test -p browser-engine-qt --locked --offline config_reload_worker_loads_and_validates_off_thread` — passed (reload and check operations)
- `cargo xtask check --locked` — all workspace tests and doc-tests passed
  (browser_config 30, browser_core 59, browser_engine_qt 117, browser_ipc 9,
  browser_storage 48, rustbrowser 45, xtask 4)
- Native smoke: `QT_QPA_PLATFORM=wayland timeout 8s target/debug/rustbrowser
  --temp-basedir` exited with expected status 124 and produced no
  `error|panic|failed|assert` matches.
