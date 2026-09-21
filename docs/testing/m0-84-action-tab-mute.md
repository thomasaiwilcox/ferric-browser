# M0-84 — Typed tab mute action

## Scope

Tab muting is now available through the shared action and command registry,
including explicit state selection.

## Implemented behavior

- `browser.tab.mute` carries a bounded stable tab ID and optional `on`, `off`,
  or `toggle` state.
- Interactive, typed IPC, and browser-owned tab-strip activation all reuse
  the same reducer-backed executor.
- The executor revalidates that the tab is live and owned by the current
  browser window, then reports the resulting muted state.
- Invalid states and stale/out-of-window IDs are rejected before mutation.

## Qualification

```text
cargo test -p ferric-browser-core -p ferric-browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p ferric-browser --locked
cargo fmt --all -- --check
```
