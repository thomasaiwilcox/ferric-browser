# M0-64 — Target-specific journey metadata for multi-tab restore

Journey transition metadata is now queued per runtime target (bounded to 256
pending entries) instead of using one global slot. Context restoration marks
every restored navigation as `session-restore`, while named/crash recovery
marks inactive restored tabs before their lazy QML load and marks the selected
tab at its immediate navigation boundary. Concurrent commits therefore keep
their transition labels attached to the correct tab.

Failed loads and profile reconfiguration clear pending transition, parent,
redirect, and navigation-request state. The queue contains only runtime
identities and typed transition/source labels; session descriptors remain the
existing safe GET-only data.

Evidence:

* `cargo test -p ferric-browser-engine-qt --locked --offline`
* `cargo xtask check`
* `cargo build -p ferric-browser --locked`
* `QT_QPA_PLATFORM=wayland timeout 10s target/debug/ferric-browser --temp-basedir`

The Wayland command is a bounded startup smoke and should exit 124 after the
GUI remains alive for the interval.
