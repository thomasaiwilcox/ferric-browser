# M0-93 — Window close command and action

The owning browser window can now be closed through the shared command path:

- `window-close` is registered as a window-scoped command.
- `action window close` and `browser.window.close` resolve to the same typed
  command.
- Command, typed IPC, and action execution queue a distinct
  `window-close-request` for the primary or secondary QML owner.
- The existing close boundary remains responsible for active-download
  confirmation, cancellation, checkpointing, and final window shutdown.

The command takes no arguments and rejects routing, unknown typed fields, and
extra positional arguments. It is distinct from `quit`: `quit` requests
coordinated application shutdown, while `window-close` expresses closure of
the current owning window and is consumed by that window's native lifecycle.

## Verification

```text
cargo test -p browser-core -p browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p rustbrowser --locked
cargo fmt --all -- --check
```
