# M0-36 screen capture ownership and visibility

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirements: MEDIA-001 and MEDIA-002.
- Files: `crates/browser-engine-qt/qml/Main.qml` and
  `crates/browser-engine-qt/src/lib.rs`.
- Observable result: Qt WebEngine desktop-media requests remain on the
  engine-owned source-selection path. The browser presents a bounded,
  host-scoped capture indicator after a screen or window is selected, showing
  the requesting origin and an active/ended state.

The indicator is created for primary, secondary, and approved popup windows.
It is cleared on view/window destruction, marked ended on navigation, and does
not persist source consent. The Stop action requests termination through the
documented Qt Quick boundary by reloading the owning view; no separate
PipeWire stream or browser-side capture pipeline is created.

## Verification

```text
cargo fmt --all -- --check
cargo test -p browser-engine-qt --locked
cargo xtask check
cargo build -p rustbrowser --locked
QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir
```

The automated checks cover the QML ownership and lifecycle boundary. Live
screen/window selection, Qt/Chromium portal mediation, PipeWire teardown,
duplicate-prompt behavior, and physical indicator placement remain manual
Wayland qualification work. Qt Quick does not expose a direct active-stream
stop method here, so stop is intentionally implemented as a reload boundary.
