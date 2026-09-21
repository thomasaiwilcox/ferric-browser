# M0-72 — Typed tab action management

## Scope

This slice extends the shared action registry with stable tab-management
subjects. Each action takes an explicit stable `tabid-*` value; actions do not
fall back to the active tab when the requested ID is missing or stale.

## Implemented behavior

- `browser.tab.focus` selects the captured live tab.
- `browser.tab.close` queues a browser-owned close operation; QML performs the
  dialog/permission cleanup and acknowledges the view before removing it.
- `browser.tab.move` accepts only `left` or `right` and preserves the existing
  pinned-group and same-window reducer rules.
- `browser.tab.mute` toggles the captured tab’s mute state through the reducer.
- Generic `action tab ...`, typed `action.execute`, native command execution,
  and action discovery use the same IDs, argument schemas, and executors.
- IPC polling drains browser-owned engine actions, so a typed close request
  reaches the same view-lifecycle boundary as an interactive command.

## Qualification

Automated evidence:

```text
cargo test -p ferric-browser-core -p ferric-browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p ferric-browser --locked
cargo fmt --all -- --check
```

The focused suite covers registry metadata, command mapping, typed IPC
mapping, direction validation, and stable-ID discovery. A live multi-window
tab-action fixture and compositor activation qualification remain.
