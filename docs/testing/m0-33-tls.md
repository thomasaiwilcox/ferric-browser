# M0-33 TLS certificate errors

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: AUTH-004.
- Files: `crates/ferric-browser-engine-qt/qml/Main.qml` and
  `crates/ferric-browser-engine-qt/src/lib.rs`.
- Observable result: Qt WebEngine certificate errors are blocking by default.
  Non-overridable errors, subresource errors, and Google-host errors are
  rejected without a bypass surface. An engine-marked overridable main-frame
  error for another host gets a browser-owned prompt showing a bounded exact
  host and error description.

The prompt's `Accept once` action calls Qt's `acceptCertificate()` for that
request only. There is no persistent exception, system trust-store write,
HSTS bypass, global ignore flag, or certificate-error setting in browser
configuration. Navigation, tab close, view destruction, and application
teardown reject a pending error and restore focus.

The host label is derived from the URL host only; paths, credentials, query,
and fragments are not displayed. Error descriptions are bounded and control
character sanitized before entering the native surface.

## Verification

```text
cargo fmt --all -- --check
cargo test -p ferric-browser-engine-qt --locked
cargo xtask check
cargo build -p ferric-browser --locked
QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir
```

The smoke is expected to end with timeout status 124 after remaining alive
with an empty startup log. Local self-signed HTTPS fixture coverage, exact
Qt overridable/non-overridable error behavior, HSTS and mixed-content tests,
and platform trust-store qualification remain manual work.
