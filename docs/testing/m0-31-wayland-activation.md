# M0-31 Wayland activation evidence

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: WL-004.
- Files: `crates/ferric-browser-engine-qt/qml/Main.qml`.
- Observable result: switcher activation calls `show`, `raise`, and Qt's
  `requestActivate()` once for the exact owning window. If the window is not
  active immediately, the browser performs one bounded 250 ms observation and
  reports `unknown` when the compositor does not expose an active result. It
  does not synthesize activation tokens, use X11 window IDs, or retry focus.

Background-tab opens continue through reducer state without calling the window
activation helper. Successful activation restores focus inside the activated
window only after Qt reports it active.

## Verification

```text
cargo fmt --all -- --check
cargo test -p ferric-browser-engine-qt -p ferric-browser-core --locked
cargo xtask check
cargo build -p ferric-browser --locked
QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir
```

The smoke is expected to end with timeout status 124 after remaining alive
with an empty startup log. Desktop-launch activation-token propagation,
mixed-workspace targeting, and live compositor denial evidence remain native
qualification work. External `window.focus` IPC now returns an operation ID;
the browser-owned Qt path completes that operation only after immediate
activation or the bounded observation timer, retaining `unknown` when the
compositor does not expose a positive result and reporting `stale` for
destroyed targets.
