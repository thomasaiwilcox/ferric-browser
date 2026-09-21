# M0-83 — Typed tab pin action

## Scope

Tab pinning is now available through the shared action and command registry,
including explicit state selection.

## Implemented behavior

- `browser.tab.pin` carries a bounded stable tab ID and optional `on`, `off`,
  or `toggle` state.
- Interactive, typed IPC, and browser-owned tab-strip activation all reuse
  the same reducer-backed executor.
- The executor revalidates that the tab is live and owned by the current
  browser window, preserves pinned-first ordering, and reports the resulting
  state and index.
- Invalid states and stale/out-of-window IDs are rejected before mutation.

## Qualification

```text
cargo test -p browser-core -p browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p rustbrowser --locked
cargo fmt --all -- --check
```
