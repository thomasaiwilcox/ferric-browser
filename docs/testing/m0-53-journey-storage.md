# M0-53 — Profile-local journey graph storage

## Scope

This slice establishes the authoritative journey data boundary for normal
profiles. The reducer records bounded, profile-local nodes and typed edges in
memory. A successful Qt navigation commit mirrors the resulting safe node into
the profile SQLite store; private and ephemeral windows have no store and
therefore cannot write journey records.

## Contract covered

- `JOURNEY-001`: nodes and edges are profile-local, current navigation is
  tracked per tab, and new committed nodes link to the prior current node.
- `JOURNEY-002`: only safe URL, bounded title, committed time, transition,
  source, durable profile UUID, per-runtime tab key, and a durable node UUID
  are retained.
- `JOURNEY-003`: retention is bounded at 50,000 nodes and 100,000 edges;
  pruning preserves current nodes; `clear_journey` removes the durable graph.
- `JOURNEY-004`: the native library manager can show a bounded chronological
  outline, including transition labels and an optional current-tab filter.

The current reducer integration labels engine commits as `navigate`. Redirect,
opener, popup, hint, restore, reopen, and branch-after-back classification are
subsequent transition-classification slices. Configurable retention is wired
through the validated profile `history.retention_days` setting; graph
neighborhoods and safe reopen actions are separate UI/action slices.

## Evidence

```text
cargo test -p ferric-browser-core --locked
cargo test -p ferric-browser-storage --locked
cargo test -p ferric-browser-engine-qt --locked
```

The storage tests cover typed edge linking, newest-first node queries, clear,
secret-bearing URL rejection, and the private no-durable-store boundary.
