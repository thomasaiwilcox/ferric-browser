# M0-37 notifications and push boundary

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: MEDIA-006.
- Files: `crates/ferric-browser-engine-qt/qml/Main.qml` and
  `crates/ferric-browser-engine-qt/src/lib.rs`.
- Observable result: normal and private Qt WebEngine profiles route
  `presentNotification` through a browser-owned consent/configuration gate and
  then use Qt's native notification presenter. Unconsented, opaque-origin, or
  desktop-disabled notifications are closed without being shown.

Notifications with a non-empty tag are coalesced by profile scope, bounded
origin, and tag;
the previous notification is closed and the browser removes entries from its
registry when Qt reports `closed()`. Untagged notifications remain independent
and revoking notification permission closes matching active notifications.
When the session notification service is available, the browser-owned DBus
presenter forwards native activation to the originating profile/window and
withdraws notifications during teardown; Qt's presenter remains the fallback,
including when the asynchronous DBus `Notify` call fails.
All active notifications are closed during browser teardown.

Push is disabled by default and is enabled only when the normal profile's
`privacy.push_service` setting is explicitly `true`. Private profiles always
keep the push service disabled. The implementation does not claim delivery
after browser exit because there is no background service in this slice.

## Verification

```text
cargo fmt --all -- --check
cargo test -p ferric-browser-engine-qt --locked
cargo xtask check
cargo build -p ferric-browser --locked
QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir
```

Static and startup checks cover the ownership boundary. Live notification
permission, native desktop presentation, click routing, revoke behavior, and
FCM service-lifetime qualification remain manual work.
