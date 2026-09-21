# M0-56 — Journey retention follows profile history settings

Durable journey commits now use Unix timestamps rather than process-local core
revisions. The active profile's validated `history.retention_days` is converted
to a cutoff and applied in the same transaction as the new node and hard-cap
pruning. `0` disables durable recording for the commit path, so the graph is
not written to the normal profile database.

Evidence:

```text
cargo test -p browser-storage -p browser-engine-qt --locked
```

The storage regression test verifies that inserting a current node with a
cutoff removes an older node and its current pointer.
