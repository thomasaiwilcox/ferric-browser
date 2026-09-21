# M0-130 macro discoverability

This slice completes the bounded macro surface with explicit, redacted
discoverability. The Qt UI exposes a session-only macro status property and
the status bar shows the active recording register and command count, or the
stored register names and counts. The diagnostics response exposes the same
state with the recording, replay, and hard-limit fields.

Only register names and command counts leave the native executor. Recorded
arguments are never rendered in the status bar or diagnostics, so URLs and
other command data remain private. The status also declares that registers
are memory-only; they are cleared when transient runtime resources are
released and are not persisted as browser history or profile state.

Verification:

```text
cargo fmt --all -- --check
cargo test -p ferric-browser-engine-qt --locked --offline
cargo xtask check --locked
```

The native Wayland smoke remains the integration check for the Qt property and
status-bar wiring.
