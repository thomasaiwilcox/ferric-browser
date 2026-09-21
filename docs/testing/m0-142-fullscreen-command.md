# M0-142 fullscreen command

Date: 2026-09-17  
Status: verified

## Scope

`fullscreen [on|off|toggle]` is now part of the shared command registry and
typed CLI/IPC contract. The Rust command boundary emits a window-scoped
fullscreen action with a validated state; the QML window applies it through
the public `showFullScreen()`/`showNormal()` boundary.

Page fullscreen requests are accepted only through the same owning window and
use the same native fullscreen boundary for primary and secondary windows.
The reserved `Ctrl+Shift+Escape` shortcut exits native fullscreen before
falling back to the browser mode escape transition.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p browser-core --locked --offline` — 57 tests
- `cargo test -p rustbrowser --locked --offline` — 36 tests
- `cargo test -p browser-engine-qt --locked --offline` — 107 tests
- `cargo xtask check --locked`
- Native Wayland startup smoke reached the event loop without diagnostics
  errors
