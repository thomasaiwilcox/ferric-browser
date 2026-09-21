# External editor cancellation evidence

Date: 2026-09-19  
Status: in-progress

## Cancellation and cleanup

An external editor request captures the focused document target and creates a
0600 scratch file only after the page extraction succeeds. Once the configured
editor starts, its child handle and a cancellation token remain associated
with the request. The existing `cancel` command now marks the request
cancelled, terminates and waits for the child, and removes the scratch file.

The same cleanup path is used for extraction timeout, editor-start failure,
stale navigation/profile teardown, and editor write-back errors. A worker that
observes cancellation does not publish a late completion, so a cancelled
editor cannot apply text after the request has been cleared.

Private and ephemeral profiles use a session-scoped temporary scratch
directory instead of durable profile storage. Rust removes that managed
directory after completion or cancellation and warns that the external editor
may create its own swap or backup files outside the browser's cleanup boundary.

The editor now starts in a dedicated Unix process group. Cancellation sends
`SIGTERM` to that group, waits up to 500 ms, then sends `SIGKILL` if needed and
reaps the tracked child. An editor that deliberately leaves the group (for
example with `setsid`) is outside this bounded guarantee. Editor stderr remains
intentionally discarded; only userscript stderr has an explicit bounded
retrieval path today.

## Verification

```text
cargo test -p browser-engine-qt --locked --offline
cargo clippy -p browser-engine-qt --all-targets --locked --offline -- -D warnings
cargo fmt --all -- --check
cargo xtask check
cargo build -p rustbrowser --locked --offline
```

Unit coverage starts bounded `/bin/sleep` and shell-descendant fixtures and
verifies the tracked child is killed and reaped. Live editor cancellation and
temporary-directory cleanup on native Wayland remain to be qualified.
