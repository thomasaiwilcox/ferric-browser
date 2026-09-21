# M0-23 internal documents evidence

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: SEC-003.
- Files: `crates/browser-engine-qt/src/url_display.cpp`,
  `crates/browser-engine-qt/src/url_display.h`,
  `crates/browser-engine-qt/src/lib.rs`,
  `crates/rustbrowser/src/main.rs`, and `crates/browser-core/src/url.rs`.
- Observable result: the internal-document namespace is reserved before Qt
  application and WebEngine object creation, while browser URL policy keeps
  it unreachable from normal page navigation.

## Evidence

- `rustbrowser_register_internal_scheme()` is called at the start of
  `run_gui()`, before `QGuiApplication::new()`, WebEngine profiles, or views
  are created.
- Qt registers `rb` with `HostAndPort` syntax and only the `SecureScheme`
  flag. It does not enable local access, CORS, service workers, or other broad
  file/secure-origin privileges.
- No URL handler is installed. Management views remain native QML surfaces;
  an `rb://` navigation cannot dispatch into privileged Rust/IPC actions.
- The Rust and Qt URL boundaries reject `rb://settings`, `javascript:`, and
  other unsupported schemes before navigation state is updated.

## Limitations

The fixed bundled-asset handler, escaped read-only data documents, strict CSP
fixtures, remote-page attack fixture, and DevTools access qualification remain
future work. This slice deliberately reserves the scheme without exposing a
privileged document renderer.

## Verification

```text
cargo test -p browser-core --locked
cargo test -p browser-engine-qt --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo xtask check
QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir
```
