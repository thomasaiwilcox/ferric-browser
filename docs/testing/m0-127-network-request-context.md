# Network request-context semantics

Date: 2026-09-17  
Status: in-progress

## Task card

- Requirement: NET-003.
- Files: `crates/ferric-browser-engine-qt/src/request_interceptor.h`,
  `crates/ferric-browser-engine-qt/src/request_interceptor.cpp`,
  `crates/ferric-browser-engine-qt/src/lib.rs`, and
  `crates/ferric-browser-engine-qt/qml/Main.qml`.
- Observable result: the synchronous interceptor consumes Qt's engine-provided
  resource type, navigation type, first-party URL, and initiator URL. It
  labels top-level, subframe, redirect, and resource requests without exposing
  paths or query data; it never uses the resource host as a substitute
  first-party site for subresource bypass decisions.

Top-level requests use their own host as the site context, while subresource
requests with missing first-party or initiator context remain ordinary
adblocking decisions but increment a bounded `unknown_context_requests`
diagnostic counter. Exact-site bypasses require a known first-party host.
Blocked and exception decision records retain bounded context labels and a
`context_known` flag. No security-sensitive deny rule currently depends on
this ordinary host-filter path, so unknown context is not silently promoted to
a security allow or guessed from focused-tab state.

## Verification

```text
cargo test -p ferric-browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p ferric-browser --locked --offline
target/debug/ferric-browser diagnostics --format json
QT_QPA_PLATFORM=wayland timeout 8s target/debug/ferric-browser --temp-basedir
```
