# M0-155 — Configuration editor lifecycle

Implemented the missing `config-edit` slice and aligned the external-editor
configuration with the specification:

- Added the typed `tools.editor` direct-argv schema, with an empty value meaning
  unconfigured and exactly one complete `{file}` argument required.
- Removed implicit `EDITOR`/`VISUAL`/`vi` fallback behavior and shell expansion.
- Added `config-edit` to the core registry, CLI, Qt IPC, and local execution
  paths.
- Added managed child-process ownership, cancellation cleanup, exit-status
  reporting, and watcher-driven atomic configuration reload after successful
  editing; ordinary watcher application is deferred while the editor is open
  so a partial save cannot become an active revision.
- Captures up to 64 KiB of editor stderr asynchronously and includes sanitized
  diagnostics in the completion status without blocking the Qt thread.
- Kept `edit-text` on the same launcher while preserving its separate temporary
  file and page write-back flow.

Evidence:

- `cargo fmt --all`
- `cargo test -p browser-config --locked --offline` — 31 passed
- `cargo test -p browser-core --locked --offline` — 59 passed
- CLI and Qt typed lifecycle tests passed.
- Qt launcher test passed for complete `{file}` substitution and rejection of
  embedded placeholders.
- Qt source regression coverage verifies the bounded asynchronous stderr path
  for configuration editors.
- `cargo xtask check --locked` — all workspace tests and doc-tests passed
  (browser_config 31, browser_core 59, browser_engine_qt 118, browser_ipc 9,
  browser_storage 48, rustbrowser 45, xtask 4).
- Native smoke: `QT_QPA_PLATFORM=wayland timeout 8s target/debug/rustbrowser
  --temp-basedir` exited with expected status 124 and produced no
  `error|panic|failed|assert` matches.
