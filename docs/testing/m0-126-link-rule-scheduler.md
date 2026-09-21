# Clean-link update scheduler

Date: 2026-09-17  
Status: in-progress

## Task card

- Requirements: LINK-005 and the normal-profile scheduling portion of NET-002.
- Files: `crates/ferric-browser-engine-qt/qml/Main.qml` and
  `docs/DEVELOPMENT_SPEC.md`.
- Observable result: when both clean-link update fields are configured, a
  normal profile starts a bounded 24-hour timer that invokes the same updater
  as the explicit `link-cleaning-update` command. The timer remains stopped
  when the source is unset, the profile is private/memory-only, or storage is
  unavailable.

The scheduler records the last attempted start in the owning QML window and
skips/restarts its timer if an explicit update occurred within the 24-hour
window. In-flight requests are not duplicated. Configuration reloads
reconfigure the timer, while the default configuration creates no clean-link
maintenance traffic.

## Verification

```text
cargo xtask check --locked
cargo build -p ferric-browser --locked --offline
target/debug/ferric-browser diagnostics --format json
QT_QPA_PLATFORM=wayland timeout 8s target/debug/ferric-browser --temp-basedir
```
