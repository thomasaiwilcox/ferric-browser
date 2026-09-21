# M0-107: snapshot-backed library commands

Requirements: STATE-005, STORE-005, SWITCH-004

The read-only durable library commands now consume the revisioned profile
snapshot instead of querying SQLite on the Qt thread. This covers history
listing, bookmark and quickmark listing, and history/bookmark/quickmark open.
They queue the bounded worker request when the snapshot is absent or stale and
return a bounded retry result while it is loading. Stored-entry opens resolve
the selected URL from the same snapshot used by the list, preserving the
stale-result boundary before creating a navigation effect.

Durable `journey-reopen` node lookup and `journey` current/search/expand queries
also run through the same profile worker; navigation targets are created only
after the returned node ID and profile identity pass the final stale-target
check.

Durable journey export preview and serialization use a worker-produced bounded
snapshot as well; Qt retains the explicit preview and non-overwriting local
destination boundary.

Bookmark and quickmark writes (including title/destination edits) plus history
clearing use acknowledged worker mutations and mark the snapshot dirty after a
successful commit. Journey queries continue to use
their separate durable graph APIs until the worker has a corresponding query
family.

Evidence:

- `cargo fmt --all -- --check`
- `cargo test -p browser-engine-qt --locked --offline` (167 tests)
- `cargo xtask check --locked` (all workspace test suites pass)

The next storage slices cover worker-backed writes, permission queries, and
the remaining durable graph/session queries.
