# M0-89 — Per-tab zoom command and controls

The registered `zoom` command accepts `in`, `out`, `reset`, or a numeric
factor. Factors are finite and clamped to `0.25..=5.0`; the reducer stores the
validated value on the live tab and emits the normal persistence effect.

Typed IPC and `browser.tab.zoom` use the same command validation. Primary and
secondary native toolbars expose zoom out/reset/in controls, and the pending
engine action updates the matching QML view by stable tab ID. Session restore
already carries the tab zoom field, so the value survives normal checkpoints
and restore descriptors.

Evidence:

- `cargo test -p browser-core -p browser-engine-qt --locked --offline`
- `cargo xtask check --locked`
- `cargo build -p rustbrowser --locked`
- `cargo fmt --all -- --check`
