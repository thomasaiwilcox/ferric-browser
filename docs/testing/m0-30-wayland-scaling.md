# M0-30 Wayland scaling evidence

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: WL-002.
- Files: `crates/browser-engine-qt/qml/Main.qml`.
- Observable result: the browser uses Qt Quick logical coordinates and reads
  the attached screen's `Screen.devicePixelRatio` and
  `Screen.logicalPixelDensity` for diagnostics only. Browser-owned overlay
  dimensions grow from the active font metrics and are bounded by the
  available window, so large configured chrome text does not rely on fixed
  physical-pixel heights. Popup hit areas fill their scaled delegates.

Multiplying the device-pixel ratio into layout coordinates is intentionally
avoided because Qt already performs high-DPI logical-pixel conversion. The
same root font is inherited by secondary and popup windows.

## Verification

```text
cargo fmt --all -- --check
cargo xtask check
cargo build -p rustbrowser --locked
QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir
```

The smoke is expected to end with timeout status 124 after staying alive with
an empty startup log. Actual 100%, 125%, 150%, and 200% output testing,
mixed-DPI monitor movement/output removal, page zoom, hint geometry, cursor
placement, and popup hit testing remain required native qualification.
