# M0-38 device lifecycle and revocation coordination

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: MEDIA-003.
- Files: `crates/ferric-browser-engine-qt/qml/Main.qml` and
  `crates/ferric-browser-engine-qt/src/lib.rs`.
- Observable result: exact-origin `permission-reset` now cancels matching
  queued requests and reloads every registered matching primary/secondary view,
  together with tracked active screen/window capture views. A failed request
  does not poison the profile; later requests still use the normal prompt path.

The implementation remains conservative about device state: Qt Quick exposes
permission requests but no direct camera/microphone stream-track lifecycle
callback or stop method. The browser therefore does not claim a device stream
is stopped merely because a rule was deleted; it requests termination through
the owning view's reload boundary and reports that boundary explicitly.

## Verification

```text
cargo fmt --all -- --check
cargo test -p ferric-browser-engine-qt --locked
cargo xtask check
cargo build -p ferric-browser --locked
QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir
```

Live unplug/replug, device switching, portal restart, call termination, and
system-audio support remain manual qualification work on the target Wayland
baseline.
