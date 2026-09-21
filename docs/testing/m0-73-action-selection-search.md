# M0-73 — Typed selection search

## Scope

This slice adds the built-in `browser.selection.search` action. It uses the
existing browser-owned one-shot selection extraction path and the configured
HTTPS search-engine templates.

## Implemented behavior

- `action selection search` uses the default configured search engine.
- `action selection search --engine ddg` selects a named configured engine.
- Typed `action.execute` accepts the same optional `engine` field.
- Selection text is bounded to 1 MiB, rejected when stale or empty, and
  percent-encoded as data in exactly one `{query}` template placeholder.
- The resulting navigation is reducer-backed and records a typed
  `selection-search` journey transition.
- Pending operations expose an operation ID and are failed or cancelled on
  extraction timeout, navigation invalidation, escape, or explicit cancel.
- Only HTTPS configured templates are accepted; no shell or page-to-native
  authority is introduced.

## Qualification

Automated evidence:

```text
cargo test -p browser-core -p browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p rustbrowser --locked
cargo fmt --all -- --check
```

The focused suite covers template encoding, action discovery, interactive
mapping, and typed IPC mapping. Live Wayland selection and engine navigation
qualification remain.
