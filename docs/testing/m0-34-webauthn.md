# M0-34 WebAuthn UX

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: AUTH-003.
- Files: `crates/browser-engine-qt/qml/Main.qml` and
  `crates/browser-engine-qt/src/lib.rs`.
- Observable result: Qt WebEngine `webAuthUxRequested` requests are handled by
  a native browser-owned modal surface. It labels the relying-party ID,
  presents engine-provided account names, collects a security-key PIN only in
  a password-style field, explains the touch/confirmation phase, and exposes
  retry/cancel actions for failures.

The request object remains the authority for state transitions. Account and
PIN values are bounded before submission; the PIN field is cleared immediately
after `setPin()` and again during teardown. Completed and cancelled states
close without issuing a second cancel. Navigation, tab/window destruction,
and application teardown cancel an outstanding request. Inactive primary tabs
cancel their requests rather than displaying a prompt for another tab.

No relying-party page object, WebChannel, macro history, diagnostics payload,
or persistent browser state receives the PIN or authenticator secret.

## Verification

```text
cargo fmt --all -- --check
cargo test -p browser-engine-qt --locked
cargo xtask check
cargo build -p rustbrowser --locked
QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir
```

The smoke is expected to end with timeout status 124 after remaining alive
with an empty startup log. Physical USB FIDO2, platform authenticator,
Bluetooth/hybrid transport, synced passkeys, user-verification error mapping,
and interactive fixture qualification remain manual work.
