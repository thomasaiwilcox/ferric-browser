# M0-75 — Typed context activation

## Scope

Switcher context rows now execute the shared `browser.context.enter` action
through the typed action boundary after checking the row's profile affinity.

## Implemented behavior

- Context rows resolve to the stable `browser.context.enter` action.
- The captured context name is passed as a typed value to `context-enter`.
- Stale rows and cross-profile contexts are rejected before mutation.
- Successful activation preserves the existing context restore and tab-sync
  behavior.

## Qualification

```text
cargo test -p ferric-browser-core -p ferric-browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p ferric-browser --locked
cargo fmt --all -- --check
```

Live switcher context activation and native cross-profile window qualification
remain; the typed routing and preflighted secondary-window path are present.
