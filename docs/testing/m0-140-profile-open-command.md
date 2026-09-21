# M0-140 profile-open command

Date: 2026-09-17  
Status: verified

## Scope

`profile-open NAME [URL]` is now present in the shared command registry and
is accepted consistently by interactive dispatch, typed IPC, and the CLI
forwarding path. Rust validates that the named durable profile exists, resolves
the optional URL through the normal navigation policy, and emits a bounded
profile-window action.

The QML window registry records profile names so the action focuses an
existing normal window for that profile when one is already open. Otherwise it
creates a normal secondary window with the profile's durable label and the
requested safe URL. The profile manager continues to use the same focus-or-
create path.

Private windows may use this command only with an explicit safe URL. That
path reads the normal profile registry without opening private storage and
reopens only the supplied URL in the selected durable profile; it never
transfers private history, permissions, session state, or marks. An omitted
URL is rejected in a transient profile, and unknown profile names are rejected
without creating storage or a window.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p browser-core --locked --offline` — 57 tests
- `cargo test -p rustbrowser --locked --offline` — 34 tests
- `cargo test -p browser-engine-qt --locked --offline` — 105 tests
