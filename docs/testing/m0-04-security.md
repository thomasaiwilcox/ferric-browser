# M0-04 engine security defaults evidence

Date: 2026-09-15  
Status: in-progress

## Task card

- Requirement: ENGINE-004.
- Files: `crates/rustbrowser/src/main.rs`,
  `crates/browser-engine-qt/src/diagnostics.rs`, and
  `crates/browser-engine-qt/qml/Main.qml`.
- Observable result: production GUI startup refuses UID 0 and refuses known
  Chromium-disabling flags from `QTWEBENGINE_CHROMIUM_FLAGS` or
  `QTWEBENGINE_FLAGS`: `--no-sandbox`, `--single-process`, and
  `--disable-web-security`. The application does not set any of those flags,
  so Qt WebEngine's normal security defaults remain in force.

## Evidence

- `cargo test --workspace --locked --offline` passes.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`
  passes.
- `cargo xtask check` passes.
- The active Wayland startup smoke completes with no QML/runtime diagnostics.

## Limitations

Renderer-helper startup/fault and site-isolation qualification, certificate/
mixed-content fixture coverage, GPU/process health reporting, and a separate
diagnostic build for intentional overrides remain required before this
requirement can be complete. Diagnostics now perform a bounded direct-child
renderer-helper sandbox probe when a QtWebEngine helper is present.
