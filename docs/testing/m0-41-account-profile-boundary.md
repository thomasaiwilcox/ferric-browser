# M0-41 account and profile boundary

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: AUTH-001.
- Files: `crates/browser-engine-qt/qml/Main.qml` and
  `crates/browser-engine-qt/src/lib.rs`.
- Observable result: all tabs in a browser window use that window's one
  `WebEngineProfile`; engine-created popups inherit the opener's profile and
  private-mode state. Normal and private profiles use distinct storage
  configuration, with private profiles retaining no persistent storage name.

Authentication remains in the engine's native request path. This boundary does
not introduce a second cookie store, copy cookies into Rust, or allow a
hostname rule to switch a popup's profile during an auth redirect.

## Verification

```text
cargo fmt --all -- --check
cargo test -p browser-engine-qt --locked
cargo xtask check
cargo build -p rustbrowser --locked
QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir
```

Same-profile local popup/redirect and authenticated restart behavior, plus
multi-profile cookie isolation, still require authorized disposable-account
qualification. The project does not use developer browser cookies.
