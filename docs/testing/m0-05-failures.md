# Renderer failure evidence

Date: 2026-09-16  
Status: in-progress

## FAIL-001 renderer crash

`WebEngineView.onRenderProcessTerminated` is connected for primary tabs,
secondary windows, and gesture-approved popups. Abnormal, crashed, and killed
renderer statuses mark only the affected live view and retain a bounded
per-view count for failures within 60 seconds. The count is used to warn that
repeated failures will not trigger an automatic reload loop; recovery is always
explicit.

The termination callback also crosses the public Rust/Qt boundary through
`note_renderer_process_terminated`, where the captured live tab target is
revalidated and reduced as `RendererTerminated`. An explicit reload, new
navigation, or committed recovery navigation restores the core renderer state
to healthy; stale callbacks cannot change a later document.

The affected window gets a browser-owned error surface with:

- a safe, redacted URL and termination reason/exit code;
- explicit Reload and Close tab/window actions;
- Copy safe URL and Site Ledger diagnostics actions; and
- a visible warning when repeated crashes are detected.

Healthy tabs remain usable. A failed background primary tab receives a
`renderer failed` tab badge and its recovery surface appears when that tab is
selected. Navigation, explicit reload, and view destruction clear the live
failure surface; the renderer failure count remains bounded state on the view.

## Verification

```text
cargo xtask check
cargo fmt --all -- --check
QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir
```

The workspace test suite, reducer regression, generated Qt/QML build, and
offscreen adapter smoke pass. The native Wayland smoke remains alive until the
intentional timeout with no QML load errors.

The deterministic `cargo xtask check` gate also validates the renderer-failure
contract: crashed/killed/abnormal statuses are named, the callback records a
bounded 60-second repeat count, stale-target recovery is routed through Rust,
and the termination callback itself cannot auto-reload a failed view.

## Remaining qualification

There is not yet a deterministic renderer fault-injection fixture, so direct
assertion of the termination signal, repeated-crash counter, and shared
renderer attribution remains outstanding. Process-wide session recovery and
GPU fallback are separate requirements.
