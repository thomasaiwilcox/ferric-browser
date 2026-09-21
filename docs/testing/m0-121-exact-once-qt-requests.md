# Exact-once Qt request resolution

Date: 2026-09-17  
Status: in-progress

## Task card

- Requirement: STATE-007.
- Files: `crates/browser-engine-qt/qml/Main.qml` and
  `crates/browser-engine-qt/src/lib.rs`.
- Observable result: terminal Qt request outcomes pass through one bounded
  resolver, and duplicate, stale, and error outcomes are visible in runtime
  diagnostics.

`Main.qml` keeps a bounded 256-entry object-identity ledger. A request is
resolved at most once through `resolveQtRequest`; invalid or already-seen
requests become no-ops. The resolver covers permissions, file dialogs,
page/auth dialogs, client certificates, WebAuthn cancellation, desktop media,
and downloads across the primary, popup, and secondary-window paths.

Each attempt is classified as `resolved`, `stale`, `duplicate`, or `error`.
QML keeps the live counters for the UI, while Rust receives bounded category
counters through `record_request_resolution` and exposes them in the
`request_resolutions` diagnostics object. The ledger is intentionally
in-memory and bounded; Qt request object lifetime and compositor/portal
qualification still require native prompt evidence beyond static and startup
coverage.

## Verification

```text
cargo test -p browser-engine-qt --locked --offline
cargo xtask check --locked
QT_QPA_PLATFORM=wayland timeout 8s target/debug/rustbrowser --temp-basedir
target/debug/rustbrowser diagnostics --format json
```
