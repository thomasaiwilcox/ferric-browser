# M0-93 — Window close command and action

The owning browser window can now be closed through the shared command path:

- `window-close` is registered as a window-scoped command.
- `action window close` and `browser.window.close` resolve to the same typed
  command.
- Command, typed IPC, and action execution queue a distinct
  `window-close-request` for the primary or secondary QML owner.
- Native compositor close requests and command-driven closes first show one
  keyboard-first browser-owned confirmation. `j`/`k` or the arrow keys select,
  Enter runs the selected action, `y` confirms, and `n`/Escape cancels; the
  same action rows remain pointer-operable.
- Confirmation precedes the existing active-work and page-state checks, so a
  clean page cannot bypass the user's close decision. During an already
  confirmed process-wide quit, child windows skip only the redundant initial
  question and retain their active-download, cancellation, checkpointing, and
  unsaved-page protections.

The command takes no arguments and rejects routing, unknown typed fields, and
extra positional arguments. It is distinct from `quit`: `quit` requests
coordinated application shutdown, while `window-close` expresses closure of
the current owning window and is consumed by that window's native lifecycle.

## Verification

```text
cargo test -p ferric-browser-core -p ferric-browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p ferric-browser --locked
cargo fmt --all -- --check
```
