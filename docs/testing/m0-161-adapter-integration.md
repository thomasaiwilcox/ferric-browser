# M0-161 adapter integration smoke

Status: in progress; the bounded offscreen bridge smoke is implemented, while
native Qt/Wayland callback and lifecycle qualification remains pending.

Latest local verification: 2026-09-19 — `cargo xtask test adapter` passed the
bounded 10-second offscreen run.

`cargo xtask test adapter` builds `ferric-browser` with the locked workspace and
starts it with `QT_QPA_PLATFORM=offscreen` and `--temp-basedir`. The child must
remain alive for ten seconds. Early exit is reported with captured stdout and
stderr; QML application-load failures and missing QML modules are rejected.
The disposable profile and child process are cleaned up by the bounded runner.

This is deliberately a narrow adapter boundary: it proves that the generated
CXX-Qt/QML application can load and enter its event loop without a display,
but does not claim real Wayland input, screen-reader behavior, portal dialogs,
WebEngine navigation, popup adoption, or callback teardown. Those require the
native Wayland and later interactive qualification environments.

## Verification

```text
cargo xtask test adapter
cargo test -p ferric-browser-engine-qt --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
```
