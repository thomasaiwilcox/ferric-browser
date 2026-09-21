# M0-139 profile-aware windows

Date: 2026-09-17  
Status: verified

## Scope

Secondary windows now rebuild their effective configuration for the selected
profile instead of inheriting the primary window's already-resolved values.
The engine composes the authored base configuration, the selected
`profiles.toml` definition, that profile's durable runtime overrides, CLI
overrides, and temporary overrides in the documented order before applying
theme, blocking, and other profile-sensitive behavior.

The profile manager's existing “Open window” action therefore gives each
normal profile window its own typed profile layer and profile-local runtime
state. Configuration reloads re-read the selected profile definition and
rebuild the active window's effective configuration transactionally. Private,
ephemeral, and temporary windows keep their memory-only override behavior.

## Verification

- `cargo fmt --all -- --check`
- `cargo test -p ferric-browser-config --locked --offline` — 30 tests
- `cargo test -p ferric-browser-engine-qt --locked --offline` — 104 tests
- `cargo test -p ferric-browser --locked --offline` — 33 tests
- `cargo xtask check --locked`
- `QT_QPA_PLATFORM=wayland timeout 8s target/debug/ferric-browser --temp-basedir`
  — expected timeout after reaching the event loop; no error, panic, failed,
  or assertion diagnostics were emitted
