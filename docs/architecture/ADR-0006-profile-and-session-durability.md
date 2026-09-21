# ADR-0006: Profile durability and private exclusions

- Status: accepted for the V1 storage boundary
- Date: 2026-09-18
- Scope: profile-local SQLite metadata, session snapshots, workers, and
  private/ephemeral state

## Decision

Normal named profiles own one profile-local Rust SQLite store and one
exclusive profile lock. SQLite migrations, WAL/FULL durability, integrity
preflight, bounded backups, and all blocking reads/writes are owned by the
storage worker. Session snapshots use UUID-backed identities and same-directory
atomic generations with a recoverable previous generation.

Private and ephemeral profiles never open the durable Rust store. Their
history, grants, journey graph, session state, and completion data are
memory-only and are cleared at the final-owner boundary. Engine cookie/site
storage remains Qt-owned and is never queried through the Rust database.

## Consequences and evidence

Crash recovery can restore only validated safe GET descriptors; forms,
credentials, private tabs, and page data are not serialized. Disk-full,
process-kill, and installed-runtime qualification remain explicit gates.

- `crates/ferric-browser-storage/src/lib.rs`
- `crates/ferric-browser-storage/src/sessions.rs`
- `crates/ferric-browser-storage/src/worker.rs`
- `crates/ferric-browser-engine-qt/src/lib.rs`
- `docs/testing/m2-01-storage.md`
- `docs/testing/m2-03-sessions.md`

## Reconsideration trigger

Revisit only for an explicit schema migration or a product decision that
changes private-state durability; no convenience fallback may write private
data into a normal profile.
