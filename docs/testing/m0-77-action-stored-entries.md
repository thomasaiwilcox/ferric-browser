# M0-77 — Typed stored-entry actions

## Scope

History entries, bookmarks, and quickmarks now expose bounded typed actions for
opening captured records. Bookmarks and quickmarks also expose deletion.

## Implemented behavior

- Stable action IDs cover history-entry open, bookmark open/delete, and
  quickmark open/delete.
- Commands resolve records by their captured ID or name in the active durable
  profile; stale or missing records fail closed.
- Stored URLs are rechecked against the safe durable-navigation policy before
  navigation.
- Switcher bookmark and quickmark rows route open/delete through the same typed
  action boundary.
- Private and ephemeral profiles continue to have no durable stored-entry
  actions.

## Qualification

```text
cargo test -p browser-core -p browser-engine-qt --locked --offline
cargo xtask check --locked
cargo build -p rustbrowser --locked
cargo fmt --all -- --check
```

Live switcher deletion and multi-profile qualification remain.
