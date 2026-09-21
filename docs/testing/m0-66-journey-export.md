# M0-66 — Reviewed journey export

The native journey manager now offers an explicit export flow. It first shows
a bounded preview of node and relationship counts, identifies memory-only
private/ephemeral data as an external disclosure, then opens a native local
Save dialog. The Rust boundary emits only sanitized URL/title/timestamp,
profile/tab identity, typed transition, source, and relationship fields; it
refuses non-local/relative paths and refuses to overwrite an existing file.
The output is written with the browser's private atomic file writer.

Ordinary IPC does not gain an export operation, and private/ephemeral data is
never copied into a durable profile or durable journey store. The selected
file is an explicit user-owned external output.

Evidence:

* `cargo test -p browser-engine-qt --locked --offline`
* `cargo xtask check`
* `cargo build -p rustbrowser --locked`
* `QT_QPA_PLATFORM=wayland timeout 10s target/debug/rustbrowser --temp-basedir`

The final smoke is bounded and should exit 124 after the GUI remains alive for
the requested interval.
