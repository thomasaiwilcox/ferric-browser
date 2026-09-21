# M0-26 page authority boundary evidence

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: SEC-002.
- Files: `crates/browser-engine-qt/qml/Main.qml`,
  `crates/browser-engine-qt/src/lib.rs`,
  `crates/rustbrowser/src/main.rs`, and `crates/browser-ipc/src/lib.rs`.
- Observable result: ordinary page JavaScript has no browser-owned native
  authority object or WebChannel route.

## Evidence

- The QML application creates native `BrowserUi` objects for Qt Quick only;
  the WebEngine views do not configure `WebChannel` or expose a QML context
  property to page content.
- Browser-owned focus and site-data operations use Qt's
  `WebEngineScript.ApplicationWorld` and one-shot `runJavaScript` calls. The
  page receives no file, shell, configuration, profile, permission, IPC, or
  unrestricted command method.
- Returned focus, hint, selection, site-data, and userscript data crosses back
  through bounded serialized results. Rust checks target identity, document
  revision/URL, types, sizes, and allowlisted actions before executing a native
  effect.
- A source-level regression test asserts that the QML boundary has no
  `WebChannel` or `contextProperty` and retains the isolated-world/result
  validation hooks.

## Limitations

The malicious remote-page attack fixture, live renderer-isolation evidence,
and complete native event/IME qualification remain future work. Installed
page-world userscripts are explicit trusted user code and are not presented as
a sandbox or as a source of trusted DOM data.

## Verification

```text
cargo test -p browser-engine-qt --locked
cargo test -p browser-ipc --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo xtask check
QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir
```
