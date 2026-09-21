# GPU fallback evidence

Date: 2026-09-16  
Status: in-progress

## FAIL-002 GPU fallback

Ferric Browser has an explicit `--software-rendering` startup mode and a
`Restart software` action on the renderer recovery surface. The action starts
a new process only after the current instance lock is released, then preserves
the selected profile root (or the disposable/safe-mode selection) and passes
the optional instance selector through. It does not clear profile data or
persist experimental flags.

The new process appends only `--disable-gpu` and
`--disable-gpu-compositing` to `QTWEBENGINE_CHROMIUM_FLAGS` and sets
`QT_QUICK_BACKEND=software`. The existing security-environment validation
still rejects sandbox-disabling flags. Primary and secondary status bars show
`SOFTWARE RENDERING`; the privacy-safe diagnostics snapshot reports the
session as degraded and GPU decode as unavailable.

## Verification

```text
cargo test -p ferric-browser -p ferric-browser-engine-qt --locked --offline
cargo xtask check
cargo fmt --all -- --check
QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --software-rendering --temp-basedir
```

The explicit startup smoke remains alive until the intentional timeout without
QML load errors. The workspace build and tests pass.

## Remaining qualification

This slice proves the browser-owned restart and reporting path. Qualification
of basic browsing, conferencing/media behavior, startup failure injection, and
performance impact on the reference GPU remains outstanding.
