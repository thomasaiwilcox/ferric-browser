# M0-32 HTTP, proxy, and client-certificate prompts

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: AUTH-002.
- Files: `crates/browser-engine-qt/qml/Main.qml` and
  `crates/browser-engine-qt/src/lib.rs`.
- Observable result: HTTP and proxy authentication requests use a native
  browser-owned surface labeled with the requesting host/proxy and bounded
  realm text. Username and password fields are transient QML values passed
  directly to Qt WebEngine, cleared before and after use, and never included in
  status text, diagnostics, scripts, or persistent browser state.

Qt's `selectClientCertificate` signal is handled for primary tabs, secondary
windows, and approved popups. The surface lists bounded subject/issuer labels
from the engine-offered certificate identities only; it calls the selection by
engine index, never handles private keys, and calls `selectNone()` when the
list is empty, the user cancels, navigation replaces the document, or the view
is destroyed. A stale or inactive primary tab is cancelled rather than shown
in a prompt belonging to another tab.

Authentication prompts retain the existing per-document three-request budget
and explicit suppression control. Prompt focus is restored through the
browser-owned focus stack after cancellation or selection.

## Verification

```text
cargo fmt --all -- --check
cargo test -p browser-engine-qt --locked
cargo xtask check
cargo build -p rustbrowser --locked
QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir
```

The smoke is expected to end with timeout status 124 after remaining alive
with an empty startup log. Manual HTTP 401, proxy 407, and mutual-TLS fixture
coverage, platform credential-store policy, repeated challenge behavior, and
compositor-level modal evidence remain qualification work.
