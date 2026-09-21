# M0-85 — Typed tab undo action

## Scope

Closed-tab recovery is now available through the shared namespaced action
registry as well as the existing `tab-undo` command.

## Implemented behavior

- `browser.tab.undo` is a no-argument tab action with a stable discovery
  example.
- Typed IPC, interactive command execution, and the browser-owned UI action
  adapter reuse the existing safe closed-tab reopen executor.
- Recovery selects the newest eligible descriptor, validates its safe URL,
  creates a fresh tab, and removes the descriptor only after successful
  activation.
- Empty or stale closed-tab descriptors fail without an unsafe navigation.

## Qualification

```text
cargo test -p ferric-browser-core -p ferric-browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p ferric-browser --locked
cargo fmt --all -- --check
```
