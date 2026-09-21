# Safe-mode recovery evidence

Date: 2026-09-16  
Status: in-progress

## FAIL-004 recovery mode

`rustbrowser --safe-mode` is a GUI-only diagnostic startup mode. It rejects
`--basedir`, `--config`, `--profile`, `--context`, and `--set` so a normal
profile cannot be accidentally selected or modified. The mode uses the same
disposable storage-root path as temporary startup, loads built-in
configuration, and leaves the normal Qt/Chromium security argument validation
in place.

Page userscripts are disabled at both the live page-world injection and
persistent `WebEngineScript` installation boundaries. The browser status bar
identifies the session as `SAFE MODE (temporary; userscripts off)`.

## Verification

```text
cargo test -p rustbrowser --locked --offline
cargo xtask check
cargo fmt --all -- --check
QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --safe-mode
QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --userscripts-off --temp-basedir
```

The CLI unit tests cover safe-mode parsing and built-in configuration
selection. The workspace build and tests pass, and the Wayland smoke remains
alive until the intentional timeout without QML load errors.
The separately named `--userscripts-off` launch also remains alive through the
same native smoke path and keeps the normal-profile storage/configuration mode.

## Remaining qualification

Safe mode is intentionally disposable and does not reset or open an existing
profile. A separately named mode for launching an existing profile with only
userscripts disabled is available as `--userscripts-off`. It keeps the selected
normal profile, configuration, storage, and session state, while disabling both
page-world and sub-frame page userscript injection. The status bar identifies
the mode as `USERSCRIPTS OFF`; it is GUI-only and cannot be combined with
`--safe-mode`.
