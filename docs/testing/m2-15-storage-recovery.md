# M2-15 storage recovery evidence

Date: 2026-09-16  
Status: in-progress

## Task card

- Requirement: STORE-006.
- Files: `crates/ferric-browser-storage/src/lib.rs`,
  `crates/ferric-browser-engine-qt/src/diagnostics.rs`,
  `crates/ferric-browser-engine-qt/src/lib.rs`, and
  `crates/ferric-browser/src/main.rs`.
- Observable result: opening an existing nonempty profile database performs a
  read-only SQLite integrity check and rejects corruption or a schema newer
  than the application before enabling WAL. The original file is never
  replaced. Older schemas are refused without a backup, migration, or in-place
  rewrite. SQLite `SQLITE_FULL` write failures become an
  explicit disk-full error rather than an opaque generic SQLite message.
  `diagnostics --format json` inspects profile database files read-only and
  reports per-profile health, schema, integrity, and recovery guidance without
  emitting profile paths or browsing data. The GUI IPC diagnostics surface
  reports the active profile's health when its durable root is available.

## Evidence

- `cargo test -p ferric-browser-storage --locked` passes 65 tests, including byte-for-
  byte preservation of a corrupt file, refusal of a future schema version, a
  deterministic `SQLITE_FULL`-to-`DiskFull` mapping, and a missing-file
  inspection that performs no write.
- `cargo test -p ferric-browser-engine-qt --locked` and
  `cargo test -p ferric-browser --locked` pass after integrating the diagnostics
  health fact.
- The recovery error explicitly preserves the original and points to
  read-only inspection or a clearly named new profile. A failed open does not
  delete the database, retry migration in a loop, or silently create a
  replacement at the same path.
- `older_schema_is_refused_without_migration_or_replacement` verifies that a
  legacy database remains byte-for-byte unchanged.

## Limitations

User-facing repair/export, actual filesystem-full
qualification, async storage workers, and stale-lock recovery remain future
slices. A profile with unavailable durable metadata stays usable
for transient browsing, but its status identifies that durable storage is not
available.
